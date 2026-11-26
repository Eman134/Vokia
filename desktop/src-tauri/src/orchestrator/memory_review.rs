use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::agents::{write_atomic, Agent};
use crate::claude::{Access, Reply};
use crate::db::{
    Db, DbState, LearningSuggestion, MemoryOriginType, NewLearningSuggestion, NewProjectMemory,
    ProjectMemory, ReviewMessageEntry, SkillScope, SuggestionKind, SuggestionOperation,
    SuggestionStatus, LEARNING_MODE_ASK, LEARNING_MODE_AUTO, LEARNING_MODE_OFF,
};
use crate::events::{Speaker, VokiaEvent};
use crate::permissions::{PermissionBroker, PermissionGate};
use crate::providers::{ModelTier, ProviderConfig, ProviderRegistry};
use crate::session_key::SessionKey;
use crate::state::AppState;
use crate::usage::{UsageEngine, DEFAULT_WARN_THRESHOLD};

use super::adapters::ProviderAdapter;
use super::agent_session::AgentCall;
use super::console::ConsoleSink;
use super::emit::{emit, emit_feed_note, now_millis};
use super::error::FlowError;
use super::fallback::{load_global_active_llm, resolve_model_for_provider, resolve_program_for, LlmTarget};

pub const MAX_MEMORIES_CHARS: usize = 12_000;
pub const MAX_MESSAGES_CHARS: usize = 24_000;
pub const MAX_WORK_STEPS: usize = 80;
pub const IDLE_REVIEW_SECS: u64 = 15 * 60;
pub const IDLE_MIN_NEW_TURNS: u32 = 3;
const IDLE_WATCHER_INTERVAL_SECS: u64 = 60;
const SKILL_DIR_NAME: &str = ".vokia/skills";
const SKILL_FILE_NAME: &str = "SKILL.md";

static NEXT_LEARNING_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewTrigger {
    CardDone,
    TurnCadence,
    Idle,
    ClearConversation,
    Manual,
}

