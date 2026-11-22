use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use tauri::{Listener, Manager};

use super::*;
use crate::agents::Agent;
use crate::events::{AgentState, EVENT_NAME};
use crate::mcp::support::build_app;
use crate::state::{
    ActiveDelegation, AppState, QueuedJob, RequestContext, MAX_WATCHDOG_RETRIES,
};

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

const PROJECT: i64 = 1;
const OTHER_PROJECT: i64 = 2;
const PING: Duration = Duration::from_secs(20);
const ITEM: &str = "card-7";

struct World {
    app: tauri::App,
    dir: std::path::PathBuf,
    events: Arc<Mutex<Vec<Value>>>,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn world() -> World {
    let dir = std::env::temp_dir().join(format!(
        "vokia-watchdog-test-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::SeqCst)
    ));
    let app = build_app(&dir);
    let events: Arc<Mutex<Vec<Value>>> = Arc::default();
    let sink = events.clone();
    app.handle().listen(EVENT_NAME, move |event| {
        if let Ok(value) = serde_json::from_str::<Value>(event.payload()) {
            sink.lock().unwrap().push(value);
        }
    });
    World { app, dir, events }
}

fn key(project_id: i64, agent: &Agent) -> (i64, Agent, Option<String>) {
    (project_id, agent.clone(), Some(ITEM.to_string()))
}

fn delegation(project_id: i64, agent: &Agent, age: Duration) -> ActiveDelegation {
    ActiveDelegation {
        project_id,
        to_agent: agent.clone(),
        item_id: Some(ITEM.to_string()),
        created_at: Instant::now().checked_sub(age).expect("relógio monotônico com mais de 1 minuto"),
        ping_interval: PING,
        activated: false,
        retries: 0,
    }
}

impl World {
    fn state(&self) -> tauri::State<'_, AppState> {
        self.app.state::<AppState>()
    }

    fn register(&self, delegation: ActiveDelegation) {
        let key = (delegation.project_id, delegation.to_agent.clone(), delegation.item_id.clone());
        self.state().flow.lock().unwrap().active_delegations.insert(key, delegation);
    }

    fn snapshot(&self, project_id: i64, agent: &Agent) -> Option<(bool, u32, Duration)> {
        let state = self.state();
        let flow = state.flow.lock().unwrap();
        flow.active_delegations
            .get(&key(project_id, agent))
            .map(|entry| (entry.activated, entry.retries, entry.created_at.elapsed()))
    }

    fn expire(&self, project_id: i64, agent: &Agent) {
        let state = self.state();
        let mut flow = state.flow.lock().unwrap();
        let entry = flow.active_delegations.get_mut(&key(project_id, agent)).expect("delegação registrada");
        entry.created_at = Instant::now().checked_sub(PING + Duration::from_secs(1)).unwrap();
    }

    fn errors(&self) -> Vec<String> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| event["type"] == "error")
            .map(|event| event["message"].as_str().unwrap().to_string())
            .collect()
    }

    fn sweep(&self) {
        sweep_silent_delegations(self.app.handle());
    }
}

#[test]
fn estado_sending_ou_thinking_do_agente_destino_marca_a_delegacao_como_ativada() {
    for state in [AgentState::Sending, AgentState::Thinking] {
        let world = world();
        let vexa = Agent::vexa();
        world.register(delegation(PROJECT, &vexa, Duration::ZERO));

        emit_state(world.app.handle(), PROJECT, vexa.clone(), state).unwrap();

        assert_eq!(world.snapshot(PROJECT, &vexa).map(|(activated, _, _)| activated), Some(true));
    }
}

#[test]
fn estado_idle_ou_waiting_nao_ativa_a_delegacao() {
    for state in [AgentState::Idle, AgentState::Waiting] {
        let world = world();
        let vexa = Agent::vexa();
        world.register(delegation(PROJECT, &vexa, Duration::ZERO));

        emit_state(world.app.handle(), PROJECT, vexa.clone(), state).unwrap();

        assert_eq!(world.snapshot(PROJECT, &vexa).map(|(activated, _, _)| activated), Some(false));
    }
}

#[test]
fn ativacao_vale_so_para_o_agente_e_o_projeto_do_estado_emitido() {
    let world = world();
    let vexa = Agent::vexa();
    let axiom = Agent::axiom();
    world.register(delegation(PROJECT, &vexa, Duration::ZERO));
    world.register(delegation(PROJECT, &axiom, Duration::ZERO));
    world.register(delegation(OTHER_PROJECT, &vexa, Duration::ZERO));

    emit_state(world.app.handle(), PROJECT, vexa.clone(), AgentState::Thinking).unwrap();

    assert_eq!(world.snapshot(PROJECT, &vexa).unwrap().0, true);
    assert_eq!(world.snapshot(PROJECT, &axiom).unwrap().0, false);
    assert_eq!(world.snapshot(OTHER_PROJECT, &vexa).unwrap().0, false);
}

