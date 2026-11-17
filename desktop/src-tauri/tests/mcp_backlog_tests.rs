use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tauri::{Listener, Manager};

use super::support::{build_app, port_of, run, send, Request};
use super::*;
use crate::agents::Agent;
use crate::db::{DbState, NewProject};
use crate::events::EVENT_NAME;

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

const MAX_TITLE: usize = 200;
const TOO_LONG: &str = "O título do card passa de 200 caracteres.";
const NO_CARDS: &str = "Nenhum card.";

pub(super) struct World {
    app: tauri::App,
    dir: PathBuf,
    pub(super) server: McpServer,
    pub(super) port: u16,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub(super) struct Caller {
    pub(super) token: Token,
    pub(super) slot: ExchangeSlot,
    pub(super) project_id: i64,
}

pub(super) struct Outcome {
    pub(super) is_error: bool,
    pub(super) text: String,
}

pub(super) fn world() -> World {
    let dir = std::env::temp_dir().join(format!(
        "vokia-mcp-backlog-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::SeqCst)
    ));
    let app = build_app(&dir);
    let server = tauri::async_runtime::block_on(McpServer::start()).expect("servidor MCP");
    server.attach(app.handle().clone());
    let port = port_of(&server);
    World {
        app,
        dir,
        server,
        port,
    }
}

impl World {
    pub(super) fn db(&self) -> &crate::db::Db {
        self.app
            .state::<DbState>()
            .inner()
            .db()
            .expect("banco de teste aberto")
    }

    pub(super) fn project(&self, name: &str) -> i64 {
        self.db()
            .insert_project(&NewProject {
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

    pub(super) fn agent(&self, id: &str, name: &str, project_id: i64) -> Caller {
        let slot = ExchangeSlot::default();
        let token = self
            .server
            .register(Binding {
                project_id,
                agent: Agent::from_stored(id).expect("agente"),
                agent_name: name.to_string(),
                slot: slot.clone(),
            })
            .expect("token");
        Caller {
            token,
            slot,
            project_id,
        }
    }

    pub(super) fn call(&self, who: &Caller, tool: &str, arguments: Value) -> Outcome {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": tool, "arguments": arguments } });
        let reply = run(send(self.port, &Request::post(self.port, &who.token, body)));
        let json = reply.json();
        assert_eq!(reply.status, 200, "{}", reply.body);
        Outcome {
            is_error: json["result"]["isError"]
                .as_bool()
                .unwrap_or_else(|| panic!("sem result.isError: {}", reply.body)),
            text: json["result"]["content"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        }
    }

    fn cards(&self, project_id: i64) -> Vec<crate::db::BacklogItem> {
        self.db().list_backlog(project_id).unwrap().items
    }

    fn create(&self, who: &Caller, title: &str) -> String {
        let outcome = self.call(who, "create_card", json!({ "title": title }));
        assert!(!outcome.is_error, "{}", outcome.text);
        self.cards(who.project_id)
            .iter()
            .find(|card| card.title == title)
            .map(|card| card.id.clone())
            .expect("o card criado deveria existir")
    }
}

#[test]
fn create_card_grava_no_projeto_do_binding_com_o_agente_dono_e_confirma_na_frase() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);

    let outcome = world.call(
        &iara,
        "create_card",
        json!({ "title": "Revisar login", "description": "Checar o token" }),
    );

    assert!(!outcome.is_error);
    assert_eq!(outcome.text, "Card item-1 criado: Revisar login.");
    let card = &world.cards(project)[0];
    assert_eq!(card.who, "iara");
    assert_eq!(card.origin, "agente");
    assert_eq!(card.description.as_deref(), Some("Checar o token"));
    assert_eq!(card.activity[0].text, "Iara criou o card na conversa.");
}

#[test]
fn move_card_aceita_coluna_em_minuscula_e_grava_a_forma_canonica() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    let id = world.create(&iara, "Um");

    let outcome = world.call(
        &iara,
        "move_card",
        json!({ "id": id, "column": "in progress" }),
    );

    assert!(!outcome.is_error);
    assert_eq!(outcome.text, "Card item-1 movido para In progress.");
    assert_eq!(
        world.cards(project)[0].column.as_deref(),
        Some("In progress")
    );
}

#[test]
fn move_card_para_a_coluna_atual_avisa_sem_mudar_nem_confirmar_no_slot() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    let id = world.create(&iara, "Um");
    iara.slot.lock().confirmations.clear();
    iara.slot.lock().changed = false;

