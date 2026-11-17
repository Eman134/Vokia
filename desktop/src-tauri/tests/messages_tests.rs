use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::agents::Agent;
use crate::db::projects::NewProject;

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        TempDir(std::env::temp_dir().join(format!(
            "vokia-messages-test-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::SeqCst)
        )))
    }

    fn open(&self) -> Db {
        Db::open(&self.0).expect("o banco de teste deveria abrir")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn project(db: &Db, name: &str) -> i64 {
    db.insert_project(&NewProject {
        name,
        symbol: "folder",
        description: "",
        path: &format!("C:/projetos/{name}"),
        origin_kind: "folder",
        origin_repo: None,
    })
    .expect("o projeto deveria ser criado")
    .id
}

fn agent(id: &str) -> Speaker {
    Speaker::Agent(Agent::from_stored(id).expect("id de agente válido no teste"))
}

fn channel_of(event: &VokiaEvent) -> Option<i64> {
    match event {
        VokiaEvent::Message { channel_id, .. } => *channel_id,
        other => panic!("o histórico deveria ter só mensagens, veio {other:?}"),
    }
}

#[test]
fn marca_de_canal_sobrevive_a_reabrir_o_banco_e_mensagem_sem_canal_continua_sem() {
    let dir = TempDir::new();
    let project_id = {
        let db = dir.open();
        let project_id = project(&db, "a");
        db.insert_message(project_id, Speaker::User, agent("davi"), "no canal", 1, Some(7), None)
            .unwrap();
        db.insert_message(project_id, agent("davi"), Speaker::User, "resposta no canal", 2, Some(7), None)
            .unwrap();
        db.insert_message(project_id, Speaker::User, agent("iara"), "sem canal", 3, None, None)
            .unwrap();
        project_id
    };

    let history = dir.open().load_history(project_id).unwrap();

    assert_eq!(
        history.iter().map(channel_of).collect::<Vec<_>>(),
        vec![Some(7), Some(7), None]
    );
}

#[test]
fn json_do_evento_traz_channel_id_so_com_canal_e_nunca_null() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    db.insert_message(project_id, Speaker::User, agent("davi"), "com", 1, Some(3), None)
        .unwrap();
    db.insert_message(project_id, Speaker::User, agent("davi"), "sem", 2, None, None)
        .unwrap();

    let json: Vec<serde_json::Value> = db
        .load_history(project_id)
        .unwrap()
        .iter()
        .map(|event| serde_json::to_value(event).unwrap())
        .collect();

    assert_eq!(json[0]["channelId"], serde_json::json!(3));
    assert!(
        json[1].as_object().unwrap().get("channelId").is_none(),
        "sem canal o campo não pode sair, nem como null: {}",
        json[1]
    );
}

#[test]
fn historico_de_um_projeto_nao_traz_a_marca_de_outro() {
    let dir = TempDir::new();
    let db = dir.open();
    let first = project(&db, "a");
    let second = project(&db, "b");
    db.insert_message(first, Speaker::User, agent("davi"), "do a", 1, Some(1), None)
        .unwrap();
    db.insert_message(second, Speaker::User, agent("davi"), "do b", 2, None, None)
        .unwrap();

    let history = db.load_history(second).unwrap();

    assert_eq!(history.iter().map(channel_of).collect::<Vec<_>>(), vec![None]);
}
