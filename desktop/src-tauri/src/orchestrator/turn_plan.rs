use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agents::Agent;

pub const MAX_TURN_PLAN_STEPS: usize = 100;
pub const MAX_PLAN_EXPLANATION_CHARS: usize = 500;
pub const MAX_PLAN_STEP_TEXT_CHARS: usize = 300;
pub const MAX_PLAN_STEP_ID_CHARS: usize = 120;

static TURN_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn next_turn_id() -> String {
    let seq = TURN_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    format!("turn-{seq}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnPlanStepStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnPlanStep {
    pub id: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_text: Option<String>,
    pub status: TurnPlanStepStatus,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnPlan {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    pub steps: Vec<TurnPlanStep>,
}

impl TurnPlan {
    pub fn normalize(self) -> Self {
        normalize_turn_plan(self)
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn completed_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|step| step.status == TurnPlanStepStatus::Completed)
            .count()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn open_steps(&self) -> Vec<&TurnPlanStep> {
        self.steps
            .iter()
            .filter(|step| step.status != TurnPlanStepStatus::Completed)
            .collect()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_all_completed(&self) -> bool {
        !self.steps.is_empty()
            && self
                .steps
                .iter()
                .all(|step| step.status == TurnPlanStepStatus::Completed)
    }
}

pub fn normalize_turn_plan(plan: TurnPlan) -> TurnPlan {
    let explanation = plan
        .explanation
        .as_deref()
        .and_then(|text| clean_text(text, MAX_PLAN_EXPLANATION_CHARS));

    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut normalized_steps: Vec<TurnPlanStep> =
        Vec::with_capacity(plan.steps.len().min(MAX_TURN_PLAN_STEPS));

    for step in plan.steps {
        if normalized_steps.len() >= MAX_TURN_PLAN_STEPS {
            break;
        }
        let Some(text) = clean_text(&step.text, MAX_PLAN_STEP_TEXT_CHARS) else {
            continue;
        };
        let active_text = step
            .active_text
            .as_deref()
            .and_then(|value| clean_text(value, MAX_PLAN_STEP_TEXT_CHARS));
        let fallback_index = normalized_steps.len() + 1;
        let base_id = clean_id(&step.id)
            .unwrap_or_else(|| format!("step-{fallback_index}"));
        let unique_id = deduplicate_id(&base_id, &mut seen_ids);
        normalized_steps.push(TurnPlanStep {
            id: unique_id,
            text,
            active_text,
            status: step.status,
        });
    }

    TurnPlan {
        explanation,
        steps: normalized_steps,
    }
}

fn clean_text(raw: &str, max_chars: usize) -> Option<String> {
    let collapsed = raw
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let trimmed = collapsed.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(truncate_chars(trimmed, max_chars))
}

fn clean_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(truncate_chars(trimmed, MAX_PLAN_STEP_ID_CHARS))
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn deduplicate_id(base: &str, seen: &mut HashSet<String>) -> String {
    if seen.insert(base.to_string()) {
        return base.to_string();
    }
    let mut suffix: usize = 2;
    loop {
        let tag = format!("-{suffix}");
        let max_prefix = MAX_PLAN_STEP_ID_CHARS.saturating_sub(tag.len());
        let prefix: String = base.chars().take(max_prefix).collect();
        let candidate = format!("{prefix}{tag}");
        if seen.insert(candidate.clone()) {
            return candidate;
        }
        suffix += 1;
    }
}

fn parse_step_status(raw: Option<&str>) -> Option<TurnPlanStepStatus> {
    let lower = raw?.trim().to_ascii_lowercase();
    match lower.as_str() {
        "pending" | "todo" | "not_started" | "queued" | "open" => {
            Some(TurnPlanStepStatus::Pending)
        }
        "in_progress" | "inprogress" | "in-progress" | "running" | "active" | "working" => {
            Some(TurnPlanStepStatus::InProgress)
        }
        "completed" | "complete" | "done" | "finished" => Some(TurnPlanStepStatus::Completed),
        _ => None,
    }
}

fn value_as_id(value: Option<&Value>) -> Option<String> {
    let value = value?;
    match value {
        Value::String(text) => clean_id(text),
        Value::Number(num) => clean_id(&num.to_string()),
        _ => None,
    }
}

fn first_non_empty_str<'a>(values: &[Option<&'a Value>]) -> Option<&'a str> {
    values
        .iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .map(str::trim)
        .find(|s| !s.is_empty())
}

