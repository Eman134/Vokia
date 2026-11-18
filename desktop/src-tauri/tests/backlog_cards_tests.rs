use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::agents::Agent;
use crate::db::{AgentActor, CardOperation, CardOutcome, Db, DbError, NewProject};

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        TempDir(std::env::temp_dir().join(format!(
            "vokia-backlog-cards-test-{}-{}",
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

fn agent(id: &str) -> Agent {
    Agent::from_stored(id).expect("id de agente válido no teste")
}

fn actor(id: &str, name: &str, current_card: Option<&str>) -> AgentActor {
    AgentActor {
        id: agent(id),
        name: name.to_string(),
        current_card: current_card.map(str::to_string),
    }
}

fn vexa() -> AgentActor {
    actor("vexa", "Vexa", None)
}

fn axiom() -> AgentActor {
    actor("axiom", "Axiom", None)
}

fn create(title: &str) -> CardOperation {
    CardOperation::Create {
        title: title.to_string(),
        description: None,
    }
}

fn move_to(id: &str, column: &str) -> CardOperation {
    CardOperation::Move {
        id: id.to_string(),
        column: column.to_string(),
    }
}

fn titles(db: &Db, project_id: i64) -> Vec<String> {
    db.list_backlog(project_id)
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.title)
        .collect()
}

fn column_of(db: &Db, project_id: i64, id: &str) -> String {
    db.list_backlog(project_id)
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.id == id)
        .and_then(|item| item.column)
        .expect("o card deveria existir com coluna")
}

fn activity_count(db: &Db, project_id: i64, id: &str) -> usize {
    db.list_backlog(project_id)
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.id == id)
        .map(|item| item.activity.len())
        .expect("o card deveria existir")
}

fn refused(result: Result<Vec<crate::db::CardResult>, DbError>) -> DbError {
    result.expect_err("a operação deveria ser recusada")
}

#[test]
fn criar_grava_card_do_agente_em_backlog_com_atividade_e_origem() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");

    let results = db
        .apply_card_operations(
            project_id,
            &vexa(),
            &[CardOperation::Create {
                title: "Revisar login".to_string(),
                description: Some("Checar o token".to_string()),
            }],
        )
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].outcome, CardOutcome::Created);
    assert_eq!(results[0].column, "Backlog");
    let item = &db.list_backlog(project_id).unwrap().items[0];
    assert_eq!(item.id, results[0].id);
    assert_eq!(item.origin, "agente");
    assert_eq!(item.who, "vexa");
    assert_eq!(item.by, "vexa");
    assert_eq!(item.description.as_deref(), Some("Checar o token"));
    assert_eq!(item.activity.len(), 1);
    assert_eq!(item.activity[0].text, "Vexa criou o card na conversa.");
}

#[test]
fn mover_para_ready_e_recusado_e_o_card_fica_onde_estava() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let id = db
        .apply_card_operations(project_id, &vexa(), &[create("Um")])
        .unwrap()[0]
        .id
        .clone();

    let error = refused(db.apply_card_operations(project_id, &vexa(), &[move_to(&id, "Ready")]));

    assert!(
        matches!(error, DbError::AgentForbiddenColumn(_)),
        "{error:?}"
    );
    assert!(error.is_card_refusal());
    assert_eq!(column_of(&db, project_id, &id), "Backlog");
    assert_eq!(activity_count(&db, project_id, &id), 1);
}

#[test]
fn mover_card_de_outro_agente_e_recusado_mas_o_card_do_pedido_atual_e_aceito() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let id = db
        .apply_card_operations(project_id, &axiom(), &[create("Da Axiom")])
        .unwrap()[0]
        .id
        .clone();

    let error =
        refused(db.apply_card_operations(project_id, &vexa(), &[move_to(&id, "In progress")]));

    assert!(
        matches!(&error, DbError::CardNotOwned(found) if *found == id),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        format!("O card {id} não é seu nem do pedido atual.")
    );
    assert_eq!(column_of(&db, project_id, &id), "Backlog");

    let current = actor("vexa", "Vexa", Some(&id));
    let results = db
        .apply_card_operations(project_id, &current, &[move_to(&id, "In progress")])
        .unwrap();

    assert_eq!(results[0].outcome, CardOutcome::Moved);
    assert_eq!(column_of(&db, project_id, &id), "In progress");
}

#[test]
fn mover_id_inexistente_e_recusado_e_nada_e_gravado() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");

    let error =
        refused(db.apply_card_operations(project_id, &vexa(), &[move_to("item-99", "Done")]));

    assert!(
        matches!(&error, DbError::BacklogItemNotFound(id) if id == "item-99"),
        "{error:?}"
    );
    assert!(error.is_card_refusal());
    assert!(titles(&db, project_id).is_empty());
}

