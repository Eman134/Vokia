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

fn proposal_of(event: &VokiaEvent) -> Option<RequestProposal> {
    match event {
        VokiaEvent::Message { request_proposal, .. } => request_proposal.clone(),
        other => panic!("esperava mensagem, veio {other:?}"),
    }
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
        db.insert_message(project_id, Speaker::User, agent("vexa"), "no canal", 1, Some(7), None, None, None)
            .unwrap();
        db.insert_message(project_id, agent("vexa"), Speaker::User, "resposta no canal", 2, Some(7), None, None, None)
            .unwrap();
        db.insert_message(project_id, Speaker::User, agent("axiom"), "sem canal", 3, None, None, None, None)
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
    db.insert_message(project_id, Speaker::User, agent("vexa"), "com", 1, Some(3), None, None, None)
        .unwrap();
    db.insert_message(project_id, Speaker::User, agent("vexa"), "sem", 2, None, None, None, None)
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
    db.insert_message(first, Speaker::User, agent("vexa"), "do a", 1, Some(1), None, None, None)
        .unwrap();
    db.insert_message(second, Speaker::User, agent("vexa"), "do b", 2, None, None, None, None)
        .unwrap();

    let history = db.load_history(second).unwrap();

    assert_eq!(history.iter().map(channel_of).collect::<Vec<_>>(), vec![None]);
}

#[test]
fn proposta_de_pedido_sobrevive_ao_historico_e_mensagem_sem_proposta_volta_sem_ela() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let proposal = RequestProposal {
        title: "Revisar login".to_string(),
        detail: "Detalhe".to_string(),
    };

    db.insert_message(project_id, agent("vexa"), Speaker::User, "com", 1, None, None, Some(&proposal), None)
        .unwrap();
    db.insert_message(project_id, agent("vexa"), Speaker::User, "sem", 2, None, None, None, None)
        .unwrap();
    let history = db.load_history(project_id).unwrap();

    assert_eq!(proposal_of(&history[0]), Some(proposal));
    assert_eq!(proposal_of(&history[1]), None);
}

#[test]
fn proposta_ilegivel_no_banco_nao_esconde_a_mensagem() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    db.insert_message(project_id, agent("vexa"), Speaker::User, "texto", 1, None, None, None, None)
        .unwrap();
    db.conn()
        .unwrap()
        .execute("UPDATE messages SET request_proposal = '{quebrado'", [])
        .unwrap();

    let history = db.load_history(project_id).unwrap();

    assert_eq!(history.len(), 1);
    assert_eq!(proposal_of(&history[0]), None);
}

#[test]
fn evento_serializa_request_proposal_em_camel_case_e_omite_quando_nao_ha() {
    let message = |request_proposal| VokiaEvent::Message {
        from: Speaker::User,
        to: Speaker::User,
        text: "oi".to_string(),
        time: 1,
        project_id: Some(1),
        channel_id: None,
        activity: None,
        request_proposal,
        usage: None,
        is_error: false,
    };
    let with = serde_json::to_value(message(Some(RequestProposal {
        title: "T".to_string(),
        detail: "D".to_string(),
    })))
    .unwrap();
    let without = serde_json::to_value(message(None)).unwrap();

    assert_eq!(with["requestProposal"], serde_json::json!({ "title": "T", "detail": "D" }));
    assert!(without.get("requestProposal").is_none());
}
