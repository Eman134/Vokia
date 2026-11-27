use tauri::{AppHandle, Manager};

use crate::agents::{Agent, Team};
use crate::db::{BacklogItem, DbError, DbState, HandoffDraft};
use crate::events::{AgentState, DecisionKind, Speaker};
use crate::mcp::SlotDecision;
use crate::orchestration::snapshot::{leader_of, placement_of};
use crate::state::{AppState, RequestContext};

use super::emit::{emit_backlog_changed, emit_feed_note, emit_state, now_millis};
use super::error::FlowError;
use super::flow::{handle_request, handle_request_from, load_orchestration, report_failure};
use super::turns::open_decision;

// Tentativas da validação automática antes de passar a decisão ao usuário.
pub(crate) const AUTO_VALIDATION_MAX_ATTEMPTS: i64 = 2;

const VALIDATION_TITLE: &str = "Validação final";
pub(super) const VALIDATION_RETURN: &str = "Devolver";
const VALIDATION_CONCLUDE: &str = "Concluir";
const FINAL_VALIDATION_AUTOMATIC: &str = "automatica";
const CLOSED_BY_USER: &str = "usuario";
const CLOSED_BY_AUTOMATIC: &str = "automatico";
const RETURN_ACTION: &str = "devolver";
const RETURN_KIND: &str = "implementar";
const NO_CRITERIA: &str = "sem critérios de aceite";

pub(crate) enum FinalCheck {
    Passed { met: usize, total: usize },
    Failed(Vec<String>),
}

// Validação automática: critérios todos marcados (ao menos um), subtarefas concluídas, sem decisão aberta no card
// e execução terminada. Não roda comando de teste do projeto.
pub(crate) fn evaluate(item: &BacklogItem, open_decision: bool, finished: bool) -> FinalCheck {
    let mut reasons = Vec::new();
    let total = item.criteria.len();
    if total == 0 {
        reasons.push(NO_CRITERIA.to_string());
    }
    for criterion in item.criteria.iter().filter(|criterion| !criterion.done) {
        reasons.push(format!("critério '{}' não marcado", criterion.text));
    }
    if let Some(children) = &item.children {
        let open = children.total.saturating_sub(children.done);
        if open > 0 {
            reasons.push(format!("{open} de {} subtarefas abertas", children.total));
        }
    }
    if open_decision {
        reasons.push("há decisão aberta no card".to_string());
    }
    if !finished {
        reasons.push("a execução não terminou".to_string());
    }
    if reasons.is_empty() {
        FinalCheck::Passed { met: total, total }
    } else {
        FinalCheck::Failed(reasons)
    }
}

