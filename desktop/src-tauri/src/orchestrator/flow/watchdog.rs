use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::agents::Agent;
use crate::db::{DbState, OrchestrationSnapshot};
use crate::events::VokiaEvent;
use crate::orchestration::stages::{self, StallAction};
use crate::projects::worktree;
use crate::state::{ActiveDelegation, AppState, RequestContext, MAX_WATCHDOG_RETRIES, MIN_DELEGATION_PING_SECS};

use super::super::emit::{emit, emit_artifacts_changed, emit_backlog_changed};
use super::super::error::FlowError;
use super::super::memory_review;
use super::super::project_context::load_project;
use super::batch::base_agent;
use super::{drain_waiting, handle_request, load_orchestration};

pub(crate) fn spawn_delegation_watchdog(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(MIN_DELEGATION_PING_SECS));
        loop {
            ticker.tick().await;
            sweep_silent_delegations(&app);
            sweep_stalled_stages(&app);
        }
    });
}

// Task parada: a etapa ficou sem movimento além do intervalo de ping, sem delegação pendente nem fluxo rodando no projeto.
// Reenvia ao nó da etapa enquanto houver tentativas; esgotadas, sobe ao líder do time desse nó.
// Bem acima do ping de delegação: cada reenvio reabre o contexto inteiro do agente, então só vale para etapa parada de fato.
const STALL_AFTER_SECS: i64 = 180;
const STALL_AFTER_MS: i64 = STALL_AFTER_SECS * 1000;
const MAX_STAGE_RESENDS: i64 = MAX_WATCHDOG_RETRIES as i64;
const STALL_RESEND_TEXT: &str = "A task {item} parou na sua etapa, sem movimento. Retome o trabalho desta etapa de onde ele parou.";
const STALL_ESCALATE_TEXT: &str = "A task {item} está parada na etapa de {agent}, sem movimento depois de {retries} reenvios. Decida como seguir: retome com quem tem o contexto, reenvie ao nó certo ou conclua a task.";
const STALL_NO_LEADER_TEXT: &str = "A task {item} parou na etapa de {agent} e não há líder para retomá-la.";

pub(super) fn sweep_stalled_stages(app: &AppHandle) {
    let db_state = app.state::<DbState>();
    let Some(db) = db_state.db() else {
        return;
    };
    let Ok(stalled) = db.stalled_task_stages(STALL_AFTER_MS) else {
        return;
    };
    let state = app.state::<AppState>();
    for stage in stalled {
        let Some(agent) = Agent::from_stored(&stage.agent) else {
            continue;
        };
        if stage_is_moving(&state, stage.project_id, &stage.item_id, &agent) {
            continue;
        }
        let item = stage.item_id.as_str();
        match stages::stall_action(stage.retries, MAX_STAGE_RESENDS) {
            StallAction::Resend => {
                if db.bump_task_stage_retry(stage.project_id, item).is_err() {
                    continue;
                }
                let text = STALL_RESEND_TEXT.replace("{item}", item);
                spawn_stage_request(app, stage.project_id, item, agent, &text);
                let _ = emit_stage(app, stage.project_id, item);
            }
            StallAction::Escalate => {
                if set_stage_state(app, stage.project_id, item, stages::STAGE_ESCALATED).is_err() {
                    continue;
                }
                let leader = load_orchestration(app)
                    .ok()
                    .flatten()
                    .and_then(|snapshot| stage_leader(&snapshot, &agent));
                match leader {
                    Some(leader) if leader != agent => {
                        let text = STALL_ESCALATE_TEXT
                            .replace("{item}", item)
                            .replace("{agent}", agent.as_str())
                            .replace("{retries}", &stage.retries.to_string());
                        spawn_stage_request(app, stage.project_id, item, leader, &text);
                    }
                    _ => {
                        let message = STALL_NO_LEADER_TEXT
                            .replace("{item}", item)
                            .replace("{agent}", agent.as_str());
                        record_on_card(app, stage.project_id, item, &message);
                        let _ = emit(app, VokiaEvent::Error { message });
                    }
                }
            }
        }
    }
}

// Líder do time em que o agente está posicionado.
fn stage_leader(snapshot: &OrchestrationSnapshot, agent: &Agent) -> Option<Agent> {
    let placement = snapshot
        .placements
        .iter()
        .find(|placement| placement.agent_id == agent.as_str())?;
    let team = snapshot
        .teams
        .iter()
        .find(|team| team.id == placement.team_id)?;
    Agent::from_stored(team.leader_agent.as_deref()?)
}

