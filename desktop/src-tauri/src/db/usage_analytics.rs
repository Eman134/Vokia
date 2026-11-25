use std::collections::BTreeMap;

use chrono::{DateTime, Duration, FixedOffset, Local, NaiveDate, TimeZone, Utc};
use rusqlite::{params_from_iter, types::Value as SqlValue};
use serde::{Deserialize, Serialize};

use crate::claude::TurnUsage;

use super::connection::Db;
use super::error::DbError;
use super::usage_pricing::{estimate_usage_price, TokenCounts, PRICING_VERSION};

pub const MAX_RANGE_DAYS: u32 = 367;
pub const DEFAULT_RANGE_DAYS: u32 = 30;
pub const VALID_ORIGINS: &[&str] = &["card", "quick", "triage", "review", "background"];
pub const COST_DISCLAIMER: &str =
    "Estimativa equivalente à API (não representa cobrança adicional sobre a sua assinatura).";

#[derive(Clone, Debug)]
pub struct NewUsageRecord {
    pub occurred_at: i64,
    pub project_id: Option<i64>,
    pub card_id: Option<String>,
    pub agent_id: String,
    pub provider_id: String,
    pub model: String,
    pub origin: String,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub thought_tokens: u64,
    pub cost_usd_estimated: Option<f64>,
    pub price_basis: Option<String>,
    pub is_fallback: bool,
    pub duration_ms: u64,
    pub prompt_id: Option<String>,
}

impl NewUsageRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn from_turn_usage(
        occurred_at: i64,
        project_id: Option<i64>,
        card_id: Option<String>,
        agent_id: impl Into<String>,
        origin: impl Into<String>,
        usage: &TurnUsage,
        duration_ms: u64,
    ) -> Self {
        let provider_id = usage
            .provider
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("unknown")
            .to_string();
        let model = usage
            .model
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("unknown")
            .to_string();
        let input_tokens = usage.input_tokens.unwrap_or(0);
        let cached_input_tokens = usage.cached_input_tokens.unwrap_or(0);
        let cache_write_tokens = usage.cache_write_tokens.unwrap_or(0);
        let output_tokens = usage.output_tokens.unwrap_or(0);
        let thought_tokens = usage.thought_tokens.unwrap_or(0);
        let counts = TokenCounts {
            input_tokens,
            cached_input_tokens,
            cache_write_tokens,
            output_tokens,
            thought_tokens,
        };
        let (cost_usd_estimated, price_basis) = match estimate_usage_price(&model, counts) {
            Some(estimate) => (Some(estimate.cost_usd), Some(estimate.price_basis)),
            None => match usage.cost_usd.filter(|cost| cost.is_finite() && *cost > 0.0) {
                Some(reported) => (Some(reported), Some("provider-reported".to_string())),
                None => (None, None),
            },
        };
        let origin_raw = origin.into();
        let origin = if VALID_ORIGINS.contains(&origin_raw.as_str()) {
            origin_raw
        } else {
            "quick".to_string()
        };
        Self {
            occurred_at,
            project_id,
            card_id,
            agent_id: agent_id.into(),
            provider_id,
            model,
            origin,
            input_tokens,
            cached_input_tokens,
            cache_write_tokens,
            output_tokens,
            thought_tokens,
            cost_usd_estimated,
            price_basis,
            is_fallback: usage.is_fallback.unwrap_or(false),
            duration_ms,
            prompt_id: None,
        }
    }
}

