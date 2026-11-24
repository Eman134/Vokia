use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::agents::Agent;
use crate::db::{AgentActor, CardOperation, CardOutcome, Db, DbError, NewCard, NewProject};

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
        responsible: None,
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
                responsible: None,
            }],
        )
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].outcome, CardOutcome::Created);
    assert_eq!(results[0].column, "Backlog");
    let item = &db.list_backlog(project_id).unwrap().items[0];
    assert_eq!(item.id, results[0].id);
    assert_eq!(item.origin, "agente");
    assert_eq!(item.state, "triagem");
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
        refused(db.apply_card_operations(project_id, &vexa(), &[move_to("item-99", "In review")]));

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

    db.move_backlog_item(project_id, &id, "Done").unwrap();
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
        &[create("Valido"), move_to("item-99", "In review")],
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

    db.move_backlog_item(project_id, &id, "Done").unwrap();
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
    let other_project =
        refused(db.apply_card_operations(second, &vexa(), &[move_to(&id, "In review")]));

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

fn ember() -> AgentActor {
    actor("ember", "Ember", None)
}

#[test]
fn ember_move_card_de_outro_agente_e_aceito_e_a_atividade_registra_quem_moveu() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "ember-move");
    let id = db
        .apply_card_operations(project_id, &axiom(), &[create("Da Axiom")])
        .unwrap()[0]
        .id
        .clone();

    let results = db
        .apply_card_operations(project_id, &ember(), &[move_to(&id, "In progress")])
        .unwrap();

    assert_eq!(results[0].outcome, CardOutcome::Moved);
    assert_eq!(column_of(&db, project_id, &id), "In progress");
    let item = &db.list_backlog(project_id).unwrap().items[0];
    assert_eq!(item.activity.last().unwrap().text, "Ember moveu de Backlog para In progress.");
}

#[test]
fn ember_continua_limitado_as_colunas_e_nao_mexe_em_card_done() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "ember-colunas");
    let id = db
        .apply_card_operations(project_id, &axiom(), &[create("Da Axiom")])
        .unwrap()[0]
        .id
        .clone();

    let error = refused(db.apply_card_operations(project_id, &ember(), &[move_to(&id, "Ready")]));
    assert!(matches!(error, DbError::AgentForbiddenColumn(_)), "{error:?}");

    db.move_backlog_item(project_id, &id, "Done").unwrap();
    let error = refused(db.apply_card_operations(project_id, &ember(), &[move_to(&id, "In progress")]));
    assert!(matches!(error, DbError::CardClosed(_)), "{error:?}");
}

#[test]
fn ember_edita_card_alheio_e_outro_agente_nao() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "ember-edita");
    let id = db
        .apply_card_operations(project_id, &axiom(), &[create("Da Axiom")])
        .unwrap()[0]
        .id
        .clone();
    let update = |title: &str| CardOperation::Update {
        id: id.clone(),
        title: Some(title.to_string()),
        description: None,
    };

    let results = db.apply_card_operations(project_id, &ember(), &[update("Ajustado pelo Ember")]).unwrap();
    assert_eq!(results[0].outcome, CardOutcome::Updated);

    let error = refused(db.apply_card_operations(project_id, &vexa(), &[update("Vexa tenta")]));
    assert!(matches!(&error, DbError::CardNotOwned(found) if *found == id), "{error:?}");
}

fn default_team() -> crate::agents::Team {
    let dir = std::env::temp_dir().join(format!(
        "vokia-responsible-test-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&dir).expect("pasta temporária do elenco");
    let team = crate::agents::AgentRegistry::load(&dir)
        .team()
        .expect("elenco padrão");
    let _ = fs::remove_dir_all(&dir);
    team
}

fn responsible_of(db: &Db, project_id: i64, id: &str) -> String {
    db.list_backlog(project_id)
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.id == id)
        .map(|item| item.who)
        .expect("o card deveria existir")
}

fn last_activity(db: &Db, project_id: i64, id: &str) -> String {
    db.list_backlog(project_id)
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.id == id)
        .and_then(|item| item.activity.last().map(|entry| entry.text.clone()))
        .expect("o card deveria ter atividade")
}

