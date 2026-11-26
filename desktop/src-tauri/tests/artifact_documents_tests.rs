use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use rusqlite::Connection;

use super::*;
use crate::db::artifact_kinds::ARTIFACT_KINDS;
use crate::db::{Db, DbError, ItemPlanInput, NewCard, NewProject};

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

const DB_FILE_NAME: &str = "vokia.db";
const PLAN_PROPOSAL: &str = "Entrega o login completo.\n\
#plano-pronto\n\
#estimativa 3 dias\n\
#lote 1|Base|Base pronta|Criar tabela; Criar API\n\
#lote 2|Tela|Tela funcionando\n\
#risco Prazo curto\n\
#etiquetas auth, ui";

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        TempDir(std::env::temp_dir().join(format!(
            "vokia-artifact-documents-test-{}-{}",
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

fn create_project(db: &Db, name: &str) -> i64 {
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

fn create_card_with_plan(db: &Db, project_id: i64, title: &str) -> (String, String) {
    let card = db
        .create_card(
            project_id,
            &NewCard {
                title: title.to_string(),
                description: None,
                origin: "usuario".to_string(),
                links: Vec::new(),
            },
        )
        .expect("o card deveria ser criado");

    db.save_item_plan(
        project_id,
        &card.id,
        &ItemPlanInput {
            state: "proposto".to_string(),
            proposal: Some(PLAN_PROPOSAL.to_string()),
            plan: None,
            agent: Some("axiom".to_string()),
            public_id: None,
        },
    )
    .expect("o plano deveria ser gravado");

    let plan_id = format!("plano-{}", card.id);
    (card.id, plan_id)
}

#[test]
fn migracao_com_banco_antigo_preserva_planos_e_criterios_intactos() {
    let dir = TempDir::new();
    let project_id;
    let plan_id;

    {
        let db = dir.open();
        project_id = create_project(&db, "migracao_test");
        let (_, pid) = create_card_with_plan(&db, project_id, "Card Inicial");
        plan_id = pid;

        let plans = db.list_artifacts(project_id).expect("list_artifacts");
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].id, plan_id);
        assert_eq!(plans[0].criteria.len(), 2);
    }

    // Reabre o banco acionando migrate()
    {
        let db = dir.open();

        // Confirma que a tabela artifact_documents existe
        let raw = dir.raw();
        let table_exists: bool = raw
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'artifact_documents'",
                [],
                |row| row.get(0),
            )
            .expect("consulta sqlite_master");
        assert!(table_exists, "a tabela artifact_documents deve existir após migração");

        // Planos e critérios permanecem intactos
        let plans = db.list_artifacts(project_id).expect("list_artifacts pós reabertura");
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].id, plan_id);
        assert_eq!(plans[0].criteria.len(), 2);

        // get_artifact unificado enxerga o plano com os critérios
        let detail = db.get_artifact(project_id, &plan_id).expect("get_artifact");
        match detail {
            ArtifactDetail::Plan(plan) => {
                assert_eq!(plan.id, plan_id);
                assert_eq!(plan.criteria.len(), 2);
            }
            ArtifactDetail::Document(_) => panic!("deveria ser ArtifactDetail::Plan"),
        }
    }
}

