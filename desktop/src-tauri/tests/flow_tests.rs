use super::*;

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
            Some(9),
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
}
