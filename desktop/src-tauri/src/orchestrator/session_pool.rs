use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex, MutexGuard, PoisonError};
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, OwnedMutexGuard};
use tokio::time::MissedTickBehavior;

use crate::agents::{is_agent_or_replica, Agent, AgentRegistry};
use crate::claude::kill_process_tree;
use crate::db::{Db, DbError, DbState};
use crate::providers::{ProviderProtocol, ProviderRegistry};
use crate::session_key::SessionKey;
use crate::state::AppState;

use super::adapters::AdapterRuntimePool;
use super::agent_session::{
    open_preloaded_session, preload_candidate, preload_targets, AgentSession, SESSION_IDLE_LIMIT,
};
use super::error::FlowError;
use super::fallback::{load_global_active_llm, load_global_fallback, resolve_program_for};
use super::flow::{load_orchestration, profile_of};
use super::project_context::{
    load_project, project_context, project_context_without_tree, project_dir, scan_structure,
};
use super::session_policy::{degrades_to_fresh, MAX_LIVE_SESSIONS, SESSION_REAP_INTERVAL_SECS};

pub(super) const NO_PID: u32 = 0;
const PRELOAD_DEBOUNCE: Duration = Duration::from_millis(300);
const SESSION_REAP_INTERVAL: Duration = Duration::from_secs(SESSION_REAP_INTERVAL_SECS);

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
    tasks: StdMutex<HashMap<(i64, String), Arc<Mutex<()>>>>,
    refinery: StdMutex<HashMap<i64, Arc<Mutex<()>>>>,
    adapters: AdapterRuntimePool,
}

impl SessionPool {
    pub(crate) fn adapters(&self) -> &AdapterRuntimePool {
        &self.adapters
    }
    // Lock por thread (agente/instância + card) no projeto: instâncias ou cards distintos rodam em paralelo.
    pub(super) fn task_lock(&self, project_id: i64, thread_key: &str) -> Arc<Mutex<()>> {
        let mut tasks = self.tasks.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(
            tasks
                .entry((project_id, thread_key.to_string()))
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
    }

    // Fila dedicada de integração/limpeza (padrão Refinery): serializa apenas merge/cleanup git por projeto.
    pub(super) fn refinery_lock(&self, project_id: i64) -> Arc<Mutex<()>> {
        let mut refinery = self.refinery.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(
            refinery
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
        self.adapters.archive_project(project_id).await;
        end_slots(self.matching(|key| key.project_id == project_id)).await;
    }

    // O mapa da orquestração é global: qualquer sessão, de qualquer projeto, guarda o prompt antigo.
    pub(crate) async fn close_everything(&self) {
        self.adapters.archive_all().await;
        end_slots(self.matching(|_| true)).await;
    }

    // Instrução extra mudou: a sessão do agente e as das réplicas (<base>-<n>), em todos os projetos,
    // fecham no fim da troca em curso e reabrem com o prompt novo.
    pub(crate) async fn retire_agent(&self, agent: &Agent) {
        self.adapters.archive_agent(agent).await;
        end_slots(self.matching(|key| is_agent_or_replica(key.agent.as_str(), agent))).await;
    }

    // Agente excluído: os slots saem do mapa e o --resume gravado some, dele e das réplicas.
    pub(crate) async fn close_agent(&self, db: &Db, agent: &Agent) -> Result<(), DbError> {
        let removed: Vec<Arc<SessionSlot>> = {
            let mut slots = self.lock();
            let keys: Vec<SessionKey> = slots
                .keys()
                .filter(|key| is_agent_or_replica(key.agent.as_str(), agent))
                .cloned()
                .collect();
            keys.iter().filter_map(|key| slots.remove(key)).collect()
        };
        self.adapters.archive_agent(agent).await;
        end_slots(removed).await;
        db.clear_agent_sessions(agent)
    }

    // Libera threads persistentes associadas a um worktree antes de apagar a pasta (evita lock no Windows).
    pub(crate) async fn release_worktree(
        &self,
        project_id: i64,
        item_id: &str,
        worktree_path: Option<&Path>,
    ) {
        self.adapters
            .archive_worktree(project_id, item_id, worktree_path)
            .await;
    }

    // Entrega correção de rumo (steering) diretamente ao turno ativo quando o provedor suporta.
    pub(crate) async fn steer_active_turn(
        &self,
        key: &SessionKey,
        message: &str,
    ) -> Result<bool, FlowError> {
        self.adapters.steer_turn(key, message).await
    }

    pub(crate) async fn interrupt_turn(&self, key: &SessionKey) -> Result<bool, FlowError> {
        self.adapters.interrupt_turn(key).await
    }

    // Na saída do app o processo é encerrado pelo pid, sem esperar o lock de um passo em andamento.
    pub(crate) fn close_all(&self) {
        self.adapters.shutdown_all();
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

    pub(super) async fn preload_project(&self, app: &AppHandle, project_id: i64) {
        let state = app.state::<AppState>();
        let Ok(cli) = state.cli.as_ref() else {
            return;
        };
        let Ok(project_dir) = project_dir(app, project_id) else {
            return;
        };
        let Ok(project) = load_project(app, project_id) else {
            return;
        };
        let Ok(data_dir) = app.path().app_data_dir() else {
            return;
        };
        let Ok(attachments_dir) = crate::attachments::project_dir(&data_dir, project_id) else {
            return;
        };
        let Ok(team) = app.state::<AgentRegistry>().team() else {
            return;
        };
        let Ok(orchestration) = load_orchestration(app) else {
            return;
        };
        let paused = app
            .state::<DbState>()
            .db()
            .and_then(|db| db.project_paused_agents(project_id).ok())
            .unwrap_or_default();
        let targets = preload_targets(orchestration.as_ref(), &team, &paused);
        if targets.is_empty() {
            return;
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
            return;
        }
        let context = project_context(&project, tree.as_ref());
        let ember_context = project_context_without_tree(&project);
        let providers = app.state::<ProviderRegistry>();
        let global_active = load_global_active_llm(app);
        let global_fallback = load_global_fallback(app);
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
                        return;
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
                        }
                        Err(err) => {
                            eprintln!("Não foi possível pré-carregar a sessão de {agent}: {err}");
                        }
                    }
                }
                ProviderProtocol::ClaudeLive
                | ProviderProtocol::CodexAppServer
                | ProviderProtocol::CodexJsonl
                | ProviderProtocol::Acp
                | ProviderProtocol::AntigravityStream
                | ProviderProtocol::OpenCodeJson
                | ProviderProtocol::GenericCli => {
                    if let Err(err) = resolve_program_for(&agent, &candidate.command, cli) {
                        eprintln!("CLI indisponível na pré-carga de {agent}: {err}");
                    }
                }
            }
        }
    }

    // Sessão com mensagem em andamento (lock tomado) fica; close_live mantém o session_id para o --resume.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) async fn reap_idle(&self, limit: Duration) {
        self.reap_idle_with_guard(limit, |_| false).await;
    }

    pub(super) async fn reap_idle_with_guard(
        &self,
        limit: Duration,
        is_externally_busy: impl Fn(&SessionKey) -> bool,
    ) {
        for slot in self.matching(|_| true) {
            let Ok(mut guard) = Arc::clone(&slot.session).try_lock_owned() else {
                continue;
            };
            if guard.is_idle(limit) {
                guard.close_live().await;
            }
        }
        self.adapters.reap_idle(is_externally_busy).await;
    }
}