// Só o próprio card, o agente da etapa (ou réplica dele) e as delegações do card seguram a etapa: outra task
// rodando no mesmo projeto não pode impedir que a parada seja vista.
pub(super) fn stage_is_moving(state: &AppState, project_id: i64, item_id: &str, agent: &Agent) -> bool {
    if has_pending_delegation(state, project_id, item_id) {
        return true;
    }
    let Ok(flow) = state.flow.lock() else {
        return true;
    };
    let base = base_agent(agent);
    let item_running = flow
        .running
        .iter()
        .any(|(key, running)| key.project_id == project_id && running == item_id);
    let agent_busy = flow
        .pending
        .keys()
        .any(|key| key.project_id == project_id && base_agent(&key.agent) == base);
    item_running || agent_busy
}

fn has_pending_delegation(state: &AppState, project_id: i64, item_id: &str) -> bool {
    let Ok(flow) = state.flow.lock() else {
        return true;
    };
    let active = flow
        .active_delegations
        .keys()
        .any(|(project, _, item)| *project == project_id && item.as_deref() == Some(item_id));
    let waiting = flow.waiting.get(&project_id).is_some_and(|queue| {
        queue
            .iter()
            .any(|entry| entry.context.item_id.as_deref() == Some(item_id))
    });
    active || waiting
}

// Pedido de retomada ao agente (ou ao líder): entra pelo mesmo caminho de um pedido novo, preso ao card.
pub(in crate::orchestrator) fn spawn_stage_request(app: &AppHandle, project_id: i64, item_id: &str, recipient: Agent, text: &str) {
    let app = app.clone();
    let text = text.to_string();
    let item_id = item_id.to_string();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let context = RequestContext::new(None, Some(item_id), None);
        let _ = handle_request(&app, &state, &text, recipient, project_id, context).await;
    });
}

// Card que foi para Done: limpa o worktree e a branch dele em segundo plano. Falha ou motivo de manter viram
// registro na atividade do card; a conclusão do card não espera nem depende disso.
pub(crate) fn schedule_done_cleanup(app: &AppHandle, project_id: i64, item_id: &str) {
    memory_review::schedule_memory_review(
        app,
        project_id,
        memory_review::ReviewTrigger::CardDone,
        None,
        Some(item_id.to_string()),
        None,
    );
    let app = app.clone();
    let item = item_id.to_string();
    tauri::async_runtime::spawn(async move {
        let outcome = done_cleanup(&app, project_id, &item).await;
        let text = match outcome {
            Ok(worktree::CleanupOutcome::Absent) => return,
            Ok(worktree::CleanupOutcome::Removed) => format!(
                "Limpei o worktree e a branch de {item}: estavam integrados à main e sem alterações."
            ),
            Ok(worktree::CleanupOutcome::Kept(reason)) => {
                format!("Mantive o worktree de {item}: {reason}.")
            }
            Err(error) => format!("Não consegui limpar o worktree de {item}: {error}."),
        };
        record_on_card(&app, project_id, &item, &text);
    });
}

// Registro no card que precisa sobreviver ao app fechado; card inexistente ou sem banco não vira erro de quem chama.
pub(in crate::orchestrator) fn record_on_card(app: &AppHandle, project_id: i64, item_id: &str, text: &str) {
    let state = app.state::<DbState>();
    let Some(db) = state.db() else {
        return;
    };
    if db.add_card_activity(project_id, item_id, text).is_ok() {
        let _ = emit_backlog_changed(app, project_id);
    }
}

async fn done_cleanup(
    app: &AppHandle,
    project_id: i64,
    item_id: &str,
) -> Result<worktree::CleanupOutcome, FlowError> {
    let refinery = app.state::<AppState>().sessions.refinery_lock(project_id);
    let _refinery_guard = refinery.lock_owned().await;
    let project = load_project(app, project_id)?;
    let root = worktree::project_worktrees_dir(
        &app.path().app_data_dir().map_err(FlowError::ArchivePath)?,
        project_id,
    );
    let safe_dir_name: String = item_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let worktree_path = root.join(safe_dir_name);
    app.state::<AppState>()
        .sessions
        .release_worktree(project_id, item_id, Some(&worktree_path))
        .await;
    worktree::cleanup_done(Path::new(&project.path), &root, item_id)
        .await
        .map_err(FlowError::Worktree)
}