#[test]
fn titulo_duplicado_ignora_caixa_e_espacos_e_so_libera_com_o_card_em_done() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let id = db
        .apply_card_operations(project_id, &vexa(), &[create("Revisar login")])
        .unwrap()[0]
        .id
        .clone();

    let error =
        refused(db.apply_card_operations(project_id, &vexa(), &[create("  revisar LOGIN ")]));

    assert!(
        matches!(&error, DbError::DuplicateCardTitle(found) if *found == id),
        "{error:?}"
    );
    assert!(error.is_card_refusal());
    assert_eq!(titles(&db, project_id), vec!["Revisar login"]);

    db.apply_card_operations(project_id, &vexa(), &[move_to(&id, "Done")])
        .unwrap();
    db.apply_card_operations(project_id, &vexa(), &[create("Revisar login")])
        .unwrap();

    assert_eq!(
        titles(&db, project_id),
        vec!["Revisar login", "Revisar login"]
    );
}

#[test]
fn dois_criar_com_o_mesmo_titulo_na_mesma_resposta_recusam_e_nenhum_grava() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");

    let error =
        refused(db.apply_card_operations(project_id, &vexa(), &[create("Igual"), create("igual")]));

    assert!(matches!(error, DbError::DuplicateCardTitle(_)), "{error:?}");
    assert!(titles(&db, project_id).is_empty());
}

#[test]
fn criar_valido_mais_mover_invalido_na_mesma_resposta_nao_grava_nenhum_dos_dois() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");

    let error = refused(db.apply_card_operations(
        project_id,
        &vexa(),
        &[create("Valido"), move_to("item-99", "Done")],
    ));

    assert!(
        matches!(error, DbError::BacklogItemNotFound(_)),
        "{error:?}"
    );
    assert!(titles(&db, project_id).is_empty());
}

#[test]
fn repetir_a_mesma_resposta_nao_duplica_o_card() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let operations = [create("Uma vez só")];

    db.apply_card_operations(project_id, &vexa(), &operations)
        .unwrap();
    let error = refused(db.apply_card_operations(project_id, &vexa(), &operations));

    assert!(matches!(error, DbError::DuplicateCardTitle(_)), "{error:?}");
    assert_eq!(titles(&db, project_id), vec!["Uma vez só"]);
}

#[test]
fn card_em_done_nao_sai_e_mover_para_a_coluna_atual_nao_muda_nem_registra_atividade() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let id = db
        .apply_card_operations(project_id, &vexa(), &[create("Um")])
        .unwrap()[0]
        .id
        .clone();

    let same = db
        .apply_card_operations(project_id, &vexa(), &[move_to(&id, "Backlog")])
        .unwrap();

    assert_eq!(same[0].outcome, CardOutcome::Unchanged);
    assert_eq!(activity_count(&db, project_id, &id), 1);

    db.apply_card_operations(project_id, &vexa(), &[move_to(&id, "Done")])
        .unwrap();
    let error =
        refused(db.apply_card_operations(project_id, &vexa(), &[move_to(&id, "In progress")]));

    assert!(
        matches!(&error, DbError::CardClosed(found) if *found == id),
        "{error:?}"
    );
    assert_eq!(column_of(&db, project_id, &id), "Done");
}

#[test]
fn coluna_em_minusculas_e_aceita_e_grava_a_forma_canonica_com_atividade_de_ida() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let id = db
        .apply_card_operations(project_id, &vexa(), &[create("Um")])
        .unwrap()[0]
        .id
        .clone();

    let results = db
        .apply_card_operations(project_id, &vexa(), &[move_to(&id, "in progress")])
        .unwrap();

    assert_eq!(results[0].column, "In progress");
    let item = &db.list_backlog(project_id).unwrap().items[0];
    assert_eq!(
        item.activity.last().unwrap().text,
        "Vexa moveu de Backlog para In progress."
    );
}

#[test]
fn coluna_inexistente_e_card_de_outro_projeto_sao_recusados() {
    let dir = TempDir::new();
    let db = dir.open();
    let first = project(&db, "a");
    let second = project(&db, "b");
    let id = db
        .apply_card_operations(first, &vexa(), &[create("Do A")])
        .unwrap()[0]
        .id
        .clone();

    let column = refused(db.apply_card_operations(first, &vexa(), &[move_to(&id, "Review")]));
    let other_project = refused(db.apply_card_operations(second, &vexa(), &[move_to(&id, "Done")]));

    assert!(
        matches!(column, DbError::InvalidBacklogColumn(_)),
        "{column:?}"
    );
    assert!(
        matches!(other_project, DbError::BacklogItemNotFound(_)),
        "{other_project:?}"
    );
    assert!(titles(&db, second).is_empty());
}