#[test]
fn criacao_de_documentos_usa_template_e_sequencia_independente_por_projeto() {
    let dir = TempDir::new();
    let db = dir.open();
    let p1 = create_project(&db, "p1");
    let p2 = create_project(&db, "p2");

    // Criação sem body usa template do catálogo
    let doc1 = db
        .create_artifact_document(
            p1,
            NewArtifactDocument {
                kind: "checklist".to_string(),
                title: "Checklist Release".to_string(),
                summary: None,
                body: None,
                tags: vec!["release".to_string()],
                author: Some("Tester".to_string()),
            },
        )
        .expect("criar doc1");

    assert_eq!(doc1.id, "checklist-1");
    assert_eq!(doc1.kind, "checklist");
    assert_eq!(doc1.title, "Checklist Release");
    assert_eq!(doc1.summary, "");
    assert_eq!(doc1.body, "- [ ] Primeiro item\n");
    assert_eq!(doc1.tags, vec!["release"]);
    assert_eq!(doc1.author, Some("Tester".to_string()));

    // Segundo documento do mesmo tipo no mesmo projeto gera seq = 2
    let doc2 = db
        .create_artifact_document(
            p1,
            NewArtifactDocument {
                kind: "checklist".to_string(),
                title: "Checklist Deploy".to_string(),
                summary: Some("Passos do deploy".to_string()),
                body: Some("- [ ] Passo 1".to_string()),
                tags: vec![],
                author: None,
            },
        )
        .expect("criar doc2");

    assert_eq!(doc2.id, "checklist-2");
    assert_eq!(doc2.body, "- [ ] Passo 1");

    // Projeto p2 deve começar com seq = 1 para o mesmo tipo
    let doc_p2 = db
        .create_artifact_document(
            p2,
            NewArtifactDocument {
                kind: "checklist".to_string(),
                title: "Checklist Outro Projeto".to_string(),
                summary: None,
                body: None,
                tags: vec![],
                author: None,
            },
        )
        .expect("criar doc_p2");

    assert_eq!(doc_p2.id, "checklist-1", "sequência deve ser independente por projeto");

    // Tipo diferente no p1 recebe seq = 1
    let espec = db
        .create_artifact_document(
            p1,
            NewArtifactDocument {
                kind: "especificacao".to_string(),
                title: "Espec Auth".to_string(),
                summary: None,
                body: None,
                tags: vec![],
                author: None,
            },
        )
        .expect("criar especificação");

    assert_eq!(espec.id, "especificacao-1");
    assert!(espec.body.contains("## Objetivo"));
}

#[test]
fn tipo_invalido_e_recusado_pelo_catalogo_rust() {
    let dir = TempDir::new();
    let db = dir.open();
    let p = create_project(&db, "p_tipo");

    let err = db
        .create_artifact_document(
            p,
            NewArtifactDocument {
                kind: "tipo_inexistente".to_string(),
                title: "Doc".to_string(),
                summary: None,
                body: None,
                tags: vec![],
                author: None,
            },
        )
        .unwrap_err();

    match err {
        DbError::UnknownArtifactKind(kind) => assert_eq!(kind, "tipo_inexistente"),
        other => panic!("erro inesperado: {other:?}"),
    }
}