// ==========================================
// 1. Extrator Claude (TodoWrite, TaskCreate, TaskUpdate + tool_result)
// ==========================================

#[derive(Clone, Debug, Default)]
pub struct ClaudePlanState {
    explanation: Option<String>,
    steps: Vec<TurnPlanStep>,
    /// Mapeia `tool_use_id` de um `TaskCreate` para o ID temporário atribuído antes do `tool_result`.
    pending_creates: HashMap<String, String>,
    temp_seq: u64,
}

impl ClaudePlanState {
    /// Remove etapas concluídas ao iniciar um novo turno preservando pendentes/em andamento.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn begin_new_turn(&mut self) {
        self.steps
            .retain(|step| step.status != TurnPlanStepStatus::Completed);
        self.pending_creates.clear();
    }

    pub fn snapshot(&self) -> TurnPlan {
        TurnPlan {
            explanation: self.explanation.clone(),
            steps: self.steps.clone(),
        }
        .normalize()
    }

    fn next_temp_id(&mut self, call_id: &str) -> String {
        let trimmed = call_id.trim();
        if !trimmed.is_empty() {
            return truncate_chars(&format!("temp:{trimmed}"), MAX_PLAN_STEP_ID_CHARS);
        }
        self.temp_seq += 1;
        format!("temp-{}", self.temp_seq)
    }
}

/// Processa chamadas de ferramenta de plano do Claude (`TodoWrite`, `TaskCreate`, `TaskUpdate`).
/// Devolve `Some(TurnPlan)` normalizado sempre que o plano mudar.
pub fn fold_claude_plan_call(
    state: &mut ClaudePlanState,
    call_id: &str,
    tool_name: &str,
    input: &Value,
) -> Option<TurnPlan> {
    match tool_name.trim().to_ascii_lowercase().as_str() {
        "todowrite" | "todo_write" => fold_claude_todo_write(state, input),
        "taskcreate" | "task_create" => fold_claude_task_create(state, call_id, input),
        "taskupdate" | "task_update" => fold_claude_task_update(state, input),
        _ => None,
    }
}

fn fold_claude_todo_write(state: &mut ClaudePlanState, input: &Value) -> Option<TurnPlan> {
    let todos = input
        .get("todos")
        .or_else(|| input.get("steps"))
        .and_then(Value::as_array)?;
    let explanation = first_non_empty_str(&[input.get("explanation")]).map(str::to_string);
    let mut steps = Vec::with_capacity(todos.len().min(MAX_TURN_PLAN_STEPS));
    for (idx, item) in todos.iter().enumerate() {
        let Some(text) = first_non_empty_str(&[
            item.get("content"),
            item.get("text"),
            item.get("subject"),
            item.get("step"),
        ]) else {
            continue;
        };
        let active_text = first_non_empty_str(&[
            item.get("activeForm"),
            item.get("activeText"),
            item.get("active_form"),
            item.get("active_text"),
        ])
        .map(str::to_string);
        let id = value_as_id(item.get("id")).unwrap_or_else(|| format!("step-{}", idx + 1));
        let status = parse_step_status(item.get("status").and_then(Value::as_str))
            .unwrap_or(TurnPlanStepStatus::Pending);
        steps.push(TurnPlanStep {
            id,
            text: text.to_string(),
            active_text,
            status,
        });
    }
    state.explanation = explanation;
    state.steps = steps;
    state.pending_creates.clear();
    Some(state.snapshot())
}

