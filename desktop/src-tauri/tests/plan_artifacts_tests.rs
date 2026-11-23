use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use rusqlite::Connection;

use crate::db::backlog::{Children, LinkedPlan};
use crate::db::{BacklogItem, Db, DbError, ItemPlanInput, NewCard, NewProject};

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

const DB_FILE_NAME: &str = "vokia.db";
const PROPOSAL: &str = "Entrega o login completo.\n\
#plano-pronto\n\
#estimativa 3 dias\n\
#lote 1|Base|Base pronta|Criar tabela; Criar API\n\
#lote 2|Tela|Tela funcionando\n\
#risco Prazo curto\n\
#etiquetas auth, ui";
const ADJUSTED_PROPOSAL: &str = "Plano ajustado.\n#lote 1|Só a base|Base mínima|Criar tabela";
const SINGLE_LOT_PROPOSAL: &str = "Resumo\n#lote 1|Único|Meta única";

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        TempDir(std::env::temp_dir().join(format!(
            "vokia-plan-artifacts-test-{}-{}",
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

fn card(db: &Db, project_id: i64, title: &str) -> String {
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
    .id
}

fn proposed(text: &str) -> ItemPlanInput {
    ItemPlanInput {
        state: "proposto".to_string(),
        proposal: Some(text.to_string()),
        plan: None,
        agent: Some("axiom".to_string()),
    }
}

fn linked(item_id: &str, plan_id: &str, text: Option<&str>) -> ItemPlanInput {
    ItemPlanInput {
        state: "vinculado".to_string(),
        proposal: text.map(str::to_string),
        plan: Some(LinkedPlan {
            id: plan_id.to_string(),
            name: format!("Plano de {item_id}"),
            meta: "proposta aceita".to_string(),
        }),
        agent: Some("axiom".to_string()),
    }
}

fn empty() -> ItemPlanInput {
    ItemPlanInput {
        state: "vazio".to_string(),
        proposal: None,
        plan: None,
        agent: None,
    }
}

fn items(db: &Db, project_id: i64) -> Vec<BacklogItem> {
    db.list_backlog(project_id).unwrap().items
}

fn item(db: &Db, project_id: i64, id: &str) -> BacklogItem {
    items(db, project_id)
        .into_iter()
        .find(|item| item.id == id)
        .unwrap_or_else(|| panic!("o card {id} deveria existir"))
}

fn progress(db: &Db, project_id: i64, id: &str) -> Option<(u32, u32)> {
    item(db, project_id, id)
        .children
        .map(|children| (children.total, children.done))
}

fn plan_card_ids(db: &Db, project_id: i64, item_id: &str) -> Vec<String> {
    let prefix = format!("plano-{item_id}.");
    let mut ids: Vec<String> = items(db, project_id)
        .into_iter()
        .map(|item| item.id)
        .filter(|id| id.starts_with(&prefix))
        .collect();
    ids.sort();
    ids
}

fn criteria(db: &Db, project_id: i64) -> Vec<(String, bool)> {
    db.list_artifacts(project_id)
        .unwrap()
        .into_iter()
        .flat_map(|artifact| artifact.criteria)
        .map(|criterion| (criterion.id, criterion.done))
        .collect()
}

#[test]
fn triar_aceitar_concluir_etapas_e_reabrir_o_banco_mantem_filhos_e_progresso() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let item_id = card(&db, project_id, "Login");
    let plan_id = format!("plano-{item_id}");

    db.save_item_plan(project_id, &item_id, &proposed(PROPOSAL)).unwrap();

    let artifacts = db.list_artifacts(project_id).unwrap();
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].id, plan_id);
    assert_eq!(artifacts[0].item_id, item_id);
    assert_eq!(artifacts[0].title, "Login");
    assert_eq!(artifacts[0].summary, "Entrega o login completo.");
    assert_eq!(artifacts[0].estimate, "3 dias");
    assert_eq!(artifacts[0].points, ["Base", "Tela"]);
    assert_eq!(artifacts[0].risks, ["Prazo curto"]);
    assert_eq!(artifacts[0].tags, ["auth", "ui"]);
    assert_eq!(artifacts[0].author.as_deref(), Some("axiom"));
    assert_eq!(
        criteria(&db, project_id),
        [("c1".to_string(), false), ("c2".to_string(), false)]
    );
    assert!(plan_card_ids(&db, project_id, &item_id).is_empty());
    assert_eq!(progress(&db, project_id, &item_id), None);

    db.save_item_plan(project_id, &item_id, &linked(&item_id, &item_id, Some(PROPOSAL)))
        .unwrap();

    assert_eq!(
        plan_card_ids(&db, project_id, &item_id),
        [
            format!("{plan_id}.1"),
            format!("{plan_id}.1.1"),
            format!("{plan_id}.1.2"),
            format!("{plan_id}.2"),
        ]
    );
    let lot_one = item(&db, project_id, &format!("{plan_id}.1"));
    assert_eq!(lot_one.title, "Base");
    assert_eq!(lot_one.parent.as_deref(), Some(item_id.as_str()));
    assert_eq!(lot_one.description.as_deref(), Some("Base pronta"));
    assert_eq!(lot_one.column.as_deref(), Some("Backlog"));
    assert_eq!(lot_one.origin, "agente");
    assert_eq!(lot_one.by, "axiom");
    let task = item(&db, project_id, &format!("{plan_id}.1.2"));
    assert_eq!(task.title, "Criar API");
    assert_eq!(task.parent.as_deref(), Some(format!("{plan_id}.1").as_str()));
    assert_eq!(
        item(&db, project_id, &format!("{plan_id}.2")).parent.as_deref(),
        Some(item_id.as_str())
    );
    assert_eq!(progress(&db, project_id, &item_id), Some((2, 0)));
    assert_eq!(progress(&db, project_id, &format!("{plan_id}.1")), Some((2, 0)));
    assert_eq!(progress(&db, project_id, &format!("{plan_id}.2")), None);

    db.set_artifact_criterion(project_id, &plan_id, "c1", true).unwrap();
    db.save_item_plan(project_id, &item_id, &linked(&item_id, &item_id, Some(PROPOSAL)))
        .unwrap();
    assert_eq!(plan_card_ids(&db, project_id, &item_id).len(), 4);
    assert_eq!(
        criteria(&db, project_id),
        [("c1".to_string(), true), ("c2".to_string(), false)]
    );

    db.move_backlog_item(project_id, &format!("{plan_id}.2"), "Done").unwrap();
    assert_eq!(progress(&db, project_id, &item_id), Some((2, 1)));
    db.close_card(project_id, &format!("{plan_id}.1.1"), "automatico").unwrap();
    assert_eq!(progress(&db, project_id, &format!("{plan_id}.1")), Some((2, 1)));
    assert_eq!(progress(&db, project_id, &item_id), Some((2, 1)));

    drop(db);
    let reopened = dir.open();

    assert_eq!(progress(&reopened, project_id, &item_id), Some((2, 1)));
    assert_eq!(progress(&reopened, project_id, &format!("{plan_id}.1")), Some((2, 1)));
    assert_eq!(plan_card_ids(&reopened, project_id, &item_id).len(), 4);
    assert_eq!(
        criteria(&reopened, project_id),
        [("c1".to_string(), true), ("c2".to_string(), false)]
    );

    reopened.archive_backlog_item(project_id, &format!("{plan_id}.2")).unwrap();
    assert_eq!(progress(&reopened, project_id, &item_id), Some((1, 0)));
    reopened.restore_backlog_item(project_id, &format!("{plan_id}.2")).unwrap();
    assert_eq!(progress(&reopened, project_id, &item_id), Some((2, 1)));
}

