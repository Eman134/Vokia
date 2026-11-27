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
        db.insert_message(project_id, Speaker::User, agent("vexa"), "no canal", 1, Some(7), None, None, None, None)
            .unwrap();
        db.insert_message(project_id, agent("vexa"), Speaker::User, "resposta no canal", 2, Some(7), None, None, None, None)
            .unwrap();
        db.insert_message(project_id, Speaker::User, agent("axiom"), "sem canal", 3, None, None, None, None, None)
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
    db.insert_message(project_id, Speaker::User, agent("vexa"), "com", 1, Some(3), None, None, None, None)
        .unwrap();
    db.insert_message(project_id, Speaker::User, agent("vexa"), "sem", 2, None, None, None, None, None)
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
    db.insert_message(first, Speaker::User, agent("vexa"), "do a", 1, Some(1), None, None, None, None)
        .unwrap();
    db.insert_message(second, Speaker::User, agent("vexa"), "do b", 2, None, None, None, None, None)
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

    db.insert_message(project_id, agent("vexa"), Speaker::User, "com", 1, None, None, None, Some(&proposal), None)
        .unwrap();
    db.insert_message(project_id, agent("vexa"), Speaker::User, "sem", 2, None, None, None, None, None)
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
    db.insert_message(project_id, agent("vexa"), Speaker::User, "texto", 1, None, None, None, None, None)
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
        item_id: None,
        activity: None,
        request_proposal,
        usage: None,
        is_error: false,
        foreign_instance: false,
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

#[test]
fn load_history_marca_is_error_apenas_em_mensagens_de_agente_de_limite_ou_falha_de_cli() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");

    let casos = [
        (agent("vexa"), "Limite esgotado", true),
        (
            agent("vexa"),
            "A CLI antigravity saiu com código 2: Error: -p took --input-format as its prompt",
            true,
        ),
        (
            agent("vexa"),
            "Não foi possível iniciar a CLI antigravity: arquivo não encontrado",
            true,
        ),
        (agent("vexa"), "Resposta normal do agente", false),
        (Speaker::User, "Limite esgotado", false),
        (Speaker::User, "A CLI antigravity saiu com código 2", false),
    ];

    for (idx, (from, text, _)) in casos.iter().enumerate() {
        db.insert_message(
            project_id,
            from.clone(),
            Speaker::User,
            text,
            (idx + 1) as u64,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    }

    let history = db.load_history(project_id).unwrap();
    assert_eq!(history.len(), casos.len());
    for (event, (_, expected_text, expected_error)) in history.iter().zip(casos.iter()) {
        match event {
            VokiaEvent::Message { text, is_error, .. } => {
                assert_eq!(text, expected_text);
                assert_eq!(
                    *is_error, *expected_error,
                    "is_error divergente para texto: {text}"
                );
            }
            other => panic!("esperava VokiaEvent::Message, veio {other:?}"),
        }
    }
}

#[test]
fn task_result_persiste_ordem_canal_e_clear_messages() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "task-result");
    let result = |objective: &str| crate::db::StoredTaskResult {
        objective: objective.to_string(),
        concluded: true,
        turns: 3,
        agents: vec![Agent::from_stored("ember").unwrap(), Agent::from_stored("vexa").unwrap()],
    };

    db.insert_task_result(project_id, Speaker::Agent(Agent::from_stored("ember").unwrap()), 20, None, None, &result("Sem canal"))
        .unwrap();
    db.insert_task_result(project_id, Speaker::Agent(Agent::from_stored("vexa").unwrap()), 10, Some(5), None, &result("Canal 5"))
        .unwrap();

    let history = db.load_history(project_id).unwrap();
    assert_eq!(history.len(), 2);
    assert!(matches!(
        &history[0],
        VokiaEvent::TaskResult { objective, channel_id: None, .. } if objective == "Sem canal"
    ));
    assert!(matches!(
        &history[1],
        VokiaEvent::TaskResult { objective, channel_id: Some(5), .. } if objective == "Canal 5"
    ));

    db.clear_messages(project_id).unwrap();
    assert!(db.load_history(project_id).unwrap().is_empty());
}

#[test]
fn task_result_com_json_corrompido_faz_fallback_para_mensagem_sem_derrubar_historico() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "task-result-corrompido");
    db.insert_task_result(
        project_id,
        Speaker::Agent(Agent::from_stored("ember").unwrap()),
        1,
        None,
        None,
        &crate::db::StoredTaskResult {
            objective: "objetivo original".to_string(),
            concluded: true,
            turns: 1,
            agents: vec![Agent::from_stored("ember").unwrap()],
        },
    )
    .unwrap();
    db.conn()
        .unwrap()
        .execute("UPDATE messages SET task_result = '{corrompido' WHERE project_id = ?1", [project_id])
        .unwrap();

    let history = db.load_history(project_id).unwrap();
    assert_eq!(history.len(), 1);
    assert!(matches!(
        &history[0],
        VokiaEvent::Message { text, channel_id: None, .. } if text.is_empty()
    ));
}

