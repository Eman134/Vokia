use std::collections::{HashMap, HashSet, VecDeque};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::claude::CliProblem;
use crate::console_log::ConsoleLog;
use crate::events::Agent;
use crate::orchestrator::fallback::{CooldownTracker, TemporaryOverride};
use crate::orchestrator::SessionPool;
use crate::permissions::PermissionBroker;
use crate::session_key::SessionKey;
use crate::task::{Cache, TaskRecord};

pub struct AppState {
    pub cli: Result<PathBuf, CliProblem>,
    pub cache: Mutex<HashMap<i64, Cache>>,
    pub flow: Mutex<FlowQueue>,
    pub sessions: SessionPool,
    pub archive: Mutex<Vec<TaskRecord>>,
    pub permissions: PermissionBroker,
    pub decisions: Mutex<HashMap<u64, PendingDecision>>,
    pub next_decision_id: AtomicU64,
    pub console: ConsoleLog,
    pub cooldowns: Mutex<CooldownTracker>,
    pub temporary_overrides: Mutex<HashMap<Agent, TemporaryOverride>>,
}

impl AppState {
    pub fn new(cli: Result<PathBuf, CliProblem>) -> Self {
        Self {
            cli,
            cache: Mutex::new(HashMap::new()),
            flow: Mutex::new(FlowQueue::default()),
            sessions: SessionPool::default(),
            archive: Mutex::new(Vec::new()),
            permissions: PermissionBroker::default(),
            decisions: Mutex::new(HashMap::new()),
            next_decision_id: AtomicU64::new(0),
            console: ConsoleLog::default(),
            cooldowns: Mutex::new(CooldownTracker::default()),
            temporary_overrides: Mutex::new(HashMap::new()),
        }
    }
}

pub struct PendingDecision {
    pub agent: Agent,
    pub project_id: i64,
    pub kind: crate::events::DecisionKind,
    pub title: String,
    pub request: String,
    pub item_id: Option<String>,
    // Opções da decisão: quando o Ember pergunta o projeto, a escolha troca o projeto do pedido.
    pub options: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RequestContext {
    pub run_id: Option<u64>,
    pub item_id: Option<String>,
    pub channel_id: Option<i64>,
    // Pedido delegado em paralelo: roda uma vez na instância, sem tarefa do projeto.
    pub parallel: bool,
    // Lote de delegações paralelas ao qual este pedido pertence (a junção espera todas).
    pub batch: Option<u64>,
    // Anexos que a mensagem do usuário referencia; são marcados como enviados ao gravar a mensagem.
    pub attachments: Vec<String>,
    // Pedido interrompido em retomada: reaproveita o registro original e não regrava a mensagem do usuário.
    pub resumed_run: Option<i64>,
    // Intervalo de verificação pedido pelo agente que delegou; o watchdog aplica padrão e limites.
    pub ping_seconds: Option<u64>,
}

impl RequestContext {
    pub fn new(run_id: Option<u64>, item_id: Option<String>, channel_id: Option<i64>) -> Self {
        let item_id = item_id
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty());
        Self {
            run_id,
            item_id,
            channel_id,
            parallel: false,
            batch: None,
            attachments: Vec::new(),
            resumed_run: None,
            ping_seconds: None,
        }
    }
}

pub struct QueuedJob {
    pub db_id: Option<i64>,
    pub text: String,
    pub context: RequestContext,
}

// Delegação paralela que esperou o teto de max_parallel liberar uma vaga.
pub struct WaitingDelegation {
    pub base: crate::events::Agent,
    pub text: String,
    pub context: RequestContext,
}

pub const DEFAULT_DELEGATION_PING_SECS: u64 = 20;
pub const MIN_DELEGATION_PING_SECS: u64 = 5;
pub const MAX_DELEGATION_PING_SECS: u64 = 300;
pub const MAX_WATCHDOG_RETRIES: u32 = 2;

// Delegação despachada; `activated` vira true quando o agente destino começa a trabalhar.
pub struct ActiveDelegation {
    pub project_id: i64,
    pub to_agent: Agent,
    pub item_id: Option<String>,
    pub created_at: Instant,
    pub ping_interval: Duration,
    pub activated: bool,
    pub retries: u32,
}

// Uma chave presente é um fluxo em andamento; os pedidos que chegam nesse meio-tempo esperam aqui.
#[derive(Default)]
pub struct FlowQueue {
    pub pending: HashMap<SessionKey, VecDeque<QueuedJob>>,
    // Item do turno em andamento de cada fluxo; sai junto com a chave em pending.
    pub running: HashMap<SessionKey, String>,
    pub waiting: HashMap<i64, VecDeque<WaitingDelegation>>,
    pub batches: HashMap<u64, Batch>,
    pub next_batch: u64,
    // Ramos abertos de cada card (por projeto): o principal, as linhas assíncronas e as saídas de gateway paralelo.
    pub open_branches: HashMap<(i64, String), u32>,
    // Card que chegou ao fim do fluxo, com o agente que chegou: a validação final roda quando o último ramo termina.
    pub ended: HashMap<(i64, String), crate::events::Agent>,
    // Card com algum ramo que parou por erro: a validação final não abre enquanto o card não tiver todos os ramos concluídos.
    pub stopped: HashSet<(i64, String)>,
    // Delegações despachadas por (projeto, agente destino, card); a nova entrada da mesma chave substitui a anterior.
    pub active_delegations: HashMap<(i64, Agent, Option<String>), ActiveDelegation>,
}

// Um lote: as trocas paralelas de um turno do líder. A junção sai quando a última volta.
pub struct Batch {
    pub project_id: i64,
    pub leader: Agent,
    pub objective: String,
    pub variables: Vec<crate::task::Variable>,
    pub channel_id: Option<i64>,
    pub item_id: Option<String>,
    pub pending: usize,
    pub results: Vec<(String, String)>,
    // Agente base de cada delegação despachada (total do placar); instâncias que estão rodando e que já voltaram.
    pub dispatched: Vec<crate::events::Agent>,
    pub started: Vec<crate::events::Agent>,
    pub returned: Vec<crate::events::Agent>,
    // Contadores da tarefa do líder: a réplica confere política e limites contra eles.
    pub counters: crate::orchestration::counters::TaskCounters,
}
