use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::agents::{replica_id, Agent};
use crate::claude::{LimitName, UsageLimit};
use crate::db::{Db, DbError, NewProject, SETTING_AUTO_PAUSE_ON_LIMIT};
use crate::mcp::support::build_app;
use crate::session_key::SessionKey;

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

struct Setup {
    app: tauri::App,
    dir: std::path::PathBuf,
}

impl Drop for Setup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn setup(name: &str) -> Setup {
    let dir = std::env::temp_dir().join(format!(
        "vokia-auto-pause-{name}-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("diretório temporário");
    Setup {
        app: build_app(&dir),
        dir,
    }
}

fn create_project(db: &Db, name: &str, path: &std::path::Path) -> i64 {
    db.insert_project(&NewProject {
        name,
        symbol: "folder",
        description: "",
        path: &path.to_string_lossy(),
        origin_kind: "folder",
        origin_repo: None,
    })
    .expect("projeto criado")
    .id
}

#[test]
fn setting_auto_pause_on_limit_e_desativada_por_padrao_e_valida_booleano() {
    let setup = setup("setting");
    let db_state = setup.app.state::<DbState>();
    let db = db_state.require().unwrap();

    assert!(!db.auto_pause_on_limit().unwrap());
    let loaded = db.load_settings().unwrap();
    assert_eq!(
        loaded.get(SETTING_AUTO_PAUSE_ON_LIMIT).map(String::as_str),
        Some("false")
    );

    db.save_setting(SETTING_AUTO_PAUSE_ON_LIMIT, "true").unwrap();
    assert!(db.auto_pause_on_limit().unwrap());

    db.save_setting(SETTING_AUTO_PAUSE_ON_LIMIT, "false").unwrap();
    assert!(!db.auto_pause_on_limit().unwrap());

    let invalid = db.save_setting(SETTING_AUTO_PAUSE_ON_LIMIT, "maybe");
    assert!(matches!(invalid, Err(DbError::InvalidSetting(_, _))));
}

#[test]
fn pausa_do_projeto_cobre_agentes_base_e_replicas_e_isola_projetos() {
    let setup = setup("pauses");
    let db_state = setup.app.state::<DbState>();
    let db = db_state.require().unwrap();
    let p1 = create_project(&db, "p1", &setup.dir.join("p1"));
    let p2 = create_project(&db, "p2", &setup.dir.join("p2"));
    let kairo = Agent::from_stored("kairo").unwrap();
    let vexa = Agent::vexa();
    let kairo_replica = replica_id(&kairo, 2);

    db.pause_project_agents(p1, &[kairo.clone(), vexa.clone()])
        .unwrap();

    assert!(db.is_agent_paused(p1, &kairo).unwrap());
    assert!(db.is_agent_paused(p1, &vexa).unwrap());
    assert!(db.is_agent_paused(p1, &kairo_replica).unwrap());
    assert!(!db.is_agent_paused(p2, &kairo).unwrap());

    db.resume_project_agents(p1).unwrap();
    assert!(!db.is_agent_paused(p1, &kairo).unwrap());
    assert!(!db.is_agent_paused(p1, &kairo_replica).unwrap());
}

#[test]
fn pause_run_pausa_todos_os_agentes_do_projeto_e_interrompe_o_run() {
    let setup = setup("pause-run");
    let db_state = setup.app.state::<DbState>();
    let db = db_state.require().unwrap();
    let project_id = create_project(&db, "p1", &setup.dir);
    let run_id = db
        .start_run(project_id, "kairo", "Implementar API", None, Some("item-51"), 1_000)
        .unwrap();
    let key = SessionKey {
        project_id,
        agent: Agent::from_stored("kairo").unwrap(),
    };
    let limit = UsageLimit {
        window: LimitName::FiveHour,
        resets_at: 0,
    };

    pause_run(setup.app.handle(), &key, Some(run_id), limit).unwrap();

    let interrupted = db.interrupted_runs().unwrap();
    assert!(interrupted.iter().any(|run| run.id == run_id));
    for agent in project_team_agents(setup.app.handle()) {
        assert!(
            db.is_agent_paused(project_id, &agent).unwrap(),
            "agente {agent} deveria estar pausado"
        );
    }
}
