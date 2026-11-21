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
    pub(super) app: tauri::App,
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

    pub(super) fn leader_name(&self) -> String {
        let team = self
            .app
            .state::<crate::agents::AgentRegistry>()
            .team()
            .expect("o elenco do teste deveria carregar");
        team.name_of(&crate::agents::Agent::axiom()).to_string()
    }

    pub(super) fn leader(&self, project_id: i64) -> Caller {
        self.agent(
            crate::agents::Agent::axiom().as_str(),
            &self.leader_name(),
            project_id,
        )
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

    pub(super) fn cards(&self, project_id: i64) -> Vec<crate::db::BacklogItem> {
        self.db().list_backlog(project_id).unwrap().items
    }

    pub(super) fn create(&self, who: &Caller, title: &str) -> String {
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
    let axiom = world.agent("axiom", "Axiom", project);

    let outcome = world.call(
        &axiom,
        "create_card",
        json!({ "title": "Revisar login", "description": "Checar o token" }),
    );

    assert!(!outcome.is_error);
    assert_eq!(outcome.text, "Card item-1 criado: Revisar login.");
    let card = &world.cards(project)[0];
    assert_eq!(card.who, "axiom");
    assert_eq!(card.origin, "agente");
    assert_eq!(card.description.as_deref(), Some("Checar o token"));
    assert_eq!(card.activity[0].text, "Axiom criou o card na conversa.");
}

#[test]
fn move_card_aceita_coluna_em_minuscula_e_grava_a_forma_canonica() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);
    let id = world.create(&axiom, "Um");

    let outcome = world.call(
        &axiom,
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
    let axiom = world.agent("axiom", "Axiom", project);
    let id = world.create(&axiom, "Um");
    axiom.slot.lock().confirmations.clear();
    axiom.slot.lock().changed = false;

    let outcome = world.call(&axiom, "move_card", json!({ "id": id, "column": "Backlog" }));

    assert!(!outcome.is_error);
    assert_eq!(outcome.text, "Card item-1 já está em Backlog.");
    assert!(!axiom.slot.lock().changed);
    assert!(axiom.slot.lock().confirmations.is_empty());
}

#[test]
fn read_card_devolve_coluna_dono_origem_descricao_e_atividade() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);
    world.call(
        &axiom,
        "create_card",
        json!({ "title": "Um", "description": "Detalhe" }),
    );

    let outcome = world.call(&axiom, "read_card", json!({ "id": "item-1" }));

    assert!(!outcome.is_error);
    let lines: Vec<&str> = outcome.text.lines().collect();
    assert_eq!(
        &lines[..5],
        [
            "Card item-1: Um",
            "Coluna: Backlog",
            "Dono: axiom",
            "Origem: agente",
            "Descrição: Detalhe"
        ]
    );
    assert!(
        lines[5].ends_with("Axiom criou o card na conversa."),
        "{}",
        lines[5]
    );
}

#[test]
fn list_backlog_lista_do_mais_novo_para_o_mais_antigo_e_filtra_por_coluna() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);
    world.create(&axiom, "Primeiro");
    let second = world.create(&axiom, "Segundo");
    world.call(
        &axiom,
        "move_card",
        json!({ "id": second, "column": "In review" }),
    );

    let all = world.call(&axiom, "list_backlog", json!({}));
    let done = world.call(&axiom, "list_backlog", json!({ "column": "in review" }));
    let ready = world.call(&axiom, "list_backlog", json!({ "column": "Ready" }));

    assert_eq!(
        all.text,
        "item-2 | Segundo | In review | axiom | agente\nitem-1 | Primeiro | Backlog | axiom | agente"
    );
    assert_eq!(done.text, "item-2 | Segundo | In review | axiom | agente");
    assert_eq!((ready.is_error, ready.text.as_str()), (false, NO_CARDS));
}