    let outcome = world.call(&iara, "move_card", json!({ "id": id, "column": "Backlog" }));

    assert!(!outcome.is_error);
    assert_eq!(outcome.text, "Card item-1 já está em Backlog.");
    assert!(!iara.slot.lock().changed);
    assert!(iara.slot.lock().confirmations.is_empty());
}

#[test]
fn read_card_devolve_coluna_dono_origem_descricao_e_atividade() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    world.call(
        &iara,
        "create_card",
        json!({ "title": "Um", "description": "Detalhe" }),
    );

    let outcome = world.call(&iara, "read_card", json!({ "id": "item-1" }));

    assert!(!outcome.is_error);
    let lines: Vec<&str> = outcome.text.lines().collect();
    assert_eq!(
        &lines[..5],
        [
            "Card item-1: Um",
            "Coluna: Backlog",
            "Dono: iara",
            "Origem: agente",
            "Descrição: Detalhe"
        ]
    );
    assert!(
        lines[5].ends_with("Iara criou o card na conversa."),
        "{}",
        lines[5]
    );
}

#[test]
fn list_backlog_lista_do_mais_novo_para_o_mais_antigo_e_filtra_por_coluna() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    world.create(&iara, "Primeiro");
    let second = world.create(&iara, "Segundo");
    world.call(
        &iara,
        "move_card",
        json!({ "id": second, "column": "Done" }),
    );

    let all = world.call(&iara, "list_backlog", json!({}));
    let done = world.call(&iara, "list_backlog", json!({ "column": "done" }));
    let ready = world.call(&iara, "list_backlog", json!({ "column": "Ready" }));

    assert_eq!(
        all.text,
        "item-2 | Segundo | Done | iara | agente\nitem-1 | Primeiro | Backlog | iara | agente"
    );
    assert_eq!(done.text, "item-2 | Segundo | Done | iara | agente");
    assert_eq!((ready.is_error, ready.text.as_str()), (false, NO_CARDS));
}

#[test]
fn list_backlog_de_projeto_sem_cards_responde_nenhum_card_sem_erro() {
    let world = world();
    let project = world.project("vazio");
    let iara = world.agent("iara", "Iara", project);

    let outcome = world.call(&iara, "list_backlog", json!({}));

    assert!(!outcome.is_error);
    assert_eq!(outcome.text, NO_CARDS);
}

#[test]
fn recusas_do_banco_voltam_como_is_error_com_a_frase_e_nada_e_gravado() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    let davi = world.agent("davi", "Davi", project);
    let own = world.create(&iara, "Da Iara");
    let foreign = world.create(&davi, "Do Davi");
    let closed = world.create(&iara, "Fechado");
    world.call(
        &iara,
        "move_card",
        json!({ "id": closed, "column": "Done" }),
    );
    let before = world.cards(project).len();

    let cases = [
        (
            "Ready",
            "move_card",
            json!({ "id": own, "column": "Ready" }),
            "Agente não move card para Ready.".to_string(),
        ),
        (
            "card alheio",
            "move_card",
            json!({ "id": foreign, "column": "In progress" }),
            format!("O card {foreign} não é seu nem do pedido atual."),
        ),
        (
            "id inexistente",
            "move_card",
            json!({ "id": "item-999", "column": "Done" }),
            "O item item-999 não existe no backlog deste projeto.".to_string(),
        ),
        (
            "título duplicado",
            "create_card",
            json!({ "title": "  da IARA " }),
            format!("Já existe o card {own} com esse título."),
        ),
        (
            "Done não volta",
            "move_card",
            json!({ "id": closed, "column": "In progress" }),
            format!("O card {closed} já está em Done."),
        ),
        (
            "coluna inexistente",
            "move_card",
            json!({ "id": own, "column": "Review" }),
            "O backlog não tem a coluna \"Review\".".to_string(),
        ),
    ];
    for (label, tool, arguments, phrase) in cases {
        let outcome = world.call(&iara, tool, arguments);

        assert!(outcome.is_error, "{label}");
        assert_eq!(outcome.text, phrase, "{label}");
    }
    assert_eq!(world.cards(project).len(), before);
    assert_eq!(column(&world, project, &own), "Backlog");
    assert_eq!(column(&world, project, &closed), "Done");
}