#[test]
fn varredura_remove_delegacao_ativada_sem_tentar_de_novo_nem_avisar_erro() {
    let world = world();
    let vexa = Agent::vexa();
    let mut active = delegation(PROJECT, &vexa, PING + Duration::from_secs(1));
    active.activated = true;
    world.register(active);

    world.sweep();

    assert!(world.snapshot(PROJECT, &vexa).is_none());
    assert!(world.errors().is_empty());
}

#[test]
fn varredura_nao_mexe_em_delegacao_silenciosa_antes_do_intervalo() {
    let world = world();
    let vexa = Agent::vexa();
    world.register(delegation(PROJECT, &vexa, Duration::from_secs(5)));

    world.sweep();

    let (activated, retries, _) = world.snapshot(PROJECT, &vexa).expect("continua registrada");
    assert!(!activated);
    assert_eq!(retries, 0);
    assert!(world.errors().is_empty());
}

#[test]
fn varredura_conta_tentativa_reinicia_o_relogio_e_so_desiste_depois_do_limite() {
    let world = world();
    let vexa = Agent::vexa();
    world.register(delegation(PROJECT, &vexa, PING + Duration::from_secs(1)));

    for attempt in 1..=MAX_WATCHDOG_RETRIES {
        world.sweep();
        let (_, retries, elapsed) = world.snapshot(PROJECT, &vexa).expect("ainda tentando");
        assert_eq!(retries, attempt);
        assert!(elapsed < PING, "o relógio recomeça a cada tentativa");
        assert!(world.errors().is_empty(), "nada de erro na tentativa {attempt}");
        world.expire(PROJECT, &vexa);
    }

    world.sweep();

    assert!(world.snapshot(PROJECT, &vexa).is_none());
    assert_eq!(
        world.errors(),
        [format!("O agente {} não respondeu à delegação do card {ITEM}.", vexa.as_str())]
    );
}

#[test]
fn delegacao_ativada_entre_tentativas_nao_gera_erro() {
    let world = world();
    let vexa = Agent::vexa();
    world.register(delegation(PROJECT, &vexa, PING + Duration::from_secs(1)));
    world.sweep();
    world.expire(PROJECT, &vexa);

    emit_state(world.app.handle(), PROJECT, vexa.clone(), AgentState::Sending).unwrap();
    world.sweep();
    world.sweep();

    assert!(world.snapshot(PROJECT, &vexa).is_none());
    assert!(world.errors().is_empty());
}

#[test]
fn desistencia_sem_card_nao_cita_card_na_mensagem() {
    let world = world();
    let vexa = Agent::vexa();
    let mut silent = delegation(PROJECT, &vexa, PING + Duration::from_secs(1));
    silent.item_id = None;
    silent.retries = MAX_WATCHDOG_RETRIES;
    world.state().flow.lock().unwrap().active_delegations.insert((PROJECT, vexa.clone(), None), silent);

    world.sweep();

    assert_eq!(world.errors(), [format!("O agente {} não respondeu à delegação.", vexa.as_str())]);
}

fn dispatched_ping(ping_seconds: Option<u64>) -> Duration {
    let world = world();
    let vexa = Agent::vexa();
    let mut context = RequestContext::new(None, None, None);
    context.ping_seconds = ping_seconds;

    start_dispatch(world.app.handle(), &world.state(), PROJECT, &vexa, "Revise", context, 1, 1, false).unwrap();

    let state = world.state();
    let flow = state.flow.lock().unwrap();
    let ping = flow.active_delegations.get(&(PROJECT, vexa, None)).expect("delegação registrada no despacho").ping_interval;
    ping
}

#[test]
fn despacho_registra_ping_padrao_de_20s_e_limita_o_pedido_entre_5_e_300s() {
    assert_eq!(dispatched_ping(None), Duration::from_secs(20));
    assert_eq!(dispatched_ping(Some(0)), Duration::from_secs(5));
    assert_eq!(dispatched_ping(Some(1)), Duration::from_secs(5));
    assert_eq!(dispatched_ping(Some(5)), Duration::from_secs(5));
    assert_eq!(dispatched_ping(Some(45)), Duration::from_secs(45));
    assert_eq!(dispatched_ping(Some(300)), Duration::from_secs(300));
    assert_eq!(dispatched_ping(Some(9999)), Duration::from_secs(300));
}

const OTHER_ITEM: &str = "card-9";