#[test]
fn list_backlog_de_projeto_sem_cards_responde_nenhum_card_sem_erro() {
    let world = world();
    let project = world.project("vazio");
    let axiom = world.agent("axiom", "Axiom", project);

    let outcome = world.call(&axiom, "list_backlog", json!({}));

    assert!(!outcome.is_error);
    assert_eq!(outcome.text, NO_CARDS);
}

#[test]
fn recusas_do_banco_voltam_como_is_error_com_a_frase_e_nada_e_gravado() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);
    let vexa = world.agent("vexa", "Vexa", project);
    let own = world.create(&axiom, "Da Axiom");
    let foreign = world.create(&vexa, "Do Vexa");
    let closed = world.create(&axiom, "Fechado");
    world
        .db()
        .move_backlog_item(project, &closed, "Done")
        .unwrap();
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
            json!({ "id": "item-999", "column": "In review" }),
            "O item item-999 não existe no backlog deste projeto.".to_string(),
        ),
        (
            "título duplicado",
            "create_card",
            json!({ "title": "  da AXIOM " }),
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
        let outcome = world.call(&axiom, tool, arguments);

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
    let axiom = world.agent("axiom", "Axiom", project);
    let vexa = world.agent("vexa", "Vexa", project);
    let foreign = world.create(&vexa, "Do Vexa");

    axiom.slot.begin(Some(&foreign), None);
    let outcome = world.call(
        &axiom,
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
    let axiom = world.agent("axiom", "Axiom", mine);
    let vexa = world.agent("vexa", "Vexa", mine);
    let davis = world.create(&vexa, "Do Vexa");

    let created = world.call(
        &axiom,
        "create_card",
        json!({ "title": "Injetado", "project_id": other, "projectId": other, "agent": "vexa" }),
    );
    let moved = world.call(
        &axiom,
        "move_card",
        json!({ "id": davis, "column": "Done", "agent": "vexa", "project_id": other }),
    );

    assert!(!created.is_error, "{}", created.text);
    let card = world
        .cards(mine)
        .into_iter()
        .find(|card| card.title == "Injetado")
        .expect("no projeto do binding");
    assert_eq!(card.who, "axiom");
    assert!(world.cards(other).is_empty());
    assert!(moved.is_error);
    assert_eq!(column(&world, mine, &davis), "Backlog");
}

#[test]
fn titulo_com_mais_de_200_caracteres_e_recusado_e_com_exatamente_200_e_aceito() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);

    let over = world.call(
        &axiom,
        "create_card",
        json!({ "title": "a".repeat(MAX_TITLE + 1) }),
    );
    let created_after_over = world.cards(project).len();
    let exact = world.call(
        &axiom,
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
    let in_a = world.agent("axiom", "Axiom", first);
    let in_b = world.agent("axiom", "Axiom", second);
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
    assert_eq!(list_b.text, "item-1 | Do B | Backlog | axiom | agente");
    assert_eq!(column(&world, first, "item-1"), "Backlog");
}

