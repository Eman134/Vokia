use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex, MutexGuard, PoisonError};
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::agents::{replica_of, Agent, AgentRegistry};
use crate::claude::kill_process_tree;
use crate::db::{Db, DbError, DbState};
use crate::providers::{ProviderProtocol, ProviderRegistry};
use crate::session_key::SessionKey;
use crate::state::AppState;

use super::agent_session::{
    open_preloaded_session, preload_candidate, preload_targets, AgentSession,
};
use super::fallback::{load_global_active_llm, load_global_fallback, resolve_program_for};
use super::flow::{load_orchestration, profile_of};
use super::project_context::{
    load_project, project_context, project_context_without_tree, project_dir, scan_structure,
};
use super::session_policy::{degrades_to_fresh, MAX_LIVE_SESSIONS, SESSION_IDLE_LIMIT_SECS};

pub(super) const NO_PID: u32 = 0;
const PRELOAD_DEBOUNCE: Duration = Duration::from_millis(300);
const PRELOAD_TTL: Duration = Duration::from_secs(SESSION_IDLE_LIMIT_SECS);

// `closing` e `pid` ficam fora do lock da sessão: o fechamento pode ser marcado e o processo encerrado com uma mensagem em andamento.
pub(super) struct SessionSlot {
    pub(super) closing: AtomicBool,
    pub(super) pid: Arc<AtomicU32>,
    pub(super) session: Arc<Mutex<AgentSession>>,
}

impl SessionSlot {
    fn new() -> Self {
        let pid = Arc::new(AtomicU32::new(NO_PID));
        let session = AgentSession {
            pid: Arc::clone(&pid),
            ..AgentSession::default()
        };
        Self {
            closing: AtomicBool::new(false),
            pid,
            session: Arc::new(Mutex::new(session)),
        }
    }

    fn is_live(&self) -> bool {
        self.pid.load(Ordering::SeqCst) != NO_PID
    }
}

#[derive(Default)]
pub(crate) struct SessionPool {
    slots: StdMutex<HashMap<SessionKey, Arc<SessionSlot>>>,
    tasks: StdMutex<HashMap<i64, Arc<Mutex<()>>>>,
}

impl SessionPool {
    // A tarefa do projeto é uma só (Cache guarda uma tarefa ativa): trocas de líder rodam uma de cada vez.
    pub(super) fn task_lock(&self, project_id: i64) -> Arc<Mutex<()>> {
        let mut tasks = self.tasks.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(
            tasks
                .entry(project_id)
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
    }

    fn slot(&self, key: SessionKey) -> Arc<SessionSlot> {
        Arc::clone(
            self.lock()
                .entry(key)
                .or_insert_with(|| Arc::new(SessionSlot::new())),
        )
    }

    // O mapa só recebe inserções e remoções simples; um lock envenenado não deixa estado pela metade.
    fn lock(&self) -> MutexGuard<'_, HashMap<SessionKey, Arc<SessionSlot>>> {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn matching(&self, belongs: impl Fn(&SessionKey) -> bool) -> Vec<Arc<SessionSlot>> {
        self.lock()
            .iter()
            .filter(|(key, _)| belongs(key))
            .map(|(_, slot)| Arc::clone(slot))
            .collect()
    }

    // A mensagem em andamento termina e quem a roda fecha a sessão (ExchangeSessions::release);
    // se nenhuma roda, o lock livre fecha aqui. A marca nunca reabre a sessão.
    pub(crate) async fn close_project(&self, project_id: i64) {
        end_slots(self.matching(|key| key.project_id == project_id)).await;
    }

    // O mapa da orquestração é global: qualquer sessão, de qualquer projeto, guarda o prompt antigo.
    pub(crate) async fn close_everything(&self) {
        end_slots(self.matching(|_| true)).await;
    }

    // Instrução extra mudou: a sessão do agente e as das réplicas (<base>-<n>), em todos os projetos,
    // fecham no fim da troca em curso e reabrem com o prompt novo.
    pub(crate) async fn retire_agent(&self, agent: &Agent) {
        end_slots(self.matching(|key| {
            key.agent == *agent
                || replica_of(key.agent.as_str()).is_some_and(|(base, _)| base == agent.as_str())
        }))
        .await;
    }

    // Gancho da remoção do agente (O4): ainda sem chamador.
    #[allow(dead_code)]
    pub(crate) async fn close_agent(&self, db: &Db, agent: Agent) -> Result<(), DbError> {
        let removed: Vec<Arc<SessionSlot>> = {
            let mut slots = self.lock();
            let keys: Vec<SessionKey> = slots
                .keys()
                .filter(|key| key.agent == agent)
                .cloned()
                .collect();
            keys.iter().filter_map(|key| slots.remove(key)).collect()
        };
        end_slots(removed).await;
        db.clear_agent_sessions(&agent)
    }