#[test]
fn frontend_balloon_oculta_origem_em_erro_e_use_feed_so_sugere_migracao_para_limite_esgotado() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let balloon_vue = fs::read_to_string(manifest.join("../src/features/balloon/Balloon.vue"))
        .expect("deve ler Balloon.vue");
    assert!(
        !balloon_vue.contains("item.origin"),
        "Balloon.vue não deve exibir rótulo de origem"
    );

    let use_feed = fs::read_to_string(manifest.join("../src/features/team/useFeed.ts"))
        .expect("deve ler useFeed.ts");
    assert!(
        use_feed.contains("event.isError && event.from !== 'user' && event.text === 'Limite esgotado'"),
        "useFeed.ts só deve sugerir migração de limite quando text === 'Limite esgotado'"
    );
}

#[test]
fn task_da_mensagem_e_do_resultado_sobrevive_a_reabrir_o_banco() {
    let dir = TempDir::new();
    let project_id = {
        let db = dir.open();
        let project_id = project(&db, "item-id");
        db.insert_message(project_id, agent("ember"), agent("vexa"), "faça a tela", 1, Some(2), Some("PP-1"), None, None, None)
            .unwrap();
        db.insert_message(project_id, Speaker::User, agent("ember"), "sem task", 2, None, None, None, None, None)
            .unwrap();
        db.insert_task_result(
            project_id,
            agent("ember"),
            3,
            None,
            Some("PP-1"),
            &crate::db::StoredTaskResult {
                objective: "tela".to_string(),
                concluded: true,
                turns: 2,
                agents: vec![Agent::from_stored("ember").unwrap()],
            },
        )
        .unwrap();
        project_id
    };

    let history = dir.open().load_history(project_id).unwrap();

    assert!(matches!(
        &history[0],
        VokiaEvent::Message { channel_id: Some(2), item_id: Some(item), .. } if item == "PP-1"
    ));
    assert!(matches!(&history[1], VokiaEvent::Message { item_id: None, .. }));
    assert!(matches!(
        &history[2],
        VokiaEvent::TaskResult { item_id: Some(item), .. } if item == "PP-1"
    ));
}

#[test]
fn banco_sem_a_coluna_item_id_migra_e_mantem_as_mensagens() {
    let dir = TempDir::new();
    let project_id = {
        let db = dir.open();
        let project_id = project(&db, "migra-item-id");
        db.insert_message(project_id, Speaker::User, agent("ember"), "antiga", 1, None, None, None, None, None)
            .unwrap();
        project_id
    };
    let raw = rusqlite::Connection::open(dir.0.join("vokia.db")).unwrap();
    raw.execute_batch("DROP INDEX IF EXISTS messages_by_item; ALTER TABLE messages DROP COLUMN item_id")
        .unwrap();
    drop(raw);

    let db = dir.open();
    db.insert_message(project_id, agent("ember"), agent("vexa"), "nova", 2, None, Some("PP-9"), None, None, None)
        .unwrap();

    let history = db.load_history(project_id).unwrap();
    assert_eq!(history.len(), 2);
    assert!(matches!(&history[0], VokiaEvent::Message { text, item_id: None, .. } if text == "antiga"));
    assert!(matches!(&history[1], VokiaEvent::Message { item_id: Some(item), .. } if item == "PP-9"));
}

#[test]
fn mensagem_de_instancia_excluida_carrega_com_id_bruto_e_marca_foreign_instance() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "instancia-excluida");
    db.insert_message(project_id, agent("vexa"), Speaker::User, "de fora", 1, None, None, None, None, None)
        .unwrap();
    db.insert_message(project_id, Speaker::User, agent("axiom"), "local", 2, None, None, None, None, None)
        .unwrap();
    db.conn()
        .unwrap()
        .execute("UPDATE messages SET from_agent = 'vexa-antiga' WHERE text = 'de fora'", [])
        .unwrap();

    let history = db.load_history(project_id).unwrap();

    assert_eq!(history.len(), 2, "a mensagem de instância excluída não pode sumir do histórico");
    match &history[0] {
        VokiaEvent::Message { from, to, text, foreign_instance, .. } => {
            assert_eq!(from.as_str(), "vexa-antiga");
            assert_eq!(*to, Speaker::User);
            assert_eq!(text, "de fora");
            assert!(*foreign_instance);
        }
        other => panic!("esperava VokiaEvent::Message, veio {other:?}"),
    }
    assert!(matches!(&history[1], VokiaEvent::Message { foreign_instance: false, .. }));

    let json = serde_json::to_value(&history[0]).unwrap();
    assert_eq!(json["from"], serde_json::json!("vexa-antiga"));
    assert_eq!(json["foreignInstance"], serde_json::json!(true));
    assert!(serde_json::to_value(&history[1]).unwrap().get("foreignInstance").is_none());
}