#[test]
fn argumentos_fora_do_schema_voltam_como_is_error() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);

    let outcome = world.call(&axiom, "create_card", json!({ "description": "sem título" }));

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
    let axiom = world.agent("axiom", "Axiom", project);
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });

    let reply = run(send(
        world.port,
        &Request::post(world.port, &axiom.token, body),
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
        [
            "list_backlog",
            "read_card",
            "create_card",
            "move_card",
            "update_card",
            "backlog_add_label",
            "backlog_remove_label",
            "backlog_set_responsible",
            "backlog_archive_item",
            "backlog_restore_item",
            "backlog_delete_item",
            "request_decision",
            "propose_request",
            "delegate",
            "get_variables",
            "set_variable",
            "get_orchestration",
            "list_agents",
            "save_item_plan",
            "artifact_set_criterion",
            "list_movements",
            "post_channel_message",
            "app_commands_list",
            "app_command_call"
        ]
    );
    assert_eq!(names.len(), 24);
    assert!(tools
        .iter()
        .all(|tool| tool["inputSchema"]["additionalProperties"] == json!(false)));
    let columns: Vec<&str> = tools[3]["inputSchema"]["properties"]["column"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|column| column.as_str().unwrap())
        .collect();
    assert_eq!(columns, ["Backlog", "In progress", "In review"]);
    assert_eq!(
        world.server.tool_names(),
        [
            "mcp__vokia__list_backlog",
            "mcp__vokia__read_card",
            "mcp__vokia__create_card",
            "mcp__vokia__move_card",
            "mcp__vokia__update_card",
            "mcp__vokia__backlog_add_label",
            "mcp__vokia__backlog_remove_label",
            "mcp__vokia__backlog_set_responsible",
            "mcp__vokia__backlog_archive_item",
            "mcp__vokia__backlog_restore_item",
            "mcp__vokia__backlog_delete_item",
            "mcp__vokia__request_decision",
            "mcp__vokia__propose_request",
            "mcp__vokia__delegate",
            "mcp__vokia__get_variables",
            "mcp__vokia__set_variable",
            "mcp__vokia__get_orchestration",
            "mcp__vokia__list_agents",
            "mcp__vokia__save_item_plan",
            "mcp__vokia__artifact_set_criterion",
            "mcp__vokia__list_movements",
            "mcp__vokia__post_channel_message",
            "mcp__vokia__app_commands_list",
            "mcp__vokia__app_command_call"
        ]
    );
}

#[test]
fn sucesso_grava_confirmacao_no_slot_e_emite_um_evento_so_quando_algo_mudou() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);
    let events: Arc<Mutex<Vec<Value>>> = Arc::default();
    let sink = events.clone();
    world.app.handle().listen(EVENT_NAME, move |event| {
        sink.lock()
            .unwrap()
            .push(serde_json::from_str(event.payload()).unwrap());
    });

    world.call(&axiom, "create_card", json!({ "title": "Um" }));
    world.call(
        &axiom,
        "move_card",
        json!({ "id": "item-1", "column": "Backlog" }),
    );
    world.call(
        &axiom,
        "move_card",
        json!({ "id": "item-1", "column": "Ready" }),
    );
    world.call(&axiom, "read_card", json!({ "id": "item-1" }));
    world.call(
        &axiom,
        "move_card",
        json!({ "id": "item-1", "column": "In review" }),
    );
    std::thread::sleep(std::time::Duration::from_millis(300));

    let slot = axiom.slot.lock();
    assert!(slot.changed);
    assert_eq!(
        slot.confirmations,
        ["Card item-1 criado: Um.", "Card item-1 movido para In review."]
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
    let axiom = world.agent("axiom", "Axiom", project);

    axiom.slot.begin(None, None);
    world.call(&axiom, "create_card", json!({ "title": "Um" }));
    let collected = axiom.slot.end().confirmations;
    let again = axiom.slot.end().confirmations;

    assert_eq!(collected, ["Card item-1 criado: Um."]);
    assert!(again.is_empty());
    assert!(!axiom.slot.lock().changed);
    assert!(axiom.slot.lock().confirmations.is_empty());
}

#[test]
fn confirmacao_nao_coletada_de_uma_troca_nao_vaza_para_a_proxima() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);

    axiom.slot.begin(None, None);
    world.call(&axiom, "create_card", json!({ "title": "Um" }));
    // A troca caiu antes do end(): a seguinte começa do zero.
    axiom.slot.begin(None, None);
    let collected = axiom.slot.end().confirmations;

    assert!(collected.is_empty(), "{collected:?}");
    assert!(!axiom.slot.lock().changed);
}

#[test]
fn card_do_pedido_so_vale_dentro_da_troca_e_deixa_de_valer_depois_do_end() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);
    let vexa = world.agent("vexa", "Vexa", project);
    let foreign = world.create(&vexa, "Do Vexa");

    axiom.slot.begin(Some(&foreign), None);
    axiom.slot.end();
    let outcome = world.call(
        &axiom,
        "move_card",
        json!({ "id": foreign, "column": "In progress" }),
    );

    assert!(outcome.is_error);
    assert_eq!(
        outcome.text,
        format!("O card {foreign} não é seu nem do pedido atual.")
    );
}

