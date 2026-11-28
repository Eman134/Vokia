use super::*;
use crate::task::TaskKind;

fn agent(id: &str) -> Agent {
    Agent::from_stored(id).expect("agente")
}

fn definition() -> TaskDefinition {
    TaskDefinition {
        kind: TaskKind::Demanda,
        nova_tarefa: true,
        objetivo: "teste".to_string(),
        criterion: String::new(),
        verify: false,
        participants: vec![agent("axiom"), agent("vexa")],
        alternate: false,
        variables: Vec::new(),
        stop: None,
    }
}

#[test]
fn cls1_demanda_promovida_dinamicamente_quando_a_definicao_ganha_estrutura() {
    let origin = agent("axiom");
    let mut conversation = TaskDefinition {
        kind: TaskKind::Conversa,
        nova_tarefa: false,
        objetivo: "O que esse projeto faz?".to_string(),
        criterion: String::new(),
        verify: false,
        participants: vec![origin.clone()],
        alternate: false,
        variables: Vec::new(),
        stop: None,
    };

    assert!(!conversation.has_task_structure());
    conversation.participants.push(agent("vexa"));
    assert!(conversation.has_task_structure());
    conversation.promote_to_demand();
    assert_eq!(conversation.kind, TaskKind::Demanda);
}

#[test]
fn delegacao_abre_turno_do_helper_com_o_requisitante_na_fila() {
    let definition = definition();
    let axiom = Turn::new(agent("axiom"), "pedido", "pedido", Vec::new());
    let delegation = (agent("vexa"), "vexa oi".to_string());

    let next = next_turn(&definition, &axiom, "Oi", Some(&delegation)).expect("turno do helper");

    assert!(next.speaker == agent("vexa"));
    assert_eq!(next.received, "vexa oi");
    assert!(next.requesters.len() == 1 && next.requesters[0] == agent("axiom"));
}

#[test]
fn fala_com_delegacao_vai_a_quem_pediu_e_nao_ao_helper() {
    let definition = definition();
    let axiom = Turn::new(agent("axiom"), "pedido", "pedido", Vec::new());
    let delegation = (agent("vexa"), "vexa oi".to_string());
    let next = next_turn(&definition, &axiom, "Oi", Some(&delegation));

    assert!(speech_recipient(&definition, &axiom, next.as_ref()) == Speaker::User);
}

#[test]
fn resposta_do_helper_sem_proximo_turno_entrega_ao_usuario_e_nao_ao_requisitante() {
    let definition = definition();
    let vexa = Turn::new(agent("vexa"), "vexa oi", "vexa oi", vec![agent("axiom")]);

    let next = next_turn(&definition, &vexa, "Oi! Em que posso ajudar?", None);

    assert!(next.is_none());
    assert!(speech_recipient(&definition, &vexa, None) == Speaker::User);
}

#[test]
fn fala_intermediaria_em_cadeia_sincrona_responde_ao_requisitante() {
    let definition = definition();
    let vexa = Turn::new(agent("vexa"), "vexa oi", "vexa oi", vec![agent("axiom")]);
    let delegation = (agent("gitta"), "gitta confere".to_string());
    let next = next_turn(&definition, &vexa, "Vou confirmar com a Gitta.", Some(&delegation));

    assert!(next.is_some());
    assert!(speech_recipient(&definition, &vexa, next.as_ref()) == Speaker::from(agent("axiom")));
    assert!(
        final_recipient(&definition, &vexa, next.as_ref(), Some(&Agent::ember()))
            == Speaker::from(agent("axiom"))
    );
}

#[test]
fn sem_delegacao_nem_requisitante_a_fala_vai_ao_usuario_e_a_conversa_termina() {
    let definition = definition();
    let axiom = Turn::new(agent("axiom"), "pedido", "pedido", Vec::new());

    assert!(next_turn(&definition, &axiom, "Oi", None).is_none());
    assert!(speech_recipient(&definition, &axiom, None) == Speaker::User);
}

// Ember→Axiom→User e Ember→Axiom→Gitta→User: o fim de cadeia entrega ao usuário mesmo com reply_to ou requesters.
#[test]
fn fim_de_cadeia_delegada_pelo_ember_entrega_ao_usuario() {
    let definition = definition();
    let ember = Agent::ember();
    let axiom = Turn::new(agent("axiom"), "release", "release", Vec::new());
    let gitta = Turn::new(agent("gitta"), "release", "release", vec![agent("axiom")]);

    assert!(final_recipient(&definition, &axiom, None, Some(&ember)) == Speaker::User);
    assert!(final_recipient(&definition, &gitta, None, Some(&ember)) == Speaker::User);
    assert!(final_recipient(&definition, &axiom, None, None) == Speaker::User);
}

