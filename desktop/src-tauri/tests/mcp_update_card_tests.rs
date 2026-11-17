use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tauri::Listener;

use super::backlog_tests::{world, Outcome, World};
use super::support::{run, send, Request};
use crate::events::EVENT_NAME;

const WITHOUT_FIELDS: &str =
    "Informe o novo título ou a nova descrição: sem nenhum dos dois não há o que atualizar.";
const TOO_LONG: &str = "O título do card passa de 200 caracteres.";
const MAX_TITLE: usize = 200;

fn update(world: &World, who: &super::backlog_tests::Caller, arguments: Value) -> Outcome {
    world.call(who, "update_card", arguments)
}

#[test]
fn editar_so_o_titulo_grava_confirma_registra_atividade_e_mantem_a_descricao() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    world.call(
        &iara,
        "create_card",
        json!({ "title": "Velho", "description": "Contexto" }),
    );

    let outcome = update(&world, &iara, json!({ "id": "item-1", "title": " Novo " }));

    assert!(!outcome.is_error, "{}", outcome.text);
    assert_eq!(outcome.text, "Card item-1 atualizado: Novo.");
    let card = &world.cards(project)[0];
    assert_eq!(card.title, "Novo");
    assert_eq!(card.description.as_deref(), Some("Contexto"));
    assert_eq!(
        card.activity.last().map(|entry| entry.text.as_str()),
        Some("Iara editou o título.")
    );
}

#[test]
fn editar_so_a_descricao_e_os_dois_de_uma_vez_registram_a_atividade_certa() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    world.call(&iara, "create_card", json!({ "title": "Um" }));

    update(
        &world,
        &iara,
        json!({ "id": "item-1", "description": "Primeira" }),
    );
    update(
        &world,
        &iara,
        json!({ "id": "item-1", "title": "Dois", "description": "Segunda" }),
    );

    let card = &world.cards(project)[0];
    assert_eq!(card.title, "Dois");
    assert_eq!(card.description.as_deref(), Some("Segunda"));
    let activity: Vec<&str> = card
        .activity
        .iter()
        .map(|entry| entry.text.as_str())
        .collect();
    assert_eq!(
        activity[1..],
        [
            "Iara editou a descrição.",
            "Iara editou o título e a descrição."
        ]
    );
}

#[test]
fn chamada_sem_nenhum_campo_ou_so_com_descricao_em_branco_e_recusada() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    world.call(
        &iara,
        "create_card",
        json!({ "title": "Um", "description": "Atual" }),
    );

    for arguments in [
        json!({ "id": "item-1" }),
        json!({ "id": "item-1", "description": "   " }),
    ] {
        let outcome = update(&world, &iara, arguments);
        assert!(outcome.is_error);
        assert_eq!(outcome.text, WITHOUT_FIELDS);
    }
    assert_eq!(
        world.cards(project)[0].description.as_deref(),
        Some("Atual")
    );
}

#[test]
fn descricao_vazia_junto_de_um_titulo_nao_apaga_a_descricao_atual() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    world.call(
        &iara,
        "create_card",
        json!({ "title": "Um", "description": "Atual" }),
    );

    let outcome = update(
        &world,
        &iara,
        json!({ "id": "item-1", "title": "Dois", "description": "" }),
    );

    assert!(!outcome.is_error, "{}", outcome.text);
    let card = &world.cards(project)[0];
    assert_eq!(card.title, "Dois");
    assert_eq!(card.description.as_deref(), Some("Atual"));
}

#[test]
fn titulo_em_branco_e_titulo_acima_de_200_sao_recusados_e_200_exatos_passam() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    world.call(&iara, "create_card", json!({ "title": "Um" }));

    let blank = update(&world, &iara, json!({ "id": "item-1", "title": "   " }));
    let long = update(
        &world,
        &iara,
        json!({ "id": "item-1", "title": "x".repeat(MAX_TITLE + 1) }),
    );
    let exact = update(
        &world,
        &iara,
        json!({ "id": "item-1", "title": "y".repeat(MAX_TITLE) }),
    );

    assert!(blank.is_error);
    assert!(long.is_error);
    assert_eq!(long.text, TOO_LONG);
    assert!(!exact.is_error, "{}", exact.text);
    assert_eq!(world.cards(project)[0].title.chars().count(), MAX_TITLE);
}

#[test]
fn card_de_outro_agente_e_recusado_e_o_card_da_troca_atual_e_aceito() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    let davi = world.agent("davi", "Davi", project);
    world.call(&davi, "create_card", json!({ "title": "Do Davi" }));

    let refused = update(&world, &iara, json!({ "id": "item-1", "title": "Roubado" }));
    iara.slot.begin(Some("item-1"), None);
    let accepted = update(
        &world,
        &iara,
        json!({ "id": "item-1", "title": "Da troca" }),
    );
    iara.slot.end();

    assert!(refused.is_error);
    assert_eq!(refused.text, "O card item-1 não é seu nem do pedido atual.");
    assert!(!accepted.is_error, "{}", accepted.text);
    assert_eq!(world.cards(project)[0].title, "Da troca");
}