const PLAN_PROPOSAL: &str = "Resumo\n#lote 1|Base|Base pronta|Criar tabela\n#lote 2|Tela|Tela funcionando";

fn linked_plan_arguments(card: &str) -> Value {
    json!({
        "item_id": card,
        "state": "vinculado",
        "proposal": PLAN_PROPOSAL,
        "plan": { "id": card, "name": "Plano", "meta": "proposta aceita" },
        "agent": "axiom"
    })
}

fn criteria_of(world: &World, project: i64) -> Vec<(String, bool)> {
    world
        .db()
        .list_artifacts(project)
        .unwrap()
        .into_iter()
        .flat_map(|artifact| artifact.criteria)
        .map(|criterion| (criterion.id, criterion.done))
        .collect()
}

#[test]
fn save_item_plan_vinculado_pelo_mcp_cria_os_filhos_e_marca_a_troca_como_alterada() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);
    let card = world.create(&axiom, "Login");
    axiom.slot.lock().changed = false;

    let outcome = world.call(&axiom, "save_item_plan", linked_plan_arguments(&card));

    assert!(!outcome.is_error, "{}", outcome.text);
    let mut ids: Vec<String> = world
        .cards(project)
        .into_iter()
        .map(|item| item.id)
        .filter(|id| id.starts_with("plano-"))
        .collect();
    ids.sort();
    assert_eq!(
        ids,
        [
            format!("plano-{card}.1"),
            format!("plano-{card}.1.1"),
            format!("plano-{card}.2")
        ]
    );
    let parent = world.cards(project).into_iter().find(|item| item.id == card).unwrap();
    let children = parent.children.expect("o card deveria ter progresso");
    assert_eq!((children.total, children.done), (2, 0));
    assert!(axiom.slot.lock().changed);
}

#[test]
fn artifact_set_criterion_marca_responde_com_a_frase_e_aparece_marcado_na_leitura() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);
    let card = world.create(&axiom, "Login");
    world.call(&axiom, "save_item_plan", linked_plan_arguments(&card));

    let outcome = world.call(
        &axiom,
        "artifact_set_criterion",
        json!({ "artifact_id": format!("plano-{card}"), "criterion_id": "c1", "done": true }),
    );

    assert!(!outcome.is_error, "{}", outcome.text);
    assert_eq!(outcome.text, format!("Critério c1 do plano plano-{card} marcado."));
    assert_eq!(
        criteria_of(&world, project),
        [("c1".to_string(), true), ("c2".to_string(), false)]
    );

    let undone = world.call(
        &axiom,
        "artifact_set_criterion",
        json!({ "artifact_id": format!("plano-{card}"), "criterion_id": "c1", "done": false }),
    );
    assert_eq!(undone.text, format!("Critério c1 do plano plano-{card} desmarcado."));
    assert_eq!(criteria_of(&world, project)[0], ("c1".to_string(), false));
}

#[test]
fn artifact_set_criterion_com_criterio_ou_plano_inexistente_volta_como_erro_com_a_frase() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);
    let card = world.create(&axiom, "Login");
    world.call(&axiom, "save_item_plan", linked_plan_arguments(&card));

    let wrong_criterion = world.call(
        &axiom,
        "artifact_set_criterion",
        json!({ "artifact_id": format!("plano-{card}"), "criterion_id": "c9", "done": true }),
    );
    let wrong_plan = world.call(
        &axiom,
        "artifact_set_criterion",
        json!({ "artifact_id": "plano-item-99", "criterion_id": "c1", "done": true }),
    );

    assert!(wrong_criterion.is_error);
    assert_eq!(wrong_criterion.text, "O critério c9 não existe neste plano.");
    assert!(wrong_plan.is_error);
    assert_eq!(wrong_plan.text, "O plano plano-item-99 não existe neste projeto.");
    assert_eq!(
        criteria_of(&world, project),
        [("c1".to_string(), false), ("c2".to_string(), false)]
    );
}