fn fold_claude_task_create(
    state: &mut ClaudePlanState,
    call_id: &str,
    input: &Value,
) -> Option<TurnPlan> {
    let text = first_non_empty_str(&[
        input.get("subject"),
        input.get("content"),
        input.get("text"),
        input.get("title"),
    ])?;
    let active_text = first_non_empty_str(&[
        input.get("activeForm"),
        input.get("activeText"),
        input.get("active_form"),
        input.get("active_text"),
    ])
    .map(str::to_string);
    let status = parse_step_status(input.get("status").and_then(Value::as_str))
        .unwrap_or(TurnPlanStepStatus::Pending);
    let temp_id = value_as_id(input.get("id")).unwrap_or_else(|| state.next_temp_id(call_id));
    if !call_id.trim().is_empty() {
        state
            .pending_creates
            .insert(call_id.trim().to_string(), temp_id.clone());
    }
    state.steps.push(TurnPlanStep {
        id: temp_id,
        text: text.to_string(),
        active_text,
        status,
    });
    Some(state.snapshot())
}

fn fold_claude_task_update(state: &mut ClaudePlanState, input: &Value) -> Option<TurnPlan> {
    let task_id = value_as_id(
        input
            .get("taskId")
            .or_else(|| input.get("task_id"))
            .or_else(|| input.get("id")),
    )?;
    let raw_status = input.get("status").and_then(Value::as_str).map(str::trim);
    let is_deleted = raw_status.is_some_and(|s| s.eq_ignore_ascii_case("deleted"));

    if is_deleted {
        let before = state.steps.len();
        state.steps.retain(|step| step.id != task_id);
        return (state.steps.len() != before).then(|| state.snapshot());
    }

    let next_status = parse_step_status(raw_status);
    let next_text = first_non_empty_str(&[
        input.get("subject"),
        input.get("content"),
        input.get("text"),
        input.get("title"),
    ]);
    let next_active = first_non_empty_str(&[
        input.get("activeForm"),
        input.get("activeText"),
        input.get("active_form"),
        input.get("active_text"),
    ]);

    if let Some(step) = state.steps.iter_mut().find(|step| step.id == task_id) {
        let mut changed = false;
        if let Some(status) = next_status {
            if step.status != status {
                step.status = status;
                changed = true;
            }
        }
        if let Some(text) = next_text {
            let trimmed = truncate_chars(text, MAX_PLAN_STEP_TEXT_CHARS);
            if step.text != trimmed {
                step.text = trimmed;
                changed = true;
            }
        }
        if let Some(active) = next_active {
            let trimmed = Some(truncate_chars(active, MAX_PLAN_STEP_TEXT_CHARS));
            if step.active_text != trimmed {
                step.active_text = trimmed;
                changed = true;
            }
        }
        return changed.then(|| state.snapshot());
    }

    // Tarefa criada em turno anterior que chegou via TaskUpdate com texto: incorpora no plano vivo.
    let text = next_text?;
    state.steps.push(TurnPlanStep {
        id: task_id,
        text: text.to_string(),
        active_text: next_active.map(str::to_string),
        status: next_status.unwrap_or(TurnPlanStepStatus::Pending),
    });
    Some(state.snapshot())
}

/// Processa o `tool_result` correspondente a um `TaskCreate` para remapear o ID temporário
/// ao ID definitivo retornado pelo Claude (ou remover a etapa temporária se houve erro).
pub fn fold_claude_plan_result(
    state: &mut ClaudePlanState,
    tool_use_id: &str,
    result_content: Option<&Value>,
    tool_use_result: Option<&Value>,
    is_error: bool,
) -> Option<TurnPlan> {
    let temp_id = state.pending_creates.remove(tool_use_id.trim())?;
    if is_error {
        let before = state.steps.len();
        state.steps.retain(|step| step.id != temp_id);
        return (state.steps.len() != before).then(|| state.snapshot());
    }
    let definitive_id = extract_claude_task_id(tool_use_result, result_content)?;
    if definitive_id == temp_id {
        return None;
    }
    let idx = state.steps.iter().position(|step| step.id == temp_id)?;
    // Se já existir outra etapa com esse ID definitivo (ex.: TaskUpdate chegou antes), funde preservando a ordem.
    if let Some(existing_idx) = state
        .steps
        .iter()
        .position(|step| step.id == definitive_id)
    {
        if existing_idx != idx {
            let updated = state.steps.remove(existing_idx);
            let target_idx = if existing_idx < idx { idx - 1 } else { idx };
            if let Some(target) = state.steps.get_mut(target_idx) {
                target.id = definitive_id;
                target.status = updated.status;
                if !updated.text.trim().is_empty() {
                    target.text = updated.text;
                }
                if updated.active_text.is_some() {
                    target.active_text = updated.active_text;
                }
            }
            return Some(state.snapshot());
        }
    }
    state.steps[idx].id = definitive_id;
    Some(state.snapshot())
}