// O último ramo do card chegou ao fim: o card vai para In review e a validação final do time do líder decide. Com um ramo
// que parou por erro (`finished` falso) a validação não abre: o card fica onde está, com a nota na atividade.
pub(super) async fn on_flow_end(
    app: &AppHandle,
    state: &AppState,
    team: &Team,
    project_id: i64,
    item_id: &str,
    speaker: &Agent,
    finished: bool,
) -> Result<(), FlowError> {
    let db_state = app.state::<DbState>();
    let Some(db) = db_state.db() else {
        return Ok(());
    };
    let item = match db.read_card(project_id, item_id) {
        Ok(item) => item,
        Err(DbError::BacklogItemNotFound(_)) => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if !finished {
        db.add_card_activity(
            project_id,
            item_id,
            &format!("Validação final de {item_id} não aberta: um ramo parou antes do fim."),
        )?;
        return emit_backlog_changed(app, project_id);
    }
    db.advance_card(project_id, item_id, "In review")?;
    emit_backlog_changed(app, project_id)?;
    let snapshot = load_orchestration(app)?;
    let leader = Agent::from_stored(&item.who)
        .filter(|agent| team.profile(agent).is_some())
        .unwrap_or_else(|| leader_of(snapshot.as_ref()));
    let automatic = snapshot.as_ref().is_some_and(|snapshot| {
        placement_of(snapshot, &leader)
            .and_then(|placement| {
                snapshot
                    .teams
                    .iter()
                    .find(|orch_team| orch_team.id == placement.team_id)
            })
            .is_some_and(|orch_team| orch_team.final_validation == FINAL_VALIDATION_AUTOMATIC)
    });
    if !automatic {
        if item.parent.is_some() {
            return Ok(());
        }
        return open_manual_validation(app, state, &leader, project_id, &item, "");
    }
    let open_decision = has_open_decision(state, project_id, item_id)?;
    match evaluate(&item, open_decision, finished) {
        FinalCheck::Passed { met, total } => {
            db.close_card(project_id, item_id, CLOSED_BY_AUTOMATIC)?;
            let text = format!("Concluí {item_id} automaticamente: critérios {met}/{total}.");
            db.add_card_activity(project_id, item_id, &text)?;
            super::flow::schedule_done_cleanup(app, project_id, item_id);
            emit_feed_note(app, project_id, Speaker::from(leader), &text, now_millis()?)?;
            emit_backlog_changed(app, project_id)
        }
        FinalCheck::Failed(reasons) if reasons == [NO_CRITERIA] => open_manual_validation(
            app,
            state,
            &leader,
            project_id,
            &item,
            "Validação automática: sem critérios de aceite.",
        ),
        FinalCheck::Failed(reasons) => {
            let motive = reasons.join("; ");
            if item.auto_attempts + 1 >= AUTO_VALIDATION_MAX_ATTEMPTS {
                let note = format!("Validação automática: {motive}.");
                return open_manual_validation(app, state, &leader, project_id, &item, &note);
            }
            db.bump_auto_attempts(project_id, item_id)?;
            db.add_card_activity(
                project_id,
                item_id,
                &format!("Não concluí {item_id}: {motive}."),
            )?;
            db.advance_card(project_id, item_id, "In progress")?;
            emit_backlog_changed(app, project_id)?;
            send_back(
                app,
                db_state.db(),
                &leader,
                speaker,
                project_id,
                &item,
                &motive,
            )
        }
    }
}

fn open_manual_validation(
    app: &AppHandle,
    state: &AppState,
    leader: &Agent,
    project_id: i64,
    item: &BacklogItem,
    note: &str,
) -> Result<(), FlowError> {
    let detail = format!(
        "Etapas do fluxo concluídas para {} · {}. {note}",
        item.id, item.title
    );
    open_decision(
        app,
        state,
        leader,
        project_id,
        Some(&item.id),
        &item.title,
        SlotDecision {
            kind: DecisionKind::Validacao,
            title: VALIDATION_TITLE.to_string(),
            detail: detail.trim().to_string(),
            options: vec![
                VALIDATION_CONCLUDE.to_string(),
                VALIDATION_RETURN.to_string(),
            ],
        },
    )?;
    emit_state(
        app,
        project_id,
        leader.clone(),
        AgentState::Waiting,
        Some(&item.id),
    )
}

fn has_open_decision(state: &AppState, project_id: i64, item_id: &str) -> Result<bool, FlowError> {
    let decisions = state
        .decisions
        .lock()
        .map_err(|_| FlowError::DecisionsPoisoned)?;
    Ok(decisions.values().any(|decision| {
        decision.project_id == project_id && decision.item_id.as_deref() == Some(item_id)
    }))
}

// O líder devolve a task a quem chegou ao fim, com o motivo; o pedido roda em segundo plano para não prender a troca atual.
fn send_back(
    app: &AppHandle,
    db: Option<&crate::db::Db>,
    leader: &Agent,
    speaker: &Agent,
    project_id: i64,
    item: &BacklogItem,
    motive: &str,
) -> Result<(), FlowError> {
    if let Some(db) = db {
        db.record_handoff(
            project_id,
            &HandoffDraft {
                run_id: None,
                from_agent: leader.to_string(),
                to_agent: speaker.to_string(),
                from_layer: None,
                to_layer: None,
                action: RETURN_ACTION.to_string(),
                kind: RETURN_KIND.to_string(),
                item_id: Some(item.id.clone()),
                return_count: 0,
                message_id: None,
                event: None,
                edge_id: None,
            },
        )?;
    }
    let text = format!("Devolvo {} · {}: {motive}.", item.id, item.title);
    let context = RequestContext::new(None, Some(item.id.clone()), None);
    let app = app.clone();
    let (leader, speaker) = (leader.clone(), speaker.clone());
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let sent = handle_request_from(
            &app,
            &state,
            &text,
            Speaker::from(leader),
            speaker,
            project_id,
            context,
        )
        .await;
        if let Err(error) = sent {
            let _ = report_failure(&app, project_id, &error);
        }
    });
    Ok(())
}

// "Concluir" na validação final: o usuário fecha o card; nenhum agente recebe mensagem.
pub(super) fn conclude_card(
    app: &AppHandle,
    project_id: i64,
    item_id: &str,
) -> Result<(), FlowError> {
    let db_state = app.state::<DbState>();
    let db = db_state
        .require()
        .map_err(|error| FlowError::DatabaseUnavailable(error.to_string()))?;
    db.close_card(project_id, item_id, CLOSED_BY_USER)?;
    db.add_card_activity(project_id, item_id, &format!("Usuário concluiu {item_id}."))?;
    super::flow::schedule_done_cleanup(app, project_id, item_id);
    emit_backlog_changed(app, project_id)
}

// Devolver (da decisão ou do botão Reabrir): o motivo é obrigatório e o líder recebe a task de volta.
pub(crate) async fn return_task(
    app: &AppHandle,
    state: &AppState,
    project_id: i64,
    item_id: &str,
    motive: &str,
) -> Result<(), FlowError> {
    let motive = motive.trim();
    if motive.is_empty() {
        return Err(DbError::ReturnMotiveRequired.into());
    }
    let db_state = app.state::<DbState>();
    let db = db_state
        .require()
        .map_err(|error| FlowError::DatabaseUnavailable(error.to_string()))?;
    let item = db.read_card(project_id, item_id)?;
    db.reopen_card(project_id, item_id)?;
    db.add_card_activity(
        project_id,
        item_id,
        &format!("Usuário devolveu {item_id}: {motive}."),
    )?;
    emit_backlog_changed(app, project_id)?;
    let team = app
        .state::<crate::agents::AgentRegistry>()
        .team()
        .map_err(FlowError::Team)?;
    let snapshot = load_orchestration(app)?;
    let leader = Agent::from_stored(&item.who)
        .filter(|agent| team.profile(agent).is_some())
        .unwrap_or_else(|| leader_of(snapshot.as_ref()));
    let text = format!("Devolvi {item_id} · {}: {motive}", item.title);
    let context = RequestContext::new(None, Some(item_id.to_string()), None);
    handle_request(app, state, &text, leader, project_id, context)
        .await
        .map(|_| ())
}
