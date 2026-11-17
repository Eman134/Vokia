use serde_json::{json, Value};
use tauri::Manager;

use super::backlog_tests::{world, World};
use super::support::{run, send, Request};
use super::*;
use crate::agents::Agent;
use crate::commands::backlog::create_request_from_proposal;
use crate::db::{CardOutcome, DbState};
use crate::events::RequestProposal;

const REGISTERED: &str = "Proposta registrada. Escreva a resposta normalmente.";
const ALREADY: &str = "Já há um pedido proposto nesta resposta.";
const EMPTY_TITLE: &str = "O título do pedido não pode ser vazio.";
const TITLE_TOO_LONG: &str = "O título do card passa de 200 caracteres.";
const MAX_TITLE: usize = 200;

fn proposal(title: &str, detail: Value) -> Value {
    json!({ "title": title, "detail": detail })
}

fn collected(title: &str, detail: &str) -> RequestProposal {
    RequestProposal {
        title: title.to_string(),
        detail: detail.to_string(),
    }
}

#[test]
fn propose_request_grava_a_proposta_no_slot_sem_criar_card() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);

    iara.slot.begin(None, None);
    let outcome = world.call(&iara, "propose_request", proposal(" Revisar login ", json!(" Detalhe ")));
    let slot = iara.slot.end();

    assert!(!outcome.is_error, "{}", outcome.text);
    assert_eq!(outcome.text, REGISTERED);
    assert_eq!(slot.proposal, Some(collected("Revisar login", "Detalhe")));
    assert!(world.cards(project).is_empty());
}

#[test]
fn detail_e_opcional() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);

    iara.slot.begin(None, None);
    let outcome = world.call(&iara, "propose_request", json!({ "title": "Sem detalhe" }));

    assert!(!outcome.is_error, "{}", outcome.text);
    assert_eq!(iara.slot.end().proposal, Some(collected("Sem detalhe", "")));
}

#[test]
fn segunda_chamada_na_mesma_resposta_e_erro_e_a_primeira_fica() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);

    iara.slot.begin(None, None);
    world.call(&iara, "propose_request", proposal("Primeira", json!("")));
    let second = world.call(&iara, "propose_request", proposal("Segunda", json!("")));
    let slot = iara.slot.end();

    assert!(second.is_error);
    assert_eq!(second.text, ALREADY);
    assert_eq!(slot.proposal.expect("a primeira fica").title, "Primeira");
}

#[test]
fn titulo_vazio_ou_acima_de_200_e_recusado_e_nada_e_gravado() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    let too_long = "x".repeat(MAX_TITLE + 1);

    iara.slot.begin(None, None);
    let empty = world.call(&iara, "propose_request", proposal("   ", json!("")));
    let long = world.call(&iara, "propose_request", proposal(&too_long, json!("")));
    let missing = world.call(&iara, "propose_request", json!({ "detail": "só detalhe" }));

    assert!(empty.is_error);
    assert_eq!(empty.text, EMPTY_TITLE);
    assert!(long.is_error);
    assert_eq!(long.text, TITLE_TOO_LONG);
    assert!(missing.is_error);
    assert!(iara.slot.end().proposal.is_none());
}

#[test]
fn titulo_com_exatamente_200_e_aceito() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);

    iara.slot.begin(None, None);
    let outcome = world.call(&iara, "propose_request", proposal(&"x".repeat(MAX_TITLE), json!("")));

    assert!(!outcome.is_error, "{}", outcome.text);
}

#[test]
fn proposta_de_uma_troca_nao_vaza_para_a_proxima_nem_para_outro_agente() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    let davi = world.agent("davi", "Davi", project);

    iara.slot.begin(None, None);
    world.call(&iara, "propose_request", proposal("Da Iara", json!("")));
    iara.slot.begin(None, None);

    assert!(davi.slot.end().proposal.is_none());
    assert!(iara.slot.end().proposal.is_none());
}

#[test]
fn tools_list_traz_propose_request_com_titulo_obrigatorio() {
    let world = world();
    let project = world.project("a");
    let iara = world.agent("iara", "Iara", project);
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });

    let reply = run(send(world.port, &Request::post(world.port, &iara.token, body)));
    let tools = reply.json()["result"]["tools"].clone();
    let tool = tools
        .as_array()
        .and_then(|tools| tools.iter().find(|tool| tool["name"] == "propose_request"))
        .unwrap_or_else(|| panic!("sem propose_request: {tools}"));

    assert_eq!(tool["inputSchema"]["required"], json!(["title"]));
    assert_eq!(tool["inputSchema"]["properties"]["title"]["maxLength"], json!(MAX_TITLE));
}

#[test]
fn absorb_mantem_a_proposta_de_uma_tentativa_anterior_e_prefere_a_mais_recente() {
    let mut outcome = ExchangeOutcome {
        proposal: Some(collected("Antiga", "")),
        ..ExchangeOutcome::default()
    };

    outcome.absorb(ExchangeOutcome::default());
    assert_eq!(outcome.proposal, Some(collected("Antiga", "")));

    outcome.absorb(ExchangeOutcome {
        proposal: Some(collected("Nova", "")),
        ..ExchangeOutcome::default()
    });
    assert_eq!(outcome.proposal, Some(collected("Nova", "")));
}

fn create_from(world: &World, project: i64, agent: &str, title: &str, detail: Option<&str>) -> Result<crate::db::CardResult, String> {
    create_request_from_proposal(
        world.app.handle().clone(),
        world.app.state::<DbState>(),
        project,
        Agent::from_stored(agent).expect("agente"),
        title.to_string(),
        detail.map(str::to_string),
    )
}

#[test]
fn comando_cria_card_de_origem_agente_no_backlog_com_a_descricao() {
    let world = world();
    let project = world.project("a");

    let card = create_from(&world, project, "iara", "Revisar login", Some("Detalhe")).expect("criação");

    assert_eq!(card.outcome, CardOutcome::Created);
    assert_eq!(card.column, "Backlog");
    let stored = world.cards(project).into_iter().find(|item| item.id == card.id).expect("card gravado");
    assert_eq!(stored.title, "Revisar login");
    assert_eq!(stored.origin, "agente");
}

#[test]
fn segundo_clique_com_o_mesmo_titulo_e_recusado_e_fica_um_card_so() {
    let world = world();
    let project = world.project("a");

    create_from(&world, project, "iara", "Revisar login", None).expect("primeiro");
    let second = create_from(&world, project, "iara", "Revisar login", None);

    assert!(second.is_err());
    assert_eq!(world.cards(project).len(), 1);
}

#[test]
fn comando_cria_no_projeto_recebido_e_recusa_agente_desconhecido() {
    let world = world();
    let first = world.project("a");
    let second = world.project("b");

    create_from(&world, second, "davi", "Do segundo", None).expect("criação");
    let unknown = create_from(&world, first, "fantasma", "Qualquer", None);

    assert!(world.cards(first).is_empty());
    assert_eq!(world.cards(second).len(), 1);
    assert!(unknown.is_err());
}