fn extract_claude_task_id(
    tool_use_result: Option<&Value>,
    result_content: Option<&Value>,
) -> Option<String> {
    if let Some(outcome) = tool_use_result {
        if let Some(id) = value_as_id(
            outcome
                .pointer("/task/id")
                .or_else(|| outcome.get("taskId"))
                .or_else(|| outcome.get("task_id"))
                .or_else(|| outcome.get("id")),
        ) {
            return Some(id);
        }
    }
    let content = result_content?;
    if let Some(id) = value_as_id(
        content
            .pointer("/task/id")
            .or_else(|| content.get("taskId"))
            .or_else(|| content.get("task_id"))
            .or_else(|| content.get("id")),
    ) {
        return Some(id);
    }
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => return None,
    };
    if let Ok(parsed) = serde_json::from_str::<Value>(text.trim()) {
        if let Some(id) = value_as_id(
            parsed
                .pointer("/task/id")
                .or_else(|| parsed.get("taskId"))
                .or_else(|| parsed.get("task_id"))
                .or_else(|| parsed.get("id")),
        ) {
            return Some(id);
        }
    }
    extract_task_id_from_text(&text)
}

fn extract_task_id_from_text(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    for marker in ["task #", "task id:", "task_id:", "taskid:", "id:"] {
        if let Some(pos) = lower.find(marker) {
            let rest = text[pos + marker.len()..].trim_start();
            let token: String = rest
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
                .collect();
            if let Some(cleaned) = clean_id(&token) {
                return Some(cleaned);
            }
        }
    }
    None
}

// ==========================================
// 2. Extrator Codex (`turn/plan/updated` e itens `todo_list`/`plan`)
// ==========================================

pub fn plan_from_codex_notification(value: &Value) -> Option<TurnPlan> {
    let payload = if let Some(method) = value.get("method").and_then(Value::as_str) {
        if !method.eq_ignore_ascii_case("turn/plan/updated") {
            return None;
        }
        value.get("params")?
    } else if let Some(kind) = value.get("type").and_then(Value::as_str) {
        match kind {
            "turn/plan/updated" | "turn.plan.updated" | "plan.updated" => {
                value.get("params").unwrap_or(value)
            }
            "item.started" | "item.updated" | "item.completed" => {
                let item = value.get("item")?;
                let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
                if !matches!(item_type, "todo_list" | "plan") {
                    return None;
                }
                item
            }
            _ => return None,
        }
    } else if value.get("plan").is_some() || value.get("steps").is_some() {
        value
    } else {
        return None;
    };

    let raw_steps = payload
        .get("plan")
        .or_else(|| payload.get("steps"))
        .or_else(|| payload.get("items"))
        .and_then(Value::as_array)?;

    let explanation = first_non_empty_str(&[payload.get("explanation")]).map(str::to_string);
    let mut steps = Vec::with_capacity(raw_steps.len().min(MAX_TURN_PLAN_STEPS));
    for (idx, entry) in raw_steps.iter().enumerate() {
        let Some(text) = first_non_empty_str(&[
            entry.get("step"),
            entry.get("text"),
            entry.get("content"),
            entry.get("title"),
        ]) else {
            continue;
        };
        let active_text = first_non_empty_str(&[
            entry.get("activeText"),
            entry.get("active_text"),
            entry.get("activeForm"),
            entry.get("active_form"),
        ])
        .map(str::to_string);
        let id = value_as_id(entry.get("id")).unwrap_or_else(|| format!("step-{}", idx + 1));
        let status = parse_step_status(entry.get("status").and_then(Value::as_str))
            .or_else(|| {
                entry.get("completed").and_then(Value::as_bool).map(|done| {
                    if done {
                        TurnPlanStepStatus::Completed
                    } else {
                        TurnPlanStepStatus::Pending
                    }
                })
            })
            .unwrap_or(TurnPlanStepStatus::Pending);
        steps.push(TurnPlanStep {
            id,
            text: text.to_string(),
            active_text,
            status,
        });
    }

    Some(TurnPlan { explanation, steps }.normalize())
}

