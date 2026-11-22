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
