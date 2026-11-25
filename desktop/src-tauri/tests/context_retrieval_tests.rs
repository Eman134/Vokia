use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::{json, Value};
use tauri::Manager;

use crate::commands::command_search::{
    command_search_exec, CommandSearchArgs, COMMAND_SEARCH_DEFAULT_LIMIT,
    COMMAND_SEARCH_MAX_LIMIT, COMMAND_SEARCH_MAX_STRONG_INLINE,
};
use crate::commands::dispatcher::{command_domain, dispatch, dispatch_for_project, list_commands};
use crate::db::history::{
    HistoryReadQuery, HistorySearchQuery, CONTEXT_RESET_MARKER, HISTORY_READ_DEFAULT_LIMIT,
    HISTORY_READ_MAX_LIMIT, HISTORY_READ_MAX_TOTAL_CHARS, HISTORY_SEARCH_DEFAULT_LIMIT,
    HISTORY_SEARCH_EXCERPT_MAX_CHARS, HISTORY_SEARCH_MAX_LIMIT, HISTORY_SEARCH_MAX_TOTAL_CHARS,
};
use crate::db::{
    work_steps_session_key, Db, DbError, DbState, NewProject, NewTurnWorkSteps, ORIGIN_FOLDER,
};
use crate::events::{Agent, Speaker};
use crate::mcp::tools::{
    check_tools_list_budget, definitions_for_scope, tools_list_payload_bytes,
    TOOLS_LIST_COORDINATOR_MAX_BYTES, TOOLS_LIST_WORKER_MAX_BYTES,
};
use crate::mcp::{Binding, ExchangeSlot, McpServer, ToolScope};

fn kairo() -> Agent {
    Agent::from_stored("kairo").expect("agente kairo")
}