// ==========================================
// 3. Extrator OpenCode e Gemini (ACP `plan` e eventos compatíveis)
// ==========================================

pub fn plan_from_acp_entries(entries: &[Value]) -> TurnPlan {
    plan_from_acp_entries_with_explanation(entries, None)
}

pub fn plan_from_acp_entries_with_explanation(
    entries: &[Value],
    explanation: Option<&str>,
) -> TurnPlan {
    let mut steps = Vec::with_capacity(entries.len().min(MAX_TURN_PLAN_STEPS));
    for (idx, entry) in entries.iter().enumerate() {
        let Some(text) = first_non_empty_str(&[
            entry.get("content"),
            entry.get("text"),
            entry.get("title"),
            entry.get("step"),
            entry.get("subject"),
        ]) else {
            continue;
        };
        let active_text = first_non_empty_str(&[
            entry.get("activeText"),
            entry.get("active_text"),
            entry.get("activeForm"),
            entry.get("active_form"),
        ])
        .map(str::to_string);
        let id = value_as_id(entry.get("id")).unwrap_or_else(|| format!("step-{}", idx + 1));
        let status = parse_step_status(entry.get("status").and_then(Value::as_str))
            .unwrap_or(TurnPlanStepStatus::Pending);
        steps.push(TurnPlanStep {
            id,
            text: text.to_string(),
            active_text,
            status,
        });
    }
    TurnPlan {
        explanation: explanation.map(str::to_string),
        steps,
    }
    .normalize()
}