#[test]
fn aceitar_duas_vezes_nao_duplica_os_filhos_nem_o_artefato() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let item_id = card(&db, project_id, "Login");
    let accept = linked(&item_id, &item_id, Some(PROPOSAL));

    db.save_item_plan(project_id, &item_id, &accept).unwrap();
    let total_after_first = items(&db, project_id).len();
    db.save_item_plan(project_id, &item_id, &accept).unwrap();

    assert_eq!(items(&db, project_id).len(), total_after_first);
    assert_eq!(db.list_artifacts(project_id).unwrap().len(), 1);
    assert_eq!(progress(&db, project_id, &item_id), Some((2, 0)));
}

#[test]
fn ajuste_depois_de_aceitar_nao_apaga_nem_altera_os_filhos_mas_recria_o_artefato() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let item_id = card(&db, project_id, "Login");
    let plan_id = format!("plano-{item_id}");
    db.save_item_plan(project_id, &item_id, &linked(&item_id, &item_id, Some(PROPOSAL)))
        .unwrap();
    db.move_backlog_item(project_id, &format!("{plan_id}.2"), "Done").unwrap();
    let before = plan_card_ids(&db, project_id, &item_id);

    db.save_item_plan(project_id, &item_id, &proposed(ADJUSTED_PROPOSAL)).unwrap();

    assert_eq!(plan_card_ids(&db, project_id, &item_id), before);
    assert_eq!(item(&db, project_id, &format!("{plan_id}.2")).title, "Tela");
    assert_eq!(item(&db, project_id, &format!("{plan_id}.1.2")).title, "Criar API");
    assert_eq!(progress(&db, project_id, &item_id), Some((2, 1)));
    let artifact = &db.list_artifacts(project_id).unwrap()[0];
    assert_eq!(artifact.summary, "Plano ajustado.");
    assert_eq!(artifact.points, ["Só a base"]);
    assert_eq!(criteria(&db, project_id), [("c1".to_string(), false)]);
}

