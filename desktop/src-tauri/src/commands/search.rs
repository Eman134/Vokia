use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::agents::{name_key, AgentProfile, AgentRegistry};
use crate::db::search::{resolve_limit, GlobalSearchQuery, GlobalSearchResponse, SearchGroup, SearchHit, SearchKind};
use crate::db::DbState;

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GlobalSearchArgs {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub project_id: Option<i64>,
    #[serde(default)]
    pub kinds: Option<Vec<SearchKind>>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub offset: Option<usize>,
}

fn parse_offset(cursor: Option<&str>, offset: Option<usize>) -> Result<usize, String> {
    if let Some(raw) = cursor.map(str::trim).filter(|value| !value.is_empty()) {
        raw.parse::<usize>()
            .map_err(|_| format!("Cursor de paginação inválido: {raw}"))
    } else {
        Ok(offset.unwrap_or(0))
    }
}

fn query_tokens(raw: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in raw.chars() {
        if ch.is_alphanumeric() {
            current.push(ch);
        } else if !current.is_empty() {
            tokens.push(name_key(&current));
            current.clear();
        }
    }
    if !current.is_empty() {
        tokens.push(name_key(&current));
    }
    tokens
}

fn highlight_first(text: &str, tokens: &[String]) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let folded = name_key(trimmed);
    let mut best: Option<(usize, usize)> = None;
    for token in tokens {
        if token.is_empty() {
            continue;
        }
        if let Some(pos) = folded.find(token.as_str()) {
            let candidate = (pos, token.len());
            if best.is_none_or(|(bpos, _)| pos < bpos) {
                best = Some(candidate);
            }
        }
    }
    let Some((start, len)) = best else {
        return Some(trimmed.to_string());
    };
    // Mapeia índices por caractere para manter fatiamento UTF-8 seguro mesmo com acentos.
    let char_start = folded[..start].chars().count();
    let char_len = folded[start..start + len].chars().count();
    let chars: Vec<char> = trimmed.chars().collect();
    if char_start >= chars.len() {
        return Some(trimmed.to_string());
    }
    let char_end = (char_start + char_len).min(chars.len());
    let before: String = chars[..char_start].iter().collect();
    let matched: String = chars[char_start..char_end].iter().collect();
    let after: String = chars[char_end..].iter().collect();
    Some(format!("{before}<mark>{matched}</mark>{after}"))
}

fn score_agent(profile: &AgentProfile, tokens: &[String]) -> Option<(f64, String)> {
    let name_folded = name_key(&profile.name);
    let id_folded = name_key(profile.id.as_str());
    let role_folded = name_key(&profile.role);
    let cli_folded = name_key(&profile.cli);
    let model_folded = name_key(&profile.model);
    let instr_folded = name_key(&profile.extra_instructions);

    let combined = format!(
        "{name_folded} {id_folded} {role_folded} {cli_folded} {model_folded} {instr_folded}"
    );
    if !tokens.iter().all(|tok| combined.contains(tok.as_str())) {
        return None;
    }

    let mut score = 0.0;
    for tok in tokens {
        if name_folded == *tok || id_folded == *tok {
            score += 12.0;
        } else if name_folded.starts_with(tok.as_str()) || id_folded.starts_with(tok.as_str()) {
            score += 9.0;
        } else if name_folded.contains(tok.as_str()) {
            score += 7.0;
        }
        if role_folded.contains(tok.as_str()) {
            score += 5.0;
        }
        if cli_folded.contains(tok.as_str()) || model_folded.contains(tok.as_str()) {
            score += 3.0;
        }
        if instr_folded.contains(tok.as_str()) {
            score += 1.5;
        }
    }

    let summary = format!("{} · {} ({})", profile.role, profile.cli, profile.model);
    let snippet_source = if !profile.extra_instructions.trim().is_empty()
        && tokens.iter().any(|tok| instr_folded.contains(tok.as_str()))
    {
        profile.extra_instructions.as_str()
    } else {
        summary.as_str()
    };
    let snippet = highlight_first(snippet_source, tokens).unwrap_or(summary);
    Some((score, snippet))
}

fn search_agents(
    profiles: &[AgentProfile],
    raw_query: &str,
    limit: usize,
    offset: usize,
) -> SearchGroup {
    let tokens = query_tokens(raw_query);
    if tokens.is_empty() {
        return SearchGroup {
            kind: SearchKind::Agents,
            total: 0,
            hits: Vec::new(),
            next_cursor: None,
        };
    }

    let mut matched: Vec<SearchHit> = profiles
        .iter()
        .filter_map(|profile| {
            let (score, snippet) = score_agent(profile, &tokens)?;
            Some(SearchHit {
                kind: SearchKind::Agents,
                id: profile.id.as_str().to_string(),
                title: profile.name.clone(),
                subtitle: Some(format!("{} · {}", profile.role, profile.cli)),
                snippet: Some(snippet),
                project_id: None,
                project_name: None,
                item_id: None,
                channel_id: None,
                from_agent: Some(profile.id.as_str().to_string()),
                to_agent: None,
                artifact_kind: None,
                column: None,
                timestamp: None,
                score,
            })
        })
        .collect();

    matched.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.title.cmp(&b.title))
    });

    let total = matched.len();
    let hits: Vec<SearchHit> = matched.into_iter().skip(offset).take(limit).collect();
    let next = offset.saturating_add(hits.len());
    let next_cursor = if hits.len() >= limit && next < total {
        Some(next.to_string())
    } else {
        None
    };

    SearchGroup {
        kind: SearchKind::Agents,
        total,
        hits,
        next_cursor,
    }
}

pub(crate) fn global_search_exec(
    app: &AppHandle,
    args: GlobalSearchArgs,
) -> Result<GlobalSearchResponse, String> {
    let offset = parse_offset(args.cursor.as_deref(), args.offset)?;
    let limit = resolve_limit(args.limit);
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let mut response = db
        .global_search(&GlobalSearchQuery {
            query: args.query.clone(),
            project_id: args.project_id,
            kinds: args.kinds,
            limit: Some(limit),
            offset,
        })
        .map_err(|error| error.to_string())?;

    if let Some(agent_group) = response
        .groups
        .iter_mut()
        .find(|group| group.kind == SearchKind::Agents)
    {
        let team = app
            .state::<AgentRegistry>()
            .team()
            .map_err(|error| error.to_string())?;
        let computed = search_agents(team.profiles(), &args.query, limit, offset);
        response.total = response
            .total
            .saturating_sub(agent_group.total)
            .saturating_add(computed.total);
        *agent_group = computed;
    }

    Ok(response)
}

#[tauri::command]
pub fn global_search(
    app: AppHandle,
    query: String,
    project_id: Option<i64>,
    kinds: Option<Vec<SearchKind>>,
    limit: Option<usize>,
    cursor: Option<String>,
    offset: Option<usize>,
) -> Result<GlobalSearchResponse, String> {
    global_search_exec(
        &app,
        GlobalSearchArgs {
            query,
            project_id,
            kinds,
            limit,
            cursor,
            offset,
        },
    )
}