/// Extrai um `TurnPlan` de notificações ACP (`session/update` com `sessionUpdate: "plan"`)
/// ou eventos de plano compatíveis emitidos por OpenCode e Antigravity/Gemini.
pub fn plan_from_acp_notification(value: &Value) -> Option<TurnPlan> {
    // 1. ACP JSON-RPC padrão: { "method": "session/update", "params": { "update": { "sessionUpdate": "plan", "entries": [...] } } }
    if let Some(update) = value
        .pointer("/params/update")
        .or_else(|| value.get("update"))
    {
        let update_kind = update
            .get("sessionUpdate")
            .or_else(|| update.get("session_update"))
            .or_else(|| update.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if update_kind.eq_ignore_ascii_case("plan") {
            let entries = update
                .get("entries")
                .or_else(|| update.get("steps"))
                .and_then(Value::as_array)?;
            let explanation = first_non_empty_str(&[update.get("explanation")]);
            return Some(plan_from_acp_entries_with_explanation(entries, explanation));
        }
    }

    // 2. Evento direto: { "type": "plan", "entries": [...] } ou { "part": { "type": "plan", "entries": [...] } }
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    let part = value.get("part");
    let part_kind = part
        .and_then(|p| p.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if kind.eq_ignore_ascii_case("plan") || part_kind.eq_ignore_ascii_case("plan") {
        let container = if part_kind.eq_ignore_ascii_case("plan") {
            part?
        } else {
            value
        };
        let entries = container
            .get("entries")
            .or_else(|| container.get("steps"))
            .or_else(|| container.get("plan"))
            .and_then(Value::as_array)?;
        let explanation = first_non_empty_str(&[container.get("explanation")]);
        return Some(plan_from_acp_entries_with_explanation(entries, explanation));
    }

    // 3. Antigravity stream-json: { "event": "step_update", "step_update": { "step_type": "plan", "steps" | "entries": [...] } }
    if value.get("event").and_then(Value::as_str) == Some("step_update") {
        let step = value.get("step_update")?;
        let step_type = step.get("step_type").and_then(Value::as_str).unwrap_or("");
        if step_type.eq_ignore_ascii_case("plan") {
            let entries = step
                .get("entries")
                .or_else(|| step.get("steps"))
                .or_else(|| step.get("plan"))
                .and_then(Value::as_array)?;
            let explanation = first_non_empty_str(&[step.get("explanation")]);
            return Some(plan_from_acp_entries_with_explanation(entries, explanation));
        }
    }

    None
}

// ==========================================
// 4. Armazenamento em memória (AppState)
// ==========================================

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnPlanSnapshot {
    pub project_id: i64,
    pub agent: Agent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_id: Option<String>,
    pub turn_id: String,
    pub plan: TurnPlan,
    pub updated_at: u64,
    pub active: bool,
}

#[derive(Default)]
struct TurnPlanStoreInner {
    by_agent: HashMap<(i64, Agent), TurnPlanSnapshot>,
    by_card: HashMap<(i64, String), TurnPlanSnapshot>,
}

#[derive(Default)]
pub struct TurnPlanStore {
    inner: Mutex<TurnPlanStoreInner>,
}

impl TurnPlanStore {
    pub fn upsert(&self, snapshot: TurnPlanSnapshot) {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(card_id) = snapshot.card_id.as_ref().filter(|id| !id.trim().is_empty()) {
            inner
                .by_card
                .insert((snapshot.project_id, card_id.clone()), snapshot.clone());
        }
        inner
            .by_agent
            .insert((snapshot.project_id, snapshot.agent.clone()), snapshot);
    }

    pub fn mark_turn_finished(&self, project_id: i64, agent: &Agent, turn_id: &str) {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(entry) = inner.by_agent.get_mut(&(project_id, agent.clone())) {
            if entry.turn_id == turn_id {
                entry.active = false;
            }
        }
        for entry in inner.by_card.values_mut() {
            if entry.project_id == project_id && &entry.agent == agent && entry.turn_id == turn_id {
                entry.active = false;
            }
        }
    }

    pub fn get(
        &self,
        agent: &Agent,
        project_id: Option<i64>,
        card_id: Option<&str>,
    ) -> Option<TurnPlanSnapshot> {
        let inner = self
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let card_id = card_id.map(str::trim).filter(|s| !s.is_empty());

        if let (Some(pid), Some(cid)) = (project_id, card_id) {
            if let Some(snap) = inner.by_agent.get(&(pid, agent.clone())) {
                if snap.card_id.as_deref() == Some(cid) {
                    return Some(snap.clone());
                }
            }
            return inner
                .by_card
                .get(&(pid, cid.to_string()))
                .filter(|snap| &snap.agent == agent)
                .cloned();
        }

        if let Some(pid) = project_id {
            return inner.by_agent.get(&(pid, agent.clone())).cloned();
        }

        if let Some(cid) = card_id {
            let from_agents = inner
                .by_agent
                .values()
                .filter(|snap| &snap.agent == agent && snap.card_id.as_deref() == Some(cid))
                .max_by_key(|snap| (snap.active, snap.updated_at))
                .cloned();
            if from_agents.is_some() {
                return from_agents;
            }
            return inner
                .by_card
                .values()
                .filter(|snap| &snap.agent == agent && snap.card_id.as_deref() == Some(cid))
                .max_by_key(|snap| (snap.active, snap.updated_at))
                .cloned();
        }

        inner
            .by_agent
            .values()
            .filter(|snap| &snap.agent == agent)
            .max_by_key(|snap| (snap.active, snap.updated_at))
            .cloned()
    }

    pub fn forget_project(&self, project_id: i64) {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        inner.by_agent.retain(|(pid, _), _| *pid != project_id);
        inner.by_card.retain(|(pid, _), _| *pid != project_id);
    }
}

#[cfg(test)]
#[path = "../../tests/turn_plan_tests.rs"]
mod tests;