#[test]
fn vincular_a_plano_de_outro_item_nao_cria_cards_nem_artefato() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let item_id = card(&db, project_id, "Login");
    let other = card(&db, project_id, "Cadastro");
    let total = items(&db, project_id).len();

    db.save_item_plan(project_id, &item_id, &linked(&item_id, &other, None)).unwrap();
    db.save_item_plan(project_id, &item_id, &linked(&item_id, &other, Some(PROPOSAL)))
        .unwrap();

    assert_eq!(items(&db, project_id).len(), total);
    assert!(db.list_artifacts(project_id).unwrap().is_empty());
    let plans = db.list_backlog(project_id).unwrap().plans;
    assert_eq!(plans[0].state, "vinculado");
    assert_eq!(plans[0].plan.as_ref().map(|plan| plan.id.as_str()), Some(other.as_str()));
}

#[test]
fn descartar_a_proposta_apaga_artefato_e_criterios() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let item_id = card(&db, project_id, "Login");
    db.save_item_plan(project_id, &item_id, &proposed(PROPOSAL)).unwrap();

    db.save_item_plan(project_id, &item_id, &empty()).unwrap();

    assert!(db.list_artifacts(project_id).unwrap().is_empty());
    assert!(criteria(&db, project_id).is_empty());
    let raw: i64 = dir
        .raw()
        .query_row("SELECT COUNT(*) FROM artifact_criteria", [], |row| row.get(0))
        .unwrap();
    assert_eq!(raw, 0);
    assert!(db.list_backlog(project_id).unwrap().plans.is_empty());
}

#[test]
fn desvincular_o_plano_aceito_mantem_artefato_criterios_e_filhos() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let item_id = card(&db, project_id, "Login");
    db.save_item_plan(project_id, &item_id, &linked(&item_id, &item_id, Some(PROPOSAL)))
        .unwrap();

    db.save_item_plan(project_id, &item_id, &empty()).unwrap();

    assert_eq!(db.list_artifacts(project_id).unwrap().len(), 1);
    assert_eq!(criteria(&db, project_id).len(), 2);
    assert_eq!(plan_card_ids(&db, project_id, &item_id).len(), 4);
    assert!(db.list_backlog(project_id).unwrap().plans.is_empty());
}

#[test]
fn fechar_por_validacao_automatica_e_reabrir_atualizam_o_progresso_do_pai() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let item_id = card(&db, project_id, "Login");
    let plan_id = format!("plano-{item_id}");
    db.save_item_plan(project_id, &item_id, &linked(&item_id, &item_id, Some(PROPOSAL)))
        .unwrap();
    let lot_two = format!("{plan_id}.2");

    db.close_card(project_id, &lot_two, "automatico").unwrap();
    assert_eq!(progress(&db, project_id, &item_id), Some((2, 1)));

    db.reopen_card(project_id, &lot_two).unwrap();
    assert_eq!(progress(&db, project_id, &item_id), Some((2, 0)));
    assert_eq!(item(&db, project_id, &lot_two).column.as_deref(), Some("In progress"));

    db.advance_card(project_id, &lot_two, "In review").unwrap();
    assert_eq!(progress(&db, project_id, &item_id), Some((2, 0)));
}

#[test]
fn salvar_o_item_com_contagem_antiga_nao_sobrescreve_o_progresso_gravado() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let item_id = card(&db, project_id, "Login");
    db.save_item_plan(project_id, &item_id, &linked(&item_id, &item_id, Some(PROPOSAL)))
        .unwrap();
    let mut stale = item(&db, project_id, &item_id);
    stale.children = Some(Children { total: 99, done: 99 });

    db.save_backlog_item(project_id, &stale).unwrap();

    assert_eq!(progress(&db, project_id, &item_id), Some((2, 0)));
}

#[test]
fn excluir_o_item_depois_de_desvincular_nao_deixa_artefato_orfao() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let item_id = card(&db, project_id, "Login");
    db.save_item_plan(project_id, &item_id, &linked(&item_id, &item_id, Some(SINGLE_LOT_PROPOSAL)))
        .unwrap();

    let blocked = db.delete_backlog_item(project_id, &item_id).unwrap_err();
    assert!(matches!(blocked, DbError::BacklogDeleteWithPlan), "{blocked:?}");

    db.save_item_plan(project_id, &item_id, &empty()).unwrap();
    db.delete_backlog_item(project_id, &item_id).unwrap();

    assert!(db.list_artifacts(project_id).unwrap().is_empty());
    let orphans: i64 = dir
        .raw()
        .query_row(
            "SELECT (SELECT COUNT(*) FROM artifacts) + (SELECT COUNT(*) FROM artifact_criteria)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(orphans, 0);
}

