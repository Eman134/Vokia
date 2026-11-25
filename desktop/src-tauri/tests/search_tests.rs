use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use rusqlite::Connection;
use serde_json::json;

use crate::commands::dispatcher::{command_domain, dispatch, list_commands};
use crate::db::{
    Db, ItemPlanInput, NewArtifactDocument, NewCard, NewProject, ORIGIN_FOLDER,
};
use crate::events::{Agent, Speaker};

use super::{build_fts_query, GlobalSearchQuery, SearchKind};

fn temp_dir(tag: &str) -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    std::env::temp_dir().join(format!(
        "vokia-search-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ))
}

struct TempDb {
    dir: PathBuf,
    db: Db,
}

impl TempDb {
    fn open(tag: &str) -> Self {
        let dir = temp_dir(tag);
        let db = Db::open(&dir).expect("abrir banco");
        Self { dir, db }
    }

    fn project(&self, name: &str, desc: &str) -> i64 {
        let path = self.dir.join(name).to_string_lossy().into_owned();
        self.db
            .insert_project(&NewProject {
                name,
                symbol: "folder",
                description: desc,
                path: &path,
                origin_kind: ORIGIN_FOLDER,
                origin_repo: None,
            })
            .expect("criar projeto")
            .id
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn sanitizacao_fts5_protege_caracteres_especiais_e_operadores() {
    assert_eq!(build_fts_query("pix"), Some("\"pix\"*".to_string()));
    assert_eq!(
        build_fts_query("foo-bar"),
        Some("\"foo\"* \"bar\"*".to_string())
    );
    assert_eq!(build_fts_query("\"aspas\""), Some("\"aspas\"*".to_string()));
    assert_eq!(build_fts_query("C++"), Some("\"C\"*".to_string()));
    assert_eq!(
        build_fts_query("OR AND NOT NEAR"),
        Some("\"OR\"* \"AND\"* \"NOT\"* \"NEAR\"*".to_string())
    );
    assert_eq!(build_fts_query("   --- +++ \"\"\"   "), None);
    assert_eq!(build_fts_query(""), None);
}

#[test]
fn migracao_fts5_faz_backfill_seguro_para_registros_pre_existentes() {
    let dir = temp_dir("backfill");
    let project_id = {
        let db = Db::open(&dir).expect("abrir inicial");
        let path = dir.join("loja").to_string_lossy().into_owned();
        let project = db
            .insert_project(&NewProject {
                name: "Loja Pix".into(),
                symbol: "cart",
                description: "Checkout instantâneo".into(),
                path: &path,
                origin_kind: ORIGIN_FOLDER,
                origin_repo: None,
            })
            .expect("projeto");
        let card = db
            .create_card(
                project.id,
                &NewCard {
                    title: "Webhook Pix legado".to_string(),
                    description: Some("Valida assinatura HMAC do PSP".to_string()),
                    origin: "usuario".to_string(),
                    links: Vec::new(),
                },
            )
            .expect("card");
        db.add_backlog_label(project.id, &card.id, "pagamentos")
            .expect("etiqueta");
        db.insert_message(
            project.id,
            Speaker::Agent(Agent::axiom()),
            Speaker::User,
            "Investiguei o webhook do PSP e o txid chega em maiúsculas.",
            1_700_000_000_000,
            None,
            Some(&card.id),
            None,
            None,
            None,
        )
        .expect("mensagem");
        db.save_item_plan(
            project.id,
            &card.id,
            &serde_json::from_value::<ItemPlanInput>(json!({
                "state": "proposto",
                "proposal": "Resumo do plano HMAC\n#lote 1|Validar HMAC|Assinatura verificada",
                "agent": "axiom"
            }))
            .expect("plano input"),
        )
        .expect("salvar plano");
        db.create_artifact_document(
            project.id,
            NewArtifactDocument {
                kind: "decisao".to_string(),
                title: "ADR Decisão de Assinatura".to_string(),
                summary: Some("Normalizar txid antes da validação".to_string()),
                body: Some("Guardamos o payload bruto para checar HMAC SHA-256.".to_string()),
                tags: vec!["seguranca".to_string()],
                author: Some("axiom".to_string()),
            },
        )
        .expect("artefato");
        project.id
    };

    // Simula banco pré-095 removendo tabelas virtuais FTS5 e triggers antes de reabrir.
    {
        let conn = Connection::open(dir.join("vokia.db")).expect("reabrir cru");
        conn.execute_batch(
            "DROP TRIGGER IF EXISTS search_projects_ai;
             DROP TRIGGER IF EXISTS search_projects_au;
             DROP TRIGGER IF EXISTS search_projects_ad;
             DROP TRIGGER IF EXISTS search_cards_ai;
             DROP TRIGGER IF EXISTS search_cards_au;
             DROP TRIGGER IF EXISTS search_cards_ad;
             DROP TRIGGER IF EXISTS search_messages_ai;
             DROP TRIGGER IF EXISTS search_messages_au;
             DROP TRIGGER IF EXISTS search_messages_ad;
             DROP TRIGGER IF EXISTS search_plans_ai;
             DROP TRIGGER IF EXISTS search_plans_au;
             DROP TRIGGER IF EXISTS search_plans_ad;
             DROP TRIGGER IF EXISTS search_artifacts_ai;
             DROP TRIGGER IF EXISTS search_artifacts_au;
             DROP TRIGGER IF EXISTS search_artifacts_ad;
             DROP TABLE IF EXISTS search_projects_fts;
             DROP TABLE IF EXISTS search_cards_fts;
             DROP TABLE IF EXISTS search_messages_fts;
             DROP TABLE IF EXISTS search_plans_fts;
             DROP TABLE IF EXISTS search_artifacts_fts;",
        )
        .expect("limpar fts");
    }

    let db = Db::open(&dir).expect("reabrir com backfill");
    let res = db
        .global_search(&GlobalSearchQuery {
            query: "HMAC".to_string(),
            project_id: Some(project_id),
            kinds: None,
            limit: None,
            offset: 0,
        })
        .expect("busca pós-backfill");

    let cards = res.groups.iter().find(|g| g.kind == SearchKind::Cards).unwrap();
    let plans = res.groups.iter().find(|g| g.kind == SearchKind::Plans).unwrap();
    let artifacts = res
        .groups
        .iter()
        .find(|g| g.kind == SearchKind::Artifacts)
        .unwrap();
    assert_eq!(cards.hits.len(), 1);
    assert_eq!(plans.hits.len(), 1);
    assert_eq!(artifacts.hits.len(), 1);
    assert!(cards.hits[0].snippet.as_deref().unwrap_or("").contains("<mark>HMAC</mark>"));

    // Busca sem acento encontra "Decisão" (unicode61 remove_diacritics 2).
    let accent_res = db
        .global_search(&GlobalSearchQuery {
            query: "decisao".to_string(),
            project_id: Some(project_id),
            kinds: Some(vec![SearchKind::Artifacts]),
            limit: None,
            offset: 0,
        })
        .expect("busca sem acento");
    assert_eq!(accent_res.groups[0].hits.len(), 1);
    assert_eq!(accent_res.groups[0].hits[0].title, "ADR Decisão de Assinatura");

    // Busca em mensagem backfilled com snippet destacado.
    let msg_res = db
        .global_search(&GlobalSearchQuery {
            query: "txid".to_string(),
            project_id: Some(project_id),
            kinds: Some(vec![SearchKind::Messages]),
            limit: None,
            offset: 0,
        })
        .expect("busca mensagem");
    assert_eq!(msg_res.groups[0].hits.len(), 1);
    assert!(msg_res.groups[0].hits[0]
        .snippet
        .as_deref()
        .unwrap_or("")
        .contains("<mark>txid</mark>"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn busca_em_cards_mensagens_artefatos_com_filtros_especiais_e_paginacao() {
    let fixture = TempDb::open("queries");
    let proj_a = fixture.project("Projeto Alpha", "Backend C++ e foo-bar");
    let proj_b = fixture.project("Projeto Beta", "Frontend Vue");

    let card_a = fixture
        .db
        .create_card(
            proj_a,
            &NewCard {
                title: "Suporte a foo-bar em C++".to_string(),
                description: Some("Aceitar \"aspas\" no parser do módulo Pix.".to_string()),
                origin: "usuario".to_string(),
                links: Vec::new(),
            },
        )
        .expect("card a");
    fixture
        .db
        .add_backlog_label(proj_a, &card_a.id, "pix")
        .expect("tag pix");

    let _card_b = fixture
        .db
        .create_card(
            proj_b,
            &NewCard {
                title: "Tela de Pix no checkout".to_string(),
                description: Some("Mostra QR Code Pix".to_string()),
                origin: "usuario".to_string(),
                links: Vec::new(),
            },
        )
        .expect("card b");

    for idx in 0..5 {
        fixture
            .db
            .insert_message(
                proj_a,
                Speaker::User,
                Speaker::Agent(Agent::axiom()),
                &format!("Mensagem #{idx} sobre pagamento pix no lote"),
                1_700_000_000_000 + idx * 1000,
                None,
                Some(&card_a.id),
                None,
                None,
                None,
            )
            .expect("msg");
    }

    // Caracteres especiais não quebram o parser FTS5.
    for tricky in ["foo-bar", "\"aspas\"", "C++", "OR NOT", "---", "foo:bar*"] {
        let res = fixture
            .db
            .global_search(&GlobalSearchQuery {
                query: tricky.to_string(),
                project_id: None,
                kinds: Some(vec![SearchKind::Cards]),
                limit: None,
                offset: 0,
            })
            .unwrap_or_else(|err| panic!("consulta {tricky:?} falhou: {err}"));
        if matches!(tricky, "foo-bar" | "\"aspas\"" | "C++") {
            assert!(
                !res.groups[0].hits.is_empty(),
                "esperava match para {tricky:?}"
            );
        }
    }

    // Filtro por projectId isola resultados.
    let only_a = fixture
        .db
        .global_search(&GlobalSearchQuery {
            query: "pix".to_string(),
            project_id: Some(proj_a),
            kinds: Some(vec![SearchKind::Cards, SearchKind::Memories]),
            limit: None,
            offset: 0,
        })
        .expect("filtro projeto A");
    assert_eq!(only_a.groups.len(), 2);
    assert_eq!(only_a.groups[0].kind, SearchKind::Cards);
    assert_eq!(only_a.groups[0].hits.len(), 1);
    assert_eq!(only_a.groups[0].hits[0].project_id, Some(proj_a));
    assert_eq!(only_a.groups[1].kind, SearchKind::Memories);
    assert!(only_a.groups[1].hits.is_empty());

    // Sem filtro de projeto encontra cards de ambos os projetos.
    let both = fixture
        .db
        .global_search(&GlobalSearchQuery {
            query: "pix".to_string(),
            project_id: None,
            kinds: Some(vec![SearchKind::Cards]),
            limit: None,
            offset: 0,
        })
        .expect("sem filtro projeto");
    assert_eq!(both.groups[0].total, 2);

    // Arquivar card remove dos resultados; restaurar devolve.
    fixture
        .db
        .archive_backlog_item(proj_a, &card_a.id)
        .expect("arquivar");
    let after_archive = fixture
        .db
        .global_search(&GlobalSearchQuery {
            query: "foo-bar".to_string(),
            project_id: Some(proj_a),
            kinds: Some(vec![SearchKind::Cards]),
            limit: None,
            offset: 0,
        })
        .expect("pós arquivo");
    assert!(after_archive.groups[0].hits.is_empty());

    fixture
        .db
        .restore_backlog_item(proj_a, &card_a.id)
        .expect("restaurar");
    let after_restore = fixture
        .db
        .global_search(&GlobalSearchQuery {
            query: "foo-bar".to_string(),
            project_id: Some(proj_a),
            kinds: Some(vec![SearchKind::Cards]),
            limit: None,
            offset: 0,
        })
        .expect("pós restauração");
    assert_eq!(after_restore.groups[0].hits.len(), 1);

    // Paginação por limit/offset e nextCursor.
    let page1 = fixture
        .db
        .global_search(&GlobalSearchQuery {
            query: "pagamento pix".to_string(),
            project_id: Some(proj_a),
            kinds: Some(vec![SearchKind::Messages]),
            limit: Some(2),
            offset: 0,
        })
        .expect("página 1");
    assert_eq!(page1.groups[0].total, 5);
    assert_eq!(page1.groups[0].hits.len(), 2);
    assert_eq!(page1.groups[0].next_cursor.as_deref(), Some("2"));

    let page2 = fixture
        .db
        .global_search(&GlobalSearchQuery {
            query: "pagamento pix".to_string(),
            project_id: Some(proj_a),
            kinds: Some(vec![SearchKind::Messages]),
            limit: Some(2),
            offset: 2,
        })
        .expect("página 2");
    assert_eq!(page2.groups[0].hits.len(), 2);
    assert_eq!(page2.groups[0].next_cursor.as_deref(), Some("4"));
    assert_ne!(page1.groups[0].hits[0].id, page2.groups[0].hits[0].id);

    let page3 = fixture
        .db
        .global_search(&GlobalSearchQuery {
            query: "pagamento pix".to_string(),
            project_id: Some(proj_a),
            kinds: Some(vec![SearchKind::Messages]),
            limit: Some(2),
            offset: 4,
        })
        .expect("página 3");
    assert_eq!(page3.groups[0].hits.len(), 1);
    assert_eq!(page3.groups[0].next_cursor, None);

    // Busca em mensagem pelo card ligado (item_id sincronizado transacionalmente).
    let by_card_id = fixture
        .db
        .global_search(&GlobalSearchQuery {
            query: card_a.id.clone(),
            project_id: Some(proj_a),
            kinds: Some(vec![SearchKind::Messages]),
            limit: None,
            offset: 0,
        })
        .expect("busca por item_id");
    assert_eq!(by_card_id.groups[0].total, 5);
}

#[test]
fn global_search_exposto_no_dispatcher_combina_fts5_e_agentes() {
    use tauri::Manager;

    assert_eq!(command_domain("global_search"), Some("search"));
    assert!(list_commands()
        .iter()
        .any(|cmd| cmd["name"] == "global_search" && cmd["domain"] == "search"));

    let dir = temp_dir("dispatcher");
    let app = crate::mcp::support::build_app(&dir);
    let db_state = app.state::<crate::db::DbState>();
    let db = db_state.require().expect("db");
    let path = dir.join("proj").to_string_lossy().into_owned();
    let project = db
        .insert_project(&NewProject {
            name: "Carteira Pix".into(),
            symbol: "wallet",
            description: "Pagamentos instantâneos".into(),
            path: &path,
            origin_kind: ORIGIN_FOLDER,
            origin_repo: None,
        })
        .expect("projeto");
    db.create_card(
        project.id,
        &NewCard {
            title: "Estorno Pix".to_string(),
            description: Some("Fluxo de devolução Pix".to_string()),
            origin: "usuario".to_string(),
            links: Vec::new(),
        },
    )
    .expect("card");

    let res = tauri::async_runtime::block_on(dispatch(
        app.handle(),
        None,
        "global_search",
        json!({
            "query": "pix",
            "projectId": project.id,
            "kinds": ["projects", "cards", "memories"]
        }),
    ))
    .expect("comando reconhecido")
    .expect("execução ok");

    let groups = res["groups"].as_array().expect("groups array");
    assert_eq!(groups.len(), 3);
    assert_eq!(groups[0]["kind"], "projects");
    assert_eq!(groups[0]["hits"].as_array().unwrap().len(), 1);
    assert_eq!(groups[1]["kind"], "cards");
    assert_eq!(groups[1]["hits"].as_array().unwrap().len(), 1);
    assert_eq!(groups[2]["kind"], "memories");
    assert_eq!(groups[2]["hits"].as_array().unwrap().len(), 0);

    // Busca por agente do AgentRegistry via dispatcher.
    let agent_res = tauri::async_runtime::block_on(dispatch(
        app.handle(),
        None,
        "global_search",
        json!({
            "query": "Axiom",
            "kinds": ["agents"]
        }),
    ))
    .expect("comando reconhecido")
    .expect("execução agentes ok");

    let agent_hits = agent_res["groups"][0]["hits"].as_array().unwrap();
    assert_eq!(agent_hits.len(), 1);
    assert_eq!(agent_hits[0]["id"], "axiom");
    assert_eq!(agent_hits[0]["title"], "Axiom");

    let _ = fs::remove_dir_all(&dir);
}
