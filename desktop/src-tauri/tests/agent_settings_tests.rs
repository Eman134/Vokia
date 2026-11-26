use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::json;
use tauri::Manager;

use super::run;
use crate::agents::{
    should_notify_agent, Agent, AgentDraft, AgentError, AgentProfile, AgentRegistry,
    NotificationKind,
};
use crate::claude::{agent_command, Access, ProjectAccess};
use crate::mcp::support::build_app;
use crate::orchestrator::adapters::{antigravity, codex};
use crate::permissions::PermissionLevel;
use crate::providers::ProviderRegistry;

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

fn temp_dir(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vokia-{prefix}-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn agent_settings_persist_new_fields_and_load_legacy_json() {
    let dir = temp_dir("agent-settings-persist");
    let providers = ProviderRegistry::seeded();

    // 1. Criar agente com todos os novos campos
    let registry = AgentRegistry::load(&dir);
    let draft = AgentDraft {
        name: "Lyra".to_string(),
        role: "Tech Lead".to_string(),
        avatar: "onix".to_string(),
        cli: "claude".to_string(),
        model: "sonnet".to_string(),
        command: "claude".to_string(),
        title: Some("  Arquiteta de APIs  ".to_string()),
        description: Some("Responsável por contratos HTTP e revisão técnica.".to_string()),
        chat_color: Some("project-teal".to_string()),
        extra_dirs: vec![
            " C:/repos/shared ".to_string(),
            "C:/repos/shared".to_string(),
            "".to_string(),
            "D:/docs".to_string(),
        ],
        notification_mode: Some("needs_me".to_string()),
        backup_mode: Some("auto".to_string()),
        ..Default::default()
    };
    let created = registry.create(&draft, &providers).expect("create lyra");
    assert_eq!(created.title.as_deref(), Some("Arquiteta de APIs"));
    assert_eq!(
        created.description.as_deref(),
        Some("Responsável por contratos HTTP e revisão técnica.")
    );
    assert_eq!(created.chat_color.as_deref(), Some("project-teal"));
    assert_eq!(created.extra_dirs, vec!["C:/repos/shared", "D:/docs"]);
    assert_eq!(created.notification_mode.as_deref(), Some("needs_me"));
    assert_eq!(created.backup_mode.as_deref(), Some("auto"));

    // 2. Recarregar do disco e conferir persistência
    let reloaded = AgentRegistry::load(&dir);
    let team = reloaded.team().unwrap();
    let stored = team.profile(&created.id).expect("lyra reloaded");
    assert_eq!(stored, &created);

    // 3. Validar rejeição de valores inválidos
    let bad_notif = AgentDraft {
        name: "BadNotif".to_string(),
        avatar: "ambar".to_string(),
        notification_mode: Some("invalid".to_string()),
        ..draft.clone()
    };
    assert!(matches!(
        reloaded.create(&bad_notif, &providers),
        Err(AgentError::InvalidNotificationMode(_))
    ));

    let bad_backup = AgentDraft {
        name: "BadBackup".to_string(),
        avatar: "ambar".to_string(),
        backup_mode: Some("nope".to_string()),
        ..draft
    };
    assert!(matches!(
        reloaded.create(&bad_backup, &providers),
        Err(AgentError::InvalidBackupMode(_))
    ));

    // 4. Retrocompatibilidade: JSON legado sem os novos campos carrega normalmente
    let legacy_dir = temp_dir("agent-settings-legacy");
    fs::write(
        legacy_dir.join("agents.json"),
        serde_json::to_string_pretty(&json!({
            "version": 1,
            "rosterMigrated": true,
            "agents": [{
                "id": "axiom",
                "name": "Axiom",
                "role": "Tech Lead",
                "avatar": "quarzo",
                "cli": "claude",
                "model": "haiku",
                "command": "claude",
                "followDefaultLlm": true,
                "fallback": []
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let legacy_registry = AgentRegistry::load(&legacy_dir);
    let legacy_team = legacy_registry.team().unwrap();
    let axiom = legacy_team.profile(&Agent::axiom()).expect("axiom");
    assert_eq!(axiom.title, None);
    assert_eq!(axiom.description, None);
    assert_eq!(axiom.chat_color, None);
    assert!(axiom.extra_dirs.is_empty());
    assert_eq!(axiom.notification_mode, None);
    assert_eq!(axiom.backup_mode, None);

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&legacy_dir);
}

#[test]
fn dispatcher_update_agent_persists_new_fields_and_returns_in_list_agents() {
    let dir = temp_dir("dispatcher-agent-settings");
    let app = build_app(&dir);

    tauri::async_runtime::block_on(async {
        let updated = run(
            app.handle(),
            None,
            None,
            "update_agent",
            json!({
                "id": "axiom",
                "draft": {
                    "name": "Axiom",
                    "role": "Tech Lead",
                    "avatar": "quarzo",
                    "cli": "claude",
                    "model": "sonnet",
                    "command": "claude",
                    "followDefaultLlm": false,
                    "fallback": [],
                    "title": "Líder técnico",
                    "description": "Coordena revisões e decisões de arquitetura",
                    "chatColor": "accent",
                    "extraDirs": ["C:/extra/one", "C:/extra/two"],
                    "notificationMode": "mute",
                    "backupMode": "ask"
                }
            }),
        )
        .await
        .expect("update_agent via dispatcher");

        assert_eq!(updated["title"], "Líder técnico");
        assert_eq!(
            updated["description"],
            "Coordena revisões e decisões de arquitetura"
        );
        assert_eq!(updated["chatColor"], "accent");
        assert_eq!(
            updated["extraDirs"],
            json!(["C:/extra/one", "C:/extra/two"])
        );
        assert_eq!(updated["notificationMode"], "mute");
        assert_eq!(updated["backupMode"], "ask");

        let listed = run(app.handle(), None, None, "list_agents", json!({}))
            .await
            .expect("list_agents via dispatcher");
        let agents = listed["agents"].as_array().expect("agents array");
        let axiom = agents
            .iter()
            .find(|a| a["id"] == "axiom")
            .expect("axiom in list_agents");
        assert_eq!(axiom["title"], "Líder técnico");
        assert_eq!(
            axiom["extraDirs"],
            json!(["C:/extra/one", "C:/extra/two"])
        );
        assert_eq!(axiom["notificationMode"], "mute");
        assert_eq!(axiom["backupMode"], "ask");
    });

    // Também verifica no AgentRegistry diretamente
    let registry = app.state::<AgentRegistry>();
    let team = registry.team().unwrap();
    let profile = team.profile(&Agent::axiom()).unwrap();
    assert_eq!(profile.notification_mode.as_deref(), Some("mute"));
    assert_eq!(profile.extra_dirs, vec!["C:/extra/one", "C:/extra/two"]);

    drop(app);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn extra_dirs_are_passed_as_add_dir_to_adapters() {
    let extra = vec![
        "C:/shared/lib".to_string(),
        "D:/reference/specs".to_string(),
    ];
    let access = Access::Project(ProjectAccess {
        dir: Path::new("C:/project"),
        attachments: Path::new("C:/attachments"),
        extra_dirs: &extra,
        level: PermissionLevel::Edicao,
        builtin_tools: true,
    });

    let assert_has_extra_dirs = |cmd: &tokio::process::Command| {
        let args: Vec<String> = cmd
            .as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        for expected in &extra {
            let found = args
                .windows(2)
                .any(|pair| pair[0] == "--add-dir" && &pair[1] == expected);
            assert!(
                found,
                "esperava --add-dir {expected} nos argumentos: {args:?}"
            );
        }
    };

    let claude_cmd = agent_command(
        Path::new("claude"),
        &[],
        &[],
        "role",
        "sonnet",
        access,
        None,
    );
    assert_has_extra_dirs(&claude_cmd);

    let codex_cmd = codex::build_command(Path::new("codex"), &[], "gpt-6-luna", access, None);
    assert_has_extra_dirs(&codex_cmd);

    let agy_cmd = antigravity::build_command_with_resume(
        Path::new("agy"),
        &[],
        "role",
        "gemini-4-hydrogen-medium",
        access,
        "msg",
        None,
    );
    assert_has_extra_dirs(&agy_cmd);
}

#[test]
fn notification_mode_controls_native_notification_eligibility() {
    let make_profile = |mode: Option<&str>| -> AgentProfile {
        AgentProfile {
            id: Agent::axiom(),
            name: "Axiom".to_string(),
            role: "Tech Lead".to_string(),
            avatar: "quarzo".to_string(),
            cli: "claude".to_string(),
            model: "sonnet".to_string(),
            model_tier: None,
            reasoning_effort: None,
            follow_default_llm: false,
            fallback: Vec::new(),
            command: "claude".to_string(),
            builtin: false,
            extra_instructions: String::new(),
            title: None,
            description: None,
            chat_color: None,
            extra_dirs: Vec::new(),
            notification_mode: mode.map(str::to_string),
            backup_mode: None,
        }
    };

    let default_profile = make_profile(Some("default"));
    assert!(should_notify_agent(
        Some(&default_profile),
        NotificationKind::TurnFinished
    ));
    assert!(should_notify_agent(
        Some(&default_profile),
        NotificationKind::NeedsUser
    ));

    let needs_me_profile = make_profile(Some("needs_me"));
    assert!(!should_notify_agent(
        Some(&needs_me_profile),
        NotificationKind::TurnFinished
    ));
    assert!(should_notify_agent(
        Some(&needs_me_profile),
        NotificationKind::NeedsUser
    ));

    let muted_profile = make_profile(Some("mute"));
    assert!(!should_notify_agent(
        Some(&muted_profile),
        NotificationKind::TurnFinished
    ));
    assert!(!should_notify_agent(
        Some(&muted_profile),
        NotificationKind::NeedsUser
    ));

    // Sem perfil ou notification_mode None segue o padrão do app
    assert!(should_notify_agent(None, NotificationKind::TurnFinished));
}
