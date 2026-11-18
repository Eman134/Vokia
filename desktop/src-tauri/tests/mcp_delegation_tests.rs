use serde_json::json;

use super::backlog_tests::world;
use super::*;
use crate::agents::Agent;

const REGISTERED: &str = "Delegação registrada. Termine a resposta agora.";
const ALREADY: &str = "Já há uma delegação registrada nesta resposta. Termine a resposta agora.";
const EMPTY_REQUEST: &str = "O pedido da delegação não pode ser vazio.";
const TO_SELF: &str = "Você não pode delegar a si mesmo.";

fn agent(id: &str) -> Agent {
    Agent::from_stored(id).expect("agente")
}

fn request_of(delegation: &Option<(Agent, String)>) -> Option<&str> {
    delegation.as_ref().map(|(_, request)| request.as_str())
}

#[test]
fn delegate_registra_agente_e_pedido_no_slot_sem_bloquear() {
    let world = world();
    let project = world.project("a");
    let axiom = world.leader(project);

    axiom.slot.begin(None, None);
    let outcome = world.call(
        &axiom,
        "delegate",
        json!({ "agent": " vexa ", "request": " Revise o login " }),
    );
    let collected = axiom.slot.end();

    assert!(!outcome.is_error, "{}", outcome.text);
    assert_eq!(outcome.text, REGISTERED);
    assert_eq!(
        collected.delegation,
        Some((agent("vexa"), "Revise o login".to_string()))
    );
}

#[test]
fn delegar_a_si_mesmo_e_recusado_e_nada_e_gravado() {
    let world = world();
    let project = world.project("a");
    let axiom = world.leader(project);

    axiom.slot.begin(None, None);
    let outcome = world.call(
        &axiom,
        "delegate",
        json!({ "agent": world.leader_name(), "request": "Faça isso" }),
    );

    assert!(outcome.is_error);
    assert_eq!(outcome.text, TO_SELF);
    assert!(axiom.slot.end().delegation.is_none());
}

#[test]
fn agente_inexistente_e_recusado_e_a_frase_lista_os_colegas_sem_o_proprio_agente() {
    let world = world();
    let project = world.project("a");
    let axiom = world.leader(project);

    axiom.slot.begin(None, None);
    let outcome = world.call(
        &axiom,
        "delegate",
        json!({ "agent": "Fantasma", "request": "Faça isso" }),
    );

    assert!(outcome.is_error);
    assert!(
        outcome
            .text
            .starts_with("Não existe agente chamado Fantasma. Colegas: "),
        "{}",
        outcome.text
    );
    assert!(outcome.text.contains("Vexa"), "{}", outcome.text);
    assert!(
        !outcome.text.contains(&world.leader_name()),
        "{}",
        outcome.text
    );
    assert!(axiom.slot.end().delegation.is_none());
}

#[test]
fn pedido_vazio_ou_em_branco_e_recusado() {
    let world = world();
    let project = world.project("a");
    let axiom = world.leader(project);

    axiom.slot.begin(None, None);
    for request in ["", "   "] {
        let outcome = world.call(
            &axiom,
            "delegate",
            json!({ "agent": "Vexa", "request": request }),
        );
        assert!(outcome.is_error);
        assert_eq!(outcome.text, EMPTY_REQUEST);
    }

    assert!(axiom.slot.end().delegation.is_none());
}

#[test]
fn segunda_delegacao_na_mesma_resposta_e_recusada_e_a_primeira_fica() {
    let world = world();
    let project = world.project("a");
    let axiom = world.leader(project);

    axiom.slot.begin(None, None);
    world.call(
        &axiom,
        "delegate",
        json!({ "agent": "Vexa", "request": "Primeiro" }),
    );
    let second = world.call(
        &axiom,
        "delegate",
        json!({ "agent": "Vexa", "request": "Segundo" }),
    );
    let collected = axiom.slot.end();

    assert!(second.is_error);
    assert_eq!(second.text, ALREADY);
    assert_eq!(request_of(&collected.delegation), Some("Primeiro"));
}

#[test]
fn delegacao_de_uma_troca_nao_vaza_para_a_proxima_nem_para_outro_agente() {
    let world = world();
    let project = world.project("a");
    let axiom = world.leader(project);
    let vexa = world.agent("vexa", "Vexa", project);

    axiom.slot.begin(None, None);
    world.call(
        &axiom,
        "delegate",
        json!({ "agent": "Vexa", "request": "Da Axiom" }),
    );
    axiom.slot.begin(None, None);

    assert!(vexa.slot.end().delegation.is_none());
    assert!(axiom.slot.end().delegation.is_none());
}

#[test]
fn argumentos_fora_do_schema_voltam_como_is_error() {
    let world = world();
    let project = world.project("a");
    let axiom = world.leader(project);

    let outcome = world.call(&axiom, "delegate", json!({ "agent": "Vexa" }));

    assert!(outcome.is_error);
    assert!(outcome.text.contains("delegate"), "{}", outcome.text);
}

#[test]
fn absorb_fica_com_a_delegacao_mais_recente() {
    let mut collected = ExchangeOutcome {
        delegation: Some((agent("vexa"), "Antiga".to_string())),
        ..ExchangeOutcome::default()
    };

    collected.absorb(ExchangeOutcome::default());
    assert_eq!(request_of(&collected.delegation), Some("Antiga"));

    collected.absorb(ExchangeOutcome {
        delegation: Some((agent("vexa"), "Nova".to_string())),
        ..ExchangeOutcome::default()
    });
    assert_eq!(request_of(&collected.delegation), Some("Nova"));
}
