use tauri::{AppHandle, Manager};

use crate::agents::{Agent, Team};
use crate::db::{AgentActor, CardOperation, CardOutcome, DbState};
use crate::state::AppState;
use crate::task::Applied;

use super::error::FlowError;
use super::markers::{parse_decision, parse_reply, ParsedDecision, ParsedReply};
use super::turns::with_cache;

const MAX_CARD_OPERATIONS: usize = 10;

pub(super) struct TextContext<'a> {
    pub(super) app: &'a AppHandle,
    pub(super) state: &'a AppState,
    pub(super) project_id: i64,
    pub(super) speaker: &'a Agent,
    pub(super) item_id: Option<&'a str>,
    pub(super) team: &'a Team,
}

// O que os marcadores de texto da resposta fizeram; vazio quando a sessão tem ferramentas.
#[derive(Default)]
pub(super) struct TextReply {
    pub(super) visible: String,
    pub(super) decision: Option<ParsedDecision>,
    pub(super) delegation: Option<(Agent, String)>,
    pub(super) applied: Applied,
    pub(super) cards: AppliedCards,
    pub(super) rejected: Vec<String>,
}

// Com ferramentas a linha de marcador é só texto: nada é lido nem executado.
pub(super) fn read_reply(
    with_tools: bool,
    context: &TextContext<'_>,
    reply: &str,
) -> Result<TextReply, FlowError> {
    if with_tools {
        return Ok(TextReply {
            visible: reply.trim().to_string(),
            ..TextReply::default()
        });
    }
    if let Some(mut decision) = parse_decision(reply) {
        decision.visible = parse_reply(&decision.visible, context.speaker, context.team).visible;
        return Ok(TextReply {
            decision: Some(decision),
            ..TextReply::default()
        });
    }
    let ParsedReply {
        visible,
        delegation,
        updates,
        cards,
    } = parse_reply(reply, context.speaker, context.team);
    let applied = with_cache(context.state, context.project_id, |cache| {
        Ok(cache.apply_reply(context.speaker, updates, context.team))
    })?;
    let mut rejected = applied.rejected.clone();
    let cards = apply_cards(
        context.app,
        context.project_id,
        context.speaker,
        context.item_id,
        context.team,
        cards,
        &mut rejected,
    )?;
    Ok(TextReply {
        visible,
        decision: None,
        delegation,
        applied,
        cards,
        rejected,
    })
}

#[derive(Default)]
pub(super) struct AppliedCards {
    pub(super) confirmations: Vec<String>,
    pub(super) changed: bool,
}

// Tudo ou nada: linha malformada, recusa de variável ou excesso de linhas impedem qualquer gravação
// (a repetição do turno não pode duplicar card); os motivos vão juntos ao agente.
fn apply_cards(
    app: &AppHandle,
    project_id: i64,
    speaker: &Agent,
    item_id: Option<&str>,
    team: &Team,
    cards: Vec<Result<CardOperation, String>>,
    rejected: &mut Vec<String>,
) -> Result<AppliedCards, FlowError> {
    let mut operations = Vec::new();
    for card in cards {
        match card {
            Ok(operation) => operations.push(operation),
            Err(reason) => rejected.push(reason),
        }
    }
    if operations.len() > MAX_CARD_OPERATIONS {
        rejected.push(format!(
            "a resposta tem {} linhas #card; o máximo é {MAX_CARD_OPERATIONS} por resposta",
            operations.len()
        ));
    }
    if !rejected.is_empty() || operations.is_empty() {
        return Ok(AppliedCards::default());
    }
    let db_state = app.state::<DbState>();
    let Some(db) = db_state.db() else {
        return Ok(AppliedCards::default());
    };
    let actor = AgentActor {
        id: speaker.clone(),
        name: team.name_of(speaker).to_string(),
        current_card: item_id.map(str::to_string),
    };
    match db.apply_card_operations(project_id, &actor, &operations) {
        Ok(results) => {
            let confirmations: Vec<String> = results
                .iter()
                .filter_map(|result| match result.outcome {
                    CardOutcome::Created => {
                        Some(format!("Card {} criado: {}.", result.id, result.title))
                    }
                    CardOutcome::Moved => {
                        Some(format!("Card {} movido para {}.", result.id, result.column))
                    }
                    CardOutcome::Updated => {
                        Some(format!("Card {} atualizado: {}.", result.id, result.title))
                    }
                    CardOutcome::Unchanged => None,
                })
                .collect();
            Ok(AppliedCards {
                changed: !confirmations.is_empty(),
                confirmations,
            })
        }
        Err(error) if error.is_card_refusal() => {
            rejected.push(error.to_string());
            Ok(AppliedCards::default())
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
#[path = "../../tests/text_fallback_tests.rs"]
mod tests;