#[test]
fn criterio_de_outro_projeto_ou_inexistente_e_recusado_e_marcar_duas_vezes_nao_falha() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_a = project(&db, "a");
    let project_b = project(&db, "b");
    let item_id = card(&db, project_a, "Login");
    let plan_id = format!("plano-{item_id}");
    db.save_item_plan(project_a, &item_id, &proposed(PROPOSAL)).unwrap();

    let foreign = db.set_artifact_criterion(project_b, &plan_id, "c1", true).unwrap_err();
    assert!(matches!(foreign, DbError::ArtifactNotFound(ref id) if id == &plan_id), "{foreign:?}");
    let missing = db.set_artifact_criterion(project_a, "plano-item-99", "c1", true).unwrap_err();
    assert!(matches!(missing, DbError::ArtifactNotFound(_)), "{missing:?}");
    let wrong = db.set_artifact_criterion(project_a, &plan_id, "c9", true).unwrap_err();
    assert!(matches!(wrong, DbError::CriterionNotFound(ref id) if id == "c9"), "{wrong:?}");
    assert_eq!(
        criteria(&db, project_a),
        [("c1".to_string(), false), ("c2".to_string(), false)]
    );

    db.set_artifact_criterion(project_a, &plan_id, "c2", true).unwrap();
    db.set_artifact_criterion(project_a, &plan_id, "c2", true).unwrap();
    assert_eq!(
        criteria(&db, project_a),
        [("c1".to_string(), false), ("c2".to_string(), true)]
    );
    db.set_artifact_criterion(project_a, &plan_id, "c2", false).unwrap();
    assert_eq!(criteria(&db, project_a)[1], ("c2".to_string(), false));
    assert!(criteria(&db, project_b).is_empty());
}

#[test]
fn migracao_converte_plano_e_micro_tarefas_sem_criar_etapas_e_nao_repete_ao_reabrir() {
    let dir = TempDir::new();
    let (project_id, planned, with_micro) = {
        let db = dir.open();
        let project_id = project(&db, "a");
        let planned = card(&db, project_id, "Login");
        let with_micro = card(&db, project_id, "Cadastro");
        (project_id, planned, with_micro)
    };
    let raw = dir.raw();
    raw.execute(
        "INSERT INTO item_plans (project_id, item_id, state, text, plan_id, plan_name, plan_meta, agent, linked_at) \
         VALUES (?1, ?2, 'vinculado', ?3, ?2, 'Plano', 'meta', 'axiom', 0)",
        rusqlite::params![project_id, planned, PROPOSAL],
    )
    .unwrap();
    raw.execute(
        "UPDATE backlog_items SET tasks = ?3 WHERE project_id = ?1 AND id = ?2",
        rusqlite::params![
            project_id,
            with_micro,
            r#"[{"title":"Micro A","status":"feito","who":"vexa"},{"title":"Micro B","status":"ready"},{"title":"  "}]"#
        ],
    )
    .unwrap();
    drop(raw);

    let db = dir.open();

    let artifacts = db.list_artifacts(project_id).unwrap();
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].id, format!("plano-{planned}"));
    assert_eq!(artifacts[0].points, ["Base", "Tela"]);
    assert_eq!(criteria(&db, project_id).len(), 2);
    assert!(plan_card_ids(&db, project_id, &planned).is_empty());
    assert_eq!(progress(&db, project_id, &planned), None);
    let micro_a = item(&db, project_id, &format!("micro-{with_micro}.1"));
    assert_eq!(micro_a.title, "Micro A");
    assert_eq!(micro_a.column.as_deref(), Some("Done"));
    assert_eq!(micro_a.parent.as_deref(), Some(with_micro.as_str()));
    assert_eq!(micro_a.who, "vexa");
    assert_eq!(
        item(&db, project_id, &format!("micro-{with_micro}.2")).column.as_deref(),
        Some("Ready")
    );
    assert_eq!(progress(&db, project_id, &with_micro), Some((2, 1)));
    let cards_before = items(&db, project_id).len();
    drop(db);

    let reopened = dir.open();

    assert_eq!(items(&reopened, project_id).len(), cards_before);
    assert_eq!(reopened.list_artifacts(project_id).unwrap().len(), 1);
    assert_eq!(progress(&reopened, project_id, &with_micro), Some((2, 1)));
    assert!(item(&reopened, project_id, &with_micro).micro.is_empty());
}