/// Calcula o delta entre dois checkpoints cumulativos de sessão sem dupla contagem.
#[allow(dead_code)]
pub fn diff_cumulative_usage(previous: TokenCounts, current: TokenCounts) -> TokenCounts {
    TokenCounts {
        input_tokens: current.input_tokens.saturating_sub(previous.input_tokens),
        cached_input_tokens: current
            .cached_input_tokens
            .saturating_sub(previous.cached_input_tokens),
        cache_write_tokens: current
            .cache_write_tokens
            .saturating_sub(previous.cache_write_tokens),
        output_tokens: current.output_tokens.saturating_sub(previous.output_tokens),
        thought_tokens: current
            .thought_tokens
            .saturating_sub(previous.thought_tokens),
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageAnalyticsQuery {
    pub days: Option<u32>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub timezone: Option<String>,
    pub project_id: Option<i64>,
    pub agent_id: Option<String>,
    pub provider_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageRange {
    pub from: String,
    pub to: String,
    pub days: u32,
    pub timezone: String,
    pub offset: String,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageTotals {
    pub turns: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub thought_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd_estimated: Option<f64>,
    pub priced_turns: u64,
    pub unpriced_turns: u64,
    pub fallback_turns: u64,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSavings {
    pub cache_savings_usd: f64,
    pub light_tier_savings_usd: f64,
    pub total_savings_usd: f64,
    pub savings_percent: f64,
    pub cache_hit_percent: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyProviderUsage {
    pub date: String,
    pub provider_id: String,
    pub turns: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub thought_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd_estimated: Option<f64>,
    pub unpriced_turns: u64,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyUsage {
    pub date: String,
    pub turns: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub thought_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd_estimated: Option<f64>,
    pub unpriced_turns: u64,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderBreakdown {
    pub provider_id: String,
    pub turns: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub thought_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd_estimated: Option<f64>,
    pub unpriced_turns: u64,
    pub fallback_turns: u64,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentBreakdown {
    pub agent_id: String,
    pub turns: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub thought_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd_estimated: Option<f64>,
    pub unpriced_turns: u64,
    pub fallback_turns: u64,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelBreakdown {
    pub provider_id: String,
    pub model: String,
    pub turns: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub thought_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd_estimated: Option<f64>,
    pub unpriced_turns: u64,
    pub price_basis: Option<String>,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageAnalyticsResponse {
    pub range: UsageRange,
    pub totals: UsageTotals,
    pub savings: UsageSavings,
    pub daily_by_provider: Vec<DailyProviderUsage>,
    pub daily: Vec<DailyUsage>,
    pub by_provider: Vec<ProviderBreakdown>,
    pub by_agent: Vec<AgentBreakdown>,
    pub by_model: Vec<ModelBreakdown>,
    pub pricing_version: String,
    pub cost_disclaimer: String,
}

#[derive(Default)]
struct Bucket {
    turns: u64,
    input_tokens: u64,
    cached_input_tokens: u64,
    cache_write_tokens: u64,
    output_tokens: u64,
    thought_tokens: u64,
    cost_sum: f64,
    priced_turns: u64,
    unpriced_turns: u64,
    fallback_turns: u64,
    duration_ms: u64,
    price_basis: Option<String>,
}

impl Bucket {
    fn add(&mut self, row: &StoredUsageRow) {
        self.turns = self.turns.saturating_add(1);
        self.input_tokens = self.input_tokens.saturating_add(row.input_tokens);
        self.cached_input_tokens = self
            .cached_input_tokens
            .saturating_add(row.cached_input_tokens);
        self.cache_write_tokens = self
            .cache_write_tokens
            .saturating_add(row.cache_write_tokens);
        self.output_tokens = self.output_tokens.saturating_add(row.output_tokens);
        self.thought_tokens = self.thought_tokens.saturating_add(row.thought_tokens);
        self.duration_ms = self.duration_ms.saturating_add(row.duration_ms);
        if row.is_fallback {
            self.fallback_turns = self.fallback_turns.saturating_add(1);
        }
        match row.cost_usd_estimated {
            Some(cost) if cost.is_finite() => {
                self.cost_sum += cost;
                self.priced_turns = self.priced_turns.saturating_add(1);
            }
            _ => {
                self.unpriced_turns = self.unpriced_turns.saturating_add(1);
            }
        }
        if self.price_basis.is_none() && row.price_basis.is_some() {
            self.price_basis = row.price_basis.clone();
        }
    }

    fn total_tokens(&self) -> u64 {
        self.input_tokens
            .saturating_add(self.cached_input_tokens)
            .saturating_add(self.cache_write_tokens)
            .saturating_add(self.output_tokens)
            .saturating_add(self.thought_tokens)
    }

    fn cost_usd_estimated(&self) -> Option<f64> {
        (self.priced_turns > 0).then_some(round_usd(self.cost_sum))
    }
}

struct StoredUsageRow {
    occurred_at: i64,
    agent_id: String,
    provider_id: String,
    model: String,
    input_tokens: u64,
    cached_input_tokens: u64,
    cache_write_tokens: u64,
    output_tokens: u64,
    thought_tokens: u64,
    cost_usd_estimated: Option<f64>,
    price_basis: Option<String>,
    is_fallback: bool,
    duration_ms: u64,
}

fn round_usd(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn round_percent(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

// Tabela de offsets padrão dos fusos IANA mais comuns para suporte completo sem dependências externas.
const IANA_OFFSETS_MINUTES: &[(&str, i32)] = &[
    ("utc", 0),
    ("etc/utc", 0),
    ("etc/gmt", 0),
    ("gmt", 0),
    ("america/sao_paulo", -180),
    ("america/fortaleza", -180),
    ("america/recife", -180),
    ("america/bahia", -180),
    ("america/belem", -180),
    ("america/maceio", -180),
    ("america/araguaina", -180),
    ("america/santarem", -180),
    ("america/manaus", -240),
    ("america/cuiaba", -240),
    ("america/campo_grande", -240),
    ("america/porto_velho", -240),
    ("america/boa_vista", -240),
    ("america/rio_branco", -300),
    ("america/eirunepe", -300),
    ("america/noronha", -120),
    ("america/argentina/buenos_aires", -180),
    ("america/buenos_aires", -180),
    ("america/montevideo", -180),
    ("america/santiago", -240),
    ("america/asuncion", -240),
    ("america/la_paz", -240),
    ("america/caracas", -240),
    ("america/bogota", -300),
    ("america/lima", -300),
    ("america/guayaquil", -300),
    ("america/panama", -300),
    ("america/mexico_city", -360),
    ("america/new_york", -300),
    ("america/toronto", -300),
    ("america/chicago", -360),
    ("america/denver", -420),
    ("america/phoenix", -420),
    ("america/los_angeles", -480),
    ("america/vancouver", -480),
    ("america/anchorage", -540),
    ("pacific/honolulu", -600),
    ("europe/london", 0),
    ("europe/lisbon", 0),
    ("europe/dublin", 0),
    ("atlantic/reykjavik", 0),
    ("europe/madrid", 60),
    ("europe/paris", 60),
    ("europe/berlin", 60),
    ("europe/rome", 60),
    ("europe/amsterdam", 60),
    ("europe/brussels", 60),
    ("europe/zurich", 60),
    ("europe/vienna", 60),
    ("europe/warsaw", 60),
    ("europe/prague", 60),
    ("europe/stockholm", 60),
    ("europe/oslo", 60),
    ("europe/copenhagen", 60),
    ("europe/athens", 120),
    ("europe/helsinki", 120),
    ("europe/bucharest", 120),
    ("europe/kyiv", 120),
    ("europe/istanbul", 180),
    ("europe/moscow", 180),
    ("africa/casablanca", 60),
    ("africa/lagos", 60),
    ("africa/cairo", 120),
    ("africa/johannesburg", 120),
    ("africa/nairobi", 180),
    ("asia/jerusalem", 120),
    ("asia/riyadh", 180),
    ("asia/dubai", 240),
    ("asia/karachi", 300),
    ("asia/kolkata", 330),
    ("asia/calcutta", 330),
    ("asia/dhaka", 360),
    ("asia/bangkok", 420),
    ("asia/jakarta", 420),
    ("asia/ho_chi_minh", 420),
    ("asia/singapore", 480),
    ("asia/shanghai", 480),
    ("asia/hong_kong", 480),
    ("asia/taipei", 480),
    ("asia/manila", 480),
    ("asia/seoul", 540),
    ("asia/tokyo", 540),
    ("australia/perth", 480),
    ("australia/adelaide", 570),
    ("australia/brisbane", 600),
    ("australia/sydney", 600),
    ("australia/melbourne", 600),
    ("pacific/auckland", 720),
];

fn format_offset(offset: FixedOffset) -> String {
    let total_seconds = offset.local_minus_utc();
    let sign = if total_seconds < 0 { '-' } else { '+' };
    let abs_minutes = total_seconds.abs() / 60;
    let hours = abs_minutes / 60;
    let minutes = abs_minutes % 60;
    format!("{sign}{hours:02}:{minutes:02}")
}

fn parse_offset_str(raw: &str) -> Option<FixedOffset> {
    let stripped = raw
        .trim()
        .strip_prefix("UTC")
        .or_else(|| raw.trim().strip_prefix("utc"))
        .or_else(|| raw.trim().strip_prefix("GMT"))
        .or_else(|| raw.trim().strip_prefix("gmt"))
        .unwrap_or(raw.trim())
        .trim();
    if stripped.is_empty() || stripped.eq_ignore_ascii_case("z") {
        return FixedOffset::east_opt(0);
    }
    let (sign, rest) = match stripped.as_bytes().first().copied() {
        Some(b'+') => (1, &stripped[1..]),
        Some(b'-') => (-1, &stripped[1..]),
        _ => return None,
    };
    let (hours, minutes): (i32, i32) = if let Some((h, m)) = rest.split_once(':') {
        (h.parse().ok()?, m.parse().ok()?)
    } else if rest.len() == 4 && rest.chars().all(|c| c.is_ascii_digit()) {
        (rest[..2].parse().ok()?, rest[2..].parse().ok()?)
    } else if (1..=2).contains(&rest.len()) && rest.chars().all(|c| c.is_ascii_digit()) {
        (rest.parse().ok()?, 0)
    } else {
        return None;
    };
    if hours > 14 || minutes >= 60 {
        return None;
    }
    let seconds = sign * (hours * 3600 + minutes * 60);
    FixedOffset::east_opt(seconds)
}

fn parse_etc_gmt(lower: &str) -> Option<FixedOffset> {
    // POSIX Etc/GMT+3 significa UTC-3 (sinal invertido na especificação IANA).
    let rest = lower.strip_prefix("etc/gmt")?;
    if rest.is_empty() {
        return FixedOffset::east_opt(0);
    }
    let (sign, digits) = match rest.as_bytes().first().copied() {
        Some(b'+') => (-1, &rest[1..]),
        Some(b'-') => (1, &rest[1..]),
        _ => return None,
    };
    let hours: i32 = digits.parse().ok()?;
    if hours > 14 {
        return None;
    }
    FixedOffset::east_opt(sign * hours * 3600)
}

pub fn resolve_timezone(timezone: Option<&str>) -> Result<(FixedOffset, String), DbError> {
    let Some(raw) = timezone.map(str::trim).filter(|s| !s.is_empty()) else {
        let local = *Local::now().offset();
        return Ok((local, format_offset(local)));
    };
    if raw.eq_ignore_ascii_case("local") {
        let local = *Local::now().offset();
        return Ok((local, format_offset(local)));
    }
    let lower = raw.to_ascii_lowercase();
    if let Some((_, minutes)) = IANA_OFFSETS_MINUTES
        .iter()
        .find(|(name, _)| *name == lower)
    {
        let offset = FixedOffset::east_opt(minutes * 60)
            .ok_or_else(|| DbError::InvalidUsageQuery(format!("Fuso horário inválido: {raw}")))?;
        return Ok((offset, raw.to_string()));
    }
    if let Some(offset) = parse_etc_gmt(&lower) {
        return Ok((offset, raw.to_string()));
    }
    if let Some(offset) = parse_offset_str(raw) {
        return Ok((offset, raw.to_string()));
    }
    Err(DbError::InvalidUsageQuery(format!(
        "Fuso horário não reconhecido: {raw}. Use um identificador IANA (ex.: America/Sao_Paulo), UTC ou deslocamento (±HH:MM)."
    )))
}

fn parse_bound_date(raw: &str, offset: FixedOffset) -> Result<NaiveDate, DbError> {
    let trimmed = raw.trim();
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return Ok(date);
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok(dt.with_timezone(&offset).date_naive());
    }
    if let Ok(ms) = trimmed.parse::<i64>() {
        if let Some(dt) = DateTime::<Utc>::from_timestamp_millis(ms) {
            return Ok(dt.with_timezone(&offset).date_naive());
        }
    }
    Err(DbError::InvalidUsageQuery(format!(
        "Data inválida: {trimmed}. Use o formato YYYY-MM-DD ou RFC3339."
    )))
}

fn resolve_date_range(
    query: &UsageAnalyticsQuery,
    offset: FixedOffset,
    now_ms: i64,
) -> Result<(NaiveDate, NaiveDate, u32), DbError> {
    let now_utc = DateTime::<Utc>::from_timestamp_millis(now_ms).unwrap_or_else(Utc::now);
    let today = now_utc.with_timezone(&offset).date_naive();

    let from_raw = query
        .from
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let to_raw = query
        .to
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let (from_date, to_date) = match (from_raw, to_raw) {
        (Some(from_str), Some(to_str)) => (
            parse_bound_date(from_str, offset)?,
            parse_bound_date(to_str, offset)?,
        ),
        (Some(from_str), None) => (parse_bound_date(from_str, offset)?, today),
        (None, Some(to_str)) => {
            let to = parse_bound_date(to_str, offset)?;
            let days = validate_days(query.days.unwrap_or(DEFAULT_RANGE_DAYS))?;
            (to - Duration::days(i64::from(days) - 1), to)
        }
        (None, None) => {
            let days = validate_days(query.days.unwrap_or(DEFAULT_RANGE_DAYS))?;
            (today - Duration::days(i64::from(days) - 1), today)
        }
    };

    if from_date > to_date {
        return Err(DbError::InvalidUsageQuery(
            "A data inicial (from) não pode ser posterior à data final (to).".to_string(),
        ));
    }
    let span_i64 = (to_date - from_date).num_days() + 1;
    if span_i64 > i64::from(MAX_RANGE_DAYS) {
        return Err(DbError::InvalidUsageQuery(format!(
            "O intervalo solicitado ({span_i64} dias) excede o máximo permitido de {MAX_RANGE_DAYS} dias."
        )));
    }
    Ok((from_date, to_date, span_i64 as u32))
}

fn validate_days(days: u32) -> Result<u32, DbError> {
    if days == 0 {
        return Err(DbError::InvalidUsageQuery(
            "O período (days) precisa ser de pelo menos 1 dia.".to_string(),
        ));
    }
    if days > MAX_RANGE_DAYS {
        return Err(DbError::InvalidUsageQuery(format!(
            "O período solicitado ({days} dias) excede o máximo permitido de {MAX_RANGE_DAYS} dias."
        )));
    }
    Ok(days)
}

fn local_day_start_ms(date: NaiveDate, offset: FixedOffset) -> Result<i64, DbError> {
    let naive = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| DbError::InvalidUsageQuery(format!("Data fora do intervalo: {date}")))?;
    let dt = offset
        .from_local_datetime(&naive)
        .single()
        .ok_or_else(|| DbError::InvalidUsageQuery(format!("Data inválida no fuso: {date}")))?;
    Ok(dt.timestamp_millis())
}

impl Db {
    pub fn record_usage(&self, record: &NewUsageRecord) -> Result<(), DbError> {
        let origin = record.origin.trim();
        if !VALID_ORIGINS.contains(&origin) {
            return Err(DbError::InvalidUsageQuery(format!(
                "Origem de uso inválida: {origin}."
            )));
        }
        let cost = record
            .cost_usd_estimated
            .filter(|value| value.is_finite() && *value >= 0.0);
        let conn = self.conn()?;
        let sql = if record.prompt_id.is_some() {
            "INSERT OR IGNORE INTO usage_records (
                occurred_at, project_id, card_id, agent_id, provider_id, model, origin,
                input_tokens, cached_input_tokens, cache_write_tokens, output_tokens, thought_tokens,
                cost_usd_estimated, price_basis, is_fallback, duration_ms, prompt_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)"
        } else {
            "INSERT INTO usage_records (
                occurred_at, project_id, card_id, agent_id, provider_id, model, origin,
                input_tokens, cached_input_tokens, cache_write_tokens, output_tokens, thought_tokens,
                cost_usd_estimated, price_basis, is_fallback, duration_ms, prompt_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)"
        };
        conn.execute(
            sql,
            rusqlite::params![
                record.occurred_at,
                record.project_id,
                record.card_id,
                record.agent_id,
                record.provider_id,
                record.model,
                origin,
                record.input_tokens as i64,
                record.cached_input_tokens as i64,
                record.cache_write_tokens as i64,
                record.output_tokens as i64,
                record.thought_tokens as i64,
                cost,
                record.price_basis,
                i64::from(record.is_fallback),
                record.duration_ms as i64,
                record.prompt_id,
            ],
        )?;
        Ok(())
    }

    pub fn query_usage_analytics(
        &self,
        query: &UsageAnalyticsQuery,
    ) -> Result<UsageAnalyticsResponse, DbError> {
        self.query_usage_analytics_at(Utc::now().timestamp_millis(), query)
    }

    pub fn query_usage_analytics_at(
        &self,
        now_ms: i64,
        query: &UsageAnalyticsQuery,
    ) -> Result<UsageAnalyticsResponse, DbError> {
        let (offset, tz_label) = resolve_timezone(query.timezone.as_deref())?;
        let (from_date, to_date, days) = resolve_date_range(query, offset, now_ms)?;
        let start_ms = local_day_start_ms(from_date, offset)?;
        let end_exclusive_ms = local_day_start_ms(to_date + Duration::days(1), offset)?;

        let mut sql = String::from(
            "SELECT occurred_at, agent_id, provider_id, model,
                    input_tokens, cached_input_tokens, cache_write_tokens, output_tokens, thought_tokens,
                    cost_usd_estimated, price_basis, is_fallback, duration_ms
             FROM usage_records
             WHERE occurred_at >= ?1 AND occurred_at < ?2",
        );
        let mut params: Vec<SqlValue> = vec![
            SqlValue::Integer(start_ms),
            SqlValue::Integer(end_exclusive_ms),
        ];
        if let Some(project_id) = query.project_id {
            params.push(SqlValue::Integer(project_id));
            sql.push_str(&format!(" AND project_id = ?{}", params.len()));
        }
        if let Some(agent_id) = query
            .agent_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            params.push(SqlValue::Text(agent_id.to_string()));
            sql.push_str(&format!(" AND agent_id = ?{}", params.len()));
        }
        if let Some(provider_id) = query
            .provider_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            params.push(SqlValue::Text(provider_id.to_string()));
            sql.push_str(&format!(" AND provider_id = ?{}", params.len()));
        }
        sql.push_str(" ORDER BY occurred_at ASC, id ASC");

        let rows: Vec<StoredUsageRow> = {
            let conn = self.conn()?;
            let mut statement = conn.prepare(&sql)?;
            let mapped = statement.query_map(params_from_iter(params), |row| {
                Ok(StoredUsageRow {
                    occurred_at: row.get(0)?,
                    agent_id: row.get(1)?,
                    provider_id: row.get(2)?,
                    model: row.get(3)?,
                    input_tokens: row.get::<_, i64>(4)?.max(0) as u64,
                    cached_input_tokens: row.get::<_, i64>(5)?.max(0) as u64,
                    cache_write_tokens: row.get::<_, i64>(6)?.max(0) as u64,
                    output_tokens: row.get::<_, i64>(7)?.max(0) as u64,
                    thought_tokens: row.get::<_, i64>(8)?.max(0) as u64,
                    cost_usd_estimated: row.get(9)?,
                    price_basis: row.get(10)?,
                    is_fallback: row.get::<_, i64>(11)? != 0,
                    duration_ms: row.get::<_, i64>(12)?.max(0) as u64,
                })
            })?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()?
        };

        let mut total_bucket = Bucket::default();
        let mut daily_provider_map: BTreeMap<(String, String), Bucket> = BTreeMap::new();
        let mut daily_map: BTreeMap<String, Bucket> = BTreeMap::new();
        let mut provider_map: BTreeMap<String, Bucket> = BTreeMap::new();
        let mut agent_map: BTreeMap<String, Bucket> = BTreeMap::new();
        let mut model_map: BTreeMap<(String, String), Bucket> = BTreeMap::new();

        let mut cache_savings_usd = 0.0_f64;
        let mut light_tier_savings_usd = 0.0_f64;

        for row in &rows {
            total_bucket.add(row);
            let day = DateTime::<Utc>::from_timestamp_millis(row.occurred_at)
                .map(|dt| dt.with_timezone(&offset).date_naive())
                .unwrap_or(from_date)
                .format("%Y-%m-%d")
                .to_string();

            daily_provider_map
                .entry((day.clone(), row.provider_id.clone()))
                .or_default()
                .add(row);
            daily_map.entry(day).or_default().add(row);
            provider_map
                .entry(row.provider_id.clone())
                .or_default()
                .add(row);
            agent_map.entry(row.agent_id.clone()).or_default().add(row);
            model_map
                .entry((row.provider_id.clone(), row.model.clone()))
                .or_default()
                .add(row);

            let counts = TokenCounts {
                input_tokens: row.input_tokens,
                cached_input_tokens: row.cached_input_tokens,
                cache_write_tokens: row.cache_write_tokens,
                output_tokens: row.output_tokens,
                thought_tokens: row.thought_tokens,
            };
            if let Some(estimate) = estimate_usage_price(&row.model, counts) {
                cache_savings_usd += estimate.cache_savings_usd;
                light_tier_savings_usd += estimate.light_tier_savings_usd;
            }
        }

        let total_savings_usd = cache_savings_usd + light_tier_savings_usd;
        let priced_actual_cost = total_bucket.cost_sum;
        let savings_denom = priced_actual_cost + total_savings_usd;
        let savings_percent = if savings_denom > 0.0 {
            round_percent((total_savings_usd / savings_denom) * 100.0)
        } else {
            0.0
        };
        let total_input_context = total_bucket
            .input_tokens
            .saturating_add(total_bucket.cached_input_tokens)
            .saturating_add(total_bucket.cache_write_tokens);
        let cache_hit_percent = if total_input_context > 0 {
            round_percent((total_bucket.cached_input_tokens as f64 / total_input_context as f64) * 100.0)
        } else {
            0.0
        };

        let daily_by_provider: Vec<DailyProviderUsage> = daily_provider_map
            .into_iter()
            .map(|((date, provider_id), bucket)| DailyProviderUsage {
                date,
                provider_id,
                turns: bucket.turns,
                input_tokens: bucket.input_tokens,
                cached_input_tokens: bucket.cached_input_tokens,
                cache_write_tokens: bucket.cache_write_tokens,
                output_tokens: bucket.output_tokens,
                thought_tokens: bucket.thought_tokens,
                total_tokens: bucket.total_tokens(),
                cost_usd_estimated: bucket.cost_usd_estimated(),
                unpriced_turns: bucket.unpriced_turns,
                duration_ms: bucket.duration_ms,
            })
            .collect();

        let daily: Vec<DailyUsage> = daily_map
            .into_iter()
            .map(|(date, bucket)| DailyUsage {
                date,
                turns: bucket.turns,
                input_tokens: bucket.input_tokens,
                cached_input_tokens: bucket.cached_input_tokens,
                cache_write_tokens: bucket.cache_write_tokens,
                output_tokens: bucket.output_tokens,
                thought_tokens: bucket.thought_tokens,
                total_tokens: bucket.total_tokens(),
                cost_usd_estimated: bucket.cost_usd_estimated(),
                unpriced_turns: bucket.unpriced_turns,
                duration_ms: bucket.duration_ms,
            })
            .collect();

        let mut by_provider: Vec<ProviderBreakdown> = provider_map
            .into_iter()
            .map(|(provider_id, bucket)| ProviderBreakdown {
                provider_id,
                turns: bucket.turns,
                input_tokens: bucket.input_tokens,
                cached_input_tokens: bucket.cached_input_tokens,
                cache_write_tokens: bucket.cache_write_tokens,
                output_tokens: bucket.output_tokens,
                thought_tokens: bucket.thought_tokens,
                total_tokens: bucket.total_tokens(),
                cost_usd_estimated: bucket.cost_usd_estimated(),
                unpriced_turns: bucket.unpriced_turns,
                fallback_turns: bucket.fallback_turns,
                duration_ms: bucket.duration_ms,
            })
            .collect();
        by_provider.sort_by(|a, b| {
            b.total_tokens
                .cmp(&a.total_tokens)
                .then_with(|| a.provider_id.cmp(&b.provider_id))
        });

        let mut by_agent: Vec<AgentBreakdown> = agent_map
            .into_iter()
            .map(|(agent_id, bucket)| AgentBreakdown {
                agent_id,
                turns: bucket.turns,
                input_tokens: bucket.input_tokens,
                cached_input_tokens: bucket.cached_input_tokens,
                cache_write_tokens: bucket.cache_write_tokens,
                output_tokens: bucket.output_tokens,
                thought_tokens: bucket.thought_tokens,
                total_tokens: bucket.total_tokens(),
                cost_usd_estimated: bucket.cost_usd_estimated(),
                unpriced_turns: bucket.unpriced_turns,
                fallback_turns: bucket.fallback_turns,
                duration_ms: bucket.duration_ms,
            })
            .collect();
        by_agent.sort_by(|a, b| {
            b.total_tokens
                .cmp(&a.total_tokens)
                .then_with(|| a.agent_id.cmp(&b.agent_id))
        });

        let mut by_model: Vec<ModelBreakdown> = model_map
            .into_iter()
            .map(|((provider_id, model), bucket)| ModelBreakdown {
                provider_id,
                model,
                turns: bucket.turns,
                input_tokens: bucket.input_tokens,
                cached_input_tokens: bucket.cached_input_tokens,
                cache_write_tokens: bucket.cache_write_tokens,
                output_tokens: bucket.output_tokens,
                thought_tokens: bucket.thought_tokens,
                total_tokens: bucket.total_tokens(),
                cost_usd_estimated: bucket.cost_usd_estimated(),
                unpriced_turns: bucket.unpriced_turns,
                price_basis: bucket.price_basis,
                duration_ms: bucket.duration_ms,
            })
            .collect();
        by_model.sort_by(|a, b| {
            b.total_tokens
                .cmp(&a.total_tokens)
                .then_with(|| a.model.cmp(&b.model))
        });

        Ok(UsageAnalyticsResponse {
            range: UsageRange {
                from: from_date.format("%Y-%m-%d").to_string(),
                to: to_date.format("%Y-%m-%d").to_string(),
                days,
                timezone: tz_label,
                offset: format_offset(offset),
            },
            totals: UsageTotals {
                turns: total_bucket.turns,
                input_tokens: total_bucket.input_tokens,
                cached_input_tokens: total_bucket.cached_input_tokens,
                cache_write_tokens: total_bucket.cache_write_tokens,
                output_tokens: total_bucket.output_tokens,
                thought_tokens: total_bucket.thought_tokens,
                total_tokens: total_bucket.total_tokens(),
                cost_usd_estimated: total_bucket.cost_usd_estimated(),
                priced_turns: total_bucket.priced_turns,
                unpriced_turns: total_bucket.unpriced_turns,
                fallback_turns: total_bucket.fallback_turns,
                duration_ms: total_bucket.duration_ms,
            },
            savings: UsageSavings {
                cache_savings_usd: round_usd(cache_savings_usd),
                light_tier_savings_usd: round_usd(light_tier_savings_usd),
                total_savings_usd: round_usd(total_savings_usd),
                savings_percent,
                cache_hit_percent,
            },
            daily_by_provider,
            daily,
            by_provider,
            by_agent,
            by_model,
            pricing_version: PRICING_VERSION.to_string(),
            cost_disclaimer: COST_DISCLAIMER.to_string(),
        })
    }
}

#[cfg(test)]
#[path = "../../tests/usage_analytics_tests.rs"]
mod tests;
