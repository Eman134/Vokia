use serde_json::{json, Value};

use super::backlog_tests::world;
use super::support::{run, send, Request};
use super::*;
use crate::events::DecisionKind;

const REGISTERED: &str = "Decisão registrada. Termine a resposta agora.";
const ALREADY: &str = "Já há uma decisão registrada nesta resposta. Termine a resposta agora.";
const EMPTY_TITLE: &str = "O título da decisão não pode ser vazio.";
const CHOICE_WITHOUT_OPTIONS: &str = "Uma decisão do tipo escolha precisa de opções.";

fn decision(kind: &str, title: &str, options: Value) -> Value {
    json!({ "kind": kind, "title": title, "detail": "Contexto", "options": options })
}

#[test]
fn request_decision_nao_bloqueia_e_grava_a_decisao_no_slot() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);

    axiom.slot.begin(None, None);
    let outcome = world.call(
        &axiom,
        "request_decision",
        decision("escolha", " Qual caminho? ", json!(["A", " B ", ""])),
    );
    let collected = axiom.slot.end();

    assert!(!outcome.is_error, "{}", outcome.text);
    assert_eq!(outcome.text, REGISTERED);
    assert_eq!(
        collected.decision,
        Some(SlotDecision {
            kind: DecisionKind::Escolha,
            title: "Qual caminho?".to_string(),
            detail: "Contexto".to_string(),
            options: vec!["A".to_string(), "B".to_string()],
        })
    );
}

#[test]
fn segunda_decisao_na_mesma_resposta_e_erro_e_a_primeira_fica() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);

    axiom.slot.begin(None, None);
    world.call(
        &axiom,
        "request_decision",
        decision("pergunta", "Primeira", json!([])),
    );
    let second = world.call(
        &axiom,
        "request_decision",
        decision("pergunta", "Segunda", json!([])),
    );
    let collected = axiom.slot.end();

    assert!(second.is_error);
    assert_eq!(second.text, ALREADY);
    assert_eq!(
        collected.decision.expect("a primeira fica").title,
        "Primeira"
    );
}

#[test]
fn escolha_sem_opcoes_ou_so_com_opcoes_em_branco_e_recusada_e_nada_e_gravado() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);

    axiom.slot.begin(None, None);
    for options in [json!([]), json!(["  ", ""])] {
        let outcome = world.call(
            &axiom,
            "request_decision",
            decision("escolha", "Qual?", options),
        );
        assert!(outcome.is_error);
        assert_eq!(outcome.text, CHOICE_WITHOUT_OPTIONS);
    }
    let without_field = world.call(
        &axiom,
        "request_decision",
        json!({ "kind": "escolha", "title": "Qual?" }),
    );

    assert!(without_field.is_error);
    assert!(axiom.slot.end().decision.is_none());
}

#[test]
fn tipo_invalido_e_titulo_vazio_sao_recusados() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);

    axiom.slot.begin(None, None);
    let kind = world.call(
        &axiom,
        "request_decision",
        decision("talvez", "Qual?", json!([])),
    );
    let title = world.call(
        &axiom,
        "request_decision",
        decision("pergunta", "   ", json!([])),
    );

    assert!(kind.is_error);
    assert!(
        kind.text.contains("aprovacao, escolha, pergunta, revisao"),
        "{}",
        kind.text
    );
    assert!(title.is_error);
    assert_eq!(title.text, EMPTY_TITLE);
    assert!(axiom.slot.end().decision.is_none());
}

#[test]
fn decisao_de_uma_troca_nao_vaza_para_a_proxima_nem_para_outro_agente() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);
    let vexa = world.agent("vexa", "Vexa", project);

    axiom.slot.begin(None, None);
    world.call(
        &axiom,
        "request_decision",
        decision("pergunta", "Da Axiom", json!([])),
    );
    axiom.slot.begin(None, None);

    assert!(vexa.slot.end().decision.is_none());
    assert!(axiom.slot.end().decision.is_none());
}

#[test]
fn tools_list_traz_request_decision_com_os_quatro_tipos() {
    let world = world();
    let project = world.project("a");
    let axiom = world.agent("axiom", "Axiom", project);
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });

    let reply = run(send(
        world.port,
        &Request::post(world.port, &axiom.token, body),
    ));
    let tools = reply.json()["result"]["tools"].clone();
    let tool = tools
        .as_array()
        .and_then(|tools| tools.iter().find(|tool| tool["name"] == "request_decision"))
        .unwrap_or_else(|| panic!("sem request_decision: {tools}"));

    assert_eq!(
        tool["inputSchema"]["properties"]["kind"]["enum"],
        json!(["aprovacao", "escolha", "pergunta", "revisao"])
    );
    assert_eq!(tool["inputSchema"]["required"], json!(["kind", "title"]));
    assert!(world
        .server
        .tool_names(crate::mcp::ToolScope::Coordinator)
        .contains(&"mcp__vokia__request_decision".to_string()));
}

#[test]
fn absorb_junta_confirmacoes_e_fica_com_a_decisao_mais_recente() {
    let first = SlotDecision {
        kind: DecisionKind::Pergunta,
        title: "Antiga".to_string(),
        detail: String::new(),
        options: Vec::new(),
    };
    let second = SlotDecision {
        title: "Nova".to_string(),
        ..first.clone()
    };
    let mut collected = ExchangeOutcome {
        confirmations: vec!["a".to_string()],
        decision: Some(first.clone()),
        ..ExchangeOutcome::default()
    };

    collected.absorb(ExchangeOutcome {
        confirmations: vec!["b".to_string()],
        ..ExchangeOutcome::default()
    });
    assert_eq!(collected.confirmations, ["a", "b"]);
    assert_eq!(collected.decision, Some(first));

    collected.absorb(ExchangeOutcome {
        decision: Some(second.clone()),
        ..ExchangeOutcome::default()
    });
    assert_eq!(collected.decision, Some(second));
}
