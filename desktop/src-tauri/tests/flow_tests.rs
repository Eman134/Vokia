use super::*;

#[test]
fn tok1_interpreter_fast_path_cobre_marcadores_e_variaveis_ativas() {
    use crate::task::needs_llm_interpreter;

    for request in ["Oi", "Crie um card de login", "@Vexa ajuste o botão"] {
        assert!(!needs_llm_interpreter(request, false), "pedido sem marcador: {request}");
    }
    for request in [
        "contem até 20 alternadamente",
        "atualize $contador",
        "pronto quando o relatório existir",
    ] {
        assert!(needs_llm_interpreter(request, false), "pedido com marcador: {request}");
    }
    assert!(needs_llm_interpreter("continue", true));
}

#[test]
fn tok1_fast_definition_trunca_utf8_e_inclui_agente_mencionado_uma_vez() {
    use crate::agents::{Agent, AgentRegistry};
    use crate::task::fast_definition;

    let dir = std::env::temp_dir().join(format!(
        "vokia-fast-definition-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let team = AgentRegistry::load(&dir).team().expect("time padrão");
    let origin = Agent::ember();
    let request = format!("@Vexa {}", "á".repeat(400));

    let definition = fast_definition(&origin, &request, &team, true);

    assert!(definition.nova_tarefa);
    assert_eq!(definition.objetivo, definition.criterion);
    assert_eq!(definition.objetivo.chars().count(), 300);
    assert_eq!(definition.participants.first(), Some(&origin));
    assert_eq!(definition.participants.get(1), Some(&team.named("Vexa").unwrap()));
    assert_eq!(definition.participants.len(), 2);
    assert!(!definition.verify);
    assert!(!definition.alternate);
    assert!(definition.variables.is_empty());
    assert!(definition.stop.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pedido_sem_canal_chega_ao_agente_identico_byte_a_byte() {
    assert_eq!(
        with_channel_note("faça isso\ne aquilo", None),
        "faça isso\ne aquilo"
    );
}

#[test]
fn nota_do_canal_entra_uma_vez_no_fim_do_pedido() {
    let found = FoundChannel {
        id: 1,
        note: channel_note("front", &["Vexa".to_string(), "Kairo".to_string()]),
    };

    let request = with_channel_note("faça isso", Some(&found));

    assert_eq!(
        request,
        "faça isso\nMensagem enviada no canal #front (membros: Vexa, Kairo)."
    );
}

mod dispatch {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::agents::replica_id;
    use crate::mcp::support::build_app;

    static NEXT_DIR: AtomicU32 = AtomicU32::new(0);
    const PROJECT: i64 = 1;
    const REQUEST: &str = "implemente o login";

    struct Setup {
        app: tauri::App,
        dir: std::path::PathBuf,
    }

    impl Drop for Setup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn setup() -> Setup {
        let dir = std::env::temp_dir().join(format!(
            "vokia-flow-dispatch-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).expect("diretório temporário");
        Setup {
            app: build_app(&dir),
            dir,
        }
    }

    fn agent(id: &str) -> Agent {
        Agent::from_stored(id).expect("agente")
    }

    fn key(agent: Agent) -> SessionKey {
        SessionKey {
            project_id: PROJECT,
            agent,
        }
    }

    fn mark_running(setup: &Setup, agents: &[Agent]) {
        let state = setup.app.state::<AppState>();
        let mut flow = state.flow.lock().unwrap();
        for agent in agents {
            flow.pending.insert(key(agent.clone()), VecDeque::new());
        }
    }

    fn dispatch(setup: &Setup, max_instances: i64, max_parallel: i64) {
        let state = setup.app.state::<AppState>();
        dispatch_request(
            setup.app.handle(),
            &state,
            PROJECT,
            &agent("vexa"),
            REQUEST,
            RequestContext::new(None, None, None),
            max_instances,
            max_parallel,
        )
        .expect("o despacho deveria aceitar o pedido");
    }

    fn waiting_texts(setup: &Setup) -> Vec<String> {
        let state = setup.app.state::<AppState>();
        let flow = state.flow.lock().unwrap();
        flow.waiting
            .get(&PROJECT)
            .map(|queue| queue.iter().map(|job| job.text.clone()).collect())
            .unwrap_or_default()
    }

    fn queued_on(setup: &Setup, agent: Agent) -> usize {
        let state = setup.app.state::<AppState>();
        let flow = state.flow.lock().unwrap();
        flow.pending.get(&key(agent)).map_or(0, VecDeque::len)
    }

    fn running_count(setup: &Setup) -> usize {
        let state = setup.app.state::<AppState>();
        let flow = state.flow.lock().unwrap();
        flow.pending.len()
    }

    #[test]
    fn no_teto_de_max_parallel_a_delegacao_espera_no_projeto_e_nao_inicia_fluxo() {
        let setup = setup();
        mark_running(&setup, &[agent("axiom")]);

        dispatch(&setup, 2, 1);

        assert_eq!(waiting_texts(&setup), vec![REQUEST.to_string()]);
        assert_eq!(running_count(&setup), 1);
    }

    #[test]
    fn instancia_livre_da_replica_tambem_espera_quando_o_teto_ja_esta_cheio() {
        let setup = setup();
        mark_running(&setup, &[agent("vexa"), agent("axiom")]);

        dispatch(&setup, 2, 2);

        assert_eq!(waiting_texts(&setup), vec![REQUEST.to_string()]);
        assert_eq!(running_count(&setup), 2);
    }

    #[test]
    fn todas_as_instancias_ocupadas_enfileiram_na_base_sem_esperar_o_teto() {
        let setup = setup();
        mark_running(&setup, &[agent("vexa"), replica_id(&agent("vexa"), 2)]);

        dispatch(&setup, 2, 8);

        assert_eq!(queued_on(&setup, agent("vexa")), 1);
        assert!(waiting_texts(&setup).is_empty());
        assert_eq!(running_count(&setup), 2);
    }

    #[test]
    fn flow_error_is_cli_failure_verdadeiro_apenas_para_claude_error() {
        let cli_exit = FlowError::Claude(
            crate::claude::ClaudeError::Exit {
                code: Some(2),
                message: "Error: -p took --input-format as its prompt".to_string(),
            }
            .with_cli("antigravity"),
        );
        assert!(cli_exit.is_cli_failure());
        assert!(!cli_exit.is_limit());

        let cli_reported = FlowError::Claude(
            crate::claude::ClaudeError::Reported("falha de rede".to_string()).with_cli("antigravity"),
        );
        assert!(cli_reported.is_cli_failure());

        assert!(!FlowError::CliUnavailable("sem cli".to_string()).is_cli_failure());
        assert!(!FlowError::UnknownDecision.is_cli_failure());
        assert!(!FlowError::QueuePoisoned.is_cli_failure());
    }

    #[test]
    fn emit_error_message_persiste_erro_de_cli_no_historico_com_is_error_true() {
        let setup = setup();
        let db_state = setup.app.state::<crate::db::DbState>();
        let db = db_state.db().expect("banco aberto");
        let project_id = db
            .insert_project(&crate::db::NewProject {
                name: "proj-ui-err",
                symbol: "folder",
                description: "",
                path: "C:/projetos/proj-ui-err",
                origin_kind: "folder",
                origin_repo: None,
            })
            .expect("projeto")
            .id;

        let err = FlowError::Claude(
            crate::claude::ClaudeError::Exit {
                code: Some(2),
                message: "Error: -p took --input-format as its prompt".to_string(),
            }
            .with_cli("antigravity"),
        );
        assert!(err.is_cli_failure());

        crate::orchestrator::emit::emit_error_message(
            setup.app.handle(),
            project_id,
            Speaker::from(agent("vexa")),
            crate::orchestrator::emit::MessageScope {
                channel_id: Some(9),
                item_id: None,
            },
            &err.to_string(),
        )
        .expect("deve emitir e gravar mensagem de erro");

        let history = db.load_history(project_id).expect("historico");
        assert_eq!(history.len(), 1);
        match &history[0] {
            crate::events::VokiaEvent::Message {
                from,
                to,
                text,
                channel_id,
                is_error,
                ..
            } => {
                assert_eq!(*from, Speaker::from(agent("vexa")));
                assert_eq!(*to, Speaker::User);
                assert_eq!(
                    text,
                    "A CLI antigravity saiu com código 2: Error: -p took --input-format as its prompt"
                );
                assert_eq!(*channel_id, Some(9));
                assert!(*is_error, "erro de CLI deve carregar com is_error = true");
            }
            other => panic!("esperava VokiaEvent::Message, veio {other:?}"),
        }
    }

    fn history_project(setup: &Setup, name: &str) -> i64 {
        let db_state = setup.app.state::<crate::db::DbState>();
        let db = db_state.db().expect("banco aberto");
        let project_id = db
            .insert_project(&crate::db::NewProject {
                name,
                symbol: "folder",
                description: "",
                path: &format!("C:/projetos/{name}"),
                origin_kind: "folder",
                origin_repo: None,
            })
            .expect("projeto")
            .id;
        project_id
    }

    fn history_of(setup: &Setup, project_id: i64) -> Vec<crate::events::VokiaEvent> {
        let db_state = setup.app.state::<crate::db::DbState>();
        let db = db_state.db().expect("banco aberto");
        db.load_history(project_id).expect("historico")
    }

    #[test]
    fn mensagem_entre_agentes_de_um_fluxo_de_canal_leva_canal_e_task() {
        use crate::orchestrator::emit::{emit_message, MessageScope};
        let setup = setup();
        let project_id = history_project(&setup, "proj-escopo-agentes");

        emit_message(
            setup.app.handle(),
            project_id,
            Speaker::from(agent("ember")),
            Speaker::from(agent("vexa")),
            "faça a tela",
            MessageScope {
                channel_id: Some(4),
                item_id: Some("PP-1"),
            },
            None,
            None,
            None,
        )
        .expect("deve emitir e gravar");

        let history = history_of(&setup, project_id);
        match history.as_slice() {
            [crate::events::VokiaEvent::Message { from, to, channel_id, item_id, .. }] => {
                assert_eq!(*from, Speaker::from(agent("ember")));
                assert_eq!(*to, Speaker::from(agent("vexa")));
                assert_eq!(*channel_id, Some(4), "o canal do fluxo vale também entre agentes");
                assert_eq!(item_id.as_deref(), Some("PP-1"));
            }
            other => panic!("esperava uma mensagem, veio {other:?}"),
        }
    }

    #[test]
    fn mensagem_de_fluxo_sem_canal_nem_task_continua_sem_os_dois() {
        use crate::orchestrator::emit::{emit_message, MessageScope};
        let setup = setup();
        let project_id = history_project(&setup, "proj-escopo-vazio");

        emit_message(
            setup.app.handle(),
            project_id,
            Speaker::from(agent("ember")),
            Speaker::from(agent("vexa")),
            "conversa solta",
            MessageScope::default(),
            None,
            None,
            None,
        )
        .expect("deve emitir e gravar");

        match history_of(&setup, project_id).as_slice() {
            [crate::events::VokiaEvent::Message { channel_id: None, item_id: None, .. }] => {}
            other => panic!("esperava mensagem sem canal e sem task, veio {other:?}"),
        }
    }
}

mod waiting_reconcile {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    use serde_json::Value;
    use tauri::{Listener, Manager};

    use super::*;
    use crate::events::{AgentState, DecisionKind, EVENT_NAME};
    use crate::mcp::support::build_app;
    use crate::orchestrator::emit::{emit_state, reconcile_waiting};
    use crate::state::{AppState, Batch, PendingDecision};

    static NEXT_DIR: AtomicU32 = AtomicU32::new(0);
    const PROJECT: i64 = 1;

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
            "vokia-waiting-reconcile-{}-{}",
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

    fn agent(id: &str) -> Agent {
        Agent::from_stored(id).expect("agente")
    }

    fn last_state_for(world: &World, target: &str) -> Option<String> {
        world
            .events
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|event| event["type"] == "agent-state" && event["agent"] == target)
            .and_then(|event| event["state"].as_str().map(str::to_string))
    }

    #[test]
    fn agente_em_waiting_sem_motivo_ativo_volta_para_idle() {
        let world = world();
        let state = world.app.state::<AppState>();
        let kairo = agent("kairo");

        emit_state(world.app.handle(), PROJECT, kairo.clone(), AgentState::Waiting, None).unwrap();
        assert_eq!(last_state_for(&world, "kairo").as_deref(), Some("waiting"));

        reconcile_waiting(world.app.handle(), &state, PROJECT);

        assert_eq!(last_state_for(&world, "kairo").as_deref(), Some("idle"));
        assert!(!state
            .flow
            .lock()
            .unwrap()
            .agent_states
            .contains_key(&(PROJECT, kairo)));
    }

    #[test]
    fn agentes_com_execucao_batch_decisao_ou_turn_waiter_permanecem_waiting_ate_encerrar() {
        let world = world();
        let state = world.app.state::<AppState>();
        let ember = agent("ember");
        let axiom = agent("axiom");
        let vexa = agent("vexa");
        let kairo = agent("kairo");

        for a in [&ember, &axiom, &vexa, &kairo] {
            emit_state(world.app.handle(), PROJECT, a.clone(), AgentState::Waiting, None).unwrap();
        }

        let ember_key = SessionKey {
            project_id: PROJECT,
            agent: ember.clone(),
        };
        {
            let mut flow = state.flow.lock().unwrap();
            flow.pending.insert(ember_key.clone(), VecDeque::new());
            flow.turn_waiters
                .insert(ember_key.clone(), vec![kairo.clone()]);
            flow.batches.insert(
                1,
                Batch {
                    project_id: PROJECT,
                    leader: axiom.clone(),
                    objective: "obj".to_string(),
                    variables: Vec::new(),
                    channel_id: None,
                    item_id: None,
                    pending: 1,
                    results: Vec::new(),
                    dispatched: Vec::new(),
                    started: Vec::new(),
                    returned: Vec::new(),
                    counters: Default::default(),
                },
            );
        }
        state.decisions.lock().unwrap().insert(
            99,
            PendingDecision {
                agent: vexa.clone(),
                project_id: PROJECT,
                kind: DecisionKind::Pergunta,
                title: "t".to_string(),
                request: "r".to_string(),
                item_id: None,
                options: Vec::new(),
            },
        );

        reconcile_waiting(world.app.handle(), &state, PROJECT);
        for name in ["ember", "axiom", "vexa", "kairo"] {
            assert_eq!(
                last_state_for(&world, name).as_deref(),
                Some("waiting"),
                "{name} deveria continuar waiting"
            );
        }

        {
            let mut flow = state.flow.lock().unwrap();
            flow.pending.remove(&ember_key);
            flow.turn_waiters.remove(&ember_key);
            flow.batches.remove(&1);
        }
        state.decisions.lock().unwrap().remove(&99);

        reconcile_waiting(world.app.handle(), &state, PROJECT);
        for name in ["ember", "axiom", "vexa", "kairo"] {
            assert_eq!(
                last_state_for(&world, name).as_deref(),
                Some("idle"),
                "{name} deveria voltar para idle"
            );
        }
    }

    #[test]
    fn abandon_flow_reconcilia_agente_preso_em_waiting() {
        let world = world();
        let state = world.app.state::<AppState>();
        let vexa = agent("vexa");
        let key = SessionKey {
            project_id: PROJECT,
            agent: vexa.clone(),
        };
        state.flow.lock().unwrap().pending.insert(key.clone(), VecDeque::new());
        emit_state(world.app.handle(), PROJECT, vexa, AgentState::Waiting, None).unwrap();

        abandon_flow(world.app.handle(), &state, &key, None);

        assert_eq!(last_state_for(&world, "vexa").as_deref(), Some("idle"));
    }

    #[tokio::test]
    async fn concluir_validacao_final_tira_lider_de_waiting() {
        let world = world();
        let db_state = world.app.state::<DbState>();
        let db = db_state.db().expect("banco aberto");
        let project = db
            .insert_project(&crate::db::NewProject {
                name: "proj-validacao",
                symbol: "folder",
                description: "",
                path: &world.dir.to_string_lossy(),
                origin_kind: "folder",
                origin_repo: None,
            })
            .expect("projeto");
        let created = db
            .create_card(
                project.id,
                &crate::db::NewCard {
                    title: "Card teste".to_string(),
                    description: None,
                    origin: "usuario".to_string(),
                    links: Vec::new(),
                },
            )
            .expect("card");
        db.advance_card(project.id, &created.id, "In review")
            .expect("in review");

        let state = world.app.state::<AppState>();
        let axiom = agent("axiom");
        state.decisions.lock().unwrap().insert(
            7,
            PendingDecision {
                agent: axiom.clone(),
                project_id: project.id,
                kind: DecisionKind::Validacao,
                title: "Validação final".to_string(),
                request: "Card teste".to_string(),
                item_id: Some(created.id),
                options: vec!["Concluir".to_string(), "Devolver".to_string()],
            },
        );
        emit_state(world.app.handle(), project.id, axiom, AgentState::Waiting, None).unwrap();

        answer_decision(world.app.handle(), &state, 7, "Concluir")
            .await
            .expect("concluir validacao");

        assert_eq!(last_state_for(&world, "axiom").as_deref(), Some("idle"));
    }
}

mod parallel_improvements {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    use tauri::Manager;

    use super::*;
    use crate::agents::replica_id;
    use crate::events::AgentState;
    use crate::mcp::support::build_app;
    use crate::state::AppState;

    static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

    fn setup() -> (tauri::App, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "vokia-parallel-improvements-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        (build_app(&dir), dir)
    }

    fn agent(id: &str) -> Agent {
        Agent::from_stored(id).expect("agent")
    }

    #[tokio::test]
    async fn task_lock_por_thread_permite_replicas_e_cards_paralelos_no_mesmo_projeto() {
        let (app, dir) = setup();
        let state = app.state::<AppState>();

        let lock_a = state.sessions.task_lock(1, "agent:axiom:PP-1");
        let lock_b = state.sessions.task_lock(1, "agent:axiom-2:PP-2");
        let _guard_a = lock_a.lock_owned().await;
        let acquired_b = tokio::time::timeout(Duration::from_millis(100), lock_b.lock_owned()).await;
        assert!(acquired_b.is_ok(), "réplica/card distinto não pode esperar task_lock");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replica_ociosa_faz_work_stealing_da_fila_da_instancia_base() {
        let (app, dir) = setup();
        let state = app.state::<AppState>();
        let vexa = agent("vexa");
        let vexa_2 = replica_id(&vexa, 2);
        {
            let mut flow = state.flow.lock().unwrap();
            flow.pending.insert(SessionKey { project_id: 1, agent: vexa.clone() }, VecDeque::new());
            flow.pending.insert(SessionKey { project_id: 1, agent: vexa_2.clone() }, VecDeque::new());
        }
        dispatch_request(
            app.handle(),
            &state,
            1,
            &vexa,
            "pedido-roubavel",
            RequestContext::new(None, None, None),
            2,
            8,
        )
        .unwrap();

        let stolen = next_job(&state, SessionKey { project_id: 1, agent: vexa_2.clone() })
            .expect("next_job")
            .expect("vexa-2 deveria roubar o pedido da fila de vexa");
        assert_eq!(stolen.text, "pedido-roubavel");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn agente_em_waiting_nao_consome_vaga_de_max_parallel() {
        let (app, dir) = setup();
        let state = app.state::<AppState>();
        let axiom = agent("axiom");
        let vexa = agent("vexa");
        {
            let mut flow = state.flow.lock().unwrap();
            flow.pending.insert(SessionKey { project_id: 1, agent: axiom.clone() }, VecDeque::new());
            flow.pending.insert(SessionKey { project_id: 1, agent: vexa.clone() }, VecDeque::new());
            flow.agent_states.insert((1, axiom), AgentState::Waiting);
        }

        dispatch_request(
            app.handle(),
            &state,
            1,
            &vexa,
            "segundo-worker",
            RequestContext::new(None, None, None),
            2,
            2,
        )
        .unwrap();

        let flow = state.flow.lock().unwrap();
        assert!(
            flow.waiting.get(&1).is_none_or(VecDeque::is_empty),
            "vexa-2 deveria iniciar sem ir para waiting quando axiom está apenas Waiting"
        );
        assert!(flow.pending.contains_key(&SessionKey { project_id: 1, agent: replica_id(&vexa, 2) }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ember_usa_multiplas_instancias_quando_ocupado() {
        let (app, dir) = setup();
        let state = app.state::<AppState>();
        let ember = Agent::ember();
        assert_eq!(pick_ember_instance(&state, 1), ember);

        state
            .flow
            .lock()
            .unwrap()
            .pending
            .insert(SessionKey { project_id: 1, agent: ember.clone() }, VecDeque::new());
        assert_eq!(pick_ember_instance(&state, 1), replica_id(&ember, 2));

        state
            .flow
            .lock()
            .unwrap()
            .pending
            .insert(SessionKey { project_id: 1, agent: replica_id(&ember, 2) }, VecDeque::new());
        assert_eq!(pick_ember_instance(&state, 1), replica_id(&ember, 3));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn queue_metrics_reporta_filas_e_esperas_com_motivo() {
        let (app, dir) = setup();
        let state = app.state::<AppState>();
        let vexa = agent("vexa");
        {
            let mut flow = state.flow.lock().unwrap();
            flow.pending.insert(SessionKey { project_id: 1, agent: vexa.clone() }, VecDeque::new());
        }
        dispatch_request(
            app.handle(),
            &state,
            1,
            &vexa,
            "espera-teto",
            RequestContext::new(None, Some("PP-9".to_string()), None),
            2,
            1,
        )
        .unwrap();

        let metrics = queue_metrics(&state).expect("queue_metrics");
        assert_eq!(metrics.active_instances, 1);
        assert_eq!(metrics.total_waiting, 1);
        assert_eq!(metrics.waiting_delegations[0].reason, WaitingReason::MaxParallel);
        assert_eq!(metrics.waiting_delegations[0].item_id.as_deref(), Some("PP-9"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn refinery_lock_e_isolado_por_projeto() {
        let (app, dir) = setup();
        let state = app.state::<AppState>();
        let r1 = state.sessions.refinery_lock(1);
        let r2 = state.sessions.refinery_lock(2);
        let _g1 = r1.lock_owned().await;
        let g2 = tokio::time::timeout(Duration::from_millis(100), r2.lock_owned()).await;
        assert!(g2.is_ok(), "refinery_lock de projetos diferentes não deve bloquear");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