fn column(world: &World, project: i64, id: &str) -> String {
    world
        .cards(project)
        .into_iter()
        .find(|card| card.id == id)
        .and_then(|card| card.column)
        .expect("card com coluna")
}

#[test]
fn o_card_do_pedido_atual_no_slot_pode_ser_movido_mesmo_com_outro_dono() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    let davi = world.agent("davi", "Davi", project);
    let foreign = world.create(&davi, "Do Davi");

    iara.slot.begin(Some(&foreign), None);
    let outcome = world.call(
        &iara,
        "move_card",
        json!({ "id": foreign, "column": "In progress" }),
    );

    assert!(!outcome.is_error, "{}", outcome.text);
    assert_eq!(column(&world, project, &foreign), "In progress");
}

#[test]
fn project_id_e_agent_nos_argumentos_sao_ignorados() {
    let world = world();
    let mine = world.project("minha");
    let other = world.project("outra");
    let iara = world.agent("iara", "Iara", mine);
    let davi = world.agent("davi", "Davi", mine);
    let davis = world.create(&davi, "Do Davi");

    let created = world.call(
        &iara,
        "create_card",
        json!({ "title": "Injetado", "project_id": other, "projectId": other, "agent": "davi" }),
    );
    let moved = world.call(
        &iara,
        "move_card",
        json!({ "id": davis, "column": "Done", "agent": "davi", "project_id": other }),
    );

    assert!(!created.is_error, "{}", created.text);
    let card = world
        .cards(mine)
        .into_iter()
        .find(|card| card.title == "Injetado")
        .expect("no projeto do binding");
    assert_eq!(card.who, "iara");
    assert!(world.cards(other).is_empty());
    assert!(moved.is_error);
    assert_eq!(column(&world, mine, &davis), "Backlog");
}

#[test]
fn titulo_com_mais_de_200_caracteres_e_recusado_e_com_exatamente_200_e_aceito() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);

    let over = world.call(
        &iara,
        "create_card",
        json!({ "title": "a".repeat(MAX_TITLE + 1) }),
    );
    let created_after_over = world.cards(project).len();
    let exact = world.call(
        &iara,
        "create_card",
        json!({ "title": "é".repeat(MAX_TITLE) }),
    );

    assert!(over.is_error);
    assert_eq!(over.text, TOO_LONG);
    assert_eq!(created_after_over, 0);
    assert!(!exact.is_error, "{}", exact.text);
    assert_eq!(world.cards(project).len(), 1);
}

#[test]
fn card_de_outro_projeto_com_o_mesmo_id_nao_aparece_nem_se_confunde() {
    let world = world();
    let first = world.project("a");
    let second = world.project("b");
    let in_a = world.agent("iara", "Iara", first);
    let in_b = world.agent("iara", "Iara", second);
    world.create(&in_a, "Só do A");

    let missing = world.call(&in_b, "read_card", json!({ "id": "item-1" }));
    let moved = world.call(
        &in_b,
        "move_card",
        json!({ "id": "item-1", "column": "Done" }),
    );
    world.create(&in_b, "Do B");
    let read_b = world.call(&in_b, "read_card", json!({ "id": "item-1" }));
    let list_b = world.call(&in_b, "list_backlog", json!({}));

    assert_eq!(
        (missing.is_error, missing.text.as_str()),
        (true, "O item item-1 não existe no backlog deste projeto.")
    );
    assert!(moved.is_error);
    assert!(
        read_b.text.starts_with("Card item-1: Do B"),
        "{}",
        read_b.text
    );
    assert_eq!(list_b.text, "item-1 | Do B | Backlog | iara | agente");
    assert_eq!(column(&world, first, "item-1"), "Backlog");
}

#[test]
fn argumentos_fora_do_schema_voltam_como_is_error() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);

    let outcome = world.call(&iara, "create_card", json!({ "description": "sem título" }));

    assert!(outcome.is_error);
    assert_eq!(
        outcome.text,
        "Argumentos inválidos para create_card: confira o schema da ferramenta."
    );
    assert!(world.cards(project).is_empty());
}