#[test]
fn artifact_set_criterion_de_outro_projeto_e_recusado_e_done_invalido_volta_como_argumento_invalido() {
    let world = world();
    let project_a = world.project("a");
    let project_b = world.project("b");
    let axiom_a = world.agent("axiom", "Axiom", project_a);
    let axiom_b = world.agent("axiom", "Axiom", project_b);
    let card = world.create(&axiom_a, "Login");
    world.call(&axiom_a, "save_item_plan", linked_plan_arguments(&card));

    let foreign = world.call(
        &axiom_b,
        "artifact_set_criterion",
        json!({ "artifact_id": format!("plano-{card}"), "criterion_id": "c1", "done": true }),
    );
    let bad_done = world.call(
        &axiom_a,
        "artifact_set_criterion",
        json!({ "artifact_id": format!("plano-{card}"), "criterion_id": "c1", "done": "sim" }),
    );
    let missing_done = world.call(
        &axiom_a,
        "artifact_set_criterion",
        json!({ "artifact_id": format!("plano-{card}"), "criterion_id": "c1" }),
    );

    assert!(foreign.is_error);
    assert_eq!(foreign.text, format!("O plano plano-{card} não existe neste projeto."));
    for outcome in [bad_done, missing_done] {
        assert!(outcome.is_error);
        assert_eq!(
            outcome.text,
            "Argumentos inválidos para artifact_set_criterion: confira o schema da ferramenta."
        );
    }
    assert_eq!(criteria_of(&world, project_a)[0], ("c1".to_string(), false));
}

#[test]
fn app_commands_list_com_dominio_roles_retorna_apenas_os_quatro_comandos_com_schemas() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);

    let outcome = world.call(
        &axiom,
        "app_commands_list",
        json!({ "domain": "roles" }),
    );

    assert!(!outcome.is_error, "{}", outcome.text);
    let list: Vec<Value> = serde_json::from_str(&outcome.text).expect("JSON array");
    assert_eq!(list.len(), 4);

    let names: Vec<&str> = list
        .iter()
        .map(|cmd| cmd["name"].as_str().expect("name"))
        .collect();
    assert_eq!(
        names,
        ["role_list", "role_create", "role_update", "role_delete"]
    );

    for cmd in &list {
        assert_eq!(cmd["domain"], "roles");
        assert!(cmd["description"].is_string());
        assert_eq!(cmd["inputSchema"]["type"], "object");
    }
}

#[test]
fn app_command_call_sem_name_ou_uionly_ou_inexistente_retorna_erro_e_comando_valido_executa() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);

    // 1. tools/call para app_command_call sem name -> erro claro (-32602)
    let body_sem_name = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "app_command_call",
            "arguments": {}
        }
    });
    let reply_sem_name = run(send(world.port, &Request::post(world.port, &axiom.token, body_sem_name)));
    let json_sem_name = reply_sem_name.json();
    assert_eq!(json_sem_name["error"]["code"], -32602);
    assert_eq!(
        json_sem_name["error"]["message"],
        "falta o nome do comando em app_command_call"
    );

    // 2. tools/call para app_command_call com name: "pick_folder" (UiOnly) -> isError: true
    let outcome_uionly = world.call(
        &axiom,
        "app_command_call",
        json!({ "name": "pick_folder" }),
    );
    assert!(outcome_uionly.is_error);
    assert_eq!(
        outcome_uionly.text,
        "Comando exclusivo da interface gráfica (não disponível via MCP/CLI)."
    );

    // 3. tools/call para app_command_call com name: "inexistente" -> isError: true
    let outcome_inexistente = world.call(
        &axiom,
        "app_command_call",
        json!({ "name": "inexistente" }),
    );
    assert!(outcome_inexistente.is_error);
    assert_eq!(outcome_inexistente.text, "comando desconhecido: inexistente");

    // 4. Execução de comando administrativo válido via app_command_call
    let outcome_valido = world.call(
        &axiom,
        "app_command_call",
        json!({ "name": "role_list" }),
    );
    assert!(!outcome_valido.is_error, "{}", outcome_valido.text);
}

