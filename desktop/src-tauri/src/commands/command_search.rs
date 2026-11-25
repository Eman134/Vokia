use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use super::dispatcher::{catalog, CommandMeta, Exposure};

pub const COMMAND_SEARCH_DEFAULT_LIMIT: usize = 5;
pub const COMMAND_SEARCH_MAX_LIMIT: usize = 10;
pub const COMMAND_SEARCH_MAX_STRONG_INLINE: usize = 3;

// Verbos genéricos ignorados ao avaliar se a consulta cobre todas as palavras significativas do nome do comando.
const GENERIC_VERBS: &[&str] = &["get", "list", "set", "read", "load", "show", "fetch", "find"];

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandSearchArgs {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub queries: Option<Vec<String>>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandParamSpec {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub enum_values: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandSearchHit {
    pub name: String,
    pub domain: String,
    pub description: String,
    pub when_to_use: String,
    pub match_kind: &'static str,
    pub strong: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Vec<CommandParamSpec>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example_call: Option<Value>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandSearchResponse {
    pub queries: Vec<String>,
    pub total: usize,
    pub results: Vec<CommandSearchHit>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MatchTier {
    Exact,
    AllWords,
    Partial,
}

impl MatchTier {
    fn rank(self) -> u8 {
        match self {
            MatchTier::Exact => 0,
            MatchTier::AllWords => 1,
            MatchTier::Partial => 2,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            MatchTier::Exact => "exact",
            MatchTier::AllWords => "all_words",
            MatchTier::Partial => "partial",
        }
    }
}

#[derive(Clone, Debug)]
struct ScoredCommand {
    catalog_index: usize,
    tier: MatchTier,
    strong: bool,
    matched_name_tokens: usize,
    matched_total_tokens: usize,
}

impl ScoredCommand {
    fn cmp_rank(&self, other: &Self) -> Ordering {
        self.tier
            .rank()
            .cmp(&other.tier.rank())
            .then_with(|| other.strong.cmp(&self.strong))
            .then_with(|| other.matched_name_tokens.cmp(&self.matched_name_tokens))
            .then_with(|| other.matched_total_tokens.cmp(&self.matched_total_tokens))
            .then_with(|| self.catalog_index.cmp(&other.catalog_index))
    }
}

fn fold_char(ch: char) -> char {
    match ch.to_lowercase().next().unwrap_or(ch) {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        other => other,
    }
}

fn tokenize(raw: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in raw.chars() {
        if ch.is_alphanumeric() {
            current.push(fold_char(ch));
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn singular_stem(token: &str) -> &str {
    if token.len() >= 4 {
        if let Some(stem) = token.strip_suffix('s') {
            if !stem.ends_with('s') {
                return stem;
            }
        }
    }
    token
}

fn token_matches_word(query_tok: &str, target_tok: &str) -> bool {
    query_tok == target_tok || singular_stem(query_tok) == singular_stem(target_tok)
}

fn token_partial_match(query_tok: &str, target_tok: &str) -> bool {
    if token_matches_word(query_tok, target_tok) {
        return true;
    }
    if query_tok.len() >= 3 && target_tok.contains(query_tok) {
        return true;
    }
    false
}

fn canonical_command_query(raw: &str) -> String {
    let trimmed = raw.trim().to_ascii_lowercase();
    let stripped = trimmed.strip_prefix("app:").unwrap_or(&trimmed);
    stripped
        .chars()
        .map(|ch| if ch == '-' || ch.is_whitespace() { '_' } else { ch })
        .collect()
}

fn score_against_query(
    catalog_index: usize,
    command: &CommandMeta,
    raw_query: &str,
) -> Option<ScoredCommand> {
    let q_tokens = tokenize(raw_query);
    if q_tokens.is_empty() {
        return None;
    }

    let name_tokens = tokenize(command.name);
    let domain_tokens = tokenize(command.domain);
    let desc_tokens = tokenize(command.description);
    let when_tokens = tokenize(command.when_to_use);

    let canonical = canonical_command_query(raw_query);
    let is_exact = canonical == command.name || q_tokens == name_tokens;

    let all_name_words = !name_tokens.is_empty()
        && name_tokens
            .iter()
            .all(|nt| q_tokens.iter().any(|qt| token_matches_word(qt, nt)));

    let significant_name_tokens: Vec<&String> = name_tokens
        .iter()
        .filter(|tok| !GENERIC_VERBS.contains(&tok.as_str()))
        .collect();

    let all_significant_words = !significant_name_tokens.is_empty()
        && significant_name_tokens
            .iter()
            .all(|sig| q_tokens.iter().any(|qt| token_matches_word(qt, sig)));

    let matched_name_tokens = q_tokens
        .iter()
        .filter(|qt| name_tokens.iter().any(|nt| token_partial_match(qt, nt)))
        .count();

    let matched_total_tokens = q_tokens
        .iter()
        .filter(|qt| {
            name_tokens.iter().any(|nt| token_partial_match(qt, nt))
                || domain_tokens.iter().any(|dt| token_partial_match(qt, dt))
                || desc_tokens.iter().any(|dt| token_partial_match(qt, dt))
                || when_tokens.iter().any(|wt| token_partial_match(qt, wt))
        })
        .count();

    if !is_exact && !all_name_words && matched_total_tokens == 0 {
        return None;
    }

    let tier = if is_exact {
        MatchTier::Exact
    } else if all_name_words {
        MatchTier::AllWords
    } else {
        MatchTier::Partial
    };

    let strong = is_exact || all_name_words || all_significant_words;

    Some(ScoredCommand {
        catalog_index,
        tier,
        strong,
        matched_name_tokens,
        matched_total_tokens,
    })
}

fn collect_queries(args: &CommandSearchArgs) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(single) = args.query.as_deref() {
        let trimmed = single.trim();
        if !trimmed.is_empty() {
            out.push(trimmed.to_string());
        }
    }
    if let Some(list) = args.queries.as_ref() {
        for item in list {
            let trimmed = item.trim();
            if !trimmed.is_empty() && !out.iter().any(|existing| existing == trimmed) {
                out.push(trimmed.to_string());
            }
        }
    }
    out
}

fn extract_param_specs(schema: &Value) -> Vec<CommandParamSpec> {
    let required_set: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let Some(props) = schema.get("properties").and_then(Value::as_object) else {
        return Vec::new();
    };

    let mut specs: Vec<CommandParamSpec> = props
        .iter()
        .map(|(name, prop)| {
            let param_type = match prop.get("type") {
                Some(Value::String(kind)) => kind.clone(),
                Some(Value::Array(kinds)) => kinds
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("|"),
                _ => "any".to_string(),
            };
            let description = prop
                .get("description")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(str::to_string);
            let enum_values = prop
                .get("enum")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            CommandParamSpec {
                name: name.clone(),
                param_type,
                required: required_set.contains(&name.as_str()),
                description,
                enum_values,
            }
        })
        .collect();

    specs.sort_by(|a, b| b.required.cmp(&a.required).then_with(|| a.name.cmp(&b.name)));
    specs
}

fn example_value_for_schema(name: &str, prop: &Value) -> Value {
    if let Some(first_enum) = prop
        .get("enum")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
    {
        return first_enum.clone();
    }
    let kind = prop.get("type").and_then(Value::as_str).unwrap_or("string");
    match kind {
        "integer" | "number" => json!(1),
        "boolean" => json!(true),
        "array" => json!([]),
        "object" => json!({}),
        _ => json!(format!("<{name}>")),
    }
}

fn build_example_call(command_name: &str, schema: &Value) -> Value {
    let required_set: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let mut args = Map::new();
    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        if !required_set.is_empty() {
            for req in &required_set {
                if let Some(prop) = props.get(*req) {
                    args.insert((*req).to_string(), example_value_for_schema(req, prop));
                }
            }
        } else {
            for (name, prop) in props.iter().take(2) {
                args.insert(name.clone(), example_value_for_schema(name, prop));
            }
        }
    }

    json!({
        "command": command_name,
        "arguments": Value::Object(args),
    })
}

pub fn search_commands(commands: &[CommandMeta], args: &CommandSearchArgs) -> CommandSearchResponse {
    let queries = collect_queries(args);
    let limit = args
        .limit
        .unwrap_or(COMMAND_SEARCH_DEFAULT_LIMIT)
        .clamp(1, COMMAND_SEARCH_MAX_LIMIT);
    let domain_filter = args
        .domain
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if queries.is_empty() {
        return CommandSearchResponse {
            queries,
            total: 0,
            results: Vec::new(),
        };
    }

    let mut scored: Vec<ScoredCommand> = Vec::new();
    for (index, command) in commands.iter().enumerate() {
        if command.exposure != Exposure::AppAndCli {
            continue;
        }
        if let Some(domain) = domain_filter {
            if command.domain != domain {
                continue;
            }
        }
        let best = queries
            .iter()
            .filter_map(|query| score_against_query(index, command, query))
            .min_by(ScoredCommand::cmp_rank);
        if let Some(entry) = best {
            scored.push(entry);
        }
    }

    scored.sort_by(ScoredCommand::cmp_rank);
    let total = scored.len();

    let mut strong_inlined = 0usize;
    let mut results = Vec::with_capacity(limit.min(total));
    for entry in scored.into_iter().take(limit) {
        let command = &commands[entry.catalog_index];
        let include_inline = entry.strong && strong_inlined < COMMAND_SEARCH_MAX_STRONG_INLINE;
        let (input_schema, parameters, example_call) = if include_inline {
            strong_inlined += 1;
            let schema = (command.schema)();
            let params = extract_param_specs(&schema);
            let example = build_example_call(command.name, &schema);
            (Some(schema), Some(params), Some(example))
        } else {
            (None, None, None)
        };

        results.push(CommandSearchHit {
            name: command.name.to_string(),
            domain: command.domain.to_string(),
            description: command.description.to_string(),
            when_to_use: command.when_to_use.to_string(),
            match_kind: entry.tier.as_str(),
            strong: entry.strong,
            input_schema,
            parameters,
            example_call,
        });
    }

    CommandSearchResponse {
        queries,
        total,
        results,
    }
}

pub(crate) fn command_search_exec(args: CommandSearchArgs) -> Result<CommandSearchResponse, String> {
    Ok(search_commands(catalog(), &args))
}

#[tauri::command]
pub fn command_search(
    query: Option<String>,
    queries: Option<Vec<String>>,
    domain: Option<String>,
    limit: Option<usize>,
) -> Result<CommandSearchResponse, String> {
    command_search_exec(CommandSearchArgs {
        query,
        queries,
        domain,
        limit,
    })
}

#[cfg(test)]
#[path = "../../tests/context_retrieval_tests.rs"]
mod tests;