fn registered_keys(world: &World) -> Vec<(i64, String, Option<String>)> {
    let state = world.state();
    let flow = state.flow.lock().unwrap();
    let mut keys: Vec<_> = flow
        .active_delegations
        .keys()
        .map(|(project_id, agent, item)| (*project_id, agent.as_str().to_string(), item.clone()))
        .collect();
    keys.sort();
    keys
}

fn reserve_session(world: &World, agent: &Agent) -> SessionKey {
    let key = SessionKey { project_id: PROJECT, agent: agent.clone() };
    let state = world.state();
    let mut flow = state.flow.lock().unwrap();
    flow.pending.insert(key.clone(), VecDeque::new());
    flow.running.insert(key.clone(), "run".to_string());
    key
}

fn register_with_item(world: &World, project_id: i64, agent: &Agent, item: Option<&str>) {
    let mut entry = delegation(project_id, agent, Duration::ZERO);
    entry.item_id = item.map(str::to_string);
    world
        .state()
        .flow
        .lock()
        .unwrap()
        .active_delegations
        .insert((project_id, agent.clone(), entry.item_id.clone()), entry);
}

#[test]
fn abandonar_o_fluxo_antes_de_sending_remove_so_a_delegacao_daquele_card() {
    let world = world();
    let vexa = Agent::vexa();
    let axiom = Agent::axiom();
    register_with_item(&world, PROJECT, &vexa, Some(ITEM));
    register_with_item(&world, PROJECT, &vexa, Some(OTHER_ITEM));
    register_with_item(&world, PROJECT, &axiom, Some(ITEM));
    register_with_item(&world, OTHER_PROJECT, &vexa, Some(ITEM));
    let key = reserve_session(&world, &vexa);

    abandon_flow(world.app.handle(), &world.state(), &key, Some(ITEM));

    assert_eq!(
        registered_keys(&world),
        [
            (PROJECT, axiom.as_str().to_string(), Some(ITEM.to_string())),
            (PROJECT, vexa.as_str().to_string(), Some(OTHER_ITEM.to_string())),
            (OTHER_PROJECT, vexa.as_str().to_string(), Some(ITEM.to_string())),
        ]
    );
    let state = world.state();
    let flow = state.flow.lock().unwrap();
    assert!(!flow.pending.contains_key(&key));
    assert!(!flow.running.contains_key(&key));
}

#[test]
fn abandonar_fluxo_sem_card_remove_a_delegacao_sem_card() {
    let world = world();
    let vexa = Agent::vexa();
    register_with_item(&world, PROJECT, &vexa, None);
    register_with_item(&world, PROJECT, &vexa, Some(ITEM));
    let key = reserve_session(&world, &vexa);

    abandon_flow(world.app.handle(), &world.state(), &key, None);

    assert_eq!(registered_keys(&world), [(PROJECT, vexa.as_str().to_string(), Some(ITEM.to_string()))]);
}

#[test]
fn fim_do_fluxo_sem_jobs_na_fila_limpa_as_delegacoes_do_agente_e_do_projeto() {
    let world = world();
    let vexa = Agent::vexa();
    let axiom = Agent::axiom();
    register_with_item(&world, PROJECT, &vexa, Some(ITEM));
    register_with_item(&world, PROJECT, &vexa, None);
    register_with_item(&world, PROJECT, &axiom, Some(ITEM));
    register_with_item(&world, OTHER_PROJECT, &vexa, Some(ITEM));
    let key = reserve_session(&world, &vexa);

    let next = next_job(&world.state(), key.clone()).unwrap();

    assert!(next.is_none());
    assert_eq!(
        registered_keys(&world),
        [
            (PROJECT, axiom.as_str().to_string(), Some(ITEM.to_string())),
            (OTHER_PROJECT, vexa.as_str().to_string(), Some(ITEM.to_string())),
        ]
    );
    let state = world.state();
    let flow = state.flow.lock().unwrap();
    assert!(!flow.pending.contains_key(&key));
    assert!(!flow.running.contains_key(&key));
}

#[test]
fn fluxo_com_job_na_fila_continua_e_mantem_as_delegacoes_registradas() {
    let world = world();
    let vexa = Agent::vexa();
    register_with_item(&world, PROJECT, &vexa, Some(ITEM));
    let key = reserve_session(&world, &vexa);
    world.state().flow.lock().unwrap().pending.get_mut(&key).unwrap().push_back(QueuedJob {
        text: "próximo".to_string(),
        context: RequestContext::new(None, Some(ITEM.to_string()), None),
    });

    let next = next_job(&world.state(), key).unwrap();

    assert_eq!(next.map(|job| job.text), Some("próximo".to_string()));
    assert_eq!(registered_keys(&world), [(PROJECT, vexa.as_str().to_string(), Some(ITEM.to_string()))]);
}