impl ReviewTrigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CardDone => "card-done",
            Self::TurnCadence => "turn-cadence",
            Self::Idle => "idle",
            Self::ClearConversation => "clear-conversation",
            Self::Manual => "manual",
        }
    }

    pub fn parse(raw: Option<&str>) -> Self {
        match raw.unwrap_or("manual").trim().to_ascii_lowercase().as_str() {
            "card-done" | "done" => Self::CardDone,
            "turn-cadence" | "cadence" => Self::TurnCadence,
            "idle" => Self::Idle,
            "clear-conversation" | "clear" => Self::ClearConversation,
            _ => Self::Manual,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewOutcome {
    pub status: String,
    pub created_suggestions: usize,
    pub auto_applied: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl ReviewOutcome {
    fn skipped(status: &str) -> Self {
        Self {
            status: status.to_string(),
            created_suggestions: 0,
            auto_applied: 0,
            provider_id: None,
            model: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PreCapturedSnapshot {
    pub messages: Vec<ReviewMessageEntry>,
    pub work_steps: Vec<String>,
    pub last_message_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BasicProviderCandidate {
    pub provider: ProviderConfig,
    pub model: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderSelectionResult {
    Selected(BasicProviderCandidate),
    PostponedUsageLimit,
    NoEnabledProvider,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedMemoryOp {
    pub op: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub fact: Option<String>,
    #[serde(default)]
    pub rationale: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedSkillSuggestion {
    pub op: String,
    pub slug: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub content: String,
    #[serde(default)]
    pub rationale: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedReviewOutput {
    #[serde(default)]
    pub memories: Vec<ParsedMemoryOp>,
    #[serde(default)]
    pub skill: Option<ParsedSkillSuggestion>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalSkillSummary {
    pub slug: String,
    pub scope: SkillScope,
    pub path: PathBuf,
    pub preview: String,
}

struct ActiveReviewGuard<'a> {
    flag: &'a AtomicBool,
}

impl<'a> Drop for ActiveReviewGuard<'a> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

fn try_acquire_review_guard(flag: &AtomicBool) -> Option<ActiveReviewGuard<'_>> {
    flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .ok()
        .map(|_| ActiveReviewGuard { flag })
}

fn now_millis_i64() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn next_id(prefix: &str, now_ms: i64) -> String {
    let seq = NEXT_LEARNING_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{now_ms}-{seq}")
}

/// Checks whether a provider has any non-expired usage window at or above `DEFAULT_WARN_THRESHOLD` (75%)
/// or is paused by `UsageLimitGate`. Respects active extra credits.
pub fn is_provider_at_or_above_warn_threshold(
    usage: &UsageEngine,
    provider_id: &str,
    model: Option<&str>,
    now_ms: u64,
) -> bool {
    if usage.is_provider_using_credits(provider_id) {
        return false;
    }
    if usage.is_provider_paused_by_limit(provider_id, model, now_ms) {
        return true;
    }
    let snapshot = usage.snapshot(Some(provider_id), now_ms);
    let Some(plan_usage) = snapshot.providers.into_iter().find(|p| p.provider_id == provider_id) else {
        return false;
    };
    if plan_usage.is_using_credits() {
        return false;
    }
    plan_usage.all_windows().into_iter().any(|window| {
        !window.is_expired(now_ms) && (window.rejected || window.percent >= DEFAULT_WARN_THRESHOLD)
    })
}

/// Dynamically selects any enabled, available provider in `ModelTier::Basic` that is below 75% usage
/// and not paused. Never hardcodes Claude as mandatory.
pub fn select_basic_provider_candidate(
    providers: &ProviderRegistry,
    usage: &UsageEngine,
    global_active: Option<&LlmTarget>,
    now_ms: u64,
) -> ProviderSelectionResult {
    let all = providers.list().unwrap_or_default();
    let enabled: Vec<ProviderConfig> = all.into_iter().filter(|p| p.enabled).collect();
    if enabled.is_empty() {
        return ProviderSelectionResult::NoEnabledProvider;
    }

    let mut ordered: Vec<ProviderConfig> = Vec::with_capacity(enabled.len());
    if let Some(active) = global_active {
        if let Some(pos) = enabled.iter().position(|p| p.id == active.cli) {
            ordered.push(enabled[pos].clone());
        }
    }
    for provider in enabled {
        if !ordered.iter().any(|p| p.id == provider.id) {
            ordered.push(provider);
        }
    }

    for provider in ordered {
        let model = resolve_model_for_provider(&provider, Some(ModelTier::Basic), "");
        if model.trim().is_empty() {
            continue;
        }
        if is_provider_at_or_above_warn_threshold(usage, &provider.id, Some(&model), now_ms) {
            continue;
        }
        return ProviderSelectionResult::Selected(BasicProviderCandidate { provider, model });
    }

    ProviderSelectionResult::PostponedUsageLimit
}

/// Redacts common secrets, tokens, private keys, and credentials before sending context to a Basic model.
pub fn redact_secrets(input: &str) -> String {
    let mut out = input.to_string();

    // 1. Redact PEM private key blocks.
    while let Some(start) = out.find("-----BEGIN ") {
        if let Some(rel_end) = out[start..].find("-----END ") {
            let end_line_start = start + rel_end;
            let end_pos = out[end_line_start..]
                .find('\n')
                .map_or(out.len(), |offset| end_line_start + offset);
            out.replace_range(start..end_pos, "[REDACTED_PRIVATE_KEY]");
        } else {
            out.replace_range(start.., "[REDACTED_PRIVATE_KEY]");
            break;
        }
    }

    // 2. Redact known token prefixes.
    const PREFIXES: &[&str] = &[
        "sk-ant-",
        "sk-proj-",
        "sk-",
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "github_pat_",
        "xoxb-",
        "xoxp-",
        "AIza",
        "AKIA",
    ];
    for prefix in PREFIXES {
        let mut cursor = 0;
        while let Some(rel) = out[cursor..].find(prefix) {
            let start = cursor + rel;
            let rest = &out[start..];
            let token_len = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')))
                .unwrap_or(rest.len());
            if token_len >= prefix.len() + 8 {
                out.replace_range(start..start + token_len, "[REDACTED_TOKEN]");
                cursor = start + "[REDACTED_TOKEN]".len();
            } else {
                cursor = start + prefix.len();
            }
        }
    }

    // 3. Redact Bearer <token> and key=value / key: value secrets line by line.
    let mut lines = Vec::new();
    for line in out.lines() {
        lines.push(redact_line_kv(line));
    }
    let mut joined = lines.join("\n");
    if out.ends_with('\n') && !joined.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

fn redact_line_kv(line: &str) -> String {
    let mut result = line.to_string();
    let lower = result.to_ascii_lowercase();
    if let Some(pos) = lower.find("bearer ") {
        let val_start = pos + "bearer ".len();
        let rest = &result[val_start..];
        let val_len = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';'))
            .unwrap_or(rest.len());
        if val_len >= 8 {
            result.replace_range(val_start..val_start + val_len, "[REDACTED]");
        }
    }
    const SECRET_KEYS: &[&str] = &[
        "api_key",
        "apikey",
        "auth_token",
        "access_token",
        "secret_key",
        "client_secret",
        "password",
        "passwd",
        "secret",
    ];
    for key in SECRET_KEYS {
        let lower_now = result.to_ascii_lowercase();
        if let Some(idx) = lower_now.find(key) {
            let after_key = idx + key.len();
            let tail = &result[after_key..];
            let trimmed_start = tail.trim_start_matches([' ', '"', '\'']);
            let sep_offset = tail.len() - trimmed_start.len();
            if let Some(first) = trimmed_start.chars().next() {
                if first == '=' || first == ':' {
                    let after_sep = &trimmed_start[1..];
                    let ws = after_sep.len() - after_sep.trim_start_matches([' ', '"', '\'']).len();
                    let val_start = after_key + sep_offset + 1 + ws;
                    if val_start < result.len() {
                        let val_rest = &result[val_start..];
                        let val_len = val_rest
                            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';'))
                            .unwrap_or(val_rest.len());
                        if val_len >= 4 && !val_rest.starts_with("[REDACTED") {
                            result.replace_range(val_start..val_start + val_len, "[REDACTED]");
                        }
                    }
                }
            }
        }
    }
    result
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    text.chars().take(max_chars).collect()
}

fn truncate_tail_chars(text: &str, max_chars: usize) -> String {
    let total = text.chars().count();
    if total <= max_chars {
        return text.to_string();
    }
    text.chars().skip(total - max_chars).collect()
}

fn is_failure_recovery_block(block: &str) -> bool {
    let lower = block.to_ascii_lowercase();
    let has_failure = lower.contains("failed")
        || lower.contains("falhou")
        || lower.contains("error")
        || lower.contains("erro")
        || block.contains('✗');
    let has_recovery = lower.contains("ok")
        || lower.contains("pass")
        || lower.contains("success")
        || lower.contains("sucesso")
        || block.contains('✓');
    has_failure && has_recovery
}

/// Prioritizes failure -> recovery `[work steps]` blocks up to `MAX_WORK_STEPS` (80).
pub fn prioritize_work_steps(blocks: &[String], max_steps: usize) -> Vec<String> {
    let mut prioritized = Vec::new();
    let mut regular = Vec::new();
    for block in blocks {
        let redacted = redact_secrets(block);
        if is_failure_recovery_block(&redacted) {
            prioritized.push(redacted);
        } else {
            regular.push(redacted);
        }
    }
    prioritized.extend(regular);
    prioritized.truncate(max_steps);
    prioritized
}

pub fn build_review_prompt(
    memories: &[ProjectMemory],
    messages: &[ReviewMessageEntry],
    work_steps: &[String],
    skills: &[LocalSkillSummary],
    card_id: Option<&str>,
    agent: Option<&str>,
) -> String {
    let mut mem_buf = String::new();
    for mem in memories {
        let tag = if mem.is_protected() { "manual/pinned" } else { mem.origin_type.as_str() };
        let line = format!("- [{}] ({}) {}\n", mem.id, tag, redact_secrets(&mem.fact));
        if mem_buf.chars().count() + line.chars().count() > MAX_MEMORIES_CHARS {
            break;
        }
        mem_buf.push_str(&line);
    }
    let mem_section = if mem_buf.is_empty() {
        "(nenhuma memória registrada)".to_string()
    } else {
        truncate_chars(&mem_buf, MAX_MEMORIES_CHARS)
    };

    let mut msg_buf = String::new();
    for msg in messages {
        let card_tag = msg
            .item_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|c| format!(" (card: {c})"))
            .unwrap_or_default();
        let line = format!(
            "[{} -> {}{}] {}\n",
            msg.from_agent,
            msg.to_agent,
            card_tag,
            redact_secrets(&msg.text)
        );
        msg_buf.push_str(&line);
    }
    let msg_section = if msg_buf.is_empty() {
        "(nenhuma mensagem recente)".to_string()
    } else {
        truncate_tail_chars(&msg_buf, MAX_MESSAGES_CHARS)
    };

    let selected_steps = prioritize_work_steps(work_steps, MAX_WORK_STEPS);
    let steps_section = if selected_steps.is_empty() {
        "(nenhum work step recente)".to_string()
    } else {
        selected_steps.join("\n")
    };

    let skills_section = if skills.is_empty() {
        "(nenhuma skill local instalada)".to_string()
    } else {
        skills
            .iter()
            .map(|s| format!("- {} ({}): {}", s.slug, s.scope.as_str(), s.preview))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let context_meta = format!(
        "card_id: {}, agent: {}",
        card_id.unwrap_or("none"),
        agent.unwrap_or("none")
    );

    format!(
        "Você é o revisor assíncrono de aprendizado do Vokia. Analise o contexto abaixo e retorne APENAS JSON estrito (sem texto fora do JSON).\n\
         Regras:\n\
         1. \"memories\": lista de operações {{\"op\":\"add\"|\"update\"|\"forget\", \"id\":\"opcional para update/forget\", \"fact\":\"texto curto e durável\", \"rationale\":\"motivo\"}}. NUNCA proponha update ou forget para memórias marcadas como (manual/pinned).\n\
         2. \"skill\": null OU no máximo 1 objeto {{\"op\":\"create\"|\"update\", \"slug\":\"kebab-case\", \"title\":\"nome\", \"description\":\"descrição\", \"content\":\"conteúdo completo de SKILL.md com frontmatter YAML (name, description)\", \"rationale\":\"motivo\"}} APENAS quando houver sinal técnico claro (correção recorrente de procedimento, armadilha não óbvia que funcionou, ou skill local incorreta). Nunca crie skill para erros pontuais de ambiente.\n\
         3. Se não houver aprendizado durável, retorne {{\"memories\":[],\"skill\":null}}.\n\n\
         Contexto: {context_meta}\n\n\
         === Memórias existentes (<= 12.000 chars) ===\n{mem_section}\n\n\
         === Skills locais ===\n{skills_section}\n\n\
         === Work steps (<= 80, falha->sucesso priorizados) ===\n{steps_section}\n\n\
         === Mensagens recentes (<= 24.000 chars) ===\n{msg_section}\n"
    )
}

/// Strict JSON parser for the Basic model's learning review output.
pub fn parse_review_response(raw: &str) -> Result<ParsedReviewOutput, String> {
    let trimmed = raw.trim();
    let json_slice = if let Some(stripped) = strip_json_fence(trimmed) {
        stripped
    } else if trimmed.starts_with('{') && trimmed.ends_with('}') {
        trimmed
    } else {
        return Err("Resposta fora do formato JSON estrito.".to_string());
    };

    let parsed: ParsedReviewOutput = serde_json::from_str(json_slice)
        .map_err(|err| format!("JSON inválido na revisão de memória: {err}"))?;

    for mem in &parsed.memories {
        match mem.op.trim().to_ascii_lowercase().as_str() {
            "add" => {
                if mem.fact.as_deref().map(str::trim).unwrap_or("").is_empty() {
                    return Err("Operação 'add' sem 'fact'.".to_string());
                }
            }
            "update" => {
                if mem.id.as_deref().map(str::trim).unwrap_or("").is_empty()
                    || mem.fact.as_deref().map(str::trim).unwrap_or("").is_empty()
                {
                    return Err("Operação 'update' exige 'id' e 'fact'.".to_string());
                }
            }
            "forget" => {
                if mem.id.as_deref().map(str::trim).unwrap_or("").is_empty() {
                    return Err("Operação 'forget' exige 'id'.".to_string());
                }
            }
            other => return Err(format!("Operação de memória inválida: {other}")),
        }
    }

    if let Some(skill) = &parsed.skill {
        let op = skill.op.trim().to_ascii_lowercase();
        if op != "create" && op != "update" {
            return Err(format!("Operação de skill inválida: {op}"));
        }
        if !is_valid_skill_slug(&skill.slug) {
            return Err(format!("Slug de skill inválido: {}", skill.slug));
        }
        if skill.content.trim().is_empty() {
            return Err("Conteúdo de SKILL.md vazio.".to_string());
        }
    }

    Ok(parsed)
}

fn strip_json_fence(trimmed: &str) -> Option<&str> {
    if !trimmed.starts_with("```") {
        return None;
    }
    let after_open = trimmed.strip_prefix("```json").or_else(|| trimmed.strip_prefix("```"))?;
    let body = after_open.trim_start_matches(['\r', '\n']);
    let end = body.rfind("```")?;
    Some(body[..end].trim())
}

pub fn is_valid_skill_slug(slug: &str) -> bool {
    let s = slug.trim();
    !s.is_empty()
        && s.len() <= 64
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("..")
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub fn ensure_valid_skill_markdown(
    slug: &str,
    title: Option<&str>,
    description: Option<&str>,
    raw_content: &str,
) -> Result<String, String> {
    if !is_valid_skill_slug(slug) {
        return Err(format!("Slug de skill inválido: {slug}"));
    }
    let trimmed = raw_content.trim();
    if trimmed.is_empty() {
        return Err("Conteúdo de SKILL.md não pode ser vazio.".to_string());
    }
    if trimmed.starts_with("---") {
        if let Some(rest) = trimmed.strip_prefix("---") {
            if let Some(end_idx) = rest.find("\n---") {
                let frontmatter = &rest[..end_idx];
                let body = rest[end_idx + 4..].trim();
                if frontmatter.contains("name:") && frontmatter.contains("description:") && !body.is_empty() {
                    return Ok(format!("{trimmed}\n"));
                }
            }
        }
    }
    let name = title
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(slug);
    let desc = description
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("Skill aprendida automaticamente pelo Vokia.");
    Ok(format!(
        "---\nname: {name}\ndescription: {desc}\n---\n\n{trimmed}\n"
    ))
}

pub fn list_local_skills(project_path: Option<&Path>, app_data_dir: Option<&Path>) -> Vec<LocalSkillSummary> {
    let mut skills = Vec::new();
    if let Some(proj) = project_path {
        collect_skills_in_root(&proj.join(SKILL_DIR_NAME), SkillScope::Project, &mut skills);
    }
    if let Some(data_dir) = app_data_dir {
        collect_skills_in_root(&data_dir.join("skills"), SkillScope::Global, &mut skills);
    }
    skills
}

fn collect_skills_in_root(root: &Path, scope: SkillScope, out: &mut Vec<LocalSkillSummary>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path().join(SKILL_FILE_NAME);
        if !path.is_file() {
            continue;
        }
        let slug = entry.file_name().to_string_lossy().to_string();
        if !is_valid_skill_slug(&slug) {
            continue;
        }
        let preview = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| raw.lines().take(6).collect::<Vec<_>>().join(" ").into())
            .unwrap_or_default();
        out.push(LocalSkillSummary {
            slug,
            scope,
            path,
            preview: truncate_chars(&preview, 200),
        });
    }
}

pub fn skill_file_path(
    slug: &str,
    scope: SkillScope,
    project_path: Option<&Path>,
    app_data_dir: Option<&Path>,
) -> Result<PathBuf, String> {
    if !is_valid_skill_slug(slug) {
        return Err(format!("Slug de skill inválido: {slug}"));
    }
    let root = match scope {
        SkillScope::Project => project_path
            .ok_or_else(|| "Caminho do projeto indisponível para salvar skill local.".to_string())?
            .join(SKILL_DIR_NAME),
        SkillScope::Global => app_data_dir
            .ok_or_else(|| "Diretório global do app indisponível para salvar skill.".to_string())?
            .join("skills"),
    };
    Ok(root.join(slug).join(SKILL_FILE_NAME))
}

/// Persists parsed review output according to `mode` (`ask` or `auto`), strictly protecting
/// any memory created or pinned manually by the user (`manual` / `pinned`).
#[allow(clippy::too_many_arguments)]
pub fn persist_review_output(
    db: &Db,
    project_id: i64,
    project_path: Option<&Path>,
    app_data_dir: Option<&Path>,
    mode: &str,
    output: &ParsedReviewOutput,
    source_agent: Option<&str>,
    source_card_id: Option<&str>,
    provider_id: Option<&str>,
    model: Option<&str>,
    now_ms: i64,
) -> Result<(usize, usize), String> {
    if mode == LEARNING_MODE_OFF {
        return Ok((0, 0));
    }

    let existing_memories = db
        .list_project_memories(project_id)
        .map_err(|e| e.to_string())?;
    let is_auto = mode == LEARNING_MODE_AUTO;

    let mut created_suggestions = 0usize;
    let mut auto_applied = 0usize;

    for mem_op in &output.memories {
        let op_lower = mem_op.op.trim().to_ascii_lowercase();
        let target_id = mem_op.id.as_deref().map(str::trim).filter(|s| !s.is_empty());

        // Enforce protection: NEVER alter or forget manual/pinned memories.
        if op_lower == "update" || op_lower == "forget" {
            let Some(tid) = target_id else {
                continue;
            };
            if existing_memories
                .iter()
                .find(|m| m.id == tid)
                .is_some_and(ProjectMemory::is_protected)
            {
                continue;
            }
        }

        let operation = match op_lower.as_str() {
            "add" => SuggestionOperation::Add,
            "update" => SuggestionOperation::Update,
            "forget" => SuggestionOperation::Forget,
            _ => continue,
        };

        let existing_fact = target_id
            .and_then(|tid| existing_memories.iter().find(|m| m.id == tid))
            .map(|m| m.fact.as_str());

        let fact_text = mem_op
            .fact
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or(existing_fact)
            .unwrap_or("");
        if fact_text.is_empty() {
            continue;
        }

        let title = match operation {
            SuggestionOperation::Add => "Nova memória sugerida",
            SuggestionOperation::Update => "Atualização de memória",
            SuggestionOperation::Forget => "Remover memória obsoleta",
            SuggestionOperation::Create => "Nova memória",
        };
        let rationale = mem_op.rationale.as_deref().unwrap_or("");
        let suggestion_id = next_id("lsug", now_ms);

        if is_auto {
            let applied_ok = match operation {
                SuggestionOperation::Add => {
                    let mem_id = next_id("mem", now_ms);
                    db.insert_project_memory(&NewProjectMemory {
                        id: &mem_id,
                        project_id,
                        fact: fact_text,
                        source: source_card_id.unwrap_or("auto-review"),
                        origin_type: MemoryOriginType::Auto,
                        pinned: false,
                        author: source_agent,
                        now_ms,
                    })
                    .is_ok()
                }
                SuggestionOperation::Update => {
                    if let Some(tid) = target_id {
                        db.update_project_memory_guarded(
                            project_id,
                            tid,
                            fact_text,
                            source_card_id.unwrap_or("auto-review"),
                            now_ms,
                        )
                        .unwrap_or(false)
                    } else {
                        false
                    }
                }
                SuggestionOperation::Forget => {
                    if let Some(tid) = target_id {
                        db.delete_project_memory_guarded(project_id, tid)
                            .unwrap_or(false)
                    } else {
                        false
                    }
                }
                SuggestionOperation::Create => false,
            };
            if applied_ok {
                db.insert_learning_suggestion(&NewLearningSuggestion {
                    id: &suggestion_id,
                    project_id,
                    kind: SuggestionKind::Memory,
                    operation,
                    status: SuggestionStatus::Applied,
                    target_memory_id: target_id,
                    skill_slug: None,
                    skill_scope: SkillScope::Project,
                    title,
                    content: fact_text,
                    previous_content: existing_fact,
                    rationale,
                    source_agent,
                    source_card_id,
                    provider_id,
                    model,
                    now_ms,
                })
                .map_err(|e| e.to_string())?;
                auto_applied += 1;
            }
        } else {
            db.insert_learning_suggestion(&NewLearningSuggestion {
                id: &suggestion_id,
                project_id,
                kind: SuggestionKind::Memory,
                operation,
                status: SuggestionStatus::Pending,
                target_memory_id: target_id,
                skill_slug: None,
                skill_scope: SkillScope::Project,
                title,
                content: fact_text,
                previous_content: existing_fact,
                rationale,
                source_agent,
                source_card_id,
                provider_id,
                model,
                now_ms,
            })
            .map_err(|e| e.to_string())?;
            created_suggestions += 1;
        }
    }

    if let Some(skill) = &output.skill {
        let slug = skill.slug.trim();
        let operation = if skill.op.trim().eq_ignore_ascii_case("update") {
            SuggestionOperation::Update
        } else {
            SuggestionOperation::Create
        };
        let valid_md = ensure_valid_skill_markdown(
            slug,
            skill.title.as_deref(),
            skill.description.as_deref(),
            &skill.content,
        )?;
        let existing_md = skill_file_path(slug, SkillScope::Project, project_path, app_data_dir)
            .ok()
            .and_then(|p| fs::read_to_string(p).ok());
        let title = skill
            .title
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(slug);
        let rationale = skill.rationale.as_deref().unwrap_or("");
        let suggestion_id = next_id("lsug", now_ms);

        if is_auto {
            if let Ok(path) = skill_file_path(slug, SkillScope::Project, project_path, app_data_dir) {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                write_atomic(&path, "SKILL.md.tmp", &valid_md).map_err(|e| e.to_string())?;
                db.insert_learning_suggestion(&NewLearningSuggestion {
                    id: &suggestion_id,
                    project_id,
                    kind: SuggestionKind::Skill,
                    operation,
                    status: SuggestionStatus::Applied,
                    target_memory_id: None,
                    skill_slug: Some(slug),
                    skill_scope: SkillScope::Project,
                    title,
                    content: &valid_md,
                    previous_content: existing_md.as_deref(),
                    rationale,
                    source_agent,
                    source_card_id,
                    provider_id,
                    model,
                    now_ms,
                })
                .map_err(|e| e.to_string())?;
                auto_applied += 1;
            }
        } else {
            db.insert_learning_suggestion(&NewLearningSuggestion {
                id: &suggestion_id,
                project_id,
                kind: SuggestionKind::Skill,
                operation,
                status: SuggestionStatus::Pending,
                target_memory_id: None,
                skill_slug: Some(slug),
                skill_scope: SkillScope::Project,
                title,
                content: &valid_md,
                previous_content: existing_md.as_deref(),
                rationale,
                source_agent,
                source_card_id,
                provider_id,
                model,
                now_ms,
            })
            .map_err(|e| e.to_string())?;
            created_suggestions += 1;
        }
    }

    Ok((created_suggestions, auto_applied))
}

pub fn apply_learning_suggestion_on_db(
    db: &Db,
    suggestion_id: &str,
    scope_override: Option<SkillScope>,
    project_path: Option<&Path>,
    app_data_dir: Option<&Path>,
    now_ms: i64,
) -> Result<LearningSuggestion, String> {
    let suggestion = db
        .get_learning_suggestion(suggestion_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Sugestão não encontrada: {suggestion_id}"))?;

    if suggestion.status != SuggestionStatus::Pending {
        return Err(format!(
            "Sugestão '{suggestion_id}' não está pendente (estado atual: {}).",
            suggestion.status.as_str()
        ));
    }

    match suggestion.kind {
        SuggestionKind::Memory => match suggestion.operation {
            SuggestionOperation::Add | SuggestionOperation::Create => {
                let mem_id = next_id("mem", now_ms);
                db.insert_project_memory(&NewProjectMemory {
                    id: &mem_id,
                    project_id: suggestion.project_id,
                    fact: &suggestion.content,
                    source: suggestion.source_card_id.as_deref().unwrap_or("review"),
                    origin_type: MemoryOriginType::Review,
                    pinned: false,
                    author: suggestion.source_agent.as_deref(),
                    now_ms,
                })
                .map_err(|e| e.to_string())?;
            }
            SuggestionOperation::Update => {
                let target_id = suggestion
                    .target_memory_id
                    .as_deref()
                    .ok_or_else(|| "Sugestão de atualização sem id da memória.".to_string())?;
                let updated = db
                    .update_project_memory_guarded(
                        suggestion.project_id,
                        target_id,
                        &suggestion.content,
                        suggestion.source_card_id.as_deref().unwrap_or("review"),
                        now_ms,
                    )
                    .map_err(|e| e.to_string())?;
                if !updated {
                    return Err("Memória protegida manualmente pelo usuário ou inexistente.".to_string());
                }
            }
            SuggestionOperation::Forget => {
                let target_id = suggestion
                    .target_memory_id
                    .as_deref()
                    .ok_or_else(|| "Sugestão de remoção sem id da memória.".to_string())?;
                let deleted = db
                    .delete_project_memory_guarded(suggestion.project_id, target_id)
                    .map_err(|e| e.to_string())?;
                if !deleted {
                    return Err("Memória protegida manualmente pelo usuário ou inexistente.".to_string());
                }
            }
        },
        SuggestionKind::Skill => {
            let slug = suggestion
                .skill_slug
                .as_deref()
                .ok_or_else(|| "Sugestão de skill sem slug.".to_string())?;
            let scope = scope_override.unwrap_or(suggestion.skill_scope);
            let valid_md = ensure_valid_skill_markdown(
                slug,
                Some(&suggestion.title),
                None,
                &suggestion.content,
            )?;
            let path = skill_file_path(slug, scope, project_path, app_data_dir)?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            write_atomic(&path, "SKILL.md.tmp", &valid_md).map_err(|e| e.to_string())?;
        }
    }

    db.resolve_learning_suggestion(suggestion_id, SuggestionStatus::Applied, now_ms)
        .map_err(|e| e.to_string())?;

    db.get_learning_suggestion(suggestion_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Sugestão não encontrada após aplicar: {suggestion_id}"))
}

pub fn dismiss_learning_suggestion_on_db(
    db: &Db,
    suggestion_id: &str,
    now_ms: i64,
) -> Result<LearningSuggestion, String> {
    let suggestion = db
        .get_learning_suggestion(suggestion_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Sugestão não encontrada: {suggestion_id}"))?;
    if suggestion.status != SuggestionStatus::Pending {
        return Err(format!(
            "Sugestão '{suggestion_id}' não está pendente."
        ));
    }
    db.resolve_learning_suggestion(suggestion_id, SuggestionStatus::Dismissed, now_ms)
        .map_err(|e| e.to_string())?;
    db.get_learning_suggestion(suggestion_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Sugestão não encontrada após descartar: {suggestion_id}"))
}

pub fn capture_review_snapshot(db: &Db, project_id: i64) -> PreCapturedSnapshot {
    let review_state = db.get_learning_review_state(project_id).unwrap_or_default();
    let messages = db
        .list_messages_for_review(project_id, review_state.last_reviewed_message_id, 120)
        .unwrap_or_default();
    let last_message_id = messages.last().map_or(review_state.last_reviewed_message_id, |m| m.id);
    let work_steps = db
        .list_work_steps_for_review(project_id, MAX_WORK_STEPS)
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.block)
        .collect();
    PreCapturedSnapshot {
        messages,
        work_steps,
        last_message_id,
    }
}

pub fn schedule_memory_review(
    app: &AppHandle,
    project_id: i64,
    trigger: ReviewTrigger,
    agent: Option<String>,
    card_id: Option<String>,
    snapshot: Option<PreCapturedSnapshot>,
) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = run_memory_review_for_project(
            &app,
            project_id,
            trigger,
            agent.as_deref(),
            card_id.as_deref(),
            snapshot,
        )
        .await;
    });
}

pub fn on_turn_completed_for_learning(
    app: &AppHandle,
    project_id: i64,
    agent: &Agent,
    item_id: Option<&str>,
) {
    let db_state = app.state::<DbState>();
    let Some(db) = db_state.db() else {
        return;
    };
    let mode = db.learning_mode().unwrap_or_else(|_| LEARNING_MODE_ASK.to_string());
    if mode == LEARNING_MODE_OFF {
        return;
    }
    let cadence = db.learning_turn_cadence().unwrap_or(20);
    let now_ms = now_millis_i64();
    let Ok(state) = db.record_turn_completed_for_learning(project_id, now_ms) else {
        return;
    };
    if cadence > 0 && state.turns_since_review >= cadence {
        schedule_memory_review(
            app,
            project_id,
            ReviewTrigger::TurnCadence,
            Some(agent.as_str().to_string()),
            item_id.map(str::to_string),
            None,
        );
    }
}

pub fn spawn_memory_review_idle_watcher(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(IDLE_WATCHER_INTERVAL_SECS)).await;
            let db_state = app.state::<DbState>();
            let Some(db) = db_state.db() else {
                continue;
            };
            let mode = db.learning_mode().unwrap_or_else(|_| LEARNING_MODE_ASK.to_string());
            if mode == LEARNING_MODE_OFF {
                continue;
            }
            let cutoff = now_millis_i64().saturating_sub((IDLE_REVIEW_SECS * 1000) as i64);
            let candidates = db
                .list_idle_learning_candidates(cutoff, IDLE_MIN_NEW_TURNS)
                .unwrap_or_default();
            for project_id in candidates {
                let _ = run_memory_review_for_project(
                    &app,
                    project_id,
                    ReviewTrigger::Idle,
                    None,
                    None,
                    None,
                )
                .await;
            }
        }
    });
}

pub async fn run_memory_review_for_project(
    app: &AppHandle,
    project_id: i64,
    trigger: ReviewTrigger,
    agent: Option<&str>,
    card_id: Option<&str>,
    snapshot: Option<PreCapturedSnapshot>,
) -> Result<ReviewOutcome, String> {
    let db_state = app.state::<DbState>();
    let db = db_state.require().map_err(|e| e.to_string())?;
    let mode = db.learning_mode().map_err(|e| e.to_string())?;
    if mode == LEARNING_MODE_OFF && trigger != ReviewTrigger::Manual {
        return Ok(ReviewOutcome::skipped("skipped_off"));
    }
    if mode == LEARNING_MODE_OFF {
        return Ok(ReviewOutcome::skipped("skipped_off"));
    }

    let app_state = app.state::<AppState>();
    let Some(_guard) = try_acquire_review_guard(&app_state.memory_review_active) else {
        return Ok(ReviewOutcome::skipped("skipped_busy"));
    };

    let snap = snapshot.unwrap_or_else(|| capture_review_snapshot(db, project_id));
    if snap.messages.is_empty() && snap.work_steps.is_empty() {
        return Ok(ReviewOutcome::skipped("skipped_no_signal"));
    }

    let providers = app.state::<ProviderRegistry>();
    let global_active = load_global_active_llm(app);
    let now_u64 = crate::usage::now_millis();
    let candidate = match select_basic_provider_candidate(
        &providers,
        &app_state.usage,
        global_active.as_ref(),
        now_u64,
    ) {
        ProviderSelectionResult::Selected(c) => c,
        ProviderSelectionResult::PostponedUsageLimit => {
            // Adia silenciosamente sem marcar erro no provedor.
            return Ok(ReviewOutcome::skipped("postponed_usage_limit"));
        }
        ProviderSelectionResult::NoEnabledProvider => {
            return Ok(ReviewOutcome::skipped("postponed_no_provider"));
        }
    };

    let project_path = db
        .project(project_id)
        .ok()
        .map(|p| PathBuf::from(p.path));
    let app_data_dir = app.path().app_data_dir().ok();
    let memories = db.list_project_memories(project_id).unwrap_or_default();
    let local_skills = list_local_skills(project_path.as_deref(), app_data_dir.as_deref());
    let prompt = build_review_prompt(
        &memories,
        &snap.messages,
        &snap.work_steps,
        &local_skills,
        card_id,
        agent,
    );

    let reply = match execute_toolless_basic_review(app, &app_state, &candidate, &prompt).await {
        Ok(reply) => reply,
        Err(err) => {
            eprintln!("[memory_review] execução discreta falhou: {err}");
            return Ok(ReviewOutcome::skipped("failed"));
        }
    };

    let parsed = match parse_review_response(&reply.text) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("[memory_review] JSON ignorado: {err}");
            return Ok(ReviewOutcome::skipped("failed"));
        }
    };

    let now_ms = now_millis_i64();
    let (created_suggestions, auto_applied) = persist_review_output(
        db,
        project_id,
        project_path.as_deref(),
        app_data_dir.as_deref(),
        &mode,
        &parsed,
        agent,
        card_id,
        Some(&candidate.provider.id),
        Some(&candidate.model),
        now_ms,
    )?;

    let _ = db.mark_learning_review_completed(project_id, snap.last_message_id, now_ms);

    if created_suggestions > 0 {
        let pending_count = db
            .count_pending_learning_suggestions(Some(project_id))
            .unwrap_or(created_suggestions);
        let _ = emit(
            app,
            VokiaEvent::LearningSuggestionsChanged {
                project_id,
                pending_count,
                new_count: created_suggestions,
            },
        );
    }

    if auto_applied > 0 {
        let _ = emit_feed_note(
            app,
            project_id,
            Speaker::Agent(Agent::ember()),
            &format!(
                "Aprendizado automático registrou {auto_applied} atualização(ões) de memória/skill ({}).",
                trigger.as_str()
            ),
            now_millis().unwrap_or(0),
        );
    }

    Ok(ReviewOutcome {
        status: "completed".to_string(),
        created_suggestions,
        auto_applied,
        provider_id: Some(candidate.provider.id),
        model: Some(candidate.model),
    })
}