// Sem isto o limite de ociosidade só seria checado na próxima mensagem ao agente, e um agente que não volta a falar
// mantém o processo da CLI vivo até o app fechar.
pub(crate) fn spawn_idle_reaper(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(SESSION_REAP_INTERVAL);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let state = app.state::<AppState>();
            state
                .sessions
                .reap_idle_with_guard(SESSION_IDLE_LIMIT, |key| {
                    let has_permission = state.permissions.has_pending(key.project_id, &key.agent);
                    let has_decision = state.decisions.lock().is_ok_and(|decisions| {
                        decisions.values().any(|decision| {
                            decision.project_id == key.project_id && decision.agent == key.agent
                        })
                    });
                    has_permission || has_decision
                })
                .await;
        }
    });
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
        app.state::<AppState>()
            .sessions
            .preload_project(&app, project_id)
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
    per_step: bool,
    pub(super) opened: HashSet<Agent>,
    touched: Vec<Arc<SessionSlot>>,
}

impl<'a> ExchangeSessions<'a> {
    pub(super) fn new(pool: &'a SessionPool, project_id: i64) -> Self {
        Self::with_mode(pool, project_id, false)
    }

    // Etapa executada uma vez: a CLI abre, responde e fecha, sem retomar nem gravar session_id.
    pub(super) fn per_step(pool: &'a SessionPool, project_id: i64) -> Self {
        Self::with_mode(pool, project_id, true)
    }

    fn with_mode(pool: &'a SessionPool, project_id: i64, per_step: bool) -> Self {
        Self {
            pool,
            project_id,
            per_step,
            opened: HashSet::new(),
            touched: Vec::new(),
        }
    }

    pub(super) fn is_per_step(&self) -> bool {
        self.per_step
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