#[test]
fn titulo_vazio_ou_so_com_espacos_e_recusado() {
    let dir = TempDir::new();
    let db = dir.open();
    let p = create_project(&db, "p_titulo");

    // Criação com título vazio
    let err_empty = db
        .create_artifact_document(
            p,
            NewArtifactDocument {
                kind: "documento".to_string(),
                title: "".to_string(),
                summary: None,
                body: None,
                tags: vec![],
                author: None,
            },
        )
        .unwrap_err();
    assert!(matches!(err_empty, DbError::EmptyArtifactTitle));

    // Criação com título só de espaços
    let err_spaces = db
        .create_artifact_document(
            p,
            NewArtifactDocument {
                kind: "documento".to_string(),
                title: "   \t\n  ".to_string(),
                summary: None,
                body: None,
                tags: vec![],
                author: None,
            },
        )
        .unwrap_err();
    assert!(matches!(err_spaces, DbError::EmptyArtifactTitle));

    // Update com título vazio
    let doc = db
        .create_artifact_document(
            p,
            NewArtifactDocument {
                kind: "documento".to_string(),
                title: "Título Válido".to_string(),
                summary: None,
                body: None,
                tags: vec![],
                author: None,
            },
        )
        .expect("criar doc");

    let err_patch = db
        .update_artifact_document(
            p,
            &doc.id,
            ArtifactDocumentPatch {
                title: Some("   ".to_string()),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(matches!(err_patch, DbError::EmptyArtifactTitle));
}

#[test]
fn patch_vazio_e_recusado_e_nao_altera_updated_at() {
    let dir = TempDir::new();
    let db = dir.open();
    let p = create_project(&db, "p_patch");

    let doc = db
        .create_artifact_document(
            p,
            NewArtifactDocument {
                kind: "checklist".to_string(),
                title: "Checklist".to_string(),
                summary: None,
                body: None,
                tags: vec![],
                author: None,
            },
        )
        .expect("criar");

    let err = db
        .update_artifact_document(p, &doc.id, ArtifactDocumentPatch::default())
        .unwrap_err();

    assert!(matches!(err, DbError::EmptyArtifactPatch));

    let current = db
        .create_artifact_document(
            p,
            NewArtifactDocument {
                kind: "documento".to_string(),
                title: "Doc".to_string(),
                summary: None,
                body: None,
                tags: vec![],
                author: None,
            },
        )
        .expect("criar doc");

    assert_eq!(doc.updated_at, doc.created_at);
    assert!(current.updated_at >= doc.updated_at);
}

#[test]
fn recusa_de_update_e_delete_em_ids_de_planos() {
    let dir = TempDir::new();
    let db = dir.open();
    let p = create_project(&db, "p_plan_ro");
    let (_, plan_id) = create_card_with_plan(&db, p, "Tarefa");

    // Update recusado em plano-*
    let err_update = db
        .update_artifact_document(
            p,
            &plan_id,
            ArtifactDocumentPatch {
                title: Some("Tentativa".to_string()),
                ..Default::default()
            },
        )
        .unwrap_err();
    match err_update {
        DbError::ArtifactReadOnly(id) => assert_eq!(id, plan_id),
        other => panic!("esperava ArtifactReadOnly, veio: {other:?}"),
    }

    // Delete recusado em plano-*
    let err_delete = db.delete_artifact_document(p, &plan_id).unwrap_err();
    match err_delete {
        DbError::ArtifactReadOnly(id) => assert_eq!(id, plan_id),
        other => panic!("esperava ArtifactReadOnly, veio: {other:?}"),
    }

    // set_artifact_check recusado em plano-*
    let err_check = db.set_artifact_check(p, &plan_id, 0, true).unwrap_err();
    match err_check {
        DbError::ArtifactReadOnly(id) => assert_eq!(id, plan_id),
        other => panic!("esperava ArtifactReadOnly, veio: {other:?}"),
    }
}

#[test]
fn concorrencia_otimista_recusa_update_com_expected_updated_at_desatualizado() {
    let dir = TempDir::new();
    let db = dir.open();
    let p = create_project(&db, "p_optimistic");

    let doc = db
        .create_artifact_document(
            p,
            NewArtifactDocument {
                kind: "decisao".to_string(),
                title: "ADR 001".to_string(),
                summary: None,
                body: None,
                tags: vec![],
                author: None,
            },
        )
        .expect("criar");

    let old_updated_at = doc.updated_at;

    // Primeiro update com expected_updated_at correto passa
    let updated = db
        .update_artifact_document(
            p,
            &doc.id,
            ArtifactDocumentPatch {
                title: Some("ADR 001 - Aprovada".to_string()),
                expected_updated_at: Some(old_updated_at),
                ..Default::default()
            },
        )
        .expect("update 1");

    assert!(updated.updated_at >= old_updated_at);

    // Segundo update com expected_updated_at antigo (desatualizado) é recusado com ArtifactConflict
    let conflict = db
        .update_artifact_document(
            p,
            &doc.id,
            ArtifactDocumentPatch {
                title: Some("Tentativa com timestamp antigo".to_string()),
                expected_updated_at: Some(old_updated_at),
                ..Default::default()
            },
        )
        .unwrap_err();

    match conflict {
        DbError::ArtifactConflict(id) => assert_eq!(id, doc.id),
        other => panic!("esperava ArtifactConflict, veio: {other:?}"),
    }

    // Update com o expected_updated_at atual passa
    let updated2 = db
        .update_artifact_document(
            p,
            &doc.id,
            ArtifactDocumentPatch {
                title: Some("ADR 001 - Revisada".to_string()),
                expected_updated_at: Some(updated.updated_at),
                ..Default::default()
            },
        )
        .expect("update 2");

    assert_eq!(updated2.title, "ADR 001 - Revisada");
}

#[test]
fn marcacao_de_checkboxes_ignora_blocos_de_codigo_markdown_e_respeita_indices() {
    let dir = TempDir::new();
    let db = dir.open();
    let p = create_project(&db, "p_check");

    let body_original = "\
# Guia de Integração
- [ ] Item 0 (aberto)
```markdown
- [ ] Código A (deve ser ignorado)
- [x] Código B (deve ser ignorado)
```
- [x] Item 1 (concluído)
- [ ] Item 2 (aberto)
";

    let doc = db
        .create_artifact_document(
            p,
            NewArtifactDocument {
                kind: "checklist".to_string(),
                title: "Guia".to_string(),
                summary: None,
                body: Some(body_original.to_string()),
                tags: vec![],
                author: None,
            },
        )
        .expect("criar");

    // Marca item 0 (que estava aberto) como feito (done: true)
    let checked0 = db.set_artifact_check(p, &doc.id, 0, true).expect("marcar item 0");
    assert!(checked0.body.contains("- [x] Item 0 (aberto)"));
    assert!(checked0.body.contains("```markdown\n- [ ] Código A (deve ser ignorado)\n- [x] Código B (deve ser ignorado)\n```"));

    // Desmarca item 1 (índice 1 fora do fence, que estava concluído)
    let unchecked1 = db.set_artifact_check(p, &doc.id, 1, false).expect("desmarcar item 1");
    assert!(unchecked1.body.contains("- [ ] Item 1 (concluído)"));
    // O código dentro do fence não foi alterado
    assert!(unchecked1.body.contains("- [ ] Código A (deve ser ignorado)"));
    assert!(unchecked1.body.contains("- [x] Código B (deve ser ignorado)"));

    // Marca item 2 (índice 2)
    let checked2 = db.set_artifact_check(p, &doc.id, 2, true).expect("marcar item 2");
    assert!(checked2.body.contains("- [x] Item 2 (aberto)"));

    // Índice 3 não existe (só há 3 itens marcáveis fora de fences: 0, 1 e 2)
    let err_out = db.set_artifact_check(p, &doc.id, 3, true).unwrap_err();
    match err_out {
        DbError::ArtifactCheckNotFound(idx) => assert_eq!(idx, 3),
        other => panic!("esperava ArtifactCheckNotFound, veio: {other:?}"),
    }
}

#[test]
fn cascade_ao_excluir_projeto_remove_documentos_associados() {
    let dir = TempDir::new();
    let db = dir.open();
    let p = create_project(&db, "p_cascade");

    db.create_artifact_document(
        p,
        NewArtifactDocument {
            kind: "checklist".to_string(),
            title: "Checklist 1".to_string(),
            summary: None,
            body: None,
            tags: vec![],
            author: None,
        },
    )
    .expect("doc 1");

    db.create_artifact_document(
        p,
        NewArtifactDocument {
            kind: "documento".to_string(),
            title: "Doc 2".to_string(),
            summary: None,
            body: None,
            tags: vec![],
            author: None,
        },
    )
    .expect("doc 2");

    let count_before: i64 = dir
        .raw()
        .query_row(
            "SELECT COUNT(*) FROM artifact_documents WHERE project_id = ?1",
            [p],
            |row| row.get(0),
        )
        .expect("count before");
    assert_eq!(count_before, 2);

    // Exclui projeto
    db.delete_project(p).expect("excluir projeto");

    let count_after: i64 = dir
        .raw()
        .query_row(
            "SELECT COUNT(*) FROM artifact_documents WHERE project_id = ?1",
            [p],
            |row| row.get(0),
        )
        .expect("count after");
    assert_eq!(count_after, 0, "documentos devem ser apagados em cascata");
}

#[test]
fn get_artifact_unificado_devolve_plan_ou_document_com_fidelidade() {
    let dir = TempDir::new();
    let db = dir.open();
    let p = create_project(&db, "p_unified");

    let (card_id, plan_id) = create_card_with_plan(&db, p, "Tarefa Backend");
    let doc = db
        .create_artifact_document(
            p,
            NewArtifactDocument {
                kind: "arquitetura".to_string(),
                title: "Arquitetura Hexagonal".to_string(),
                summary: Some("Desenho da solução".to_string()),
                body: Some("## Diagrama".to_string()),
                tags: vec!["arch".to_string()],
                author: Some("Axiom".to_string()),
            },
        )
        .expect("criar doc");

    // get_artifact para plano
    let plan_detail = db.get_artifact(p, &plan_id).expect("get plano");
    match plan_detail {
        ArtifactDetail::Plan(plan) => {
            assert_eq!(plan.id, plan_id);
            assert_eq!(plan.item_id, card_id);
            assert_eq!(plan.title, "Tarefa Backend");
            assert_eq!(plan.criteria.len(), 2);
        }
        ArtifactDetail::Document(_) => panic!("deveria ser Plan"),
    }

    // get_artifact para documento
    let doc_detail = db.get_artifact(p, &doc.id).expect("get doc");
    match doc_detail {
        ArtifactDetail::Document(document) => {
            assert_eq!(document.id, doc.id);
            assert_eq!(document.kind, "arquitetura");
            assert_eq!(document.title, "Arquitetura Hexagonal");
            assert_eq!(document.summary, "Desenho da solução");
            assert_eq!(document.body, "## Diagrama");
            assert_eq!(document.tags, vec!["arch"]);
            assert_eq!(document.author, Some("Axiom".to_string()));
        }
        ArtifactDetail::Plan(_) => panic!("deveria ser Document"),
    }

    // get_artifact inexistente
    let err_not_found = db.get_artifact(p, "documento-999").unwrap_err();
    match err_not_found {
        DbError::ArtifactNotFound(id) => assert_eq!(id, "documento-999"),
        other => panic!("esperava ArtifactNotFound, veio: {other:?}"),
    }

    // list_artifact_summaries reúne ambos ordenados por updated_at DESC
    let summaries = db.list_artifact_summaries(p).expect("list summaries");
    assert_eq!(summaries.len(), 2);
    let ids: Vec<&str> = summaries.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&plan_id.as_str()));
    assert!(ids.contains(&doc.id.as_str()));
    assert!(summaries[0].updated_at >= summaries[1].updated_at);
}