#[test]
fn agente_que_nao_e_o_responsavel_nao_troca_o_responsavel_de_card_alheio() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "responsavel-recusa");
    let team = default_team();
    let id = db
        .apply_card_operations(project_id, &axiom(), &[create("Da Axiom")])
        .unwrap()[0]
        .id
        .clone();

    let error = db
        .set_backlog_responsible(project_id, &id, &agent("vexa"), &team, Some(&agent("vexa")))
        .expect_err("vexa não pode trocar o responsável de card da axiom");

    assert!(matches!(&error, DbError::ResponsibleNotAllowed(found) if *found == id), "{error:?}");
    assert_eq!(responsible_of(&db, project_id, &id), "axiom");
    assert_eq!(activity_count(&db, project_id, &id), 1);
}

#[test]
fn ember_troca_o_responsavel_de_card_alheio_e_a_atividade_registra_quem_trocou() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "responsavel-ember");
    let team = default_team();
    let id = db
        .apply_card_operations(project_id, &axiom(), &[create("Da Axiom")])
        .unwrap()[0]
        .id
        .clone();

    let outcome = db
        .set_backlog_responsible(project_id, &id, &agent("vexa"), &team, Some(&agent("ember")))
        .expect("o ember pode trocar o responsável");

    assert_eq!(outcome, CardOutcome::Updated);
    assert_eq!(responsible_of(&db, project_id, &id), "vexa");
    assert!(
        last_activity(&db, project_id, &id).contains("Ember trocou o responsável"),
        "{}",
        last_activity(&db, project_id, &id)
    );
}

#[test]
fn responsavel_atual_troca_o_proprio_card_e_outro_agente_nao_troca_depois() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "responsavel-dono");
    let team = default_team();
    let id = db
        .apply_card_operations(project_id, &axiom(), &[create("Da Axiom")])
        .unwrap()[0]
        .id
        .clone();
    db.set_backlog_responsible(project_id, &id, &agent("vexa"), &team, Some(&agent("axiom")))
        .expect("a axiom, dona do card, passa para a vexa");

    let error = db
        .set_backlog_responsible(project_id, &id, &agent("axiom"), &team, Some(&agent("axiom")))
        .expect_err("a axiom já não é a responsável");

    assert!(matches!(error, DbError::ResponsibleNotAllowed(_)), "{error:?}");
    assert_eq!(responsible_of(&db, project_id, &id), "vexa");
}

#[test]
fn interface_do_usuario_sem_solicitante_troca_e_a_atividade_diz_voce() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "responsavel-usuario");
    let team = default_team();
    let id = db
        .apply_card_operations(project_id, &axiom(), &[create("Da Axiom")])
        .unwrap()[0]
        .id
        .clone();

    db.set_backlog_responsible(project_id, &id, &agent("vexa"), &team, None)
        .expect("a interface do usuário pode trocar");

    assert_eq!(responsible_of(&db, project_id, &id), "vexa");
    assert!(last_activity(&db, project_id, &id).starts_with("Você trocou"));
}

#[test]
fn card_sem_responsavel_aceita_qualquer_agente_e_o_mesmo_responsavel_nao_registra_nada() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "responsavel-livre");
    let team = default_team();
    // Card criado pelo Ember fica sem responsável definido.
    let id = db
        .apply_card_operations(project_id, &ember(), &[create("Sem dono")])
        .unwrap()[0]
        .id
        .clone();

    let outcome = db
        .set_backlog_responsible(project_id, &id, &agent("vexa"), &team, Some(&agent("vexa")))
        .expect("card sem dono aceita qualquer agente");
    assert_eq!(outcome, CardOutcome::Updated);
    assert_eq!(activity_count(&db, project_id, &id), 2);

    let outcome = db
        .set_backlog_responsible(project_id, &id, &agent("vexa"), &team, Some(&agent("axiom")))
        .expect("trocar para quem já é o responsável não faz nada");
    assert_eq!(outcome, CardOutcome::Unchanged);
    assert_eq!(activity_count(&db, project_id, &id), 2);
}

