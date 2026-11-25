use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use super::connection::Db;
use super::error::DbError;

pub const DEFAULT_SEARCH_LIMIT: usize = 20;
pub const MAX_SEARCH_LIMIT: usize = 100;
const MAX_QUERY_TOKENS: usize = 16;
const SNIPPET_TOKENS: i64 = 18;
const SNIPPET_START: &str = "<mark>";
const SNIPPET_END: &str = "</mark>";
const SNIPPET_ELLIPSIS: &str = "…";
const MS_PER_DAY: f64 = 86_400_000.0;
const RECENCY_HALF_LIFE_DAYS: f64 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchKind {
    Projects,
    Agents,
    Cards,
    Plans,
    Artifacts,
    Memories,
    Messages,
}

pub const ALL_SEARCH_KINDS: [SearchKind; 7] = [
    SearchKind::Projects,
    SearchKind::Agents,
    SearchKind::Cards,
    SearchKind::Plans,
    SearchKind::Artifacts,
    SearchKind::Memories,
    SearchKind::Messages,
];

#[derive(Clone, Debug, Default)]
pub struct GlobalSearchQuery {
    pub query: String,
    pub project_id: Option<i64>,
    pub kinds: Option<Vec<SearchKind>>,
    pub limit: Option<usize>,
    pub offset: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub kind: SearchKind,
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
    pub score: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchGroup {
    pub kind: SearchKind,
    pub total: usize,
    pub hits: Vec<SearchHit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSearchResponse {
    pub query: String,
    pub total: usize,
    pub groups: Vec<SearchGroup>,
}

/// Sanitiza texto livre do usuário para uma consulta FTS5 segura com busca por prefixo.
/// Evita erros de sintaxe em operadores (`AND`, `OR`, `NOT`, `NEAR`), aspas, hífen, `+`, `*`, `:`, etc.
pub fn build_fts_query(raw: &str) -> Option<String> {
    let mut terms = Vec::new();
    let mut current = String::new();
    for ch in raw.chars() {
        if ch.is_alphanumeric() {
            current.push(ch);
        } else if !current.is_empty() {
            terms.push(std::mem::take(&mut current));
            if terms.len() >= MAX_QUERY_TOKENS {
                break;
            }
        }
    }
    if !current.is_empty() && terms.len() < MAX_QUERY_TOKENS {
        terms.push(current);
    }
    if terms.is_empty() {
        return None;
    }
    let formatted: Vec<String> = terms
        .into_iter()
        .map(|term| {
            let escaped = term.replace('"', "\"\"");
            format!("\"{escaped}\"*")
        })
        .collect();
    Some(formatted.join(" "))
}

pub fn resolve_kinds(kinds: Option<&[SearchKind]>) -> Vec<SearchKind> {
    match kinds {
        Some(list) if !list.is_empty() => {
            let mut seen = HashSet::with_capacity(list.len());
            let mut out = Vec::with_capacity(list.len());
            for &kind in list {
                if seen.insert(kind) {
                    out.push(kind);
                }
            }
            out
        }
        _ => ALL_SEARCH_KINDS.to_vec(),
    }
}

pub fn resolve_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_SEARCH_LIMIT)
        .clamp(1, MAX_SEARCH_LIMIT)
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn empty_group(kind: SearchKind) -> SearchGroup {
    SearchGroup {
        kind,
        total: 0,
        hits: Vec::new(),
        next_cursor: None,
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn compact_title(text: &str, max_chars: usize) -> String {
    let first_line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .trim();
    let count = first_line.chars().count();
    if count <= max_chars {
        first_line.to_string()
    } else {
        let prefix: String = first_line.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{prefix}…")
    }
}

fn next_cursor_for(offset: usize, limit: usize, total: usize, returned: usize) -> Option<String> {
    let next = offset.saturating_add(returned);
    if returned >= limit && next < total {
        Some(next.to_string())
    } else {
        None
    }
}

fn table_exists(conn: &Connection, table: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get(0),
    )
}

impl Db {
    pub fn global_search(
        &self,
        query: &GlobalSearchQuery,
    ) -> Result<GlobalSearchResponse, DbError> {
        let conn = self.conn()?;
        let kinds = resolve_kinds(query.kinds.as_deref());
        let limit = resolve_limit(query.limit);
        let offset = query.offset;
        let fts_query = build_fts_query(&query.query);
        let now_ms = now_millis();

        let mut groups = Vec::with_capacity(kinds.len());
        let mut total = 0usize;

        for kind in kinds {
            let group = match (&fts_query, kind) {
                (None, _) => empty_group(kind),
                (_, SearchKind::Agents) => empty_group(kind),
                (Some(fts), SearchKind::Projects) => {
                    search_projects(&conn, fts, query.project_id, limit, offset, now_ms)?
                }
                (Some(fts), SearchKind::Cards) => {
                    search_cards(&conn, fts, query.project_id, limit, offset, now_ms)?
                }
                (Some(fts), SearchKind::Plans) => {
                    search_plans(&conn, fts, query.project_id, limit, offset, now_ms)?
                }
                (Some(fts), SearchKind::Artifacts) => {
                    search_artifacts(&conn, fts, query.project_id, limit, offset, now_ms)?
                }
                (Some(fts), SearchKind::Memories) => {
                    search_memories(&conn, fts, query.project_id, limit, offset, now_ms)?
                }
                (Some(fts), SearchKind::Messages) => {
                    search_messages(&conn, fts, query.project_id, limit, offset, now_ms)?
                }
            };
            total += group.total;
            groups.push(group);
        }

        Ok(GlobalSearchResponse {
            query: query.query.trim().to_string(),
            total,
            groups,
        })
    }
}

fn search_projects(
    conn: &Connection,
    fts_query: &str,
    project_id: Option<i64>,
    limit: usize,
    offset: usize,
    now_ms: i64,
) -> Result<SearchGroup, DbError> {
    let total: usize = conn.query_row(
        "SELECT COUNT(*) \
         FROM search_projects_fts f \
         JOIN projects p ON p.id = f.rowid \
         WHERE search_projects_fts MATCH ?1 \
           AND (?2 IS NULL OR p.id = ?2)",
        params![fts_query, project_id],
        |row| row.get::<_, i64>(0),
    )? as usize;

    if total == 0 {
        return Ok(empty_group(SearchKind::Projects));
    }

    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.path, p.description, \
                COALESCE((SELECT MAX(m.created_at) FROM messages m WHERE m.project_id = p.id), p.created_at) AS ts, \
                snippet(search_projects_fts, -1, ?5, ?6, ?7, ?8) AS snip, \
                (-bm25(search_projects_fts, 10.0, 5.0, 3.0, 4.0) \
                 + (1.0 / (1.0 + MAX(0.0, (?3 - COALESCE((SELECT MAX(m.created_at) FROM messages m WHERE m.project_id = p.id), p.created_at)) / ?4) / ?9))) AS rank_score \
         FROM search_projects_fts f \
         JOIN projects p ON p.id = f.rowid \
         WHERE search_projects_fts MATCH ?1 \
           AND (?2 IS NULL OR p.id = ?2) \
         ORDER BY rank_score DESC, ts DESC, p.id DESC \
         LIMIT ?10 OFFSET ?11",
    )?;

    let rows = stmt.query_map(
        params![
            fts_query,
            project_id,
            now_ms as f64,
            MS_PER_DAY,
            SNIPPET_START,
            SNIPPET_END,
            SNIPPET_ELLIPSIS,
            SNIPPET_TOKENS,
            RECENCY_HALF_LIFE_DAYS,
            limit as i64,
            offset as i64,
        ],
        |row| {
            let pid: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let path: String = row.get(2)?;
            let description: String = row.get(3)?;
            let ts: i64 = row.get(4)?;
            let snippet: Option<String> = row.get(5)?;
            let score: f64 = row.get(6)?;
            let subtitle = if description.trim().is_empty() {
                Some(path)
            } else {
                Some(description)
            };
            Ok(SearchHit {
                kind: SearchKind::Projects,
                id: pid.to_string(),
                title: name.clone(),
                subtitle,
                snippet: non_empty(snippet),
                project_id: Some(pid),
                project_name: Some(name),
                item_id: None,
                channel_id: None,
                from_agent: None,
                to_agent: None,
                artifact_kind: None,
                column: None,
                timestamp: Some(ts),
                score,
            })
        },
    )?;

    let hits: Vec<SearchHit> = rows.collect::<rusqlite::Result<_>>()?;
    let next_cursor = next_cursor_for(offset, limit, total, hits.len());
    Ok(SearchGroup {
        kind: SearchKind::Projects,
        total,
        hits,
        next_cursor,
    })
}

fn search_cards(
    conn: &Connection,
    fts_query: &str,
    project_id: Option<i64>,
    limit: usize,
    offset: usize,
    now_ms: i64,
) -> Result<SearchGroup, DbError> {
    let total: usize = conn.query_row(
        "SELECT COUNT(*) \
         FROM search_cards_fts f \
         JOIN backlog_items b ON b.rowid = f.rowid \
         WHERE search_cards_fts MATCH ?1 \
           AND b.archived_at IS NULL \
           AND (?2 IS NULL OR b.project_id = ?2)",
        params![fts_query, project_id],
        |row| row.get::<_, i64>(0),
    )? as usize;

    if total == 0 {
        return Ok(empty_group(SearchKind::Cards));
    }

    let mut stmt = conn.prepare(
        "SELECT b.project_id, p.name, b.id, b.title, b.\"column\", b.who, \
                COALESCE(b.cycle_started_at, p.created_at) AS ts, \
                snippet(search_cards_fts, -1, ?5, ?6, ?7, ?8) AS snip, \
                (-bm25(search_cards_fts, 10.0, 8.0, 2.5, 5.0) \
                 + (1.0 / (1.0 + MAX(0.0, (?3 - COALESCE(b.cycle_started_at, p.created_at)) / ?4) / ?9))) AS rank_score \
         FROM search_cards_fts f \
         JOIN backlog_items b ON b.rowid = f.rowid \
         JOIN projects p ON p.id = b.project_id \
         WHERE search_cards_fts MATCH ?1 \
           AND b.archived_at IS NULL \
           AND (?2 IS NULL OR b.project_id = ?2) \
         ORDER BY rank_score DESC, b.position DESC, b.rowid DESC \
         LIMIT ?10 OFFSET ?11",
    )?;

    let rows = stmt.query_map(
        params![
            fts_query,
            project_id,
            now_ms as f64,
            MS_PER_DAY,
            SNIPPET_START,
            SNIPPET_END,
            SNIPPET_ELLIPSIS,
            SNIPPET_TOKENS,
            RECENCY_HALF_LIFE_DAYS,
            limit as i64,
            offset as i64,
        ],
        |row| {
            let pid: i64 = row.get(0)?;
            let project_name: String = row.get(1)?;
            let id: String = row.get(2)?;
            let title: String = row.get(3)?;
            let column: String = row.get(4)?;
            let who: String = row.get(5)?;
            let ts: Option<i64> = row.get(6)?;
            let snippet: Option<String> = row.get(7)?;
            let score: f64 = row.get(8)?;
            let subtitle = Some(format!("{id} · {column}"));
            Ok(SearchHit {
                kind: SearchKind::Cards,
                id: id.clone(),
                title,
                subtitle,
                snippet: non_empty(snippet),
                project_id: Some(pid),
                project_name: Some(project_name),
                item_id: Some(id),
                channel_id: None,
                from_agent: Some(who),
                to_agent: None,
                artifact_kind: None,
                column: Some(column),
                timestamp: ts,
                score,
            })
        },
    )?;

    let hits: Vec<SearchHit> = rows.collect::<rusqlite::Result<_>>()?;
    let next_cursor = next_cursor_for(offset, limit, total, hits.len());
    Ok(SearchGroup {
        kind: SearchKind::Cards,
        total,
        hits,
        next_cursor,
    })
}

fn search_plans(
    conn: &Connection,
    fts_query: &str,
    project_id: Option<i64>,
    limit: usize,
    offset: usize,
    now_ms: i64,
) -> Result<SearchGroup, DbError> {
    let total: usize = conn.query_row(
        "SELECT COUNT(*) \
         FROM search_plans_fts f \
         JOIN artifacts a ON a.rowid = f.rowid \
         WHERE search_plans_fts MATCH ?1 \
           AND (?2 IS NULL OR a.project_id = ?2)",
        params![fts_query, project_id],
        |row| row.get::<_, i64>(0),
    )? as usize;

    if total == 0 {
        return Ok(empty_group(SearchKind::Plans));
    }

    let mut stmt = conn.prepare(
        "SELECT a.project_id, p.name, a.id, a.item_id, a.title, a.summary, a.author, a.updated_at, \
                snippet(search_plans_fts, -1, ?5, ?6, ?7, ?8) AS snip, \
                (-bm25(search_plans_fts, 8.0, 5.0, 2.0, 4.0) \
                 + (1.0 / (1.0 + MAX(0.0, (?3 - a.updated_at) / ?4) / ?9))) AS rank_score \
         FROM search_plans_fts f \
         JOIN artifacts a ON a.rowid = f.rowid \
         JOIN projects p ON p.id = a.project_id \
         WHERE search_plans_fts MATCH ?1 \
           AND (?2 IS NULL OR a.project_id = ?2) \
         ORDER BY rank_score DESC, a.updated_at DESC, a.rowid DESC \
         LIMIT ?10 OFFSET ?11",
    )?;

    let rows = stmt.query_map(
        params![
            fts_query,
            project_id,
            now_ms as f64,
            MS_PER_DAY,
            SNIPPET_START,
            SNIPPET_END,
            SNIPPET_ELLIPSIS,
            SNIPPET_TOKENS,
            RECENCY_HALF_LIFE_DAYS,
            limit as i64,
            offset as i64,
        ],
        |row| {
            let pid: i64 = row.get(0)?;
            let project_name: String = row.get(1)?;
            let id: String = row.get(2)?;
            let item_id: String = row.get(3)?;
            let title: String = row.get(4)?;
            let summary: String = row.get(5)?;
            let author: Option<String> = row.get(6)?;
            let updated_at: i64 = row.get(7)?;
            let snippet: Option<String> = row.get(8)?;
            let score: f64 = row.get(9)?;
            Ok(SearchHit {
                kind: SearchKind::Plans,
                id,
                title,
                subtitle: non_empty(Some(summary)).or_else(|| Some(item_id.clone())),
                snippet: non_empty(snippet),
                project_id: Some(pid),
                project_name: Some(project_name),
                item_id: Some(item_id),
                channel_id: None,
                from_agent: author,
                to_agent: None,
                artifact_kind: Some("plano".to_string()),
                column: None,
                timestamp: Some(updated_at),
                score,
            })
        },
    )?;

    let hits: Vec<SearchHit> = rows.collect::<rusqlite::Result<_>>()?;
    let next_cursor = next_cursor_for(offset, limit, total, hits.len());
    Ok(SearchGroup {
        kind: SearchKind::Plans,
        total,
        hits,
        next_cursor,
    })
}

fn search_artifacts(
    conn: &Connection,
    fts_query: &str,
    project_id: Option<i64>,
    limit: usize,
    offset: usize,
    now_ms: i64,
) -> Result<SearchGroup, DbError> {
    let total: usize = conn.query_row(
        "SELECT COUNT(*) \
         FROM search_artifacts_fts f \
         JOIN artifact_documents d ON d.rowid = f.rowid \
         WHERE search_artifacts_fts MATCH ?1 \
           AND (?2 IS NULL OR d.project_id = ?2)",
        params![fts_query, project_id],
        |row| row.get::<_, i64>(0),
    )? as usize;

    if total == 0 {
        return Ok(empty_group(SearchKind::Artifacts));
    }

    let mut stmt = conn.prepare(
        "SELECT d.project_id, p.name, d.id, d.kind, d.title, d.summary, d.author, d.updated_at, \
                snippet(search_artifacts_fts, -1, ?5, ?6, ?7, ?8) AS snip, \
                (-bm25(search_artifacts_fts, 8.0, 5.0, 2.0, 4.0) \
                 + (1.0 / (1.0 + MAX(0.0, (?3 - d.updated_at) / ?4) / ?9))) AS rank_score \
         FROM search_artifacts_fts f \
         JOIN artifact_documents d ON d.rowid = f.rowid \
         JOIN projects p ON p.id = d.project_id \
         WHERE search_artifacts_fts MATCH ?1 \
           AND (?2 IS NULL OR d.project_id = ?2) \
         ORDER BY rank_score DESC, d.updated_at DESC, d.rowid DESC \
         LIMIT ?10 OFFSET ?11",
    )?;

    let rows = stmt.query_map(
        params![
            fts_query,
            project_id,
            now_ms as f64,
            MS_PER_DAY,
            SNIPPET_START,
            SNIPPET_END,
            SNIPPET_ELLIPSIS,
            SNIPPET_TOKENS,
            RECENCY_HALF_LIFE_DAYS,
            limit as i64,
            offset as i64,
        ],
        |row| {
            let pid: i64 = row.get(0)?;
            let project_name: String = row.get(1)?;
            let id: String = row.get(2)?;
            let doc_kind: String = row.get(3)?;
            let title: String = row.get(4)?;
            let summary: String = row.get(5)?;
            let author: Option<String> = row.get(6)?;
            let updated_at: i64 = row.get(7)?;
            let snippet: Option<String> = row.get(8)?;
            let score: f64 = row.get(9)?;
            Ok(SearchHit {
                kind: SearchKind::Artifacts,
                id,
                title,
                subtitle: non_empty(Some(summary)),
                snippet: non_empty(snippet),
                project_id: Some(pid),
                project_name: Some(project_name),
                item_id: None,
                channel_id: None,
                from_agent: author,
                to_agent: None,
                artifact_kind: Some(doc_kind),
                column: None,
                timestamp: Some(updated_at),
                score,
            })
        },
    )?;

    let hits: Vec<SearchHit> = rows.collect::<rusqlite::Result<_>>()?;
    let next_cursor = next_cursor_for(offset, limit, total, hits.len());
    Ok(SearchGroup {
        kind: SearchKind::Artifacts,
        total,
        hits,
        next_cursor,
    })
}

fn search_messages(
    conn: &Connection,
    fts_query: &str,
    project_id: Option<i64>,
    limit: usize,
    offset: usize,
    now_ms: i64,
) -> Result<SearchGroup, DbError> {
    let total: usize = conn.query_row(
        "SELECT COUNT(*) \
         FROM search_messages_fts f \
         JOIN messages m ON m.id = f.rowid \
         WHERE search_messages_fts MATCH ?1 \
           AND (?2 IS NULL OR m.project_id = ?2)",
        params![fts_query, project_id],
        |row| row.get::<_, i64>(0),
    )? as usize;

    if total == 0 {
        return Ok(empty_group(SearchKind::Messages));
    }

    let mut stmt = conn.prepare(
        "SELECT m.id, m.project_id, p.name, m.from_agent, m.to_agent, m.text, m.created_at, \
                m.channel_id, m.item_id, \
                snippet(search_messages_fts, -1, ?5, ?6, ?7, ?8) AS snip, \
                (-bm25(search_messages_fts, 6.0, 2.0, 1.5, 4.0) \
                 + (1.0 / (1.0 + MAX(0.0, (?3 - m.created_at) / ?4) / ?9))) AS rank_score \
         FROM search_messages_fts f \
         JOIN messages m ON m.id = f.rowid \
         JOIN projects p ON p.id = m.project_id \
         WHERE search_messages_fts MATCH ?1 \
           AND (?2 IS NULL OR m.project_id = ?2) \
         ORDER BY rank_score DESC, m.created_at DESC, m.id DESC \
         LIMIT ?10 OFFSET ?11",
    )?;

    let rows = stmt.query_map(
        params![
            fts_query,
            project_id,
            now_ms as f64,
            MS_PER_DAY,
            SNIPPET_START,
            SNIPPET_END,
            SNIPPET_ELLIPSIS,
            SNIPPET_TOKENS,
            RECENCY_HALF_LIFE_DAYS,
            limit as i64,
            offset as i64,
        ],
        |row| {
            let id: i64 = row.get(0)?;
            let pid: i64 = row.get(1)?;
            let project_name: String = row.get(2)?;
            let from_agent: String = row.get(3)?;
            let to_agent: String = row.get(4)?;
            let text: String = row.get(5)?;
            let created_at: i64 = row.get(6)?;
            let channel_id: Option<i64> = row.get(7)?;
            let item_id: Option<String> = row.get(8)?;
            let snippet: Option<String> = row.get(9)?;
            let score: f64 = row.get(10)?;
            let title = compact_title(&text, 96);
            let title = if title.is_empty() {
                format!("{from_agent} → {to_agent}")
            } else {
                title
            };
            let subtitle = item_id
                .as_ref()
                .map(|card| format!("{from_agent} → {to_agent} · {card}"))
                .or_else(|| Some(format!("{from_agent} → {to_agent}")));
            Ok(SearchHit {
                kind: SearchKind::Messages,
                id: id.to_string(),
                title,
                subtitle,
                snippet: non_empty(snippet),
                project_id: Some(pid),
                project_name: Some(project_name),
                item_id,
                channel_id,
                from_agent: Some(from_agent),
                to_agent: Some(to_agent),
                artifact_kind: None,
                column: None,
                timestamp: Some(created_at),
                score,
            })
        },
    )?;

    let hits: Vec<SearchHit> = rows.collect::<rusqlite::Result<_>>()?;
    let next_cursor = next_cursor_for(offset, limit, total, hits.len());
    Ok(SearchGroup {
        kind: SearchKind::Messages,
        total,
        hits,
        next_cursor,
    })
}

// Memórias ainda não têm tabela dedicada no SQLite; quando ela existir com `search_memories_fts`, entra automaticamente.
fn search_memories(
    conn: &Connection,
    fts_query: &str,
    project_id: Option<i64>,
    limit: usize,
    offset: usize,
    now_ms: i64,
) -> Result<SearchGroup, DbError> {
    if !table_exists(conn, "project_memories")? || !table_exists(conn, "search_memories_fts")? {
        return Ok(empty_group(SearchKind::Memories));
    }

    let total: usize = conn.query_row(
        "SELECT COUNT(*) \
         FROM search_memories_fts f \
         JOIN project_memories m ON m.rowid = f.rowid \
         WHERE search_memories_fts MATCH ?1 \
           AND (?2 IS NULL OR m.project_id = ?2)",
        params![fts_query, project_id],
        |row| row.get::<_, i64>(0),
    )? as usize;

    if total == 0 {
        return Ok(empty_group(SearchKind::Memories));
    }

    let mut stmt = conn.prepare(
        "SELECT m.id, m.project_id, p.name, m.fact, m.source, m.updated_at, \
                snippet(search_memories_fts, -1, ?5, ?6, ?7, ?8) AS snip, \
                (-bm25(search_memories_fts, 8.0, 3.0) \
                 + (1.0 / (1.0 + MAX(0.0, (?3 - m.updated_at) / ?4) / ?9))) AS rank_score \
         FROM search_memories_fts f \
         JOIN project_memories m ON m.rowid = f.rowid \
         JOIN projects p ON p.id = m.project_id \
         WHERE search_memories_fts MATCH ?1 \
           AND (?2 IS NULL OR m.project_id = ?2) \
         ORDER BY rank_score DESC, m.updated_at DESC, m.rowid DESC \
         LIMIT ?10 OFFSET ?11",
    )?;

    let rows = stmt.query_map(
        params![
            fts_query,
            project_id,
            now_ms as f64,
            MS_PER_DAY,
            SNIPPET_START,
            SNIPPET_END,
            SNIPPET_ELLIPSIS,
            SNIPPET_TOKENS,
            RECENCY_HALF_LIFE_DAYS,
            limit as i64,
            offset as i64,
        ],
        |row| {
            let id: String = row.get(0)?;
            let pid: i64 = row.get(1)?;
            let project_name: String = row.get(2)?;
            let fact: String = row.get(3)?;
            let source: Option<String> = row.get(4)?;
            let updated_at: Option<i64> = row.get(5)?;
            let snippet: Option<String> = row.get(6)?;
            let score: f64 = row.get(7)?;
            Ok(SearchHit {
                kind: SearchKind::Memories,
                id,
                title: compact_title(&fact, 120),
                subtitle: non_empty(source),
                snippet: non_empty(snippet),
                project_id: Some(pid),
                project_name: Some(project_name),
                item_id: None,
                channel_id: None,
                from_agent: None,
                to_agent: None,
                artifact_kind: None,
                column: None,
                timestamp: updated_at,
                score,
            })
        },
    )?;

    let hits: Vec<SearchHit> = rows.collect::<rusqlite::Result<_>>()?;
    let next_cursor = next_cursor_for(offset, limit, total, hits.len());
    Ok(SearchGroup {
        kind: SearchKind::Memories,
        total,
        hits,
        next_cursor,
    })
}

#[cfg(test)]
#[path = "../../tests/search_tests.rs"]
mod tests;