const RENAMED_PROPOSAL: &str = "Entrega o login completo.\n\
#plano-pronto\n\
#lote 1|Base|Base pronta|Criar tabela\n\
#lote 2|Tela nova|Tela funcionando\n\
#lote 3|Deploy|Em produção";
const KEPT_AND_CHANGED_META: &str = "Entrega o login completo.\n\
#lote 1|Base|Base pronta|Criar tabela; Criar API\n\
#lote 2|Tela|Tela revisada";
const WITHOUT_LOT_TWO: &str = "Entrega o login completo.\n#lote 1|Base|Base pronta|Criar tabela; Criar API";
const WITHOUT_LOT_ONE: &str = "Entrega o login completo.\n#lote 2|Tela|Tela funcionando";
const NO_LOTS_PROPOSAL: &str = "Só um texto livre, sem etapas.\n#plano-pronto\n#risco Nenhum";

fn archived(dir: &TempDir, project_id: i64, id: &str) -> bool {
    dir.raw()
        .query_row(
            "SELECT archived_at IS NOT NULL FROM backlog_items WHERE project_id = ?1 AND id = ?2",
            rusqlite::params![project_id, id],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| panic!("o card {id} deveria existir no banco"))
}

fn stored_plan_text(dir: &TempDir, project_id: i64, item_id: &str) -> String {
    dir.raw()
        .query_row(
            "SELECT text FROM item_plans WHERE project_id = ?1 AND item_id = ?2",
            rusqlite::params![project_id, item_id],
            |row| row.get(0),
        )
        .expect("o item deveria ter plano gravado")
}

// Os fluxos de agente levam o card a In progress/In review; move_backlog_item só aceita colunas movíveis à mão.
fn set_column(dir: &TempDir, project_id: i64, id: &str, column: &str) {
    let changed = dir
        .raw()
        .execute(
            "UPDATE backlog_items SET \"column\" = ?3 WHERE project_id = ?1 AND id = ?2",
            rusqlite::params![project_id, id, column],
        )
        .unwrap();
    assert_eq!(changed, 1, "o card {id} deveria existir");
}

fn accepted(db: &Db, project_id: i64, title: &str) -> (String, String) {
    let item_id = card(db, project_id, title);
    db.save_item_plan(project_id, &item_id, &linked(&item_id, &item_id, Some(PROPOSAL)))
        .unwrap();
    let plan_id = format!("plano-{item_id}");
    (item_id, plan_id)
}

#[test]
fn editar_plano_mantem_marcacao_do_criterio_igual_e_desmarca_o_que_mudou_de_texto() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let item_id = card(&db, project_id, "Login");
    let plan_id = format!("plano-{item_id}");
    db.save_item_plan(project_id, &item_id, &proposed(PROPOSAL)).unwrap();
    db.set_artifact_criterion(project_id, &plan_id, "c1", true).unwrap();
    db.set_artifact_criterion(project_id, &plan_id, "c2", true).unwrap();

    let artifact = db
        .update_item_plan(project_id, &item_id, KEPT_AND_CHANGED_META, None)
        .unwrap();

    let marks: Vec<(String, String, bool)> = artifact
        .criteria
        .iter()
        .map(|criterion| (criterion.id.clone(), criterion.text.clone(), criterion.done))
        .collect();
    assert_eq!(
        marks,
        [
            ("c1".to_string(), "Base pronta".to_string(), true),
            ("c2".to_string(), "Tela revisada".to_string(), false),
        ]
    );
    assert_eq!(
        criteria(&db, project_id),
        [("c1".to_string(), true), ("c2".to_string(), false)]
    );
    assert_eq!(stored_plan_text(&dir, project_id, &item_id), KEPT_AND_CHANGED_META);
    assert!(plan_card_ids(&db, project_id, &item_id).is_empty(), "plano proposto não cria etapas");
}

#[test]
fn editar_plano_aceito_renomeia_insere_e_arquiva_etapas_sem_duplicar() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let (item_id, plan_id) = accepted(&db, project_id, "Login");
    set_column(&dir, project_id, &format!("{plan_id}.1.2"), "Ready");

    db.update_item_plan(project_id, &item_id, RENAMED_PROPOSAL, None).unwrap();

    assert_eq!(
        plan_card_ids(&db, project_id, &item_id),
        [
            format!("{plan_id}.1"),
            format!("{plan_id}.1.1"),
            format!("{plan_id}.2"),
            format!("{plan_id}.3"),
        ]
    );
    assert_eq!(item(&db, project_id, &format!("{plan_id}.2")).title, "Tela nova");
    let added = item(&db, project_id, &format!("{plan_id}.3"));
    assert_eq!(added.title, "Deploy");
    assert_eq!(added.parent.as_deref(), Some(item_id.as_str()));
    assert_eq!(added.description.as_deref(), Some("Em produção"));
    assert!(archived(&dir, project_id, &format!("{plan_id}.1.2")), "tarefa removida em Ready é arquivada");
    assert_eq!(progress(&db, project_id, &item_id), Some((3, 0)));
    assert_eq!(progress(&db, project_id, &format!("{plan_id}.1")), Some((1, 0)));

    let total = items(&db, project_id).len();
    db.update_item_plan(project_id, &item_id, RENAMED_PROPOSAL, None).unwrap();
    assert_eq!(items(&db, project_id).len(), total, "repetir a edição não duplica cards");
}