#[test]
fn catalogo_artifact_kinds_bate_com_check_do_schema_lendo_sqlite_master() {
    let dir = TempDir::new();
    let db = dir.open();
    let _ = create_project(&db, "p_kinds");

    let raw = dir.raw();
    let ddl: String = raw
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'artifact_documents'",
            [],
            |row| row.get(0),
        )
        .expect("ler ddl de artifact_documents");

    // Extrai o conteúdo dentro de CHECK (kind IN (...))
    let start_idx = ddl
        .find("kind IN (")
        .expect("CHECK com kind IN não encontrado no DDL");
    let after_prefix = &ddl[start_idx + "kind IN (".len()..];
    let end_idx = after_prefix.find(')').expect("fecha parênteses do CHECK");
    let inside = &after_prefix[..end_idx];

    let check_kinds: Vec<String> = inside
        .split(',')
        .map(|item| item.trim().trim_matches('\'').to_string())
        .collect();

    let catalog_kinds: Vec<String> = ARTIFACT_KINDS
        .iter()
        .map(|item| item.kind.to_string())
        .collect();

    assert_eq!(
        check_kinds, catalog_kinds,
        "Os tipos no CHECK do SQLite ({:?}) devem bater exatamente com ARTIFACT_KINDS ({:?})",
        check_kinds, catalog_kinds
    );
}
