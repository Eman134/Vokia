use tauri::AppHandle;

use crate::events::{GoalState, VokiaEvent};
use crate::task::{truncate, TaskDefinition};

use super::emit::{emit, report_limits};
use super::error::FlowError;
use super::loop_guard::PassCounter;
use super::prompts::{variables_listing, GOAL_VERIFIER_ROLE};
use super::turns::{with_cache, Exchange};

const GOAL_CHECK_LIMIT: u32 = 4;
const GOAL_HISTORY_LINES: usize = 12;
const GOAL_LINE_MAX_CHARS: usize = 600;
const GOAL_REASON_MAX_CHARS: usize = 200;
const VERDICT_ACHIEVED: &str = "ATINGIDO";
const VERDICT_INCOMPLETE: &str = "INCOMPLETO";
const VERDICT_NO_PROGRESS: &str = "SEM_AVANCO";
const REASON_PREFIX: &str = "Motivo:";
const NEXT_PREFIX: &str = "Proximo:";
const VERDICT_DECORATION: &[char] = &['`', '"', '\'', '*'];
const UNAVAILABLE_PREFIX: &str = "Verificador indisponível: ";
const CHECK_LIMIT_REASON: &str = "limite de verificações atingido";

#[derive(Default)]
pub(super) struct GoalTracker {
    step: usize,
    idle_turns: u32,
    last_advance_step: usize,
    checks: u32,
    advanced_since_check: bool,
}

impl GoalTracker {
    pub(super) fn register_turn(&mut self, advanced: bool) {
        self.step += 1;
        if advanced {
            self.idle_turns = 0;
            self.last_advance_step = self.step;
            self.advanced_since_check = true;
        } else {
            self.idle_turns += 1;
        }
    }

    pub(super) fn idle_turns(&self) -> u32 {
        self.idle_turns
    }

    pub(super) fn last_advance_step(&self) -> usize {
        self.last_advance_step
    }

    pub(super) fn checks(&self) -> u32 {
        self.checks
    }

    pub(super) fn reset_idle(&mut self) {
        self.idle_turns = 0;
    }

    fn take_check(&mut self) -> Option<bool> {
        if self.checks >= GOAL_CHECK_LIMIT {
            return None;
        }
        self.checks += 1;
        Some(std::mem::take(&mut self.advanced_since_check))
    }
}

pub(super) enum Trigger {
    EndOfChain,
    Stall { state: GoalState, message: String },
}

pub(super) enum Settlement {
    Achieved { reason: String },
    Continue { instruction: String },
    Stalled { message: String },
    Ended,
}

enum Verdict {
    Achieved,
    Incomplete { next: String },
    NoProgress,
}

struct Judgement {
    verdict: Verdict,
    reason: String,
}

pub(super) async fn settle(
    exchange: &Exchange<'_>,
    transcript: &[String],
    goal: &mut GoalTracker,
    passes: &PassCounter,
    trigger: Trigger,
) -> Result<Settlement, FlowError> {
    let Exchange {
        app,
        key,
        definition,
        team,
        ..
    } = exchange;
    let project_id = key.project_id;
    let objective = definition.objetivo.as_str();
    let judgement = if matches!(trigger, Trigger::Stall { .. }) || definition.verify {
        match goal.take_check() {
            Some(advanced) => {
                check_goal(exchange, transcript, passes, advanced, goal.checks()).await?
            }
            None => Some(Judgement {
                verdict: Verdict::NoProgress,
                reason: CHECK_LIMIT_REASON.to_string(),
            }),
        }
    } else {
        None
    };
    let checks = goal.checks();
    Ok(match (judgement, trigger) {
        (
            Some(Judgement {
                verdict: Verdict::Achieved,
                reason,
            }),
            _,
        ) => {
            emit_goal(
                app,
                project_id,
                objective,
                Some(GoalState::Atingido),
                &reason,
                checks,
            )?;
            Settlement::Achieved { reason }
        }
        (
            Some(Judgement {
                verdict: Verdict::Incomplete { next },
                reason,
            }),
            _,
        ) => {
            emit_goal(
                app,
                project_id,
                objective,
                Some(GoalState::Incompleto),
                &reason,
                checks,
            )?;
            goal.reset_idle();
            Settlement::Continue {
                instruction: continuation_instruction(&reason, &next),
            }
        }
        (
            Some(Judgement {
                verdict: Verdict::NoProgress,
                reason,
            }),
            Trigger::EndOfChain,
        ) => {
            emit_goal(
                app,
                project_id,
                objective,
                Some(GoalState::SemAvanco),
                &reason,
                checks,
            )?;
            Settlement::Stalled {
                message: sem_avanco_message(objective, &reason, checks, &passes.history(team)),
            }
        }
        (None, Trigger::EndOfChain) => Settlement::Ended,
        (judgement, Trigger::Stall { state, message }) => {
            let reason = judgement.map_or_else(String::new, |judgement| judgement.reason);
            emit_goal(app, project_id, objective, Some(state), &reason, checks)?;
            Settlement::Stalled {
                message: with_verifier_reason(message, &reason),
            }
        }
    })
}

