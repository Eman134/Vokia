use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::agents::{is_agent_or_replica, Agent};
use crate::claude::{ClaudeError, ToolCategory, ToolStatus, TurnActivity};
use crate::db::{DbState, SETTING_CONTINUATION_ENABLED, SETTING_CONTINUATION_MAX_COUNT};
use crate::session_key::SessionKey;

use super::error::FlowError;
use super::turn_plan::{TurnPlan, TurnPlanStep, TurnPlanStepStatus};
use super::work_steps::{preserve_head_and_tail, HANDOFF_WORK_STEPS_BUDGET};

pub const DEFAULT_CONTINUATION_ENABLED: bool = true;
pub const DEFAULT_CONTINUATION_MAX_COUNT: u32 = 5;
pub const MAX_CONTINUATION_MAX_COUNT: u32 = 50;
pub const MAX_NEXT_STEP_NUDGES: u32 = 2;
pub const MAX_FINISH_CHECK_NUDGES: u32 = 3;
pub const MAX_RECOVERY_NUDGES: u32 = 1;
pub const NEXT_STEP_MAX_CHARS: usize = 400;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationConfig {
    pub enabled: bool,
    pub max_count: u32,
}

impl Default for ContinuationConfig {
    fn default() -> Self {
        Self {
            enabled: DEFAULT_CONTINUATION_ENABLED,
            max_count: DEFAULT_CONTINUATION_MAX_COUNT,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationTrigger {
    Plan,
    NextStep,
    FinishCheck,
    Compaction,
    ProviderExit,
}

impl ContinuationTrigger {
    pub fn label_pt(self) -> &'static str {
        match self {
            Self::Plan => "etapas pendentes",
            Self::NextStep => "próximo passo anunciado",
            Self::FinishCheck => "verificação pendente",
            Self::Compaction => "recuperação de compactação",
            Self::ProviderExit => "retomada após queda da CLI",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationStopReason {
    Disabled,
    Cancelled,
    UsageLimitPaused,
    PendingHumanDecision,
    Stagnated,
    MaxCountReached,
    NextStepCapReached,
    FinishCheckCapReached,
    RecoveryCapReached,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContinuationDecision {
    Continue {
        trigger: ContinuationTrigger,
        attempt: u32,
        max_attempts: u32,
        prompt: String,
        marker: String,
    },
    Stopped {
        reason: ContinuationStopReason,
        trigger: Option<ContinuationTrigger>,
    },
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnFailureKind {
    Compaction,
    ProviderExit,
    Other,
}

#[derive(Clone, Debug, Default)]
pub struct TurnEndInput {
    pub reply_text: String,
    pub turn_plan: Option<TurnPlan>,
    pub executed_action_tool: bool,
    pub has_pending_human_decision: bool,
    pub usage_limit_paused: bool,
    pub cancelled: bool,
    pub pending_verification_items: Vec<String>,
    pub has_verification_evidence: bool,
    pub compaction_failed: bool,
    pub provider_exited_abruptly: bool,
    pub work_steps_blocks: Vec<String>,
}

/// Motor anti-abandono e continuação automática de turnos.
/// Avalia o estado ao encerrar cada turno antes de devolver controle ao usuário
/// ou encerrar a micro-tarefa, aplicando os 4 gatilhos ordenados e regras de segurança.
#[derive(Clone, Debug)]
pub struct PlanContinuation {
    config: ContinuationConfig,
    total_count: u32,
    plan_count: u32,
    next_step_count: u32,
    finish_check_count: u32,
    recovery_count: u32,
    last_plan_signature: Option<String>,
    cancelled: bool,
}

impl Default for PlanContinuation {
    fn default() -> Self {
        Self::new(ContinuationConfig::default())
    }
}

impl PlanContinuation {
    pub fn new(config: ContinuationConfig) -> Self {
        Self {
            config,
            total_count: 0,
            plan_count: 0,
            next_step_count: 0,
            finish_check_count: 0,
            recovery_count: 0,
            last_plan_signature: None,
            cancelled: false,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn config(&self) -> ContinuationConfig {
        self.config
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn count(&self) -> u32 {
        self.total_count
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn plan_count(&self) -> u32 {
        self.plan_count
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn next_step_count(&self) -> u32 {
        self.next_step_count
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn finish_check_count(&self) -> u32 {
        self.finish_check_count
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn recovery_count(&self) -> u32 {
        self.recovery_count
    }

    pub fn consider(&mut self, input: &TurnEndInput) -> ContinuationDecision {
        // Regras globais de segurança antes de qualquer gatilho.
        if input.cancelled || self.cancelled {
            self.cancelled = true;
            return ContinuationDecision::Stopped {
                reason: ContinuationStopReason::Cancelled,
                trigger: None,
            };
        }
        if !self.config.enabled {
            return ContinuationDecision::Stopped {
                reason: ContinuationStopReason::Disabled,
                trigger: None,
            };
        }
        if input.usage_limit_paused {
            return ContinuationDecision::Stopped {
                reason: ContinuationStopReason::UsageLimitPaused,
                trigger: None,
            };
        }
        if input.has_pending_human_decision {
            return ContinuationDecision::Stopped {
                reason: ContinuationStopReason::PendingHumanDecision,
                trigger: None,
            };
        }

        // Gatilho 4 (queda/compactação): quando o turno falhou abruptamente em vez de encerrar normalmente.
        if input.compaction_failed || input.provider_exited_abruptly {
            let trigger = if input.compaction_failed {
                ContinuationTrigger::Compaction
            } else {
                ContinuationTrigger::ProviderExit
            };
            if self.recovery_count >= MAX_RECOVERY_NUDGES {
                return ContinuationDecision::Stopped {
                    reason: ContinuationStopReason::RecoveryCapReached,
                    trigger: Some(trigger),
                };
            }
            if self.total_count >= self.config.max_count {
                return ContinuationDecision::Stopped {
                    reason: ContinuationStopReason::MaxCountReached,
                    trigger: Some(trigger),
                };
            }
            self.total_count += 1;
            self.recovery_count += 1;
            let open_steps = input
                .turn_plan
                .as_ref()
                .map(TurnPlan::open_steps)
                .unwrap_or_default();
            let prompt = build_recovery_prompt(trigger, &input.work_steps_blocks, &open_steps);
            let marker =
                format_continuation_marker(self.total_count, self.config.max_count, trigger);
            return ContinuationDecision::Continue {
                trigger,
                attempt: self.total_count,
                max_attempts: self.config.max_count,
                prompt,
                marker,
            };
        }

        // Gatilho 1: `plan` — etapas pendentes ou em andamento no TurnPlan (088),
        // com proteção contra estagnação pela assinatura serializada de `openSteps`.
        if let Some(plan) = input.turn_plan.as_ref() {
            let open_steps = plan.open_steps();
            if !open_steps.is_empty() && !is_waiting_for_human(&input.reply_text) {
                let signature = open_steps_signature(plan);
                if self.last_plan_signature.as_deref() == Some(signature.as_str()) {
                    return ContinuationDecision::Stopped {
                        reason: ContinuationStopReason::Stagnated,
                        trigger: Some(ContinuationTrigger::Plan),
                    };
                }
                if self.total_count >= self.config.max_count {
                    return ContinuationDecision::Stopped {
                        reason: ContinuationStopReason::MaxCountReached,
                        trigger: Some(ContinuationTrigger::Plan),
                    };
                }
                self.last_plan_signature = Some(signature);
                self.total_count += 1;
                self.plan_count += 1;
                let prompt = build_plan_prompt(&open_steps);
                let marker = format_continuation_marker(
                    self.total_count,
                    self.config.max_count,
                    ContinuationTrigger::Plan,
                );
                return ContinuationDecision::Continue {
                    trigger: ContinuationTrigger::Plan,
                    attempt: self.total_count,
                    max_attempts: self.config.max_count,
                    prompt,
                    marker,
                };
            }
            self.last_plan_signature = None;
        }

        // Gatilho 2: `next_step` — resposta curta (<= 400 chars) anunciando trabalho sem execução.
        if !input.executed_action_tool && detect_next_step_announcement(&input.reply_text) {
            if self.next_step_count >= MAX_NEXT_STEP_NUDGES {
                return ContinuationDecision::Stopped {
                    reason: ContinuationStopReason::NextStepCapReached,
                    trigger: Some(ContinuationTrigger::NextStep),
                };
            }
            if self.total_count >= self.config.max_count {
                return ContinuationDecision::Stopped {
                    reason: ContinuationStopReason::MaxCountReached,
                    trigger: Some(ContinuationTrigger::NextStep),
                };
            }
            self.total_count += 1;
            self.next_step_count += 1;
            let prompt = build_next_step_prompt(&input.reply_text);
            let marker = format_continuation_marker(
                self.total_count,
                self.config.max_count,
                ContinuationTrigger::NextStep,
            );
            return ContinuationDecision::Continue {
                trigger: ContinuationTrigger::NextStep,
                attempt: self.total_count,
                max_attempts: self.config.max_count,
                prompt,
                marker,
            };
        }

        // Gatilho 3: `finish_check` — critérios de aceite / verificação pendentes sem evidência.
        if !input.pending_verification_items.is_empty() && !input.has_verification_evidence {
            // Nunca interrompe quando o agente fez uma pergunta real esperando o usuário.
            if !is_waiting_for_human(&input.reply_text) {
                if self.finish_check_count >= MAX_FINISH_CHECK_NUDGES {
                    return ContinuationDecision::Stopped {
                        reason: ContinuationStopReason::FinishCheckCapReached,
                        trigger: Some(ContinuationTrigger::FinishCheck),
                    };
                }
                if self.total_count >= self.config.max_count {
                    return ContinuationDecision::Stopped {
                        reason: ContinuationStopReason::MaxCountReached,
                        trigger: Some(ContinuationTrigger::FinishCheck),
                    };
                }
                self.total_count += 1;
                self.finish_check_count += 1;
                let prompt = build_finish_check_prompt(&input.pending_verification_items);
                let marker = format_continuation_marker(
                    self.total_count,
                    self.config.max_count,
                    ContinuationTrigger::FinishCheck,
                );
                return ContinuationDecision::Continue {
                    trigger: ContinuationTrigger::FinishCheck,
                    attempt: self.total_count,
                    max_attempts: self.config.max_count,
                    prompt,
                    marker,
                };
            }
        }

        ContinuationDecision::Complete
    }
}

pub fn format_continuation_marker(
    attempt: u32,
    max_attempts: u32,
    trigger: ContinuationTrigger,
) -> String {
    format!(
        "Continuação automática {attempt}/{max_attempts} · {}",
        trigger.label_pt()
    )
}

/// Serializa as etapas abertas (`pending` ou `in_progress`) do `TurnPlan` em uma
/// assinatura determinística para detecção de estagnação entre turnos consecutivos.
pub fn open_steps_signature(plan: &TurnPlan) -> String {
    let open: Vec<(&str, &str, TurnPlanStepStatus)> = plan
        .open_steps()
        .into_iter()
        .map(|step| (step.id.as_str(), step.text.as_str(), step.status))
        .collect();
    serde_json::to_string(&open).unwrap_or_default()
}

/// Detecta respostas curtas (<= 400 chars) em PT-BR ou EN anunciando trabalho futuro
/// sem tê-lo executado, respeitando exceções obrigatórias para perguntas reais e pedidos
/// de aprovação humana.
pub fn detect_next_step_announcement(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.chars().count() > NEXT_STEP_MAX_CHARS {
        return false;
    }

    let normalized = normalize_for_match(trimmed);
    if normalized.is_empty() {
        return false;
    }

    // Exceção 1: espera explícita por aprovação ou confirmação humana.
    if contains_approval_wait(&normalized) {
        return false;
    }

    // Exceção 2: perguntas reais esperando decisão humana.
    if trimmed.contains('?') {
        if is_decision_question(&normalized) {
            return false;
        }
        return is_trivial_continue_question(&normalized);
    }

    if is_choice_prompt(&normalized) {
        return false;
    }

    NEXT_STEP_PATTERNS
        .iter()
        .any(|pattern| normalized.contains(pattern))
}

/// Indica se a resposta está claramente aguardando resposta ou aprovação humana.
pub fn is_waiting_for_human(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let normalized = normalize_for_match(trimmed);
    if contains_approval_wait(&normalized) || is_choice_prompt(&normalized) {
        return true;
    }
    if trimmed.contains('?') && !is_trivial_continue_question(&normalized) {
        return true;
    }
    false
}

/// Indica se a resposta pede aprovação ou confirmação explícita do usuário.
pub fn is_approval_wait(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    contains_approval_wait(&normalize_for_match(trimmed))
}

const APPROVAL_WAIT_PATTERNS: &[&str] = &[
    "se voce aprovar",
    "se aprovar",
    "aguardando sua confirmacao",
    "aguardando confirmacao",
    "aguardo sua confirmacao",
    "aguardo confirmacao",
    "aguardando sua aprovacao",
    "aguardando aprovacao",
    "aguardo sua aprovacao",
    "aguardo aprovacao",
    "aguardando seu ok",
    "aguardo seu ok",
    "aguardando sua resposta",
    "aguardo sua resposta",
    "aguardando sua decisao",
    "aguardo sua decisao",
    "se voce concordar",
    "me confirme",
    "por favor confirme",
    "confirme se",
    "pode confirmar",
    "antes de prosseguir, confirme",
    "antes de continuar, confirme",
    "if you approve",
    "once you approve",
    "waiting for your confirmation",
    "awaiting your confirmation",
    "waiting for your approval",
    "awaiting your approval",
    "waiting for your response",
    "please confirm",
    "let me know if you want",
    "let me know how you",
    "let me know which",
];

const CHOICE_PROMPT_PATTERNS: &[&str] = &[
    "qual opcao voce prefere",
    "escolha uma opcao",
    "por favor escolha",
    "please choose",
    "which option do you prefer",
    "which approach do you prefer",
];

const TRIVIAL_CONTINUE_QUESTIONS: &[&str] = &[
    "shall i continue?",
    "should i continue?",
    "shall i proceed?",
    "should i proceed?",
    "want me to continue?",
    "would you like me to continue?",
    "do you want me to continue?",
    "do you want me to proceed?",
    "posso continuar?",
    "devo continuar?",
    "quer que eu continue?",
    "quer que eu prossiga?",
    "posso prosseguir?",
    "devo prosseguir?",
];

const DECISION_QUESTION_MARKERS: &[&str] = &[
    " ou ",
    " or ",
    "prefere",
    "prefer",
    "qual ",
    "quais ",
    "which ",
    "what should ",
    "remover",
    "deletar",
    "apagar",
    "excluir",
    "delete",
    "remove",
    "manter",
    "compatibilidade",
    "opcao",
    "option",
    "aprova",
    "approve",
    "confirma",
    "confirm",
];

const NEXT_STEP_PATTERNS: &[&str] = &[
    // PT-BR
    "agora vou ",
    "vou agora ",
    "deixa eu ",
    "deixe-me ",
    "deixe me ",
    "vou rodar",
    "vou executar",
    "vou verificar",
    "vou checar",
    "vou conferir",
    "vou corrigir",
    "vou ajustar",
    "vou implementar",
    "vou atualizar",
    "vou testar",
    "vou investigar",
    "vou analisar",
    "vou ler ",
    "vou abrir ",
    "vou editar",
    "vou criar ",
    "vou aplicar",
    "vou continuar",
    "vou prosseguir",
    "vou fazer ",
    "vou procurar",
    "vou buscar",
    "vou validar",
    "vou compilar",
    "em seguida vou ",
    "na sequencia vou ",
    "proximo passo:",
    "o proximo passo e",
    // EN
    "let me now ",
    "let me run",
    "let me check",
    "let me verify",
    "let me test",
    "let me fix",
    "let me inspect",
    "let me look",
    "let me read",
    "let me update",
    "let me edit",
    "let me implement",
    "let me investigate",
    "let me apply",
    "let me continue",
    "let me proceed",
    "now i'll ",
    "now i will ",
    "i'll now ",
    "i will now ",
    "i'm going to ",
    "i am going to ",
    "i'll run",
    "i'll check",
    "i'll verify",
    "i'll fix",
    "i'll test",
    "i'll implement",
    "i'll update",
    "i'll edit",
    "i'll inspect",
    "i'll investigate",
    "i'll continue",
    "i'll proceed",
    "next, i'll ",
    "next i'll ",
    "next, i will ",
    "next i will ",
    "next step:",
];

fn contains_approval_wait(normalized: &str) -> bool {
    APPROVAL_WAIT_PATTERNS
        .iter()
        .any(|pattern| normalized.contains(pattern))
}

fn is_choice_prompt(normalized: &str) -> bool {
    CHOICE_PROMPT_PATTERNS
        .iter()
        .any(|pattern| normalized.contains(pattern))
}

fn is_decision_question(normalized: &str) -> bool {
    DECISION_QUESTION_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker))
}

fn is_trivial_continue_question(normalized: &str) -> bool {
    TRIVIAL_CONTINUE_QUESTIONS
        .iter()
        .any(|question| normalized.ends_with(question) || normalized == *question)
}

fn normalize_for_match(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for ch in text.chars() {
        for lower in ch.to_lowercase() {
            let mapped = match lower {
                'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
                'é' | 'è' | 'ê' | 'ë' => 'e',
                'í' | 'ì' | 'î' | 'ï' => 'i',
                'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
                'ú' | 'ù' | 'û' | 'ü' => 'u',
                'ç' => 'c',
                'ñ' => 'n',
                '’' | '‘' => '\'',
                other => other,
            };
            folded.push(mapped);
        }
    }
    folded
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_plan_prompt(open_steps: &[&TurnPlanStep]) -> String {
    let mut lines = Vec::with_capacity(open_steps.len() + 3);
    lines.push(
        "Continuação automática: ainda restam etapas abertas no plano do turno. Conclua apenas o que falta sem repetir etapas já concluídas:"
            .to_string(),
    );
    for step in open_steps {
        let status = match step.status {
            TurnPlanStepStatus::InProgress => "em andamento",
            TurnPlanStepStatus::Pending => "pendente",
            TurnPlanStepStatus::Completed => "concluída",
        };
        lines.push(format!("- [{status}] {}", step.text));
    }
    lines.push(
        "Execute diretamente as etapas pendentes, atualize o status do plano e só encerre quando terminar ou precisar de uma decisão explícita do usuário."
            .to_string(),
    );
    lines.join("\n")
}

fn build_next_step_prompt(previous_reply: &str) -> String {
    let excerpt = preserve_head_and_tail(previous_reply.trim(), 240);
    format!(
        "Continuação automática: você anunciou a próxima ação (\"{excerpt}\") e encerrou o turno sem executá-la. Execute diretamente a ação anunciada agora usando as ferramentas disponíveis e só responda ao concluir."
    )
}

fn build_finish_check_prompt(pending_items: &[String]) -> String {
    let mut lines = Vec::with_capacity(pending_items.len() + 3);
    lines.push(
        "Continuação automática: ainda há critérios ou verificações pendentes antes de concluir:"
            .to_string(),
    );
    for item in pending_items.iter().take(12) {
        let trimmed = item.trim();
        if !trimmed.is_empty() {
            lines.push(format!("- {trimmed}"));
        }
    }
    lines.push(
        "Execute a verificação necessária (ex.: testes, build ou checagem dos critérios acima) e reporte a evidência concreta antes de finalizar."
            .to_string(),
    );
    lines.join("\n")
}

fn build_recovery_prompt(
    trigger: ContinuationTrigger,
    work_steps_blocks: &[String],
    open_steps: &[&TurnPlanStep],
) -> String {
    let mut sections: Vec<String> = Vec::new();
    let header = match trigger {
        ContinuationTrigger::Compaction => {
            "Retomada automática após falha na compactação de contexto: continue a tarefa de onde parou sem refazer passos já concluídos."
        }
        _ => {
            "Retomada automática após saída inesperada da CLI do provedor: continue a tarefa de onde parou sem refazer passos já concluídos."
        }
    };
    sections.push(header.to_string());

    if !open_steps.is_empty() {
        let mut plan_lines = vec!["Etapas ainda abertas no plano:".to_string()];
        for step in open_steps {
            let status = match step.status {
                TurnPlanStepStatus::InProgress => "em andamento",
                TurnPlanStepStatus::Pending => "pendente",
                TurnPlanStepStatus::Completed => "concluída",
            };
            plan_lines.push(format!("- [{status}] {}", step.text));
        }
        sections.push(plan_lines.join("\n"));
    }

    let cleaned_blocks: Vec<&str> = work_steps_blocks
        .iter()
        .map(|block| block.trim())
        .filter(|block| !block.is_empty())
        .collect();
    if !cleaned_blocks.is_empty() {
        let joined = cleaned_blocks.join("\n");
        let bounded = preserve_head_and_tail(&joined, HANDOFF_WORK_STEPS_BUDGET);
        sections.push(format!(
            "Passos já executados antes da interrupção (não repita o que já foi concluído):\n{bounded}"
        ));
    }

    sections.join("\n\n")
}

/// Classifica um erro de turno de forma agnóstica a provedor para decidir
/// se é elegível a retomada única por falha de compactação ou queda abrupta da CLI.
pub fn classify_turn_failure(error: &FlowError) -> TurnFailureKind {
    if error.is_limit() || error.is_model_unavailable() || error.is_provider_disabled() {
        return TurnFailureKind::Other;
    }
    match error {
        FlowError::Claude(claude_err) => classify_claude_error(claude_err),
        _ => TurnFailureKind::Other,
    }
}

fn classify_claude_error(error: &ClaudeError) -> TurnFailureKind {
    match error {
        ClaudeError::ForCli { inner, .. } => classify_claude_error(inner),
        ClaudeError::UsageLimit(_)
        | ClaudeError::LimitExhausted
        | ClaudeError::ModelUnavailable { .. }
        | ClaudeError::ProviderAuthOrBilling { .. } => TurnFailureKind::Other,
        ClaudeError::Reported(msg) => {
            if is_compaction_error_text(msg) {
                TurnFailureKind::Compaction
            } else {
                TurnFailureKind::Other
            }
        }
        ClaudeError::Exit { message, .. } | ClaudeError::SessionClosed(message) => {
            if is_compaction_error_text(message) {
                TurnFailureKind::Compaction
            } else {
                TurnFailureKind::ProviderExit
            }
        }
        ClaudeError::MissingResult => TurnFailureKind::ProviderExit,
        ClaudeError::Spawn(_)
        | ClaudeError::InvalidJson(_)
        | ClaudeError::Timeout(_)
        | ClaudeError::Mcp(_) => TurnFailureKind::Other,
    }
}

fn is_compaction_error_text(text: &str) -> bool {
    let lower = normalize_for_match(text);
    lower.contains("compaction")
        || lower.contains("compact_context")
        || lower.contains("context_compaction")
        || lower.contains("failed to compact")
        || lower.contains("auto-compact")
        || lower.contains("compactacao")
}

/// Verifica se a atividade do turno executou alguma ferramenta de ação relevante.
pub fn executed_action_tool(activity: Option<&TurnActivity>) -> bool {
    activity.is_some_and(|act| {
        act.tools.iter().any(|tool| {
            matches!(
                tool.category,
                ToolCategory::Edit
                    | ToolCategory::Command
                    | ToolCategory::Mcp
                    | ToolCategory::App
                    | ToolCategory::Web
            )
        })
    })
}

/// Verifica se a atividade do turno contém evidência concreta de verificação
/// (comando de teste, build, lint ou typecheck concluído com sucesso).
pub fn has_verification_evidence(activity: Option<&TurnActivity>) -> bool {
    let Some(activity) = activity else {
        return false;
    };
    activity.tools.iter().any(|tool| {
        if tool.category != ToolCategory::Command || tool.status != ToolStatus::Ok {
            return false;
        }
        let target = tool.target.to_ascii_lowercase();
        let exit_ok = tool.exit_code.is_none_or(|code| code == 0);
        if !exit_ok {
            return false;
        }
        VERIFICATION_CMD_HINTS
            .iter()
            .any(|hint| target.contains(hint))
    })
}

const VERIFICATION_CMD_HINTS: &[&str] = &[
    "test",
    "check",
    "typecheck",
    "lint",
    "build",
    "clippy",
    "pytest",
    "vitest",
    "jest",
    "playwright",
    "tsc",
    "verify",
];

/// Carrega a configuração de continuação automática do SQLite, aplicando padrões seguros.
pub fn load_continuation_config(app: &AppHandle) -> ContinuationConfig {
    let db_state = app.state::<DbState>();
    let Some(db) = db_state.db() else {
        return ContinuationConfig::default();
    };
    let Ok(settings) = db.load_settings() else {
        return ContinuationConfig::default();
    };
    let enabled = settings
        .get(SETTING_CONTINUATION_ENABLED)
        .map_or(DEFAULT_CONTINUATION_ENABLED, |v| v == "true");
    let max_count = settings
        .get(SETTING_CONTINUATION_MAX_COUNT)
        .and_then(|v| v.parse::<u32>().ok())
        .map_or(DEFAULT_CONTINUATION_MAX_COUNT, |v| {
            v.min(MAX_CONTINUATION_MAX_COUNT)
        });
    ContinuationConfig { enabled, max_count }
}

/// Token cooperativo de cancelamento imediato para cadeias de continuação ativas.
#[derive(Clone, Debug)]
pub struct ContinuationCancelToken {
    cancelled: Arc<AtomicBool>,
}

impl ContinuationCancelToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cancelled, &other.cancelled)
    }
}

impl Default for ContinuationCancelToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Registro global de cadeias de continuação ativas por `SessionKey` em `AppState`,
/// permitindo cancelamento instantâneo quando o usuário envia nova mensagem, clica em
/// Parar, pausa um agente/time ou desativa a continuação automática.
#[derive(Default)]
pub struct ContinuationRegistry {
    active: Mutex<HashMap<SessionKey, ContinuationCancelToken>>,
}

impl ContinuationRegistry {
    pub fn register(&self, key: &SessionKey) -> ContinuationCancelToken {
        let token = ContinuationCancelToken::new();
        let mut active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(previous) = active.insert(key.clone(), token.clone()) {
            previous.cancel();
        }
        token
    }

    pub fn unregister(&self, key: &SessionKey, token: &ContinuationCancelToken) {
        let mut active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        if active.get(key).is_some_and(|current| current.ptr_eq(token)) {
            active.remove(key);
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn cancel_session(&self, key: &SessionKey) {
        let mut active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(token) = active.remove(key) {
            token.cancel();
        }
    }

    pub fn cancel_agent(&self, project_id: i64, agent: &Agent) {
        let mut active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        let keys: Vec<SessionKey> = active
            .keys()
            .filter(|key| {
                key.project_id == project_id && is_agent_or_replica(key.agent.as_str(), agent)
            })
            .cloned()
            .collect();
        for key in keys {
            if let Some(token) = active.remove(&key) {
                token.cancel();
            }
        }
    }

    pub fn cancel_project(&self, project_id: i64) {
        let mut active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        let keys: Vec<SessionKey> = active
            .keys()
            .filter(|key| key.project_id == project_id)
            .cloned()
            .collect();
        for key in keys {
            if let Some(token) = active.remove(&key) {
                token.cancel();
            }
        }
    }

    pub fn cancel_all(&self) {
        let mut active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        for (_, token) in active.drain() {
            token.cancel();
        }
    }
}

pub(super) struct ContinuationGuard<'a> {
    registry: &'a ContinuationRegistry,
    key: SessionKey,
    token: ContinuationCancelToken,
}

impl<'a> ContinuationGuard<'a> {
    pub(super) fn register(registry: &'a ContinuationRegistry, key: &SessionKey) -> Self {
        let token = registry.register(key);
        Self {
            registry,
            key: key.clone(),
            token,
        }
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
}

impl Drop for ContinuationGuard<'_> {
    fn drop(&mut self) {
        self.registry.unregister(&self.key, &self.token);
    }
}

#[cfg(test)]
#[path = "../../tests/continuation_tests.rs"]
mod tests;
