use super::*;
use crate::agents::Agent;
use crate::db::{DbState, NewProject};
use crate::mcp::support;
use tauri::Manager;

#[test]
fn tok4_provider_sessions_are_scoped_and_prefixed_ids_are_not_claude_resumes() {
    let session_id = "sid:with:colon";
    let stored = encode_provider_session("opencode", "model-a", Some("item-1"), session_id);

    assert_eq!(stored, "opencode:model-a:item-1:sid:with:colon");
    assert_eq!(
        decode_provider_session(&stored, "opencode", "model-a", Some("item-1")),
        Some(session_id.to_string())
    );
    assert_eq!(decode_provider_session(&stored, "opencode", "model-a", Some("item-2")), None);
    assert_eq!(decode_provider_session(&stored, "opencode", "model-a", None), None);
    assert_eq!(decode_provider_session(&stored, "antigravity", "model-a", Some("item-1")), None);
    assert_eq!(decode_provider_session(&stored, "opencode", "model-b", Some("item-1")), None);

    let data_dir = std::env::temp_dir().join(format!(
        "vokia-claude-session-filter-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&data_dir).unwrap();
    let app = support::build_app(&data_dir);
    let project_id = app
        .state::<DbState>()
        .db()
        .expect("banco de teste")
        .insert_project(&NewProject {
            name: "sessao",
            symbol: "folder",
            description: "",
            path: "C:/test/project",
            origin_kind: "folder",
            origin_repo: None,
        })
        .expect("projeto de teste")
        .id;
    let store = SessionStore {
        app: app.handle(),
        key: SessionKey { project_id, agent: Agent::ember() },
    };
    store.save(&stored).expect("sessão prefixada deve ser gravada");
    let mut session = AgentSession::default();
    let place = Place::for_preload(PathBuf::from("C:/test/project"));
    let provider_key = ("claude".to_string(), "model-a".to_string());

    let attempt = support::run(session.first_attempt(&store, &place, &provider_key))
        .expect("o filtro de sessão Claude deve ser avaliado");

    assert!(matches!(attempt, Some(Attempt::Open)));
    assert_eq!(session.session_id, None);
    drop(store);
    drop(app);
    let _ = std::fs::remove_dir_all(&data_dir);
}

#[test]
fn ember_lider_geral_e_lideres_de_time_coordenam_e_os_demais_trabalham() {
    use crate::db::{OrchestrationSettings, OrchestrationSnapshot, Team as OrchTeam};
    use crate::mcp::ToolScope;

    let team = |id: &str, leader: &str| OrchTeam {
        id: id.into(),
        name: id.into(),
        position: 0,
        leader_agent: Some(leader.into()),
        instructions: String::new(),
        final_validation: String::new(),
        start_x: None,
        start_y: None,
        end_x: None,
        end_y: None,
        judge_full_flow: false,
    };
    let snapshot = OrchestrationSnapshot {
        settings: OrchestrationSettings {
            leader_agent: "axiom".to_string(),
            ..Default::default()
        },
        layers: Vec::new(),
        teams: vec![team("produto", "triage")],
        placements: Vec::new(),
        edges: Vec::new(),
        gateways: Vec::new(),
        task_stages: Vec::new(),
    };
    let agent = |id: &str| Agent::from_stored(id).expect("agente");

    assert_eq!(tool_scope(Some(&snapshot), &Agent::ember()), ToolScope::Coordinator);
    assert_eq!(tool_scope(Some(&snapshot), &agent("axiom")), ToolScope::Coordinator);
    assert_eq!(tool_scope(Some(&snapshot), &agent("triage")), ToolScope::Coordinator);
    assert_eq!(tool_scope(Some(&snapshot), &agent("kairo")), ToolScope::Worker);
    assert_eq!(tool_scope(None, &agent("axiom")), ToolScope::Coordinator, "sem mapa o líder padrão é o Axiom");
    assert_eq!(tool_scope(None, &agent("kairo")), ToolScope::Worker);
}

#[test]
fn detect_previous_provider_identifica_troca_sem_falso_positivo_no_resume_claude() {
    let claude = ("claude".to_string(), "sonnet".to_string());
    let codex = ("codex".to_string(), "gpt-5.3-codex".to_string());
    let claude_opus = ("claude".to_string(), "opus".to_string());

    // Retomada normal do mesmo provedor Claude (session_id nu sem ':') não é troca.
    assert_eq!(
        detect_previous_provider(None, None, Some("sess-claude-123"), &claude),
        None
    );
    // Troca de Claude (session_id nu) para Codex é detectada.
    assert_eq!(
        detect_previous_provider(None, None, Some("sess-claude-123"), &codex),
        Some(("claude".to_string(), String::new()))
    );
    // Fallback imediato (failed_from) tem prioridade.
    assert_eq!(
        detect_previous_provider(Some(&claude), None, None, &codex),
        Some(claude.clone())
    );
    // Troca de modelo dentro do mesmo provedor via sessão ativa.
    assert_eq!(
        detect_previous_provider(None, Some(&claude), None, &claude_opus),
        Some(claude.clone())
    );
    // Sessão prefixada de outro provedor detecta troca.
    assert_eq!(
        detect_previous_provider(
            None,
            None,
            Some("codex:gpt-5.3-codex:item-1:sid-1"),
            &claude
        ),
        Some(codex)
    );
}

#[test]
fn record_provider_switch_registra_origem_e_destino_na_atividade_do_card() {
    use crate::db::NewCard;

    let data_dir = std::env::temp_dir().join(format!(
        "vokia-provider-switch-card-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&data_dir).unwrap();
    let app = support::build_app(&data_dir);
    let db_state = app.state::<DbState>();
    let db = db_state.db().expect("banco de teste");
    let project = db
        .insert_project(&NewProject {
            name: "handoff-card",
            symbol: "folder",
            description: "",
            path: "C:/test/handoff-card",
            origin_kind: "folder",
            origin_repo: None,
        })
        .expect("projeto");
    let card = db
        .create_card(
            project.id,
            &NewCard {
                title: "Troca de provedor".into(),
                description: Some("Card de teste".into()),
                origin: "usuario".into(),
                links: Vec::new(),
            },
        )
        .expect("card");

    record_provider_switch_card_activity(
        app.handle(),
        project.id,
        &card.id,
        &("claude".to_string(), "sonnet".to_string()),
        &("codex".to_string(), "gpt-5.3-codex".to_string()),
        Some(OverrideReason::UsageLimit),
    );

    let updated = db.read_card(project.id, &card.id).expect("card atualizado");
    let last = updated.activity.last().expect("atividade registrada");
    assert!(
        last.text.contains("claude (sonnet)")
            && last.text.contains("codex (gpt-5.3-codex)")
            && last.text.contains("limite de uso"),
        "atividade inesperada: {}",
        last.text
    );

    drop(app);
    let _ = std::fs::remove_dir_all(&data_dir);
}