#[test]
fn tools_list_traz_as_ferramentas_e_move_card_nao_oferece_ready() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });

    let reply = run(send(
        world.port,
        &Request::post(world.port, &iara.token, body),
    ));

    let tools = reply.json()["result"]["tools"]
        .as_array()
        .cloned()
        .expect("lista");
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["list_backlog", "read_card", "create_card", "move_card", "request_decision", "delegate"]
    );
    assert!(tools
        .iter()
        .all(|tool| tool["inputSchema"]["additionalProperties"] == json!(false)));
    let columns: Vec<&str> = tools[3]["inputSchema"]["properties"]["column"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|column| column.as_str().unwrap())
        .collect();
    assert_eq!(columns, ["Backlog", "In progress", "In review", "Done"]);
    assert_eq!(
        world.server.tool_names(),
        [
            "mcp__vokia__list_backlog",
            "mcp__vokia__read_card",
            "mcp__vokia__create_card",
            "mcp__vokia__move_card",
            "mcp__vokia__request_decision",
            "mcp__vokia__delegate"
        ]
    );
}

#[test]
fn sucesso_grava_confirmacao_no_slot_e_emite_um_evento_so_quando_algo_mudou() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    let events: Arc<Mutex<Vec<Value>>> = Arc::default();
    let sink = events.clone();
    world.app.handle().listen(EVENT_NAME, move |event| {
        sink.lock()
            .unwrap()
            .push(serde_json::from_str(event.payload()).unwrap());
    });

    world.call(&iara, "create_card", json!({ "title": "Um" }));
    world.call(
        &iara,
        "move_card",
        json!({ "id": "item-1", "column": "Backlog" }),
    );
    world.call(
        &iara,
        "move_card",
        json!({ "id": "item-1", "column": "Ready" }),
    );
    world.call(&iara, "read_card", json!({ "id": "item-1" }));
    world.call(
        &iara,
        "move_card",
        json!({ "id": "item-1", "column": "Done" }),
    );
    std::thread::sleep(std::time::Duration::from_millis(300));

    let slot = iara.slot.lock();
    assert!(slot.changed);
    assert_eq!(
        slot.confirmations,
        ["Card item-1 criado: Um.", "Card item-1 movido para Done."]
    );
    let seen: Vec<Value> = events.lock().unwrap().clone();
    let backlog: Vec<&Value> = seen
        .iter()
        .filter(|event| event["type"] == json!("backlog-changed"))
        .collect();
    assert_eq!(backlog.len(), 2, "{seen:?}");
    assert!(backlog
        .iter()
        .all(|event| event["projectId"] == json!(project)));
}

#[test]
fn confirmacao_da_troca_sai_no_end_e_o_slot_fica_vazio_depois_do_envio() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);

    iara.slot.begin(None, None);
    world.call(&iara, "create_card", json!({ "title": "Um" }));
    let collected = iara.slot.end().confirmations;
    let again = iara.slot.end().confirmations;

    assert_eq!(collected, ["Card item-1 criado: Um."]);
    assert!(again.is_empty());
    assert!(!iara.slot.lock().changed);
    assert!(iara.slot.lock().confirmations.is_empty());
}

#[test]
fn confirmacao_nao_coletada_de_uma_troca_nao_vaza_para_a_proxima() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);

    iara.slot.begin(None, None);
    world.call(&iara, "create_card", json!({ "title": "Um" }));
    // A troca caiu antes do end(): a seguinte começa do zero.
    iara.slot.begin(None, None);
    let collected = iara.slot.end().confirmations;

    assert!(collected.is_empty(), "{collected:?}");
    assert!(!iara.slot.lock().changed);
}

#[test]
fn card_do_pedido_so_vale_dentro_da_troca_e_deixa_de_valer_depois_do_end() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    let davi = world.agent("davi", "Davi", project);
    let foreign = world.create(&davi, "Do Davi");

    iara.slot.begin(Some(&foreign), None);
    iara.slot.end();
    let outcome = world.call(
        &iara,
        "move_card",
        json!({ "id": foreign, "column": "In progress" }),
    );

    assert!(outcome.is_error);
    assert_eq!(
        outcome.text,
        format!("O card {foreign} não é seu nem do pedido atual.")
    );
}