#[test]
fn remover_a_ultima_etapa_em_backlog_arquiva_a_etapa_e_suas_tarefas() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let (item_id, plan_id) = accepted(&db, project_id, "Login");
    let with_task = "Entrega o login completo.\n#lote 1|Base|Base pronta|Criar tabela; Criar API\n#lote 2|Tela|Tela funcionando|Montar form";
    db.update_item_plan(project_id, &item_id, with_task, None).unwrap();
    assert!(!archived(&dir, project_id, &format!("{plan_id}.2.1")));

    db.update_item_plan(project_id, &item_id, WITHOUT_LOT_TWO, None).unwrap();

    assert!(archived(&dir, project_id, &format!("{plan_id}.2")));
    assert!(archived(&dir, project_id, &format!("{plan_id}.2.1")), "a tarefa da etapa removida também sai");
    assert_eq!(
        plan_card_ids(&db, project_id, &item_id),
        [format!("{plan_id}.1"), format!("{plan_id}.1.1"), format!("{plan_id}.1.2")]
    );
    assert_eq!(progress(&db, project_id, &item_id), Some((1, 0)));
}

#[test]
fn remover_a_primeira_etapa_arquiva_o_card_dela_e_nao_renomeia_o_card_iniciado() {
    // #lote 2 sem o #lote 1 é "remover a etapa 1": o card plano-X.1 sai e plano-X.2 continua sendo a Tela.
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let (item_id, plan_id) = accepted(&db, project_id, "Login");
    let first = format!("{plan_id}.1");
    set_column(&dir, project_id, &first, "In progress");

    let result = db.update_item_plan(project_id, &item_id, WITHOUT_LOT_ONE, None);

    assert!(
        matches!(result, Err(DbError::PlanStageStarted(ref id)) if id == &first),
        "remover a etapa 1 em andamento deveria ser recusado: {:?}",
        result.map(|artifact| artifact.points)
    );
    assert_eq!(item(&db, project_id, &first).title, "Base");
    assert!(!archived(&dir, project_id, &format!("{plan_id}.2")), "a etapa 2 (Tela) não foi removida");
}

#[test]
fn remover_etapa_iniciada_e_recusado_e_nada_do_plano_muda() {
    for column in ["In progress", "In review", "Done"] {
        let dir = TempDir::new();
        let db = dir.open();
        let project_id = project(&db, "a");
        let (item_id, plan_id) = accepted(&db, project_id, "Login");
        let stage = format!("{plan_id}.2");
        set_column(&dir, project_id, &stage, column);
        let updated_before = db.list_artifacts(project_id).unwrap()[0].updated_at;

        let refused = db
            .update_item_plan(project_id, &item_id, WITHOUT_LOT_TWO, None)
            .unwrap_err();

        assert!(
            matches!(refused, DbError::PlanStageStarted(ref id) if id == &stage),
            "{column}: {refused:?}"
        );
        assert!(!archived(&dir, project_id, &stage), "{column}: etapa iniciada não pode ser arquivada");
        assert_eq!(stored_plan_text(&dir, project_id, &item_id), PROPOSAL, "{column}: o texto do plano deveria voltar atrás");
        let artifact = &db.list_artifacts(project_id).unwrap()[0];
        assert_eq!(artifact.points, ["Base", "Tela"], "{column}");
        assert_eq!(artifact.updated_at, updated_before, "{column}");
    }
}

