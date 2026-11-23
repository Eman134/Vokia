use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use rusqlite::{params, Connection};

use super::*;
use crate::agents::Agent;
use crate::db::projects::NewProject;
use crate::events::Speaker;

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

const DB_FILE_NAME: &str = "vokia.db";
const STARTED_AT: i64 = 1_000;
const MESSAGE_LIMIT: usize = 30;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        TempDir(std::env::temp_dir().join(format!(
            "vokia-runs-test-{}-{}",
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

fn speaker(id: &str) -> Speaker {
    if id == "user" {
        return Speaker::User;
    }
    Speaker::Agent(Agent::from_stored(id).expect("id de agente válido no teste"))
}

fn message(db: &Db, project_id: i64, from: &str, to: &str, text: &str, at: i64) {
    db.insert_message(project_id, speaker(from), speaker(to), text, at as u64, None, None, None, None, None)
        .expect("a mensagem deveria ser gravada");
}

fn handoff(dir: &TempDir, project_id: i64, from: &str, to: &str, item: Option<&str>, at: i64) {
    dir.raw()
        .execute(
            "INSERT INTO orch_handoffs (project_id, from_agent, to_agent, action, kind, item_id, return_count, created_at) \
             VALUES (?1, ?2, ?3, 'delegar', 'implementar', ?4, 0, ?5)",
            params![project_id, from, to, item, at],
        )
        .expect("a delegação deveria ser gravada");
}

fn interrupted(db: &Db, project_id: i64, agent: &str, started_at: i64) -> InterruptedRun {
    let id = db
        .start_run(project_id, agent, "Pedido original", None, Some("item-1"), started_at)
        .unwrap();
    db.interrupt_run(id).unwrap();
    db.interrupted_runs()
        .unwrap()
        .into_iter()
        .find(|run| run.id == id)
        .expect("o pedido deveria estar interrompido")
}

fn texts(progress: &RunProgress) -> Vec<String> {
    progress.messages.iter().map(|message| message.text.clone()).collect()
}

fn routes(progress: &RunProgress) -> Vec<(String, String)> {
    progress
        .handoffs
        .iter()
        .map(|handoff| (handoff.from_agent.clone(), handoff.to_agent.clone()))
        .collect()
}

fn state_of(dir: &TempDir, id: i64) -> String {
    dir.raw()
        .query_row("SELECT state FROM orch_runs WHERE id = ?1", [id], |row| row.get(0))
        .expect("o pedido deveria existir")
}

#[test]
fn pedido_aberto_por_delegacao_nao_corta_a_janela_do_pedido_interrompido() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let run = interrupted(&db, project_id, "vexa", STARTED_AT);
    message(&db, project_id, "vexa", "user", "Vou delegar a API", STARTED_AT);
    handoff(&dir, project_id, "vexa", "hefesto", Some("item-1"), STARTED_AT + 100);
    db.start_run(project_id, "hefesto", "Implementar API", None, Some("item-1"), STARTED_AT + 200)
        .unwrap();
    handoff(&dir, project_id, "vexa", "axiom-2", None, STARTED_AT + 250);
    db.start_run(project_id, "axiom", "Revisar", None, None, STARTED_AT + 300).unwrap();
    message(&db, project_id, "hefesto", "vexa", "API pronta", STARTED_AT + 400);

    let progress = db.run_progress(&run).unwrap();

    assert_eq!(
        routes(&progress),
        [
            ("vexa".to_string(), "hefesto".to_string()),
            ("vexa".to_string(), "axiom-2".to_string())
        ]
    );
    assert_eq!(progress.handoffs[0].item_id.as_deref(), Some("item-1"));
    assert_eq!(texts(&progress), ["Vou delegar a API", "API pronta"]);
    assert!(!progress.is_empty());
}

#[test]
fn novo_pedido_independente_depois_do_interrompido_nao_traz_o_que_vem_depois() {
    for next_agent in ["vexa", "axiom"] {
        let dir = TempDir::new();
        let db = dir.open();
        let project_id = project(&db, "a");
        let run = interrupted(&db, project_id, "vexa", STARTED_AT);
        message(&db, project_id, "vexa", "user", "antes", STARTED_AT + 10);
        db.start_run(project_id, next_agent, "Outro pedido", None, None, STARTED_AT + 500)
            .unwrap();
        message(&db, project_id, next_agent, "user", "depois", STARTED_AT + 500);
        handoff(&dir, project_id, next_agent, "hefesto", None, STARTED_AT + 600);

        let progress = db.run_progress(&run).unwrap();

        assert_eq!(texts(&progress), ["antes"], "{next_agent}");
        assert!(progress.handoffs.is_empty(), "{next_agent}");
    }
}

#[test]
fn mensagens_e_delegacoes_de_outro_projeto_ou_anteriores_ao_pedido_nao_entram() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_a = project(&db, "a");
    let project_b = project(&db, "b");
    let run = interrupted(&db, project_a, "vexa", STARTED_AT);
    message(&db, project_a, "user", "vexa", "pedido gravado antes do registro", STARTED_AT - 1);
    message(&db, project_b, "vexa", "user", "de outro projeto", STARTED_AT + 10);
    handoff(&dir, project_b, "vexa", "hefesto", None, STARTED_AT + 10);
    db.start_run(project_b, "vexa", "Pedido do B", None, None, STARTED_AT + 20).unwrap();
    message(&db, project_a, "vexa", "user", "do projeto certo", STARTED_AT + 30);

    let progress = db.run_progress(&run).unwrap();

    assert_eq!(texts(&progress), ["do projeto certo"]);
    assert!(progress.handoffs.is_empty());
}

#[test]
fn mais_de_30_mensagens_traz_as_30_mais_recentes_em_ordem_crescente() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let run = interrupted(&db, project_id, "vexa", STARTED_AT);
    for n in 1..=35 {
        message(&db, project_id, "vexa", "user", &format!("m{n}"), STARTED_AT + n);
    }

    let progress = db.run_progress(&run).unwrap();

    let expected: Vec<String> = (6..=35).map(|n| format!("m{n}")).collect();
    assert_eq!(progress.messages.len(), MESSAGE_LIMIT);
    assert_eq!(texts(&progress), expected);
}

