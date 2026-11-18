use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex, MutexGuard, PoisonError};

use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::agents::Agent;
use crate::claude::kill_process_tree;
use crate::db::{Db, DbError};
use crate::session_key::SessionKey;

use super::agent_session::AgentSession;
use super::session_policy::{degrades_to_fresh, MAX_LIVE_SESSIONS};

pub(super) const NO_PID: u32 = 0;

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