async fn execute_toolless_basic_review(
    app: &AppHandle,
    state: &AppState,
    candidate: &BasicProviderCandidate,
    prompt: &str,
) -> Result<Reply, FlowError> {
    let agent = Agent::ember();
    let default_cli = state
        .cli
        .as_deref()
        .unwrap_or_else(|_| Path::new("claude"));
    let (program, user_args) = resolve_program_for(&agent, &candidate.provider.default_command, default_cli)?;
    let adapter = ProviderAdapter::from_config(candidate.provider.clone());
    let broker = PermissionBroker::default();
    let granted = std::sync::Arc::new(std::sync::Mutex::new(HashSet::new()));
    let gate = PermissionGate {
        app,
        broker: &broker,
        agent: agent.clone(),
        project_id: 0,
        item_id: None,
        granted: &granted,
        grant_per_session: false,
    };
    let pid = AtomicU32::new(0);
    let console = ConsoleSink::new(
        app,
        SessionKey {
            project_id: 0,
            agent,
        },
        None,
    );
    let call = AgentCall::for_text_ask(
        adapter.clone(),
        program,
        user_args,
        "Você é um revisor silencioso de memórias e skills. Responda apenas JSON válido.".to_string(),
        candidate.model.clone(),
    );
    adapter
        .run_fresh(
            Access::TextOnly,
            gate,
            &pid,
            &call,
            prompt,
            &console,
            state.sessions.adapters(),
        )
        .await
        .map_err(FlowError::Claude)
}

#[cfg(test)]
#[path = "../../tests/memory_review_tests.rs"]
mod tests;