#[test]
fn remover_tarefa_iniciada_e_recusado_apontando_a_tarefa() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let (item_id, plan_id) = accepted(&db, project_id, "Login");
    let task = format!("{plan_id}.1.2");
    set_column(&dir, project_id, &task, "In progress");
    let one_task = "Entrega o login completo.\n#lote 1|Base|Base pronta|Criar tabela\n#lote 2|Tela|Tela funcionando";

    let refused = db.update_item_plan(project_id, &item_id, one_task, None).unwrap_err();

    assert!(matches!(refused, DbError::PlanStageStarted(ref id) if id == &task), "{refused:?}");
    assert!(!archived(&dir, project_id, &task));
    assert_eq!(stored_plan_text(&dir, project_id, &item_id), PROPOSAL);
}
#[test]
fn editar_com_updated_at_antigo_e_recusado_por_conflito_sem_gravar() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let item_id = card(&db, project_id, "Login");
    let plan_id = format!("plano-{item_id}");
    db.save_item_plan(project_id, &item_id, &proposed(PROPOSAL)).unwrap();
    let read = db.list_artifacts(project_id).unwrap()[0].updated_at;

    let first = db
        .update_item_plan(project_id, &item_id, KEPT_AND_CHANGED_META, Some(read))
        .unwrap();
    assert!(first.updated_at > read, "a gravação deveria avançar updated_at");

    let stale = db
        .update_item_plan(project_id, &item_id, PROPOSAL, Some(read))
        .unwrap_err();

    assert!(matches!(stale, DbError::ArtifactConflict(ref id) if id == &plan_id), "{stale:?}");
    assert_eq!(stored_plan_text(&dir, project_id, &item_id), KEPT_AND_CHANGED_META);
    assert_eq!(db.list_artifacts(project_id).unwrap()[0].updated_at, first.updated_at);
}

#[test]
fn editar_plano_inexistente_de_outro_item_ou_sem_lote_e_recusado() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let bare = card(&db, project_id, "Sem plano");
    let item_id = card(&db, project_id, "Login");
    let borrower = card(&db, project_id, "Usa plano alheio");
    db.save_item_plan(project_id, &item_id, &proposed(PROPOSAL)).unwrap();
    db.save_item_plan(project_id, &borrower, &linked(&borrower, &item_id, None)).unwrap();

    let none = db.update_item_plan(project_id, &bare, PROPOSAL, None).unwrap_err();
    assert!(matches!(none, DbError::PlanNotFound), "{none:?}");
    let foreign = db.update_item_plan(project_id, &borrower, PROPOSAL, None).unwrap_err();
    assert!(matches!(foreign, DbError::PlanNotFound), "{foreign:?}");
    let no_lots = db.update_item_plan(project_id, &item_id, NO_LOTS_PROPOSAL, None).unwrap_err();
    assert!(matches!(no_lots, DbError::PlanWithoutStages), "{no_lots:?}");
    assert_eq!(stored_plan_text(&dir, project_id, &item_id), PROPOSAL);
}

#[test]
fn importar_plano_cria_card_com_plano_proposto_sem_etapas() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let before = items(&db, project_id).len();

    let imported = db.import_plan(project_id, "  Login importado  ", PROPOSAL, Some("axiom")).unwrap();

    assert_eq!(items(&db, project_id).len(), before + 1);
    assert_eq!(imported.title, "Login importado");
    let artifacts = db.list_artifacts(project_id).unwrap();
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].id, format!("plano-{}", imported.id));
    assert_eq!(artifacts[0].item_id, imported.id);
    assert_eq!(artifacts[0].points, ["Base", "Tela"]);
    assert_eq!(artifacts[0].author.as_deref(), Some("axiom"));
    assert_eq!(stored_plan_text(&dir, project_id, &imported.id), PROPOSAL);
    let plans = db.list_backlog(project_id).unwrap().plans;
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].state, "proposto");
    assert!(plan_card_ids(&db, project_id, &imported.id).is_empty());
}

#[test]
fn importar_plano_sem_lote_ou_em_projeto_inexistente_nao_deixa_card_orfao() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let before = items(&db, project_id).len();

    let no_lots = db.import_plan(project_id, "Livre", NO_LOTS_PROPOSAL, None).unwrap_err();
    assert!(matches!(no_lots, DbError::PlanWithoutStages), "{no_lots:?}");
    let blank = db.import_plan(project_id, "Vazio", "   \n ", None).unwrap_err();
    assert!(matches!(blank, DbError::InvalidBacklogPlan(_)), "{blank:?}");
    let missing_project = db.import_plan(project_id + 99, "Outro", PROPOSAL, None).unwrap_err();
    assert!(!matches!(missing_project, DbError::PlanWithoutStages), "{missing_project:?}");

    assert_eq!(items(&db, project_id).len(), before);
    let orphans: i64 = dir
        .raw()
        .query_row(
            "SELECT (SELECT COUNT(*) FROM backlog_items) + (SELECT COUNT(*) FROM item_plans) \
             + (SELECT COUNT(*) FROM artifacts) + (SELECT COUNT(*) FROM artifact_criteria)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(orphans, before as i64);

    let ok = db.import_plan(project_id, "Válido", SINGLE_LOT_PROPOSAL, None).unwrap();
    assert_eq!(items(&db, project_id).len(), before + 1);
    assert_eq!(db.list_artifacts(project_id).unwrap()[0].item_id, ok.id);
}

fn table_count(dir: &TempDir, table: &str) -> i64 {
    dir.raw()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
        .unwrap()
}

