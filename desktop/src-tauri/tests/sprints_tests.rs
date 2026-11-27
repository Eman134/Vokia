use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use rusqlite::{params, Connection};

use super::{SPRINT_STATUS_ACTIVE, SPRINT_STATUS_CLOSED, SPRINT_STATUS_PLANNED};
use crate::db::{BacklogItem, Db, DbError, NewCard, NewProject, NewSprint, SprintPatch};

const DB_FILE_NAME: &str = "vokia.db";

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        TempDir(std::env::temp_dir().join(format!(
            "vokia-sprints-test-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::SeqCst)
        )))
    }

    fn open(&self) -> Db {
        Db::open(&self.0).expect("o banco de teste deveria abrir")
    }

    fn raw(&self) -> Connection {
        Connection::open(self.0.join(DB_FILE_NAME)).expect("a conexão direta deveria abrir")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn create_project(db: &Db, name: &str, path: &str) -> i64 {
    db.insert_project(&NewProject {
        name,
        symbol: "folder",
        description: "",
        path,
        origin_kind: "folder",
        origin_repo: None,
    })
    .expect("o projeto deveria ser criado")
    .id
}

fn create_card(db: &Db, project_id: i64, title: &str) -> BacklogItem {
    db.create_card(
        project_id,
        &NewCard {
            title: title.to_string(),
            description: None,
            origin: "usuario".to_string(),
            links: Vec::new(),
        },
    )
    .expect("o card deveria ser criado")
}

#[test]
fn active_sprint_is_unique_per_project() {
    let dir = TempDir::new();
    let db = dir.open();
    let p1 = create_project(&db, "p1", "C:/projetos/p1");
    let p2 = create_project(&db, "p2", "C:/projetos/p2");

    let first = db
        .create_sprint(
            p1,
            NewSprint {
                name: "Sprint 1".to_string(),
                goal: None,
                starts_at: None,
                ends_at: None,
                status: Some(SPRINT_STATUS_ACTIVE.to_string()),
            },
        )
        .expect("primeira sprint ativa deveria ser criada");
    assert_eq!(first.id, "sprint-1");
    assert_eq!(first.status, SPRINT_STATUS_ACTIVE);

    let dup_create = db.create_sprint(
        p1,
        NewSprint {
            name: "Sprint 2".to_string(),
            goal: None,
            starts_at: None,
            ends_at: None,
            status: Some(SPRINT_STATUS_ACTIVE.to_string()),
        },
    );
    assert!(matches!(dup_create, Err(DbError::ActiveSprintExists(ref id)) if id == "sprint-1"));

    let planned = db
        .create_sprint(
            p1,
            NewSprint {
                name: "Sprint 2".to_string(),
                goal: None,
                starts_at: None,
                ends_at: None,
                status: Some(SPRINT_STATUS_PLANNED.to_string()),
            },
        )
        .expect("sprint planejada deveria ser criada");

    let dup_update = db.update_sprint(
        p1,
        &planned.id,
        SprintPatch {
            status: Some(SPRINT_STATUS_ACTIVE.to_string()),
            ..Default::default()
        },
    );
    assert!(matches!(dup_update, Err(DbError::ActiveSprintExists(ref id)) if id == "sprint-1"));

    let other_project_active = db.create_sprint(
        p2,
        NewSprint {
            name: "Sprint P2".to_string(),
            goal: None,
            starts_at: None,
            ends_at: None,
            status: Some(SPRINT_STATUS_ACTIVE.to_string()),
        },
    );
    assert!(other_project_active.is_ok());
}

#[test]
fn invalid_sprint_period_is_rejected() {
    let dir = TempDir::new();
    let db = dir.open();
    let p1 = create_project(&db, "p1", "C:/projetos/p1");

    let err_create = db.create_sprint(
        p1,
        NewSprint {
            name: "Sprint Inválida".to_string(),
            goal: None,
            starts_at: Some(2000),
            ends_at: Some(1000),
            status: None,
        },
    );
    assert!(matches!(err_create, Err(DbError::InvalidSprintPeriod)));

    let valid = db
        .create_sprint(
            p1,
            NewSprint {
                name: "Sprint Válida".to_string(),
                goal: None,
                starts_at: Some(1000),
                ends_at: Some(2000),
                status: None,
            },
        )
        .expect("sprint válida deveria ser criada");

    let err_update = db.update_sprint(
        p1,
        &valid.id,
        SprintPatch {
            ends_at: Some(Some(500)),
            ..Default::default()
        },
    );
    assert!(matches!(err_update, Err(DbError::InvalidSprintPeriod)));
}

#[test]
fn close_sprint_migrates_only_pending_top_level_cards() {
    let dir = TempDir::new();
    let db = dir.open();
    let p1 = create_project(&db, "p1", "C:/projetos/p1");

    let s1 = db
        .create_sprint(
            p1,
            NewSprint {
                name: "Sprint 1".to_string(),
                goal: None,
                starts_at: None,
                ends_at: None,
                status: Some(SPRINT_STATUS_ACTIVE.to_string()),
            },
        )
        .unwrap();
    let s2 = db
        .create_sprint(
            p1,
            NewSprint {
                name: "Sprint 2".to_string(),
                goal: None,
                starts_at: None,
                ends_at: None,
                status: Some(SPRINT_STATUS_PLANNED.to_string()),
            },
        )
        .unwrap();

    let pending = create_card(&db, p1, "Card pendente");
    let done = create_card(&db, p1, "Card concluído");
    let archived = create_card(&db, p1, "Card arquivado");
    let subtask = create_card(&db, p1, "Subtarefa pendente");

    db.set_item_sprint(p1, &pending.id, Some(&s1.id)).unwrap();
    db.set_item_sprint(p1, &done.id, Some(&s1.id)).unwrap();
    db.set_item_sprint(p1, &archived.id, Some(&s1.id)).unwrap();
    db.set_item_sprint(p1, &subtask.id, Some(&s1.id)).unwrap();

    let conn = dir.raw();
    conn.execute(
        "UPDATE backlog_items SET \"column\" = 'Done' WHERE project_id = ?1 AND id = ?2",
        params![p1, done.id],
    )
    .unwrap();
    conn.execute(
        "UPDATE backlog_items SET archived_at = 123456 WHERE project_id = ?1 AND id = ?2",
        params![p1, archived.id],
    )
    .unwrap();
    conn.execute(
        "UPDATE backlog_items SET parent = ?3 WHERE project_id = ?1 AND id = ?2",
        params![p1, subtask.id, pending.id],
    )
    .unwrap();

    let closed_self = db.close_sprint(p1, &s1.id, Some(&s1.id));
    assert!(matches!(closed_self, Err(DbError::SprintClosed(_))));

    let closed = db
        .close_sprint(p1, &s1.id, Some(&s2.id))
        .expect("encerramento com migração deveria funcionar");
    assert_eq!(closed.status, SPRINT_STATUS_CLOSED);
    assert_eq!(closed.total_items, 1);
    assert_eq!(closed.done_items, 1);

    let sprint_of = |item_id: &str| -> Option<String> {
        conn.query_row(
            "SELECT sprint_id FROM backlog_items WHERE project_id = ?1 AND id = ?2",
            params![p1, item_id],
            |row| row.get(0),
        )
        .unwrap()
    };

    assert_eq!(sprint_of(&pending.id).as_deref(), Some(s2.id.as_str()));
    assert_eq!(sprint_of(&done.id).as_deref(), Some(s1.id.as_str()));
    assert_eq!(sprint_of(&archived.id).as_deref(), Some(s1.id.as_str()));
    assert_eq!(sprint_of(&subtask.id).as_deref(), Some(s1.id.as_str()));
}

#[test]
fn delete_sprint_clears_sprint_id_on_cards() {
    let dir = TempDir::new();
    let db = dir.open();
    let p1 = create_project(&db, "p1", "C:/projetos/p1");

    let s1 = db
        .create_sprint(
            p1,
            NewSprint {
                name: "Sprint 1".to_string(),
                goal: None,
                starts_at: None,
                ends_at: None,
                status: None,
            },
        )
        .unwrap();
    let card = create_card(&db, p1, "Card vinculado");
    db.set_item_sprint(p1, &card.id, Some(&s1.id)).unwrap();

    db.delete_sprint(p1, &s1.id)
        .expect("exclusão da sprint deveria funcionar");

    let snapshot = db.list_backlog(p1).unwrap();
    assert!(snapshot.sprints.is_empty());
    let stored = snapshot.items.iter().find(|i| i.id == card.id).unwrap();
    assert_eq!(stored.sprint_id, None);
}

#[test]
fn backlog_aggregates_interaction_count_and_last_activity_at() {
    let dir = TempDir::new();
    let db = dir.open();
    let p1 = create_project(&db, "p1", "C:/projetos/p1");

    let s1 = db
        .create_sprint(
            p1,
            NewSprint {
                name: "Sprint 1".to_string(),
                goal: None,
                starts_at: None,
                ends_at: None,
                status: None,
            },
        )
        .unwrap();

    let mut card = create_card(&db, p1, "Card com interações");
    db.set_item_sprint(p1, &card.id, Some(&s1.id)).unwrap();

    // save_backlog_item sem sprint_id preenchido no payload preserva o vínculo existente
    card.sprint_id = None;
    card.questions = serde_json::from_value(serde_json::json!([
        { "id": "q1", "text": "Qual prazo?", "options": ["Hoje", "Amanhã"] }
    ]))
    .unwrap();
    card.activity = serde_json::from_value(serde_json::json!([
        { "id": "a1", "text": "Iniciado", "at": "2026-01-01 10:00" }
    ]))
    .unwrap();
    db.save_backlog_item(p1, &card)
        .expect("save_backlog_item deveria preservar sprint_id");

    let conn = dir.raw();
    conn.execute(
        "UPDATE backlog_items SET cycle_started_at = 5000 WHERE project_id = ?1 AND id = ?2",
        params![p1, card.id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages (project_id, from_agent, to_agent, text, created_at, item_id) \
         VALUES (?1, 'kairo', 'ember', 'm1', 10000, ?2), (?1, 'ember', 'kairo', 'm2', 25000, ?2)",
        params![p1, card.id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO orch_handoffs (project_id, from_agent, to_agent, action, kind, item_id, return_count, created_at) \
         VALUES (?1, 'axiom', 'kairo', 'delegar', 'implementar', ?2, 0, 30000)",
        params![p1, card.id],
    )
    .unwrap();

    let snapshot = db.list_backlog(p1).unwrap();
    let stored = snapshot.items.iter().find(|i| i.id == card.id).unwrap();
    assert_eq!(stored.sprint_id.as_deref(), Some(s1.id.as_str()));
    // 2 messages + 1 handoff + 1 activity + 1 question = 5
    assert_eq!(stored.interaction_count, 5);
    assert!(stored.last_activity_at.unwrap_or_default() >= 30000);
}
