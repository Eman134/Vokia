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
}