#[test]
fn id_inexistente_e_recusado_e_nada_e_gravado() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    world.call(&iara, "create_card", json!({ "title": "Um" }));

    let outcome = update(&world, &iara, json!({ "id": "item-99", "title": "Novo" }));

    assert!(outcome.is_error);
    assert!(outcome.text.contains("item-99"), "{}", outcome.text);
    assert_eq!(world.cards(project)[0].title, "Um");
}

#[test]
fn titulo_duplicado_e_recusado_mas_mudar_so_a_caixa_do_proprio_titulo_passa() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    world.call(&iara, "create_card", json!({ "title": "Um" }));
    world.call(&iara, "create_card", json!({ "title": "Dois" }));

    let duplicate = update(&world, &iara, json!({ "id": "item-2", "title": " um " }));
    let own_case = update(&world, &iara, json!({ "id": "item-2", "title": "DOIS" }));

    assert!(duplicate.is_error);
    assert!(duplicate.text.contains("item-1"), "{}", duplicate.text);
    assert!(!own_case.is_error, "{}", own_case.text);
    let titles: Vec<String> = world
        .cards(project)
        .into_iter()
        .map(|card| card.title)
        .collect();
    assert_eq!(titles, ["Um", "DOIS"]);
}

#[test]
fn card_em_ready_e_card_em_done_nao_sao_editados_pelo_agente() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    world.call(&iara, "create_card", json!({ "title": "Um" }));

    world
        .db()
        .move_backlog_item(project, "item-1", "Ready")
        .expect("o usuário move para Ready");
    let in_ready = update(&world, &iara, json!({ "id": "item-1", "title": "Novo" }));
    world
        .db()
        .move_backlog_item(project, "item-1", "Done")
        .expect("o usuário move para Done");
    let in_done = update(&world, &iara, json!({ "id": "item-1", "title": "Novo" }));

    assert!(in_ready.is_error);
    assert_eq!(
        in_ready.text,
        "O card item-1 está em Ready: só o usuário altera."
    );
    assert!(in_done.is_error);
    assert_eq!(in_done.text, "O card item-1 já está em Done.");
    assert_eq!(world.cards(project)[0].title, "Um");
}

#[test]
fn card_de_outro_projeto_com_o_mesmo_id_nao_e_tocado() {
    let world = world();
    let first = world.project("a");
    let second = world.project("b");
    let in_first = world.agent("iara", "Iara", first);
    let in_second = world.agent("iara", "Iara", second);
    world.call(&in_first, "create_card", json!({ "title": "Do A" }));
    world.call(&in_second, "create_card", json!({ "title": "Do B" }));

    update(
        &world,
        &in_second,
        json!({ "id": "item-1", "title": "Do B editado", "project_id": first }),
    );

    assert_eq!(world.cards(first)[0].title, "Do A");
    assert_eq!(world.cards(second)[0].title, "Do B editado");
}

#[test]
fn confirmacao_vai_ao_slot_e_o_evento_sai_so_quando_algo_mudou() {
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
    iara.slot.begin(None, None);

    let changed = update(&world, &iara, json!({ "id": "item-1", "title": "Dois" }));
    let same = update(&world, &iara, json!({ "id": "item-1", "title": "Dois" }));
    let refused = update(&world, &iara, json!({ "id": "item-1" }));
    std::thread::sleep(std::time::Duration::from_millis(300));

    assert_eq!(changed.text, "Card item-1 atualizado: Dois.");
    assert_eq!(same.text, "Card item-1 já está assim: nada mudou.");
    assert!(refused.is_error);
    assert_eq!(
        iara.slot.end().confirmations,
        ["Card item-1 atualizado: Dois."]
    );
    let seen = events.lock().unwrap().clone();
    let backlog = seen
        .iter()
        .filter(|event| event["type"] == json!("backlog-changed"))
        .count();
    assert_eq!(backlog, 2, "{seen:?}");
}

#[test]
fn update_card_aparece_em_tools_list_com_titulo_limitado_e_so_id_obrigatorio() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });

    let reply = run(send(
        world.port,
        &Request::post(world.port, &iara.token, body),
    ));
    let tools = reply.json()["result"]["tools"].clone();
    let tool = tools
        .as_array()
        .and_then(|tools| tools.iter().find(|tool| tool["name"] == "update_card"))
        .unwrap_or_else(|| panic!("sem update_card: {tools}"));

    assert_eq!(tool["inputSchema"]["required"], json!(["id"]));
    assert_eq!(
        tool["inputSchema"]["properties"]["title"]["maxLength"],
        json!(MAX_TITLE)
    );
    assert!(world
        .server
        .tool_names()
        .contains(&"mcp__vokia__update_card".to_string()));
}