pub(super) fn achieved_message(objective: &str, reason: &str) -> String {
    format!("Objetivo atingido: {objective}. {reason}")
        .trim_end()
        .to_string()
}

pub(super) fn emit_goal(
    app: &AppHandle,
    project_id: i64,
    objective: &str,
    state: Option<GoalState>,
    reason: &str,
    checks: u32,
) -> Result<(), FlowError> {
    emit(
        app,
        VokiaEvent::GoalState {
            project_id,
            state,
            objective: objective.to_string(),
            reason: reason.to_string(),
            checks,
        },
    )
}

async fn check_goal(
    exchange: &Exchange<'_>,
    transcript: &[String],
    passes: &PassCounter,
    advanced: bool,
    checks: u32,
) -> Result<Option<Judgement>, FlowError> {
    let Exchange {
        app,
        state,
        cli,
        key,
        definition,
        team,
        ..
    } = exchange;
    let listing = with_cache(state, key.project_id, |cache| {
        Ok(variables_listing(cache, &key.agent))
    })?;
    let message = verifier_message(
        definition,
        transcript,
        &listing,
        advanced,
        &passes.history(team),
    );
    let profile = super::flow::profile_of(team, &key.agent)?;
    let reply = match super::fallback::ask_with_fallback(
        app,
        state,
        cli,
        &key.agent,
        profile,
        &message,
        GOAL_VERIFIER_ROLE,
    )
    .await
    {
        Ok(reply) => reply,
        Err(error) => {
            notify_unavailable(
                app,
                key.project_id,
                &definition.objetivo,
                checks,
                &error.to_string(),
            )?;
            return Ok(None);
        }
    };
    report_limits(app, reply.limits)?;
    match parse_verdict(&reply.text) {
        Some(judgement) => Ok(Some(judgement)),
        None => {
            notify_unavailable(
                app,
                key.project_id,
                &definition.objetivo,
                checks,
                "resposta fora do formato",
            )?;
            Ok(None)
        }
    }
}

fn notify_unavailable(
    app: &AppHandle,
    project_id: i64,
    objective: &str,
    checks: u32,
    detail: &str,
) -> Result<(), FlowError> {
    emit_goal(
        app,
        project_id,
        objective,
        None,
        &format!("{UNAVAILABLE_PREFIX}{detail}"),
        checks,
    )
}

fn verifier_message(
    definition: &TaskDefinition,
    transcript: &[String],
    listing: &str,
    advanced: bool,
    history: &str,
) -> String {
    let skip = transcript.len().saturating_sub(GOAL_HISTORY_LINES);
    let lines: Vec<String> = transcript[skip..]
        .iter()
        .map(|line| truncate(line, GOAL_LINE_MAX_CHARS))
        .collect();
    let advanced = if advanced { "sim" } else { "não" };
    format!(
        "Objetivo: {}\nCritério: {}\nVariáveis:\n{listing}\nHouve avanço de estado desde a última verificação: {advanced}\nRepasses desde o último avanço: {history}\nConversa:\n{}",
        definition.objetivo,
        definition.criterion,
        lines.join("\n"),
    )
}

fn parse_verdict(reply: &str) -> Option<Judgement> {
    let mut lines = reply.lines().map(str::trim).filter(|line| !line.is_empty());
    let head = lines.next()?.trim_matches(VERDICT_DECORATION).trim();
    let rest: Vec<&str> = lines.collect();
    let verdict = if head.eq_ignore_ascii_case(VERDICT_ACHIEVED) {
        Verdict::Achieved
    } else if head.eq_ignore_ascii_case(VERDICT_INCOMPLETE) {
        Verdict::Incomplete {
            next: field(&rest, NEXT_PREFIX),
        }
    } else if head.eq_ignore_ascii_case(VERDICT_NO_PROGRESS) {
        Verdict::NoProgress
    } else {
        return None;
    };
    Some(Judgement {
        verdict,
        reason: field(&rest, REASON_PREFIX),
    })
}

fn field(lines: &[&str], prefix: &str) -> String {
    lines
        .iter()
        .find_map(|line| line.strip_prefix(prefix))
        .map_or_else(String::new, |value| {
            truncate(value.trim(), GOAL_REASON_MAX_CHARS)
        })
}

fn continuation_instruction(reason: &str, next: &str) -> String {
    let next_step = if next.is_empty() {
        String::new()
    } else {
        format!(" Próximo passo: {next}.")
    };
    format!(
        "O verificador disse que o objetivo ainda não foi atingido: {reason}.{next_step} Continue o pedido original."
    )
}

fn sem_avanco_message(objective: &str, reason: &str, checks: u32, history: &str) -> String {
    format!(
        "A tarefa parou: o verificador não viu avanço rumo a \"{objective}\". Motivo: {reason}. Verificações: {checks}. Repasses desde o último avanço: {history}."
    )
}

fn with_verifier_reason(message: String, reason: &str) -> String {
    if reason.is_empty() {
        message
    } else {
        format!("{message} Verificador: {reason}.")
    }
}
