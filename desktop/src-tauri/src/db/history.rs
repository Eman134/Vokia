use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use super::connection::Db;
use super::error::DbError;
use super::messages::StoredTaskResult;
use super::search::build_fts_query;
use super::work_steps::TurnWorkStepsRecord;

pub const HISTORY_SEARCH_DEFAULT_LIMIT: usize = 5;
pub const HISTORY_SEARCH_MAX_LIMIT: usize = 10;
pub const HISTORY_SEARCH_EXCERPT_MAX_CHARS: usize = 400;
pub const HISTORY_SEARCH_MAX_TOTAL_CHARS: usize = 8_000;

pub const HISTORY_READ_DEFAULT_LIMIT: usize = 10;
pub const HISTORY_READ_MAX_LIMIT: usize = 20;
pub const HISTORY_READ_MAX_TOTAL_CHARS: usize = 16_000;

pub const CONTEXT_RESET_MARKER: &str = "[context reset]";

const WORK_STEPS_CANDIDATE_BATCH: usize = 20;
const WORK_STEPS_RESERVED_BUDGET_CHARS: usize = 4_500;
const MS_PER_DAY: f64 = 86_400_000.0;
const RECENCY_HALF_LIFE_DAYS: f64 = 30.0;

#[derive(Clone, Debug, Default)]
pub struct HistorySearchQuery {
    pub project_id: i64,
    pub query: String,
    pub agent: Option<String>,
    pub channel_id: Option<i64>,
    pub card_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySearchHit {
    pub message_id: i64,
    pub timestamp: i64,
    pub from: String,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    pub excerpt: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySearchResponse {
    pub project_id: i64,
    pub query: String,
    pub total: usize,
    pub truncated: bool,
    pub results: Vec<HistorySearchHit>,
}

#[derive(Clone, Debug, Default)]
pub struct HistoryReadQuery {
    pub project_id: i64,
    pub message_id: Option<i64>,
    pub before_message_id: Option<i64>,
    pub limit: Option<usize>,
    pub offset: usize,
    pub agent: Option<String>,
    pub channel_id: Option<i64>,
    pub card_id: Option<String>,
    pub include_work_steps: bool,
    pub work_steps_offset: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryReadMessage {
    pub message_id: i64,
    pub timestamp: i64,
    pub from: String,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    pub text: String,
    pub total_chars: usize,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_offset: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryWorkStepEntry {
    pub id: i64,
    pub agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub block: String,
    pub step_count: usize,
    pub omitted_count: usize,
    pub created_at: i64,
}

impl From<TurnWorkStepsRecord> for HistoryWorkStepEntry {
    fn from(record: TurnWorkStepsRecord) -> Self {
        Self {
            id: record.id,
            agent: record.agent,
            card_id: record.card_id,
            provider: record.provider,
            block: record.block,
            step_count: record.step_count,
            omitted_count: record.omitted_count,
            created_at: record.created_at,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryReadResponse {
    pub project_id: i64,
    pub messages: Vec<HistoryReadMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_offset: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_before_message_id: Option<i64>,
    pub has_more: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub work_steps: Vec<HistoryWorkStepEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_work_steps_offset: Option<usize>,
}

#[derive(Clone, Debug)]
struct RawMessageRow {
    id: i64,
    project_id: i64,
    from_agent: String,
    to_agent: String,
    text: String,
    created_at: i64,
    channel_id: Option<i64>,
    item_id: Option<String>,
    task_result: Option<String>,
}

impl RawMessageRow {
    fn effective_text(&self) -> String {
        if !self.text.trim().is_empty() {
            return self.text.clone();
        }
        if let Some(raw_json) = self.task_result.as_deref() {
            if let Ok(result) = serde_json::from_str::<StoredTaskResult>(raw_json) {
                if !result.objective.trim().is_empty() {
                    return format!("[resultado da tarefa] {}", result.objective.trim());
                }
            }
        }
        self.text.clone()
    }
}

pub fn is_context_reset_marker(text: &str) -> bool {
    let Some(first_line) = text.lines().map(str::trim).find(|line| !line.is_empty()) else {
        return false;
    };
    let lower = first_line.to_ascii_lowercase();
    lower.starts_with(CONTEXT_RESET_MARKER)
        || lower.starts_with("[reset de contexto]")
        || lower.starts_with("[session reset]")
        || lower.starts_with("--- context reset ---")
        || lower == "/reset"
        || lower == "/clear"
}

fn ensure_project_exists(conn: &Connection, project_id: i64) -> Result<(), DbError> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
        [project_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(DbError::ProjectNotFound(project_id));
    }
    Ok(())
}

fn table_exists(conn: &Connection, table: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get(0),
    )
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn normalize_opt(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

fn decode_message_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawMessageRow> {
    Ok(RawMessageRow {
        id: row.get(0)?,
        project_id: row.get(1)?,
        from_agent: row.get(2)?,
        to_agent: row.get(3)?,
        text: row.get(4)?,
        created_at: row.get(5)?,
        channel_id: row.get(6)?,
        item_id: row.get(7)?,
        task_result: row.get(8)?,
    })
}

fn find_last_reset_marker_id(
    conn: &Connection,
    project_id: i64,
    channel_id: Option<i64>,
    card_id: Option<&str>,
    agent: Option<&str>,
) -> Result<Option<i64>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, text FROM messages \
         WHERE project_id = ?1 \
           AND ((?2 IS NULL AND channel_id IS NULL) OR (?2 IS NOT NULL AND (channel_id IS NULL OR channel_id = ?2))) \
           AND ((?3 IS NULL AND item_id IS NULL) OR (?3 IS NOT NULL AND (item_id IS NULL OR item_id = ?3))) \
           AND (?4 IS NULL OR from_agent = ?4 OR to_agent = ?4 OR from_agent = 'system') \
           AND ( \
               lower(trim(text)) LIKE '[context reset]%' \
               OR lower(trim(text)) LIKE '[reset de contexto]%' \
               OR lower(trim(text)) LIKE '[session reset]%' \
               OR lower(trim(text)) LIKE '--- context reset ---%' \
               OR lower(trim(text)) = '/reset' \
               OR lower(trim(text)) = '/clear' \
           ) \
         ORDER BY id DESC \
         LIMIT 20",
    )?;
    let rows = stmt.query_map(params![project_id, channel_id, card_id, agent], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, text) = row?;
        if is_context_reset_marker(&text) {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

fn search_messages_fts(
    conn: &Connection,
    fts_query: &str,
    project_id: i64,
    reset_after_id: Option<i64>,
    channel_id: Option<i64>,
    card_id: Option<&str>,
    agent: Option<&str>,
    fetch_limit: usize,
) -> Result<Vec<RawMessageRow>, DbError> {
    let now_ms = now_millis();
    let mut stmt = conn.prepare(
        "SELECT m.id, m.project_id, m.from_agent, m.to_agent, m.text, m.created_at, \
                m.channel_id, m.item_id, m.task_result \
         FROM search_messages_fts f \
         JOIN messages m ON m.id = f.rowid \
         WHERE search_messages_fts MATCH ?1 \
           AND m.project_id = ?2 \
           AND (?3 IS NULL OR m.id > ?3) \
           AND (?4 IS NULL OR m.channel_id = ?4) \
           AND (?5 IS NULL OR m.item_id = ?5) \
           AND (?6 IS NULL OR m.from_agent = ?6 OR m.to_agent = ?6) \
         ORDER BY ( \
             -bm25(search_messages_fts, 6.0, 2.0, 1.5, 4.0) \
             + (1.0 / (1.0 + MAX(0.0, (?7 - m.created_at) / ?8) / ?9)) \
         ) DESC, m.created_at DESC, m.id DESC \
         LIMIT ?10",
    )?;
    let rows = stmt.query_map(
        params![
            fts_query,
            project_id,
            reset_after_id,
            channel_id,
            card_id,
            agent,
            now_ms as f64,
            MS_PER_DAY,
            RECENCY_HALF_LIFE_DAYS,
            fetch_limit as i64,
        ],
        decode_message_row,
    )?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(DbError::from)
}

fn escape_like_pattern(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len() + 2);
    escaped.push('%');
    for ch in raw.chars() {
        match ch {
            '\\' | '%' | '_' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            other => escaped.push(other),
        }
    }
    escaped.push('%');
    escaped
}

fn search_messages_like(
    conn: &Connection,
    raw_query: &str,
    project_id: i64,
    reset_after_id: Option<i64>,
    channel_id: Option<i64>,
    card_id: Option<&str>,
    agent: Option<&str>,
    fetch_limit: usize,
) -> Result<Vec<RawMessageRow>, DbError> {
    let terms: Vec<String> = raw_query
        .split_whitespace()
        .filter(|term| !term.is_empty())
        .take(8)
        .map(|term| escape_like_pattern(&term.to_lowercase()))
        .collect();
    if terms.is_empty() {
        return Ok(Vec::new());
    }

    let mut sql = String::from(
        "SELECT m.id, m.project_id, m.from_agent, m.to_agent, m.text, m.created_at, \
                m.channel_id, m.item_id, m.task_result \
         FROM messages m \
         WHERE m.project_id = ?1 \
           AND (?2 IS NULL OR m.id > ?2) \
           AND (?3 IS NULL OR m.channel_id = ?3) \
           AND (?4 IS NULL OR m.item_id = ?4) \
           AND (?5 IS NULL OR m.from_agent = ?5 OR m.to_agent = ?5)",
    );

    let mut bind_values: Vec<rusqlite::types::Value> = vec![
        rusqlite::types::Value::Integer(project_id),
        reset_after_id
            .map(rusqlite::types::Value::Integer)
            .unwrap_or(rusqlite::types::Value::Null),
        channel_id
            .map(rusqlite::types::Value::Integer)
            .unwrap_or(rusqlite::types::Value::Null),
        card_id
            .map(|c| rusqlite::types::Value::Text(c.to_string()))
            .unwrap_or(rusqlite::types::Value::Null),
        agent
            .map(|a| rusqlite::types::Value::Text(a.to_string()))
            .unwrap_or(rusqlite::types::Value::Null),
    ];

    for term in terms {
        let idx = bind_values.len() + 1;
        sql.push_str(&format!(
            " AND (lower(m.text) LIKE ?{idx} ESCAPE '\\' OR lower(COALESCE(m.task_result, '')) LIKE ?{idx} ESCAPE '\\')"
        ));
        bind_values.push(rusqlite::types::Value::Text(term));
    }

    let limit_idx = bind_values.len() + 1;
    sql.push_str(&format!(
        " ORDER BY m.created_at DESC, m.id DESC LIMIT ?{limit_idx}"
    ));
    bind_values.push(rusqlite::types::Value::Integer(fetch_limit as i64));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(bind_values), decode_message_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(DbError::from)
}

fn serialized_char_len<T: Serialize>(value: &T) -> usize {
    serde_json::to_string(value)
        .map(|json| json.chars().count())
        .unwrap_or(usize::MAX)
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

fn fold_str(input: &str) -> String {
    input.chars().map(fold_char).collect()
}

fn build_excerpt(text: &str, raw_query: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() || max_chars == 0 {
        return String::new();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= max_chars {
        return trimmed.to_string();
    }

    let folded_chars: Vec<char> = chars.iter().copied().map(fold_char).collect();
    let folded_str: String = folded_chars.iter().collect();

    let query_terms: Vec<String> = raw_query
        .split_whitespace()
        .map(fold_str)
        .filter(|term| !term.is_empty())
        .collect();

    let mut match_char_idx: Option<usize> = None;
    for term in &query_terms {
        if let Some(byte_pos) = folded_str.find(term.as_str()) {
            let char_pos = folded_str[..byte_pos].chars().count();
            if match_char_idx.is_none_or(|best| char_pos < best) {
                match_char_idx = Some(char_pos);
            }
        }
    }

    let Some(center) = match_char_idx else {
        return truncate_chars_with_ellipsis(&chars, max_chars);
    };

    // Janela centrada no primeiro termo encontrado, respeitando exatamente `max_chars` (incluindo reticências).
    let half = max_chars / 2;
    let mut start = center.saturating_sub(half);
    let mut end = (start + max_chars).min(chars.len());
    if end == chars.len() {
        start = end.saturating_sub(max_chars);
    }

    let need_prefix = start > 0;
    let need_suffix = end < chars.len();
    let ellipsis_overhead = usize::from(need_prefix) + usize::from(need_suffix);
    let content_limit = max_chars.saturating_sub(ellipsis_overhead);
    if end.saturating_sub(start) > content_limit {
        let excess = end.saturating_sub(start) - content_limit;
        if need_suffix {
            end = end.saturating_sub(excess);
        } else {
            start = start.saturating_add(excess);
        }
    }

    let mut out = String::with_capacity(max_chars);
    if need_prefix {
        out.push('…');
    }
    for &ch in &chars[start..end] {
        out.push(ch);
    }
    if need_suffix {
        out.push('…');
    }
    out
}

fn truncate_chars_with_ellipsis(chars: &[char], max_chars: usize) -> String {
    if chars.len() <= max_chars {
        return chars.iter().collect();
    }
    if max_chars <= 1 {
        return "…".chars().take(max_chars).collect();
    }
    let mut out: String = chars[..max_chars - 1].iter().collect();
    out.push('…');
    out
}

fn infer_agent_from_message(row: &RawMessageRow) -> Option<&str> {
    if row.from_agent != "user" && !row.from_agent.trim().is_empty() {
        Some(row.from_agent.as_str())
    } else if row.to_agent != "user" && !row.to_agent.trim().is_empty() {
        Some(row.to_agent.as_str())
    } else {
        None
    }
}

fn make_read_message(row: &RawMessageRow, text: String, total_chars: usize, next_offset: Option<usize>) -> HistoryReadMessage {
    HistoryReadMessage {
        message_id: row.id,
        timestamp: row.created_at,
        from: row.from_agent.clone(),
        to: row.to_agent.clone(),
        channel_id: row.channel_id,
        item_id: row.item_id.clone(),
        text,
        total_chars,
        truncated: next_offset.is_some(),
        next_offset,
    }
}

// Escolhe por busca binária o maior prefixo de `chars[start..]` que mantém o JSON serializado dentro de `max_total_chars`.
fn fit_message_slice(
    row: &RawMessageRow,
    chars: &[char],
    start: usize,
    build_response: &impl Fn(HistoryReadMessage) -> HistoryReadResponse,
    max_total_chars: usize,
) -> (HistoryReadMessage, bool) {
    let total = chars.len();
    let clamped_start = start.min(total);
    let remaining_len = total.saturating_sub(clamped_start);

    // Tenta caber inteiro primeiro (caminho rápido).
    let full_slice: String = chars[clamped_start..].iter().collect();
    let full_msg = make_read_message(row, full_slice, total, None);
    if serialized_char_len(&build_response(full_msg.clone())) <= max_total_chars {
        return (full_msg, true);
    }

    if remaining_len == 0 {
        return (full_msg, false);
    }

    let mut lo = 0usize;
    let mut hi = remaining_len;
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        let end = clamped_start + mid;
        let next_offset = (end < total).then_some(end);
        let slice: String = chars[clamped_start..end].iter().collect();
        let candidate = make_read_message(row, slice, total, next_offset);
        if serialized_char_len(&build_response(candidate)) <= max_total_chars {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }

    let end = clamped_start + lo;
    let next_offset = (end < total).then_some(end);
    let slice: String = chars[clamped_start..end].iter().collect();
    let fitted = make_read_message(row, slice, total, next_offset);
    (fitted, lo > 0)
}

fn pack_work_steps(
    response: &mut HistoryReadResponse,
    candidates: &[TurnWorkStepsRecord],
    db_has_more: bool,
    work_steps_offset: usize,
    budget_cap: usize,
) {
    if candidates.is_empty() {
        response.next_work_steps_offset = None;
        return;
    }
    let start_count = response.work_steps.len();
    let mut idx = start_count;
    while idx < candidates.len() {
        let entry = HistoryWorkStepEntry::from(candidates[idx].clone());
        let has_remaining_after = (idx + 1) < candidates.len() || db_has_more;
        response.work_steps.push(entry);
        response.next_work_steps_offset =
            has_remaining_after.then_some(work_steps_offset + response.work_steps.len());

        if serialized_char_len(response) <= budget_cap {
            idx += 1;
        } else {
            response.work_steps.pop();
            response.next_work_steps_offset = Some(work_steps_offset + response.work_steps.len());
            break;
        }
    }

    if response.work_steps.len() == candidates.len() && !db_has_more {
        response.next_work_steps_offset = None;
    } else if response.work_steps.len() < candidates.len() || db_has_more {
        response.next_work_steps_offset = Some(work_steps_offset + response.work_steps.len());
    }
}

impl Db {
    pub fn history_search(
        &self,
        query: &HistorySearchQuery,
    ) -> Result<HistorySearchResponse, DbError> {
        let conn = self.conn()?;
        ensure_project_exists(&conn, query.project_id)?;

        let trimmed_query = query.query.trim();
        if trimmed_query.is_empty() {
            return Ok(HistorySearchResponse {
                project_id: query.project_id,
                query: String::new(),
                total: 0,
                truncated: false,
                results: Vec::new(),
            });
        }

        let limit = query
            .limit
            .unwrap_or(HISTORY_SEARCH_DEFAULT_LIMIT)
            .clamp(1, HISTORY_SEARCH_MAX_LIMIT);
        let agent = normalize_opt(query.agent.as_deref());
        let card_id = normalize_opt(query.card_id.as_deref());
        let reset_after_id =
            find_last_reset_marker_id(&conn, query.project_id, query.channel_id, card_id, agent)?;

        let fetch_limit = limit.saturating_mul(3).max(limit + 5);
        let mut raw_hits: Vec<RawMessageRow> = Vec::new();
        let has_fts = table_exists(&conn, "search_messages_fts")?;
        if has_fts {
            if let Some(fts_query) = build_fts_query(trimmed_query) {
                if let Ok(rows) = search_messages_fts(
                    &conn,
                    &fts_query,
                    query.project_id,
                    reset_after_id,
                    query.channel_id,
                    card_id,
                    agent,
                    fetch_limit,
                ) {
                    raw_hits = rows;
                }
            }
        }
        if raw_hits.is_empty() {
            raw_hits = search_messages_like(
                &conn,
                trimmed_query,
                query.project_id,
                reset_after_id,
                query.channel_id,
                card_id,
                agent,
                fetch_limit,
            )?;
        }

        let mut response = HistorySearchResponse {
            project_id: query.project_id,
            query: trimmed_query.to_string(),
            total: 0,
            truncated: false,
            results: Vec::with_capacity(limit),
        };

        for row in raw_hits {
            if response.results.len() >= limit {
                break;
            }
            if row.project_id != query.project_id {
                continue;
            }
            if is_context_reset_marker(&row.text) {
                continue;
            }
            let effective = row.effective_text();
            if effective.trim().is_empty() {
                continue;
            }
            let excerpt = build_excerpt(&effective, trimmed_query, HISTORY_SEARCH_EXCERPT_MAX_CHARS);
            let hit = HistorySearchHit {
                message_id: row.id,
                timestamp: row.created_at,
                from: row.from_agent,
                to: row.to_agent,
                channel_id: row.channel_id,
                item_id: row.item_id,
                excerpt,
            };

            response.results.push(hit);
            response.total = response.results.len();
            if serialized_char_len(&response) > HISTORY_SEARCH_MAX_TOTAL_CHARS {
                response.results.pop();
                response.total = response.results.len();
                response.truncated = true;
                break;
            }
        }

        Ok(response)
    }

    pub fn history_read(&self, query: &HistoryReadQuery) -> Result<HistoryReadResponse, DbError> {
        let conn = self.conn()?;
        ensure_project_exists(&conn, query.project_id)?;

        let agent_filter = normalize_opt(query.agent.as_deref());
        let card_filter = normalize_opt(query.card_id.as_deref());

        if let Some(target_id) = query.message_id {
            let maybe_row = conn
                .query_row(
                    "SELECT id, project_id, from_agent, to_agent, text, created_at, channel_id, item_id, task_result \
                     FROM messages WHERE id = ?1",
                    [target_id],
                    decode_message_row,
                )
                .optional()?;
            let Some(row) = maybe_row else {
                return Err(DbError::MessageNotFound(target_id));
            };
            if row.project_id != query.project_id {
                return Err(DbError::CrossProjectAccess(format!(
                    "Acesso negado: a mensagem {target_id} pertence a outro projeto."
                )));
            }
            drop(conn);

            let effective_text = row.effective_text();
            let chars: Vec<char> = effective_text.chars().collect();

            let (ws_candidates, ws_db_has_more) = if query.include_work_steps {
                let ws_agent = agent_filter.or_else(|| infer_agent_from_message(&row));
                let ws_card = card_filter.or(row.item_id.as_deref());
                self.list_history_work_steps(
                    query.project_id,
                    ws_agent,
                    ws_card,
                    WORK_STEPS_CANDIDATE_BATCH,
                    query.work_steps_offset,
                )?
            } else {
                (Vec::new(), false)
            };

            let mut response = HistoryReadResponse {
                project_id: query.project_id,
                messages: Vec::new(),
                next_offset: None,
                next_cursor: None,
                next_before_message_id: None,
                has_more: false,
                work_steps: Vec::new(),
                next_work_steps_offset: None,
            };

            // Quando há work steps pedidos e disponíveis, reserva orçamento garantido para o primeiro bloco
            // (ou até ~4.5k chars) antes de alocar o restante para a mensagem longa, garantindo progresso nos dois cursores.
            if !ws_candidates.is_empty() {
                let remaining_msg_chars = chars.len().saturating_sub(query.offset.min(chars.len()));
                let initial_ws_budget = if remaining_msg_chars > 9_000 {
                    // Garante espaço para pelo menos 1 bloco (até ~4.5k chars), preservando ~11k para o texto da mensagem.
                    let base_len = serialized_char_len(&response);
                    (base_len + WORK_STEPS_RESERVED_BUDGET_CHARS).min(HISTORY_READ_MAX_TOTAL_CHARS / 2)
                } else {
                    HISTORY_READ_MAX_TOTAL_CHARS
                };
                pack_work_steps(
                    &mut response,
                    &ws_candidates,
                    ws_db_has_more,
                    query.work_steps_offset,
                    initial_ws_budget,
                );
                // Se nenhum bloco coube no orçamento reservado (ex.: bloco no teto de 4.000 chars com escapes JSON),
                // tenta incluir pelo menos o primeiro bloco antes de fatiar a mensagem.
                if response.work_steps.is_empty() {
                    pack_work_steps(
                        &mut response,
                        &ws_candidates[..1],
                        ws_candidates.len() > 1 || ws_db_has_more,
                        query.work_steps_offset,
                        HISTORY_READ_MAX_TOTAL_CHARS.saturating_sub(2_000),
                    );
                }
            }

            let fixed_work_steps = response.work_steps.clone();
            let fixed_next_ws = response.next_work_steps_offset;
            let (fitted_msg, _) = fit_message_slice(
                &row,
                &chars,
                query.offset,
                &|msg| HistoryReadResponse {
                    project_id: query.project_id,
                    next_offset: msg.next_offset,
                    messages: vec![msg],
                    next_cursor: None,
                    next_before_message_id: None,
                    has_more: false,
                    work_steps: fixed_work_steps.clone(),
                    next_work_steps_offset: fixed_next_ws,
                },
                HISTORY_READ_MAX_TOTAL_CHARS,
            );
            response.next_offset = fitted_msg.next_offset;
            response.messages.push(fitted_msg);

            // Reaproveita eventual sobra do orçamento de 16.000 chars para anexar mais blocos de `[work steps]`.
            if !ws_candidates.is_empty() && response.work_steps.len() < ws_candidates.len() {
                pack_work_steps(
                    &mut response,
                    &ws_candidates,
                    ws_db_has_more,
                    query.work_steps_offset,
                    HISTORY_READ_MAX_TOTAL_CHARS,
                );
            }

            return Ok(response);
        }

        // Modo lista: mensagens recentes ou página anterior a `before_message_id`.
        if let Some(before_id) = query.before_message_id {
            let owner: Option<i64> = conn
                .query_row(
                    "SELECT project_id FROM messages WHERE id = ?1",
                    [before_id],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(owner_project_id) = owner {
                if owner_project_id != query.project_id {
                    return Err(DbError::CrossProjectAccess(format!(
                        "Acesso negado: o cursor {before_id} pertence a outro projeto."
                    )));
                }
            }
        }

        let reset_after_id = find_last_reset_marker_id(
            &conn,
            query.project_id,
            query.channel_id,
            card_filter,
            agent_filter,
        )?;
        let limit = query
            .limit
            .unwrap_or(HISTORY_READ_DEFAULT_LIMIT)
            .clamp(1, HISTORY_READ_MAX_LIMIT);

        let mut stmt = conn.prepare(
            "SELECT id, project_id, from_agent, to_agent, text, created_at, channel_id, item_id, task_result \
             FROM messages \
             WHERE project_id = ?1 \
               AND (?2 IS NULL OR id > ?2) \
               AND (?3 IS NULL OR id < ?3) \
               AND (?4 IS NULL OR channel_id = ?4) \
               AND (?5 IS NULL OR item_id = ?5) \
               AND (?6 IS NULL OR from_agent = ?6 OR to_agent = ?6) \
             ORDER BY id DESC \
             LIMIT ?7",
        )?;
        let fetched: Vec<RawMessageRow> = stmt
            .query_map(
                params![
                    query.project_id,
                    reset_after_id,
                    query.before_message_id,
                    query.channel_id,
                    card_filter,
                    agent_filter,
                    (limit + 1) as i64,
                ],
                decode_message_row,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        drop(conn);

        let filtered: Vec<RawMessageRow> = fetched
            .into_iter()
            .filter(|row| !is_context_reset_marker(&row.text))
            .collect();
        let has_older_in_db = filtered.len() > limit;
        let page_newest_first: Vec<RawMessageRow> = filtered.into_iter().take(limit).collect();

        let (ws_candidates, ws_db_has_more) = if query.include_work_steps {
            self.list_history_work_steps(
                query.project_id,
                agent_filter,
                card_filter,
                WORK_STEPS_CANDIDATE_BATCH,
                query.work_steps_offset,
            )?
        } else {
            (Vec::new(), false)
        };

        let mut response = HistoryReadResponse {
            project_id: query.project_id,
            messages: Vec::new(),
            next_offset: None,
            next_cursor: None,
            next_before_message_id: None,
            has_more: has_older_in_db,
            work_steps: Vec::new(),
            next_work_steps_offset: None,
        };

        if !ws_candidates.is_empty() {
            let base_len = serialized_char_len(&response);
            let reserve = if page_newest_first.is_empty() {
                HISTORY_READ_MAX_TOTAL_CHARS
            } else {
                (base_len + WORK_STEPS_RESERVED_BUDGET_CHARS).min(HISTORY_READ_MAX_TOTAL_CHARS / 2)
            };
            pack_work_steps(
                &mut response,
                &ws_candidates,
                ws_db_has_more,
                query.work_steps_offset,
                reserve,
            );
        }

        // Empacota do mais recente para o mais antigo para priorizar o contexto mais novo dentro dos 16.000 chars.
        let mut chosen_newest_first: Vec<HistoryReadMessage> = Vec::new();
        let mut dropped_for_budget = false;

        for row in &page_newest_first {
            let effective = row.effective_text();
            let chars: Vec<char> = effective.chars().collect();
            let total_chars = chars.len();
            let candidate = make_read_message(row, effective, total_chars, None);

            chosen_newest_first.push(candidate);
            response.messages = chosen_newest_first.iter().rev().cloned().collect();
            let has_more_now = has_older_in_db || dropped_for_budget;
            apply_pagination_fields(&mut response, has_more_now);

            if serialized_char_len(&response) <= HISTORY_READ_MAX_TOTAL_CHARS {
                continue;
            }

            chosen_newest_first.pop();
            if chosen_newest_first.is_empty() {
                // Até a mensagem mais recente excede o orçamento: fatia com `offset` e devolve `nextOffset`.
                let fixed_ws = response.work_steps.clone();
                let fixed_next_ws = response.next_work_steps_offset;
                let has_more_single = has_older_in_db || page_newest_first.len() > 1;
                let (fitted, _) = fit_message_slice(
                    row,
                    &chars,
                    query.offset,
                    &|msg| {
                        let oldest_id = msg.message_id;
                        HistoryReadResponse {
                            project_id: query.project_id,
                            next_offset: msg.next_offset,
                            messages: vec![msg],
                            next_cursor: has_more_single.then(|| oldest_id.to_string()),
                            next_before_message_id: has_more_single.then_some(oldest_id),
                            has_more: has_more_single,
                            work_steps: fixed_ws.clone(),
                            next_work_steps_offset: fixed_next_ws,
                        }
                    },
                    HISTORY_READ_MAX_TOTAL_CHARS,
                );
                response.next_offset = fitted.next_offset;
                chosen_newest_first.push(fitted);
                if page_newest_first.len() > 1 {
                    dropped_for_budget = true;
                }
                break;
            } else {
                dropped_for_budget = true;
                break;
            }
        }

        chosen_newest_first.reverse();
        response.messages = chosen_newest_first;
        apply_pagination_fields(&mut response, has_older_in_db || dropped_for_budget);

        // Aproveita eventual sobra do orçamento de 16.000 chars para anexar mais blocos `[work steps]`.
        if !ws_candidates.is_empty() && response.work_steps.len() < ws_candidates.len() {
            pack_work_steps(
                &mut response,
                &ws_candidates,
                ws_db_has_more,
                query.work_steps_offset,
                HISTORY_READ_MAX_TOTAL_CHARS,
            );
        }

        Ok(response)
    }
}

fn apply_pagination_fields(response: &mut HistoryReadResponse, has_more: bool) {
    response.has_more = has_more;
    if has_more {
        let oldest = response.messages.first().map(|msg| msg.message_id);
        response.next_before_message_id = oldest;
        response.next_cursor = oldest.map(|id| id.to_string());
    } else {
        response.next_before_message_id = None;
        response.next_cursor = None;
    }
}