fn temp_dir(tag: &str) -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    std::env::temp_dir().join(format!(
        "vokia-ctx-ret-{tag}-{}-{}",
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

    fn project(&self, name: &str) -> i64 {
        let path = self.dir.join(name).to_string_lossy().into_owned();
        self.db
            .insert_project(&NewProject {
                name,
                symbol: "folder",
                description: "",
                path: &path,
                origin_kind: ORIGIN_FOLDER,
                origin_repo: None,
            })
            .expect("criar projeto")
            .id
    }

    fn insert_msg(
        &self,
        project_id: i64,
        from: Speaker,
        to: Speaker,
        text: &str,
        time: u64,
        channel_id: Option<i64>,
        item_id: Option<&str>,
    ) -> i64 {
        self.db
            .insert_message(project_id, from, to, text, time, channel_id, item_id, None, None, None)
            .expect("inserir mensagem");
        let conn = self.db.conn().expect("conn");
        conn.query_row(
            "SELECT MAX(id) FROM messages WHERE project_id = ?1",
            [project_id],
            |row| row.get(0),
        )
        .expect("ler id da mensagem")
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

// Critério 1: command_search com query "create artifact" retorna resultados relevantes,
// ordenados por nome exato -> todas as palavras -> parcial -> catálogo, e embute parâmetros/schema/exemplo
// nos até 3 resultados fortes.
#[test]
fn command_search_retorna_create_artifact_com_schema_inline_e_respeita_limites() {
    assert_eq!(command_domain("command_search"), Some("search"));
    assert_eq!(command_domain("history_search"), Some("search"));
    assert_eq!(command_domain("history_read"), Some("search"));

    let catalog_names: Vec<String> = list_commands()
        .into_iter()
        .filter_map(|cmd| cmd["name"].as_str().map(str::to_string))
        .collect();
    assert!(catalog_names.contains(&"command_search".to_string()));
    assert!(catalog_names.contains(&"history_search".to_string()));
    assert!(catalog_names.contains(&"history_read".to_string()));

    let res = command_search_exec(CommandSearchArgs {
        query: Some("create artifact".to_string()),
        ..Default::default()
    })
    .expect("command_search ok");

    assert_eq!(res.results.len(), COMMAND_SEARCH_DEFAULT_LIMIT);
    let top = &res.results[0];
    assert_eq!(top.name, "create_artifact");
    assert_eq!(top.match_kind, "exact");
    assert!(top.strong);
    assert!(top.input_schema.is_some(), "resultado forte precisa incluir inputSchema");
    let params = top
        .parameters
        .as_ref()
        .expect("resultado forte precisa incluir parameters");
    assert!(params.iter().any(|p| p.name == "projectId" && p.required));
    assert!(params.iter().any(|p| p.name == "kind" && p.required));
    assert!(params.iter().any(|p| p.name == "title" && p.required));
    let example = top
        .example_call
        .as_ref()
        .expect("resultado forte precisa incluir exampleCall");
    assert_eq!(example["command"], "create_artifact");
    assert!(example["arguments"]["projectId"].is_number());
    assert!(example["arguments"]["title"].is_string());

    // Verbos genéricos como get/list são ignorados para qualificar resultados fortes:
    // "artifact" qualifica list_artifacts e get_artifact como fortes, mas no máximo 3 fortes recebem inline.
    let strong_with_schema = res
        .results
        .iter()
        .filter(|hit| hit.input_schema.is_some())
        .count();
    assert!(strong_with_schema >= 1 && strong_with_schema <= COMMAND_SEARCH_MAX_STRONG_INLINE);
    for hit in res.results.iter().filter(|hit| !hit.strong) {
        assert!(hit.input_schema.is_none());
        assert!(hit.parameters.is_none());
        assert!(hit.example_call.is_none());
    }

    // `queries` (array) aceita múltiplas variações, deduplica e limita até 3 fortes inline + máximo 10 resultados.
    let multi = command_search_exec(CommandSearchArgs {
        queries: Some(vec![
            "list_projects".to_string(),
            "list_channels".to_string(),
            "list_agents".to_string(),
            "list_providers".to_string(),
            "artifact".to_string(),
        ]),
        limit: Some(50),
        ..Default::default()
    })
    .expect("multi query ok");
    assert_eq!(multi.results.len(), COMMAND_SEARCH_MAX_LIMIT);
    let inlined_count = multi
        .results
        .iter()
        .filter(|hit| hit.input_schema.is_some())
        .count();
    assert_eq!(
        inlined_count, COMMAND_SEARCH_MAX_STRONG_INLINE,
        "no máximo 3 resultados fortes devem embutir schema/exemplo"
    );

    // Ordenação: nome exato -> todas as palavras -> parcial -> ordem estável do catálogo.
    let reversed = command_search_exec(CommandSearchArgs {
        query: Some("artifact create".to_string()),
        ..Default::default()
    })
    .expect("reversed query ok");
    assert_eq!(reversed.results[0].name, "create_artifact");
    assert_eq!(reversed.results[0].match_kind, "all_words");
    assert!(reversed.results[0].strong);

    // Comandos UiOnly nunca entram na busca.
    let ui_only = command_search_exec(CommandSearchArgs {
        query: Some("pick_folder".to_string()),
        ..Default::default()
    })
    .expect("ui_only query ok");
    assert!(ui_only.results.iter().all(|hit| hit.name != "pick_folder"));
}

// Critério 2: history_search recupera termo discutido 30 turnos atrás, aplica limites 400 / 8.000 chars,
// respeita isolamento de projeto, marcador de reset de contexto e fallback LIKE.
#[test]
fn history_search_recupera_turnos_antigos_com_limites_reset_like_e_isolamento() {
    let fixture = TempDb::open("history-search");
    let proj_a = fixture.project("projeto-a");
    let proj_b = fixture.project("projeto-b");

    // Mensagem muito antiga (30+ turnos antes) com texto longo (> 900 chars) contendo termo-chave no meio.
    let long_prefix = "contexto inicial ".repeat(35);
    let long_suffix = " detalhes posteriores ".repeat(35);
    let target_text = format!(
        "{long_prefix}Decisão arquitetural: adotar token_rotacao_quantico no handshake TLS.{long_suffix}"
    );
    let old_msg_id = fixture.insert_msg(
        proj_a,
        Speaker::Agent(Agent::axiom()),
        Speaker::User,
        &target_text,
        1_700_000_000_000,
        None,
        Some("card-42"),
    );

    // 32 turnos intermediários no projeto A.
    for turn in 1..=32 {
        fixture.insert_msg(
            proj_a,
            Speaker::User,
            Speaker::Agent(Agent::axiom()),
            &format!("Turno intermediário #{turn} discutindo ajustes gerais de UI e testes."),
            1_700_000_000_000 + turn * 1_000,
            None,
            Some("card-42"),
        );
    }

    // Projeto B também menciona o mesmo termo — jamais pode vazar na busca do projeto A.
    fixture.insert_msg(
        proj_b,
        Speaker::Agent(kairo()),
        Speaker::User,
        "SEGREDO_PROJETO_B usando token_rotacao_quantico em outra conta.",
        1_700_000_999_000,
        None,
        Some("card-99"),
    );

    let search = fixture
        .db
        .history_search(&HistorySearchQuery {
            project_id: proj_a,
            query: "token_rotacao_quantico".to_string(),
            card_id: Some("card-42".to_string()),
            ..Default::default()
        })
        .expect("history_search ok");

    assert_eq!(search.project_id, proj_a);
    assert_eq!(search.results.len(), 1);
    assert_eq!(search.results[0].message_id, old_msg_id);
    assert_eq!(search.results[0].timestamp, 1_700_000_000_000);
    assert!(search.results[0].excerpt.contains("token_rotacao_quantico"));
    assert!(
        search.results[0].excerpt.chars().count() <= HISTORY_SEARCH_EXCERPT_MAX_CHARS,
        "excerto excedeu 400 chars: {}",
        search.results[0].excerpt.chars().count()
    );
    let serialized = serde_json::to_string(&search).expect("serializar search");
    assert!(!serialized.contains("SEGREDO_PROJETO_B"));
    assert!(serialized.chars().count() <= HISTORY_SEARCH_MAX_TOTAL_CHARS);

    // Padrão 5 resultados, máximo 10, cada excerto <= 400 chars e total <= 8.000 chars.
    for idx in 1..=14 {
        let verbose = format!(
            "Relatório #{idx} sobre protocolo_alfa_omega: {}",
            "bloco de texto detalhado para validar teto de excerto ".repeat(20)
        );
        fixture.insert_msg(
            proj_a,
            Speaker::Agent(kairo()),
            Speaker::User,
            &verbose,
            1_700_001_000_000 + idx * 1_000,
            None,
            None,
        );
    }

    let default_page = fixture
        .db
        .history_search(&HistorySearchQuery {
            project_id: proj_a,
            query: "protocolo_alfa_omega".to_string(),
            ..Default::default()
        })
        .expect("default limit");
    assert_eq!(default_page.results.len(), HISTORY_SEARCH_DEFAULT_LIMIT);

    let max_page = fixture
        .db
        .history_search(&HistorySearchQuery {
            project_id: proj_a,
            query: "protocolo_alfa_omega".to_string(),
            limit: Some(50),
            ..Default::default()
        })
        .expect("max limit");
    assert_eq!(max_page.results.len(), HISTORY_SEARCH_MAX_LIMIT);
    for hit in &max_page.results {
        assert!(hit.excerpt.chars().count() <= HISTORY_SEARCH_EXCERPT_MAX_CHARS);
    }
    let max_json = serde_json::to_string(&max_page).expect("json max page");
    assert!(max_json.chars().count() <= HISTORY_SEARCH_MAX_TOTAL_CHARS);

    // Marcador de reset de contexto: mensagens anteriores somem da busca, mensagens posteriores aparecem.
    fixture.insert_msg(
        proj_a,
        Speaker::User,
        Speaker::Agent(Agent::axiom()),
        CONTEXT_RESET_MARKER,
        1_700_002_000_000,
        None,
        None,
    );
    let post_reset_id = fixture.insert_msg(
        proj_a,
        Speaker::Agent(Agent::axiom()),
        Speaker::User,
        "Nova sessão após reset discutindo protocolo_alfa_omega versão 2.",
        1_700_002_005_000,
        None,
        None,
    );

    let after_reset = fixture
        .db
        .history_search(&HistorySearchQuery {
            project_id: proj_a,
            query: "protocolo_alfa_omega".to_string(),
            ..Default::default()
        })
        .expect("busca pós reset");
    assert_eq!(after_reset.results.len(), 1);
    assert_eq!(after_reset.results[0].message_id, post_reset_id);

    // Fallback LIKE quando a tabela FTS5 não está disponível.
    {
        let conn = fixture.db.conn().expect("conn");
        conn.execute_batch(
            "DROP TRIGGER IF EXISTS search_messages_ai;
             DROP TRIGGER IF EXISTS search_messages_au;
             DROP TRIGGER IF EXISTS search_messages_ad;
             DROP TABLE IF EXISTS search_messages_fts;",
        )
        .expect("remover fts5");
    }
    let like_fallback = fixture
        .db
        .history_search(&HistorySearchQuery {
            project_id: proj_a,
            query: "versão 2".to_string(),
            ..Default::default()
        })
        .expect("fallback LIKE ok");
    assert_eq!(like_fallback.results.len(), 1);
    assert_eq!(like_fallback.results[0].message_id, post_reset_id);
}

// Critério 3: history_read com messageId de resposta longa + includeWorkSteps respeita teto 16.000 chars
// e pagina com nextOffset e nextWorkStepsOffset até recuperar 100% do conteúdo.
#[test]
fn history_read_pagina_mensagem_longa_e_work_steps_dentro_de_16000_chars() {
    let fixture = TempDb::open("history-read-long");
    let proj_a = fixture.project("projeto-longo");

    // Resposta longa de ~26.000 caracteres com marcadores determinísticos no início, meio e fim.
    let long_body = format!(
        "INICIO_RESPOSTA|{}|MEIO_RESPOSTA|{}|FIM_RESPOSTA",
        "bloco-A-0123456789 ".repeat(700),
        "bloco-B-9876543210 ".repeat(700),
    );
    let total_expected_chars = long_body.chars().count();
    assert!(total_expected_chars > 24_000);

    let msg_id = fixture.insert_msg(
        proj_a,
        Speaker::Agent(kairo()),
        Speaker::User,
        &long_body,
        1_700_000_100_000,
        None,
        Some("card-096"),
    );

    // Registra 4 blocos `[work steps]` grandes (~3.200 chars cada) para forçar paginação de workStepsOffset.
    let session_key = work_steps_session_key(proj_a, "kairo", Some("card-096"));
    for turn in 1..=4 {
        let block = format!(
            "[work steps]\n$ cargo test turno_{turn} (exit 0)\n  {}\n[end work steps]",
            format!("passo_detalhado_t{turn} ").repeat(130)
        );
        fixture
            .db
            .insert_turn_work_steps(&NewTurnWorkSteps {
                project_id: proj_a,
                agent: "kairo",
                session_key: &session_key,
                card_id: Some("card-096"),
                provider: Some("claude"),
                block: &block,
                step_count: 5,
                omitted_count: 0,
                created_at: 1_700_000_000_000 + turn as i64 * 1_000,
            })
            .expect("inserir work steps");
    }

    // Primeira chamada: deve caber em <= 16.000 chars serializados e devolver ambos os cursores.
    let first = fixture
        .db
        .history_read(&HistoryReadQuery {
            project_id: proj_a,
            message_id: Some(msg_id),
            include_work_steps: true,
            ..Default::default()
        })
        .expect("primeira leitura");

    let first_json = serde_json::to_string(&first).expect("json first");
    assert!(
        first_json.chars().count() <= HISTORY_READ_MAX_TOTAL_CHARS,
        "resposta excedeu 16.000 chars: {}",
        first_json.chars().count()
    );
    assert_eq!(first.messages.len(), 1);
    assert!(first.messages[0].truncated);
    assert!(first.next_offset.is_some(), "esperava nextOffset para mensagem longa");
    assert!(!first.work_steps.is_empty(), "esperava blocos [work steps]");
    assert!(
        first.next_work_steps_offset.is_some(),
        "esperava nextWorkStepsOffset para continuar blocos [work steps]"
    );

    // Segue `nextOffset` até reconstruir a mensagem completa sem perda.
    let mut reconstructed = first.messages[0].text.clone();
    let mut next_offset = first.next_offset;
    let mut pages = 1;
    while let Some(offset) = next_offset {
        pages += 1;
        assert!(pages <= 10, "loop excessivo ao paginar mensagem");
        let page = fixture
            .db
            .history_read(&HistoryReadQuery {
                project_id: proj_a,
                message_id: Some(msg_id),
                offset,
                include_work_steps: false,
                ..Default::default()
            })
            .expect("próxima página de texto");
        let page_json = serde_json::to_string(&page).expect("json page");
        assert!(page_json.chars().count() <= HISTORY_READ_MAX_TOTAL_CHARS);
        reconstructed.push_str(&page.messages[0].text);
        next_offset = page.next_offset;
    }
    assert_eq!(reconstructed, long_body);

    // Segue `nextWorkStepsOffset` até ler todos os 4 blocos `[work steps]` (do mais recente ao mais antigo).
    let mut collected_ws: Vec<String> = first
        .work_steps
        .iter()
        .map(|entry| entry.block.clone())
        .collect();
    let mut ws_offset = first.next_work_steps_offset;
    let mut ws_pages = 1;
    while let Some(offset) = ws_offset {
        ws_pages += 1;
        assert!(ws_pages <= 10, "loop excessivo ao paginar work steps");
        let page = fixture
            .db
            .history_read(&HistoryReadQuery {
                project_id: proj_a,
                message_id: Some(msg_id),
                offset: total_expected_chars,
                include_work_steps: true,
                work_steps_offset: offset,
                ..Default::default()
            })
            .expect("próxima página de work steps");
        let page_json = serde_json::to_string(&page).expect("json ws page");
        assert!(page_json.chars().count() <= HISTORY_READ_MAX_TOTAL_CHARS);
        for entry in &page.work_steps {
            collected_ws.push(entry.block.clone());
        }
        ws_offset = page.next_work_steps_offset;
    }
    assert_eq!(collected_ws.len(), 4);
    assert!(collected_ws[0].contains("turno_4"));
    assert!(collected_ws[3].contains("turno_1"));
}

#[test]
fn history_read_pagina_mensagens_recentes_e_anteriores_com_limites_10_e_20() {
    let fixture = TempDb::open("history-read-pages");
    let proj = fixture.project("projeto-paginado");

    let mut ids = Vec::new();
    for i in 1..=25 {
        let id = fixture.insert_msg(
            proj,
            Speaker::User,
            Speaker::Agent(Agent::axiom()),
            &format!("Mensagem sequencial #{i:02}"),
            1_700_000_000_000 + i * 1_000,
            None,
            None,
        );
        ids.push(id);
    }

    // Padrão: 10 mensagens mais recentes em ordem cronológica.
    let recent = fixture
        .db
        .history_read(&HistoryReadQuery {
            project_id: proj,
            ..Default::default()
        })
        .expect("recentes");
    assert_eq!(recent.messages.len(), HISTORY_READ_DEFAULT_LIMIT);
    assert_eq!(recent.messages.first().unwrap().message_id, ids[15]);
    assert_eq!(recent.messages.last().unwrap().message_id, ids[24]);
    assert!(recent.has_more);
    let cursor = recent.next_before_message_id.expect("cursor esperado");
    assert_eq!(cursor, ids[15]);

    // Página anterior a `cursor` com limit acima do teto (50 -> clamp 20) devolve as 15 restantes.
    let older = fixture
        .db
        .history_read(&HistoryReadQuery {
            project_id: proj,
            before_message_id: Some(cursor),
            limit: Some(50),
            ..Default::default()
        })
        .expect("anteriores");
    assert_eq!(older.messages.len(), 15);
    assert_eq!(older.messages.first().unwrap().message_id, ids[0]);
    assert_eq!(older.messages.last().unwrap().message_id, ids[14]);
    assert!(!older.has_more);
    assert!(older.next_cursor.is_none());

    // Clamp máximo de 20 quando há 25 mensagens disponíveis.
    let max_recent = fixture
        .db
        .history_read(&HistoryReadQuery {
            project_id: proj,
            limit: Some(99),
            ..Default::default()
        })
        .expect("max 20");
    assert_eq!(max_recent.messages.len(), HISTORY_READ_MAX_LIMIT);
    assert!(max_recent.has_more);
}

// Critério 4: recusa acesso cross-project tanto via Db/dispatcher quanto via MCP binding.
#[test]
fn history_read_e_history_search_recusam_acesso_cross_project() {
    let dir = temp_dir("cross-project");
    let app = crate::mcp::support::build_app(&dir);
    let db_state = app.state::<DbState>();
    let db = db_state.require().expect("db");

    let path_a = dir.join("proj-a").to_string_lossy().into_owned();
    let path_b = dir.join("proj-b").to_string_lossy().into_owned();
    let proj_a = db
        .insert_project(&NewProject {
            name: "Projeto A",
            symbol: "folder",
            description: "",
            path: &path_a,
            origin_kind: ORIGIN_FOLDER,
            origin_repo: None,
        })
        .expect("proj A")
        .id;
    let proj_b = db
        .insert_project(&NewProject {
            name: "Projeto B",
            symbol: "folder",
            description: "",
            path: &path_b,
            origin_kind: ORIGIN_FOLDER,
            origin_repo: None,
        })
        .expect("proj B")
        .id;

    db.insert_message(
        proj_a,
        Speaker::User,
        Speaker::Agent(Agent::axiom()),
        "Mensagem do projeto A com termo_comum",
        1_700_000_001_000,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("msg A");
    db.insert_message(
        proj_b,
        Speaker::User,
        Speaker::Agent(kairo()),
        "Mensagem secreta do projeto B com termo_comum",
        1_700_000_002_000,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("msg B");

    let msg_b_id: i64 = db
        .conn()
        .expect("conn")
        .query_row(
            "SELECT id FROM messages WHERE project_id = ?1",
            [proj_b],
            |row| row.get(0),
        )
        .expect("id msg B");

    // 1. Db::history_read com project_id = A e message_id pertencente a B -> CrossProjectAccess.
    let err = db
        .history_read(&HistoryReadQuery {
            project_id: proj_a,
            message_id: Some(msg_b_id),
            ..Default::default()
        })
        .expect_err("deveria recusar messageId de outro projeto");
    assert!(
        matches!(err, DbError::CrossProjectAccess(_)),
        "esperava CrossProjectAccess, obteve {err:?}"
    );
    assert!(err.to_string().contains("Acesso negado"));

    // 2. Cursor beforeMessageId pertencente a outro projeto também é recusado.
    let err_cursor = db
        .history_read(&HistoryReadQuery {
            project_id: proj_a,
            before_message_id: Some(msg_b_id),
            ..Default::default()
        })
        .expect_err("deveria recusar beforeMessageId de outro projeto");
    assert!(matches!(err_cursor, DbError::CrossProjectAccess(_)));

    // 3. Dispatcher direto: leitura do messageId de B sob projectId A é recusada.
    let direct_err = tauri::async_runtime::block_on(dispatch(
        app.handle(),
        Some(&Agent::axiom()),
        "history_read",
        json!({ "projectId": proj_a, "messageId": msg_b_id }),
    ))
    .expect("comando reconhecido")
    .expect_err("deveria recusar cross-project no dispatcher");
    assert!(direct_err.contains("Acesso negado"));

    // 4. Dispatcher com escopo MCP vinculado ao projeto A: tentar passar projectId = B em history_search ou history_read falha.
    let mcp_search_err = tauri::async_runtime::block_on(dispatch_for_project(
        app.handle(),
        Some(&Agent::axiom()),
        Some(proj_a),
        "history_search",
        json!({ "projectId": proj_b, "query": "termo_comum" }),
    ))
    .expect("comando reconhecido")
    .expect_err("deveria recusar projectId divergente do binding");
    assert!(mcp_search_err.contains("Acesso negado"));

    let mcp_read_err = tauri::async_runtime::block_on(dispatch_for_project(
        app.handle(),
        Some(&Agent::axiom()),
        Some(proj_a),
        "history_read",
        json!({ "projectId": proj_b }),
    ))
    .expect("comando reconhecido")
    .expect_err("deveria recusar projectId divergente em history_read");
    assert!(mcp_read_err.contains("Acesso negado"));

    // 5. Via servidor MCP real (commands/call), omitindo projectId usa automaticamente o projeto vinculado (A) e nunca vaza B.
    tauri::async_runtime::block_on(async {
        let server = McpServer::start().await.expect("mcp start");
        server.attach(app.handle().clone());
        let port = crate::mcp::support::port_of(&server);
        let token = server
            .register(Binding {
                project_id: proj_a,
                agent: Agent::axiom(),
                agent_name: "Axiom".to_string(),
                slot: ExchangeSlot::default(),
                scope: ToolScope::Coordinator,
            })
            .expect("register token");

        let reply = crate::mcp::support::send(
            port,
            &crate::mcp::support::Request::post(
                port,
                &token,
                json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "commands/call",
                    "params": {
                        "name": "history_search",
                        "arguments": { "query": "termo_comum" }
                    }
                }),
            ),
        )
        .await;
        assert_eq!(reply.status, 200);
        let result = &reply.json()["result"];
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().expect("texto json");
        let parsed: Value = serde_json::from_str(text).expect("parse history_search");
        assert_eq!(parsed["projectId"], proj_a);
        assert_eq!(parsed["results"].as_array().unwrap().len(), 1);
        assert!(!text.contains("projeto B"));

        // Tentar ler messageId do projeto B pelo MCP vinculado ao projeto A -> isError: true.
        let cross_reply = crate::mcp::support::send(
            port,
            &crate::mcp::support::Request::post(
                port,
                &token,
                json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "commands/call",
                    "params": {
                        "name": "history_read",
                        "arguments": { "messageId": msg_b_id }
                    }
                }),
            ),
        )
        .await;
        assert_eq!(cross_reply.status, 200);
        assert_eq!(cross_reply.json()["result"]["isError"], true);
        let err_text = cross_reply.json()["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert!(err_text.contains("Acesso negado"), "esperava recusa clara, veio: {err_text}");
    });

    let _ = fs::remove_dir_all(&dir);
}

// Critério 5: teste de orçamento em bytes de tools/list para impedir inflação silenciosa de contexto.
#[test]
fn tools_list_permanece_enxuto_dentro_do_orcamento_em_bytes_e_alerta_em_inflacao() {
    let coordinator_defs = definitions_for_scope(ToolScope::Coordinator);
    let worker_defs = definitions_for_scope(ToolScope::Worker);

    // Novos comandos sob demanda NÃO entram em tools/list: ficam apenas em COMMANDS (commands/call).
    for deferred in ["command_search", "history_search", "history_read"] {
        assert!(
            coordinator_defs
                .iter()
                .all(|def| def["name"].as_str() != Some(deferred)),
            "{deferred} não deve inflar tools/list"
        );
    }

    let coordinator_bytes = check_tools_list_budget(
        "Coordinator",
        &coordinator_defs,
        TOOLS_LIST_COORDINATOR_MAX_BYTES,
    )
    .expect("tools/list de Coordinator deveria estar dentro do teto de bytes");
    let worker_bytes =
        check_tools_list_budget("Worker", &worker_defs, TOOLS_LIST_WORKER_MAX_BYTES)
            .expect("tools/list de Worker deveria estar dentro do teto de bytes");

    assert!(coordinator_bytes > 0);
    assert!(worker_bytes > 0);
    assert!(worker_bytes < coordinator_bytes);

    // Simula adição de texto excessivo em uma ferramenta de tools/list: o guardião deve falhar alertando.
    let mut inflated = coordinator_defs.clone();
    inflated[0]["description"] = Value::String("descrição inflada desnecessariamente ".repeat(120));
    let inflated_bytes = tools_list_payload_bytes(&inflated);
    assert!(inflated_bytes > TOOLS_LIST_COORDINATOR_MAX_BYTES);

    let err = check_tools_list_budget("Coordinator", &inflated, TOOLS_LIST_COORDINATOR_MAX_BYTES)
        .expect_err("inflação de tools/list precisa falhar o teste de orçamento");
    assert!(
        err.contains("Orçamento de contexto de tools/list") && err.contains("tokens fixos"),
        "mensagem de alerta inesperada: {err}"
    );
}