#[test]
fn reordenar_coluna_atualiza_posicoes_em_transacao_sem_colidir_novos_cards() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "reordenar");
    let created = db
        .apply_card_operations(
            project_id,
            &vexa(),
            &[
                create("Primeiro"),
                create("Segundo"),
                create("Terceiro"),
                create("Quarto"),
            ],
        )
        .unwrap();
    let ids: Vec<String> = created.into_iter().map(|result| result.id).collect();
    db.move_backlog_item(project_id, &ids[1], "Ready").unwrap();

    db.reorder_column_items(
        project_id,
        "Backlog",
        &[ids[3].clone(), ids[0].clone(), ids[2].clone()],
    )
    .expect("reordenar coluna deveria funcionar");

    let backlog_titles: Vec<String> = db
        .list_backlog(project_id)
        .unwrap()
        .items
        .into_iter()
        .filter(|item| item.column.as_deref() == Some("Backlog"))
        .map(|item| item.title)
        .collect();
    assert_eq!(backlog_titles, vec!["Quarto", "Primeiro", "Terceiro"]);

    let fifth = db
        .apply_card_operations(project_id, &vexa(), &[create("Quinto")])
        .expect("novo card após reordenação não deve colidir")
        .pop()
        .unwrap();
    assert_eq!(fifth.id, "quinto");
}

#[test]
fn reordenar_coluna_recusa_duplicata_coluna_invalida_ou_card_de_outra_coluna() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "reordenar-erros");
    let created = db
        .apply_card_operations(
            project_id,
            &vexa(),
            &[create("Um"), create("Dois"), create("Três")],
        )
        .unwrap();
    let ids: Vec<String> = created.into_iter().map(|result| result.id).collect();
    db.move_backlog_item(project_id, &ids[2], "Ready").unwrap();

    assert!(matches!(
        db.reorder_column_items(project_id, "Inexistente", &[ids[0].clone()]),
        Err(DbError::InvalidBacklogColumn(_))
    ));
    assert!(matches!(
        db.reorder_column_items(project_id, "Backlog", &[ids[0].clone(), ids[0].clone()]),
        Err(DbError::InvalidBacklogReorder(_))
    ));
    assert!(matches!(
        db.reorder_column_items(project_id, "Backlog", &[ids[1].clone(), "item-99".to_string()]),
        Err(DbError::BacklogItemNotFound(_))
    ));
    assert!(matches!(
        db.reorder_column_items(project_id, "Backlog", &[ids[1].clone(), ids[2].clone()]),
        Err(DbError::InvalidBacklogReorder(_))
    ));

    // Rollback garante que a ordem original foi mantida.
    assert_eq!(titles(&db, project_id), vec!["Um", "Dois", "Três"]);
}

#[test]
fn definir_prioridade_valida_valores_normaliza_e_limpa_com_vazio() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "prioridade");
    let id = db
        .apply_card_operations(project_id, &vexa(), &[create("Com prioridade")])
        .unwrap()[0]
        .id
        .clone();

    for (input, expected) in [
        ("Baixa", "Baixa"),
        ("média", "Média"),
        ("Alta", "Alta"),
        ("Urgente", "Urgente"),
    ] {
        db.set_item_priority(project_id, &id, input)
            .expect("prioridade válida deveria ser aceita");
        let card = db.read_card(project_id, &id).unwrap();
        assert_eq!(card.priority.as_deref(), Some(expected));
    }

    db.set_item_priority(project_id, &id, "   ")
        .expect("vazio deveria limpar a prioridade");
    assert_eq!(db.read_card(project_id, &id).unwrap().priority, None);

    assert!(matches!(
        db.set_item_priority(project_id, &id, "Crítica"),
        Err(DbError::InvalidBacklogPriority(_))
    ));
    assert!(matches!(
        db.set_item_priority(project_id, "item-99", "Alta"),
        Err(DbError::BacklogItemNotFound(_))
    ));
}