const DEAD_WORKTREE_SWEEP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(300);
const DEAD_WORKTREE_INITIAL_DELAY: std::time::Duration = std::time::Duration::from_secs(10);

// Varredura periódica de worktrees mortos (concluídos, arquivados ou órfãos) em todos os projetos.
pub(crate) fn spawn_dead_worktree_sweeper(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(DEAD_WORKTREE_INITIAL_DELAY).await;
        let mut interval = tokio::time::interval(DEAD_WORKTREE_SWEEP_INTERVAL);
        loop {
            interval.tick().await;
            sweep_all_projects(&app).await;
        }
    });
}

async fn sweep_all_projects(app: &AppHandle) {
    let Ok(data_dir) = app.path().app_data_dir() else {
        return;
    };
    let db_state = app.state::<DbState>();
    let Some(db) = db_state.db() else {
        return;
    };
    let Ok(projects) = db.list_projects() else {
        return;
    };
    for project in projects {
        let Ok(snapshot) = db.list_backlog(project.id) else {
            continue;
        };
        let active_ids: std::collections::HashSet<String> = snapshot
            .items
            .iter()
            .filter(|item| item.column.as_deref() != Some("Done"))
            .map(|item| item.id.clone())
            .collect();
        let root = worktree::project_worktrees_dir(&data_dir, project.id);
        let _ = worktree::sweep_dead_worktrees(Path::new(&project.path), &root, &active_ids).await;
    }
}

// Leva a etapa atual da task à tela e aos eventos; sem banco, a etapa simplesmente não sai.
pub(crate) fn emit_stage(app: &AppHandle, project_id: i64, item_id: &str) -> Result<(), FlowError> {
    let state = app.state::<DbState>();
    let Some(db) = state.db() else {
        return Ok(());
    };
    match db.task_stage(project_id, item_id) {
        Ok(Some(stage)) => emit(app, VokiaEvent::TaskStage { stage }),
        _ => Ok(()),
    }
}

// Único caminho para mudar a etapa de uma task: o banco grava etapa, card filho, pai e artefato numa transação;
// aqui a tela recebe a etapa, o quadro e os artefatos já atualizados.
pub(crate) fn set_stage_state(
    app: &AppHandle,
    project_id: i64,
    item_id: &str,
    stage_state: &str,
) -> Result<(), FlowError> {
    app.state::<DbState>()
        .require()?
        .set_task_stage_state(project_id, item_id, stage_state)?;
    publish_stage(app, project_id, item_id)
}

// Etapa gravada que mudou: a task, o quadro e os artefatos são avisados juntos.
pub(crate) fn publish_stage(app: &AppHandle, project_id: i64, item_id: &str) -> Result<(), FlowError> {
    emit_stage(app, project_id, item_id)?;
    emit_backlog_changed(app, project_id)?;
    emit_artifacts_changed(app, project_id)
}

// Delegação ativada já cumpriu o papel do registro e sai; a silenciosa recebe novas tentativas de drenar a fila
// e, esgotadas, vira erro para o usuário.
pub(super) fn sweep_silent_delegations(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (retry_projects, gave_up) = {
        let Ok(mut flow) = state.flow.lock() else {
            return;
        };
        flow.active_delegations.retain(|_, delegation| !delegation.activated);
        let mut retry_projects = HashSet::new();
        let mut gave_up = Vec::new();
        for (key, delegation) in flow.active_delegations.iter_mut() {
            if delegation.created_at.elapsed() < delegation.ping_interval {
                continue;
            }
            if delegation.retries < MAX_WATCHDOG_RETRIES {
                delegation.retries += 1;
                delegation.created_at = Instant::now();
                retry_projects.insert(delegation.project_id);
            } else {
                gave_up.push(key.clone());
            }
        }
        let gave_up: Vec<ActiveDelegation> = gave_up
            .iter()
            .filter_map(|key| flow.active_delegations.remove(key))
            .collect();
        (retry_projects, gave_up)
    };
    for project_id in retry_projects {
        drain_waiting(app, &state, project_id);
    }
    for delegation in gave_up {
        let target = delegation
            .item_id
            .as_ref()
            .map_or_else(String::new, |item| format!(" do card {item}"));
        let message = format!(
            "O agente {} não respondeu à delegação{target}.",
            delegation.to_agent.as_str()
        );
        if let Some(item) = &delegation.item_id {
            record_on_card(app, delegation.project_id, item, &message);
        }
        let _ = emit(app, VokiaEvent::Error { message });
    }
}