#[test]
fn modo_alternado_mantem_revezamento_e_entrega_ao_usuario_no_fim() {
    let mut definition = definition();
    definition.alternate = true;
    let ember = Agent::ember();
    let axiom = Turn::new(agent("axiom"), "pedido", "pedido", Vec::new());
    let next = next_turn(&definition, &axiom, "Minha vez.", None).expect("próximo do revezamento");

    assert!(next.speaker == agent("vexa"));
    assert!(
        final_recipient(&definition, &axiom, Some(&next), Some(&ember))
            == Speaker::from(agent("vexa"))
    );
    assert!(final_recipient(&definition, &axiom, None, Some(&ember)) == Speaker::User);
}

#[test]
fn contexto_gravado_antes_do_reply_to_ainda_carrega() {
    let legacy = r#"{"run_id":null,"item_id":"item-28","channel_id":null,"parallel":false,"batch":null,"attachments":[],"resumed_run":null,"ping_seconds":null}"#;
    let context: crate::state::RequestContext = serde_json::from_str(legacy).expect("contexto antigo");

    assert!(context.reply_to.is_none());
}

#[test]
fn cls1_settlement_ended_persiste_task_result_somente_para_demanda_ou_conversa_promovida() {
    use std::path::{Path, PathBuf};

    use crate::agents::AgentRegistry;
    use crate::claude::ProjectAccess;
    use crate::db::{DbState, NewProject};
    use crate::permissions::PermissionLevel;
    use crate::session_key::SessionKey;
    use crate::state::AppState;
    use crate::task::{TaskOutcome, TaskKind};
    use tauri::Manager;

    async fn finish_as_ended(
        app: &tauri::App,
        team: &crate::agents::Team,
        origin: &Agent,
        project_id: i64,
        definition: &TaskDefinition,
        project_dir: &Path,
        attachments: &Path,
    ) {
        let state = app.state::<AppState>();
        let exchange = Exchange {
            app: app.handle(),
            state: &state,
            cli: Path::new("claude"),
            project: ProjectAccess {
                dir: project_dir,
                attachments,
                extra_dirs: &[],
                level: PermissionLevel::Edicao,
                builtin_tools: true,
            },
            project_context: "",
            request: "Oi",
            item_id: None,
            channel_id: None,
            key: SessionKey { project_id, agent: origin.clone() },
            definition,
            team,
            orchestration: None,
            run_id: None,
            reply_to: None,
        };
        let mut counters = Counters::default();
        counters.kind = definition.kind;
        let mut transcript = Vec::new();

        let step = resolve(&exchange, &mut transcript, &mut counters, origin, Trigger::EndOfChain)
            .await
            .expect("Settlement::Ended deveria concluir o turno");
        assert!(matches!(step, Step::End(TaskOutcome::Finished)));
    }

    let data_dir = std::env::temp_dir().join(format!(
        "vokia-ended-task-result-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&data_dir).unwrap();
    let app = crate::mcp::support::build_app(&data_dir);
    let team = app.state::<AgentRegistry>().team().expect("time de teste");
    let origin = Agent::ember();
    let db_state = app.state::<DbState>();
    let db = db_state.db().expect("banco de teste");
    let add_project = |name: &str| {
        let project_path = format!("C:/test/{name}");
        db.insert_project(&NewProject {
            name,
            symbol: "folder",
            description: "",
            path: &project_path,
            origin_kind: "folder",
            origin_repo: None,
        })
        .expect("projeto de teste")
        .id
    };
    let conversation_id = add_project("conversa");
    let demand_id = add_project("demanda");
    let promoted_id = add_project("promovida");
    let project_dir = PathBuf::from("C:/test/project");
    let attachments = PathBuf::from("C:/test/attachments");

    let make_definition = |kind| TaskDefinition {
        kind,
        nova_tarefa: false,
        objetivo: "Oi".to_string(),
        criterion: String::new(),
        verify: false,
        participants: vec![origin.clone()],
        alternate: false,
        variables: Vec::new(),
        stop: None,
    };
    let conversation = make_definition(TaskKind::Conversa);
    let demand = make_definition(TaskKind::Demanda);
    let mut promoted = make_definition(TaskKind::Conversa);
    promoted.promote_to_demand();

    crate::mcp::support::run(async {
        finish_as_ended(
            &app,
            &team,
            &origin,
            conversation_id,
            &conversation,
            &project_dir,
            &attachments,
        )
        .await;
        finish_as_ended(&app, &team, &origin, demand_id, &demand, &project_dir, &attachments).await;
        finish_as_ended(&app, &team, &origin, promoted_id, &promoted, &project_dir, &attachments).await;
    });

    assert!(db.load_history(conversation_id).unwrap().is_empty());
    for project_id in [demand_id, promoted_id] {
        let history = db.load_history(project_id).unwrap();
        assert!(matches!(
            history.as_slice(),
            [VokiaEvent::TaskResult { objective, concluded: true, .. }] if objective == "Oi"
        ));
    }
    drop(db_state);
    drop(app);
    let _ = std::fs::remove_dir_all(&data_dir);
}