fn created_ids(db: &Db, project_id: i64, titles: &[&str]) -> Vec<String> {
    let operations: Vec<CardOperation> = titles.iter().map(|title| create(title)).collect();
    db.apply_card_operations(project_id, &vexa(), &operations)
        .expect("a criação deveria ser aceita")
        .into_iter()
        .map(|result| result.id)
        .collect()
}

#[test]
fn id_do_card_deriva_do_titulo_sem_acento_caixa_ou_pontuacao() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "slug-basico");

    let ids = created_ids(&db, project_id, &["Revisar Autenticação & Sessões!", "  Ajustar   o  CSS  "]);

    assert_eq!(ids, vec!["revisar-autenticacao-sessoes", "ajustar-o-css"]);
}

#[test]
fn id_do_card_corta_em_fronteira_de_palavra_no_limite_de_tamanho() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "slug-limite");

    let ids = created_ids(
        &db,
        project_id,
        &[
            "Implementar migracao completa do banco de dados legado",
            &"a".repeat(60),
        ],
    );

    assert_eq!(ids[0], "implementar-migracao-completa-do-banco");
    assert!(ids[0].len() <= 40);
    assert_eq!(ids[1], "a".repeat(40));
}

#[test]
fn id_do_card_colidido_ganha_sufixo_numerico_crescente() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "slug-colisao");

    let ids = created_ids(
        &db,
        project_id,
        &["Revisar login", "Revisar login!", "revisar-login", "Revisar  login?"],
    );

    assert_eq!(ids, vec!["revisar-login", "revisar-login-2", "revisar-login-3", "revisar-login-4"]);
}

#[test]
fn id_do_card_nao_colide_com_slug_que_ja_termina_em_numero() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "slug-sufixo");

    let ids = created_ids(&db, project_id, &["Revisar login 2", "Revisar login", "Revisar login!"]);

    assert_eq!(ids, vec!["revisar-login-2", "revisar-login", "revisar-login-3"]);
}

#[test]
fn id_do_card_e_unico_apenas_dentro_do_projeto() {
    let dir = TempDir::new();
    let db = dir.open();
    let first = project(&db, "slug-projeto-a");
    let second = project(&db, "slug-projeto-b");

    assert_eq!(created_ids(&db, first, &["Revisar login"]), vec!["revisar-login"]);
    assert_eq!(created_ids(&db, second, &["Revisar login"]), vec!["revisar-login"]);
}

#[test]
fn titulo_sem_letras_ou_digitos_cai_no_id_sequencial() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "slug-fallback");

    let ids = created_ids(&db, project_id, &["Primeiro", "!!! ???", "日本語"]);

    assert_eq!(ids, vec!["primeiro", "item-2", "item-3"]);
}

#[test]
fn card_criado_pelo_usuario_tambem_recebe_id_derivado_do_titulo() {
    let dir = TempDir::new();
    let db = dir.open();
    let project_id = project(&db, "slug-usuario");
    let new_card = |title: &str| NewCard {
        title: title.to_string(),
        description: None,
        origin: "usuario".to_string(),
        links: Vec::new(),
    };

    let first = db.create_card(project_id, &new_card("Tornar a tela responsiva")).unwrap();
    let second = db.create_card(project_id, &new_card("Tornar a tela responsiva!")).unwrap();

    assert_eq!(first.id, "tornar-a-tela-responsiva");
    assert_eq!(second.id, "tornar-a-tela-responsiva-2");
    assert_eq!(db.read_card(project_id, &first.id).unwrap().title, "Tornar a tela responsiva");
}
