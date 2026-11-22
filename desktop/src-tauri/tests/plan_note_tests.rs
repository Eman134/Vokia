use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use rusqlite::Connection;

use crate::db::{BacklogItem, Db, NewCard, NewProject};

const DB_FILE_NAME: &str = "vokia.db";

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        TempDir(std::env::temp_dir().join(format!(
            "vokia-plan-note-test-{}-{}",
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

fn project(db: &Db) -> i64 {
    db.insert_project(&NewProject {
        name: "a",
        symbol: "folder",
        description: "",
        path: "C:/projetos/a",
        origin_kind: "folder",
        origin_repo: None,
    })
    .expect("o projeto deveria ser criado")
    .id
}

fn card(db: &Db, project_id: i64) -> BacklogItem {
    db.create_card(
        project_id,
        &NewCard {
            title: "Checkout com Pix".to_string(),
            description: None,
            origin: "usuario".to_string(),
            links: Vec::new(),
        },
    )
    .expect("o card deveria ser criado")
}

fn stored(db: &Db, project_id: i64, id: &str) -> BacklogItem {
    db.list_backlog(project_id)
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.id == id)
        .unwrap_or_else(|| panic!("o card {id} deveria existir"))
}

fn save_note(db: &Db, project_id: i64, mut item: BacklogItem, note: Option<&str>) -> BacklogItem {
    item.plan_note = note.map(str::to_string);
    db.save_backlog_item(project_id, &item).unwrap();
    stored(db, project_id, &item.id)
}

#[test]
fn nota_do_plano_volta_igual_com_quebras_de_linha_e_sem_espacos_nas_bordas() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db);
    let item = card(&db, project_id);

    let saved = save_note(&db, project_id, item, Some("  Reaproveite o cartão.\nNão mexa no schema.\n "));

    assert_eq!(saved.plan_note.as_deref(), Some("Reaproveite o cartão.\nNão mexa no schema."));
}

#[test]
fn nota_vazia_ou_so_espacos_vira_nula_e_apaga_a_anterior() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db);
    let item = card(&db, project_id);
    let with_note = save_note(&db, project_id, item, Some("Expira em 30 min."));

    let cleared = save_note(&db, project_id, with_note, Some(" \n\t "));

    assert_eq!(cleared.plan_note, None);
    let raw: Option<String> = dir
        .raw()
        .query_row(
            "SELECT plan_note FROM backlog_items WHERE project_id = ?1 AND id = ?2",
            rusqlite::params![project_id, cleared.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(raw, None);
}

#[test]
fn nota_sem_valor_nao_aparece_no_json_do_item() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db);
    let item = card(&db, project_id);

    let json = serde_json::to_value(stored(&db, project_id, &item.id)).unwrap();
    assert!(json.get("planNote").is_none());

    let saved = save_note(&db, project_id, item, Some("Expira em 30 min."));
    let json = serde_json::to_value(saved).unwrap();
    assert_eq!(json["planNote"], "Expira em 30 min.");
}

#[test]
fn banco_sem_a_coluna_ganha_plan_note_ao_reabrir_e_mantem_os_itens() {
    let dir = TempDir::new();
    let (project_id, id) = {
        let db = dir.open();
        let project_id = project(&db);
        (project_id, card(&db, project_id).id)
    };
    dir.raw()
        .execute_batch("ALTER TABLE backlog_items DROP COLUMN plan_note")
        .unwrap();

    let db = dir.open();

    let item = stored(&db, project_id, &id);
    assert_eq!(item.title, "Checkout com Pix");
    assert_eq!(item.plan_note, None);
    let saved = save_note(&db, project_id, item, Some("Expira em 30 min."));
    assert_eq!(saved.plan_note.as_deref(), Some("Expira em 30 min."));
}