    // Na saída do app o processo é encerrado pelo pid, sem esperar o lock de um passo em andamento.
    pub(crate) fn close_all(&self) {
        for slot in self.matching(|_| true) {
            let pid = slot.pid.swap(NO_PID, Ordering::SeqCst);
            if pid != NO_PID {
                kill_process_tree(pid);
            }
        }
    }

    // Abre espaço para um novo processo fechando a sessão ociosa menos recente; sessões em uso (lock tomado) ficam.
    pub(super) async fn make_room(&self, requester: &Arc<SessionSlot>) {
        loop {
            let live: Vec<Arc<SessionSlot>> = self
                .matching(|_| true)
                .into_iter()
                .filter(|slot| !Arc::ptr_eq(slot, requester) && slot.is_live())
                .collect();
            if live.len() < MAX_LIVE_SESSIONS {
                return;
            }
            let mut oldest: Option<OwnedMutexGuard<AgentSession>> = None;
            for slot in &live {
                let Ok(guard) = Arc::clone(&slot.session).try_lock_owned() else {
                    continue;
                };
                if oldest
                    .as_ref()
                    .is_none_or(|current| guard.last_used < current.last_used)
                {
                    oldest = Some(guard);
                }
            }
            let Some(mut idle) = oldest else {
                return;
            };
            idle.end().await;
        }
    }

    fn live_count(&self) -> usize {
        self.matching(|_| true)
            .into_iter()
            .filter(|slot| slot.is_live())
            .count()
    }

    pub(super) fn has_preload_capacity(live_count: usize) -> bool {
        live_count < MAX_LIVE_SESSIONS
    }

    pub(super) async fn preload_project(&self, app: &AppHandle, project_id: i64) -> usize {
        let state = app.state::<AppState>();
        let Ok(cli) = state.cli.as_ref() else {
            return 0;
        };
        let Ok(project_dir) = project_dir(app, project_id) else {
            return 0;
        };
        let Ok(project) = load_project(app, project_id) else {
            return 0;
        };
        let Ok(data_dir) = app.path().app_data_dir() else {
            return 0;
        };
        let Ok(attachments_dir) = crate::attachments::project_dir(&data_dir, project_id) else {
            return 0;
        };
        let Ok(team) = app.state::<AgentRegistry>().team() else {
            return 0;
        };
        let Ok(orchestration) = load_orchestration(app) else {
            return 0;
        };
        let paused = app
            .state::<DbState>()
            .db()
            .and_then(|db| db.project_paused_agents(project_id).ok())
            .unwrap_or_default();
        let targets = preload_targets(orchestration.as_ref(), &team, &paused);
        if targets.is_empty() {
            return 0;
        }
        // Só agente técnico precisa da árvore; o Ember pré-carregado fica com a identidade do projeto.
        let tree = if Self::has_preload_capacity(self.live_count())
            && targets.iter().any(|target| *target != Agent::ember())
        {
            scan_structure(&project_dir).await
        } else {
            None
        };
        if !is_selected_project(app, project_id) {
            return 0;
        }
        let context = project_context(&project, tree.as_ref());
        let ember_context = project_context_without_tree(&project);
        let providers = app.state::<ProviderRegistry>();
        let global_active = load_global_active_llm(app);
        let global_fallback = load_global_fallback(app);
        let mut adopted = 0;
        for agent in targets {
            let Ok(profile) = profile_of(&team, &agent) else {
                continue;
            };
            let Some(candidate) = preload_candidate(
                &agent,
                profile,
                &providers,
                &state,
                global_active.as_ref(),
                &global_fallback,
                project.permission_level,
            ) else {
                continue;
            };
            match candidate.provider.protocol {
                ProviderProtocol::ClaudeLive if candidate.provider.has_tools() => {
                    if !Self::has_preload_capacity(self.live_count()) {
                        continue;
                    }
                    if !is_selected_project(app, project_id) {
                        return adopted;
                    }
                    let slot = self.slot(SessionKey {
                        project_id,
                        agent: agent.clone(),
                    });
                    if slot.closing.load(Ordering::SeqCst) {
                        continue;
                    }
                    let Ok(mut guard) = Arc::clone(&slot.session).try_lock_owned() else {
                        continue;
                    };
                    if slot.closing.load(Ordering::SeqCst) || guard.live.is_some() {
                        continue;
                    }
                    match open_preloaded_session(
                        app,
                        project_id,
                        &project_dir,
                        &attachments_dir,
                        project.permission_level,
                        &agent,
                        profile,
                        &team,
                        orchestration.as_ref(),
                        &paused,
                        if agent == Agent::ember() { &ember_context } else { &context },
                        &candidate,
                        cli,
                    ) {
                        Ok((live, resume, place, provider_key)) => {
                            guard.adopt_preload(live, resume, place, provider_key);
                            adopted += 1;
                        }
                        Err(err) => {
                            eprintln!("Não foi possível pré-carregar a sessão de {agent}: {err}");
                        }
                    }
                }
                ProviderProtocol::ClaudeLive
                | ProviderProtocol::CodexJsonl
                | ProviderProtocol::AntigravityStream
                | ProviderProtocol::OpenCodeJson
                | ProviderProtocol::GenericCli => {
                    if let Err(err) = resolve_program_for(&agent, &candidate.command, cli) {
                        eprintln!("CLI indisponível na pré-carga de {agent}: {err}");
                    }
                }
            }
        }
        adopted
    }