#[test]
fn atividade_ilegivel_vira_none_e_atividade_valida_traz_as_ferramentas() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let run = interrupted(&db, project_id, "vexa", STARTED_AT);
    message(&db, project_id, "vexa", "user", "quebrada", STARTED_AT + 1);
    message(&db, project_id, "vexa", "user", "valida", STARTED_AT + 2);
    let raw = dir.raw();
    raw.execute("UPDATE messages SET activity = '{nao e json' WHERE text = 'quebrada'", [])
        .unwrap();
    raw.execute(
        "UPDATE messages SET activity = ?1 WHERE text = 'valida'",
        [r#"{"tools":[{"callId":"t1","name":"Edit","category":"edit","target":"src/a.rs","status":"ok"}]}"#],
    )
    .unwrap();

    let progress = db.run_progress(&run).unwrap();

    assert_eq!(texts(&progress), ["quebrada", "valida"]);
    assert!(progress.messages[0].activity.is_none());
    let tools = &progress.messages[1].activity.as_ref().expect("atividade válida").tools;
    assert_eq!(tools.len(), 1);
    assert_eq!((tools[0].name.as_str(), tools[0].target.as_str()), ("Edit", "src/a.rs"));
}

#[test]
fn pedido_sem_nada_registrado_tem_progresso_vazio() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let run = interrupted(&db, project_id, "vexa", STARTED_AT);

    assert!(db.run_progress(&run).unwrap().is_empty());
}

#[test]
fn reivindicar_volta_a_em_andamento_e_o_segundo_clique_e_recusado() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let run = interrupted(&db, project_id, "vexa", STARTED_AT);

    let claimed = db.claim_interrupted_run(run.id).unwrap();

    assert_eq!(claimed.id, run.id);
    assert_eq!(claimed.started_at, STARTED_AT);
    assert_eq!(claimed.item_id.as_deref(), Some("item-1"));
    assert_eq!(state_of(&dir, run.id), "em_andamento");
    assert!(db.interrupted_runs().unwrap().is_empty());
    let second = db.claim_interrupted_run(run.id).unwrap_err();
    assert!(matches!(second, DbError::RunNotInterrupted(id) if id == run.id), "{second:?}");
    let missing = db.claim_interrupted_run(run.id + 999).unwrap_err();
    assert!(matches!(missing, DbError::RunNotInterrupted(_)), "{missing:?}");
    let total: i64 = dir
        .raw()
        .query_row("SELECT COUNT(*) FROM orch_runs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(total, 1, "reivindicar não cria registro novo");
}

#[test]
fn liberar_devolve_aos_interrompidos_e_nao_reverte_pedido_concluido() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let run = interrupted(&db, project_id, "vexa", STARTED_AT);
    db.claim_interrupted_run(run.id).unwrap();

    db.release_run(run.id).unwrap();

    assert_eq!(state_of(&dir, run.id), "interrompido");
    assert_eq!(db.interrupted_runs().unwrap().len(), 1);
    db.claim_interrupted_run(run.id).unwrap();
    db.finish_run(run.id, STARTED_AT + 5_000).unwrap();
    db.release_run(run.id).unwrap();
    assert_eq!(state_of(&dir, run.id), "concluido");
    assert!(db.interrupted_runs().unwrap().is_empty());
    let again = db.claim_interrupted_run(run.id).unwrap_err();
    assert!(matches!(again, DbError::RunNotInterrupted(_)), "{again:?}");
}

#[test]
fn reivindicado_sem_fim_volta_a_interrompido_ao_reabrir_o_banco() {
    let dir = TempDir::new();
    let run_id = {
        let db = dir.open();
        let project_id = project(&db, "a");
        let run = interrupted(&db, project_id, "vexa", STARTED_AT);
        db.claim_interrupted_run(run.id).unwrap();
        run.id
    };

    let reopened = dir.open();

    assert_eq!(state_of(&dir, run_id), "interrompido");
    let listed = reopened.interrupted_runs().unwrap();
    assert_eq!(listed.iter().map(|run| run.id).collect::<Vec<_>>(), [run_id]);
    assert_eq!(listed[0].started_at, STARTED_AT, "a janela do progresso continua a mesma");
}

#[test]
fn fluxo_paralelo_de_outro_agente_fica_fora_e_delegacao_em_cadeia_entra() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let run = interrupted(&db, project_id, "vexa", STARTED_AT);
    message(&db, project_id, "user", "axiom", "pedido paralelo", STARTED_AT + 5);
    message(&db, project_id, "axiom", "user", "resposta paralela", STARTED_AT + 6);
    handoff(&dir, project_id, "axiom", "ember", None, STARTED_AT + 7);
    handoff(&dir, project_id, "vexa", "hefesto-2", Some("item-1"), STARTED_AT + 10);
    handoff(&dir, project_id, "hefesto", "ember", Some("item-1"), STARTED_AT + 20);
    message(&db, project_id, "ember", "hefesto", "revisado", STARTED_AT + 30);

    let progress = db.run_progress(&run).unwrap();

    assert_eq!(
        routes(&progress),
        [
            ("vexa".to_string(), "hefesto-2".to_string()),
            ("hefesto".to_string(), "ember".to_string())
        ]
    );
    assert_eq!(texts(&progress), ["revisado"]);
}