#[test]
fn meta_tools_e_commands_list_e_commands_call_sem_regressao_nas_tools_existentes() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);

    // 1. McpServer::tool_names() inclui mcp__vokia__app_commands_list e mcp__vokia__app_command_call
    let tool_names = world.server.tool_names();
    assert!(tool_names.iter().any(|name| name == "mcp__vokia__app_commands_list"));
    assert!(tool_names.iter().any(|name| name == "mcp__vokia__app_command_call"));
    assert_eq!(tool_names.len(), 24);

    // 2. commands/list aceita filtro por domínio
    let body_list = json!({
        "jsonrpc": "2.0",
        "id": 10,
        "method": "commands/list",
        "params": { "domain": "roles" }
    });
    let reply_list = run(send(world.port, &Request::post(world.port, &axiom.token, body_list)));
    let commands = reply_list.json()["result"]["commands"].as_array().cloned().expect("commands");
    assert_eq!(commands.len(), 4);

    // 3. commands/call direto continua funcionando
    let body_call = json!({
        "jsonrpc": "2.0",
        "id": 11,
        "method": "commands/call",
        "params": {
            "name": "role_list",
            "arguments": {}
        }
    });
    let reply_call = run(send(world.port, &Request::post(world.port, &axiom.token, body_call)));
    assert_eq!(reply_call.status, 200);
    assert_eq!(reply_call.json()["result"]["isError"], false);

    // 4. Tool existente de backlog continua funcionando
    let card_id = world.create(&axiom, "Verificação regressão");
    assert!(!card_id.is_empty());
}

#[test]
fn app_commands_list_e_commands_list_retornam_when_to_use_e_schema_com_filtro_domain() {
    let world = world();
    let project = world.project("mcp3");
    let axiom = world.agent("axiom", "Axiom", project);

    // 1. app_commands_list sem filtro devolve comandos com when_to_use preenchido
    let outcome_todos = world.call(
        &axiom,
        "app_commands_list",
        json!({}),
    );
    assert!(!outcome_todos.is_error);
    let json_todos: Value = serde_json::from_str(&outcome_todos.text).expect("json app_commands_list");
    let cmds = json_todos.as_array().expect("array commands");
    assert!(!cmds.is_empty());
    for cmd in cmds {
        let when = cmd["when_to_use"].as_str().unwrap_or_default();
        assert!(!when.is_empty(), "when_to_use vazio no comando {}", cmd["name"]);
        assert!(cmd["inputSchema"].is_object(), "schema ausente no comando {}", cmd["name"]);
    }

    // 2. app_commands_list com filtro domain: roles devolve apenas os 4 comandos de roles
    let outcome_roles = world.call(
        &axiom,
        "app_commands_list",
        json!({ "domain": "roles" }),
    );
    assert!(!outcome_roles.is_error);
    let json_roles: Value = serde_json::from_str(&outcome_roles.text).expect("json app_commands_list roles");
    let roles_cmds = json_roles.as_array().expect("array roles");
    assert_eq!(roles_cmds.len(), 4);
    for cmd in roles_cmds {
        assert_eq!(cmd["domain"], "roles");
        let when = cmd["when_to_use"].as_str().unwrap_or_default();
        assert!(!when.is_empty());
    }

    // 3. commands/list via JSON-RPC também retorna when_to_use
    let body_list = json!({
        "jsonrpc": "2.0",
        "id": 99,
        "method": "commands/list",
        "params": { "domain": "agents" }
    });
    let reply_list = run(send(world.port, &Request::post(world.port, &axiom.token, body_list)));
    assert_eq!(reply_list.status, 200);
    let agents_cmds = reply_list.json()["result"]["commands"].as_array().cloned().expect("agents commands");
    assert!(!agents_cmds.is_empty());
    for cmd in &agents_cmds {
        assert_eq!(cmd["domain"], "agents");
        assert!(!cmd["when_to_use"].as_str().unwrap_or_default().is_empty());
    }
}