#[test]
fn excluir_plano_aceito_apaga_artefato_criterios_vinculo_e_todas_as_etapas_e_mantem_o_card_pai() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let (item_id, plan_id) = accepted(&db, project_id, "Login");
    assert_eq!(plan_card_ids(&db, project_id, &item_id).len(), 4);

    db.delete_plan_cascade(project_id, &plan_id).unwrap();

    assert!(db.list_artifacts(project_id).unwrap().is_empty());
    assert!(plan_card_ids(&db, project_id, &item_id).is_empty());
    assert!(db.list_backlog(project_id).unwrap().plans.is_empty());
    assert_eq!(item(&db, project_id, &item_id).title, "Login");
    assert_eq!(progress(&db, project_id, &item_id), None);
    for table in ["artifacts", "artifact_criteria", "item_plans"] {
        assert_eq!(table_count(&dir, table), 0, "{table} não deveria ter órfãos");
    }
    assert_eq!(table_count(&dir, "backlog_items"), 1);
}

#[test]
fn excluir_plano_so_apaga_as_etapas_do_proprio_plano_mesmo_com_prefixo_de_id_parecido() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let other_project = project(&db, "b");
    let mut accepted_plans = Vec::new();
    for index in 0..10 {
        accepted_plans.push(accepted(&db, project_id, &format!("Card {index}")));
    }
    let (other_item, other_plan) = accepted(&db, other_project, "Outro");
    let (target_item, target_plan) = accepted_plans.remove(0);
    let ids: Vec<String> = accepted_plans.iter().map(|(_, plan)| plan.clone()).collect();
    assert!(ids.iter().any(|id| id.starts_with(&target_plan)), "o cenário precisa de prefixo parecido");

    db.delete_plan_cascade(project_id, &target_plan).unwrap();

    assert!(plan_card_ids(&db, project_id, &target_item).is_empty());
    for (item_id, plan_id) in &accepted_plans {
        assert_eq!(plan_card_ids(&db, project_id, item_id).len(), 4, "{plan_id} deveria manter as etapas");
    }
    assert_eq!(db.list_artifacts(project_id).unwrap().len(), 9);
    assert_eq!(plan_card_ids(&db, other_project, &other_item).len(), 4);
    assert_eq!(db.list_artifacts(other_project).unwrap()[0].id, other_plan);
}

#[test]
fn excluir_plano_ainda_proposto_apaga_artefato_e_criterios_sem_etapas() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let item_id = card(&db, project_id, "Login");
    db.save_item_plan(project_id, &item_id, &proposed(PROPOSAL)).unwrap();

    db.delete_plan_cascade(project_id, &format!("plano-{item_id}")).unwrap();

    for table in ["artifacts", "artifact_criteria", "item_plans"] {
        assert_eq!(table_count(&dir, table), 0, "{table} não deveria ter órfãos");
    }
    assert_eq!(item(&db, project_id, &item_id).title, "Login");
}

#[test]
fn excluir_plano_inexistente_ou_de_outro_projeto_e_recusado_e_nada_muda() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let other_project = project(&db, "b");
    let (item_id, plan_id) = accepted(&db, project_id, "Login");

    let missing = db.delete_plan_cascade(project_id, "plano-999").unwrap_err();
    assert!(matches!(missing, DbError::ArtifactNotFound(ref id) if id == "plano-999"), "{missing:?}");
    let foreign = db.delete_plan_cascade(other_project, &plan_id).unwrap_err();
    assert!(matches!(foreign, DbError::ArtifactNotFound(_)), "{foreign:?}");

    assert_eq!(db.list_artifacts(project_id).unwrap().len(), 1);
    assert_eq!(plan_card_ids(&db, project_id, &item_id).len(), 4);
}

#[test]
fn excluir_o_item_com_cascade_remove_item_plano_e_etapas_filhas() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "a");
    let item_id = card(&db, project_id, "Login");
    db.save_item_plan(project_id, &item_id, &linked(&item_id, &item_id, Some(SINGLE_LOT_PROPOSAL)))
        .unwrap();

    let blocked = db
        .delete_backlog_item_with_cascade(project_id, &item_id, false)
        .unwrap_err();
    assert!(matches!(blocked, DbError::BacklogDeleteWithPlan), "{blocked:?}");

    db.delete_backlog_item_with_cascade(project_id, &item_id, true)
        .unwrap();

    assert!(db.list_artifacts(project_id).unwrap().is_empty());
    let remaining: i64 = dir
        .raw()
        .query_row(
            "SELECT (SELECT COUNT(*) FROM backlog_items) + (SELECT COUNT(*) FROM item_plans) + (SELECT COUNT(*) FROM artifacts) + (SELECT COUNT(*) FROM artifact_criteria)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(remaining, 0);
}