    pub(super) async fn expire_unused_preloads(&self, project_id: i64) {
        for slot in self.matching(|key| key.project_id == project_id) {
            let Ok(mut guard) = Arc::clone(&slot.session).try_lock_owned() else {
                continue;
            };
            if guard.is_unused_preload()
                && guard
                    .preloaded_at
                    .is_some_and(|at| at.elapsed() >= PRELOAD_TTL)
            {
                guard.close_live().await;
            }
        }
    }
}

fn is_selected_project(app: &AppHandle, project_id: i64) -> bool {
    app.state::<DbState>()
        .db()
        .and_then(|db| db.selected_project().ok())
        .is_some_and(|project| project.id == project_id)
}

pub(crate) fn schedule_preload(app: AppHandle, project_id: i64) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(PRELOAD_DEBOUNCE).await;
        if !is_selected_project(&app, project_id) {
            return;
        }
        let adopted = app
            .state::<AppState>()
            .sessions
            .preload_project(&app, project_id)
            .await;
        if adopted == 0 {
            return;
        }
        tokio::time::sleep(PRELOAD_TTL).await;
        app.state::<AppState>()
            .sessions
            .expire_unused_preloads(project_id)
            .await;
    });
}

async fn end_slots(slots: Vec<Arc<SessionSlot>>) {
    for slot in &slots {
        slot.closing.store(true, Ordering::SeqCst);
    }
    for slot in slots {
        let mut session = slot.session.lock().await;
        if slot.closing.swap(false, Ordering::SeqCst) {
            session.end().await;
        }
    }
}

pub(super) struct Held {
    pub(super) slot: Arc<SessionSlot>,
    pub(super) guard: OwnedMutexGuard<AgentSession>,
}

// Cada passo reserva só a instância que fala; o lock de uma instância não impede as outras.
pub(super) struct ExchangeSessions<'a> {
    pool: &'a SessionPool,
    project_id: i64,
    pub(super) opened: HashSet<Agent>,
    touched: Vec<Arc<SessionSlot>>,
}

impl<'a> ExchangeSessions<'a> {
    pub(super) fn new(pool: &'a SessionPool, project_id: i64) -> Self {
        Self {
            pool,
            project_id,
            opened: HashSet::new(),
            touched: Vec::new(),
        }
    }

    pub(super) async fn hold(&mut self, agent: &Agent) -> Held {
        let slot = self.pool.slot(SessionKey {
            project_id: self.project_id,
            agent: agent.clone(),
        });
        let guard = Arc::clone(&slot.session).lock_owned().await;
        if !self.touched.iter().any(|known| Arc::ptr_eq(known, &slot)) {
            self.touched.push(Arc::clone(&slot));
        }
        Held { slot, guard }
    }

    pub(super) async fn review(&mut self, agent: &Agent, stalls: u32, repeats: u32) {
        let mut held = self.hold(agent).await;
        if degrades_to_fresh(stalls, repeats, held.guard.recoveries) {
            held.guard.degrade().await;
        }
    }

    // Sessão marcada para fechar durante a mensagem fecha aqui, antes de qualquer outra mensagem usá-la.
    pub(super) async fn release(self) {
        for slot in self.touched {
            if slot.closing.swap(false, Ordering::SeqCst) {
                slot.session.lock().await.end().await;
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/session_pool_tests.rs"]
mod tests;
