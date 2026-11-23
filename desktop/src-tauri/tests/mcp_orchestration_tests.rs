use serde_json::{json, Value};

use super::backlog_tests::world;
use super::tools::definitions;
use crate::db::HandoffDraft;

const GET_ORCHESTRATION_DESCRIPTION: &str =
    "Lê os times, as camadas e as regras de passagem do Vokia, com a posição de cada agente.";
const LIST_AGENTS_DESCRIPTION: &str = "Lista os agentes do elenco, com nome, papel e instruções.";
const SAVE_ITEM_PLAN_DESCRIPTION: &str = "Grava o plano de um card do backlog. state vazio limpa o plano; proposto exige proposal; vinculado exige plan com id (ID público curto), name (título bem resumido) e meta (estimativa para execução por LLM em minutos, ex: '~5min', '~15min').";
const LIST_MOVEMENTS_DESCRIPTION: &str =
    "Lista as passagens de trabalho recentes entre agentes do projeto.";
const SAVE_ITEM_PLAN_BAD_ARGUMENTS: &str =
    "Argumentos inválidos para save_item_plan: confira o schema da ferramenta.";

fn description_of(tool: &str) -> String {
    definitions()
        .iter()
        .find(|definition| definition["name"] == tool)
        .unwrap_or_else(|| panic!("a ferramenta {tool} deveria estar na lista"))["description"]
        .as_str()
        .expect("descrição")
        .to_string()
}

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|error| panic!("resposta não é JSON ({error}): {text}"))
}

#[test]
fn as_quatro_ferramentas_de_orquestracao_tem_descricao_em_portugues() {
    assert_eq!(description_of("get_orchestration"), GET_ORCHESTRATION_DESCRIPTION);
    assert_eq!(description_of("list_agents"), LIST_AGENTS_DESCRIPTION);
    assert_eq!(description_of("save_item_plan"), SAVE_ITEM_PLAN_DESCRIPTION);
    assert_eq!(description_of("list_movements"), LIST_MOVEMENTS_DESCRIPTION);
}

#[test]
fn get_orchestration_devolve_o_mapa_real_do_projeto_com_a_posicao_do_lider() {
    let world = world();
    let project = world.project("a");
    let axiom = world.leader(project);

    let outcome = world.call(&axiom, "get_orchestration", json!({}));

    assert!(!outcome.is_error, "{}", outcome.text);
    let snapshot = json_of(&outcome.text);
    let layers: Vec<&str> = snapshot["layers"]
        .as_array()
        .expect("camadas")
        .iter()
        .map(|layer| layer["id"].as_str().expect("id"))
        .collect();
    assert_eq!(layers, ["lideranca", "implementacao", "revisao"]);
    let leader = snapshot["placements"]
        .as_array()
        .expect("posições")
        .iter()
        .find(|placement| placement["agentId"] == "axiom")
        .expect("o líder deveria estar posicionado");
    assert_eq!(leader["layerId"], "lideranca");
    assert_eq!(snapshot["settings"]["enforcement"], "off");
}

#[test]
fn list_agents_devolve_o_elenco_real_com_nome_e_papel() {
    let world = world();
    let project = world.project("a");
    let axiom = world.leader(project);

    let outcome = world.call(&axiom, "list_agents", json!({}));

    assert!(!outcome.is_error, "{}", outcome.text);
    let listing = json_of(&outcome.text);
    let agents = listing["agents"].as_array().expect("agentes");
    let vexa = agents
        .iter()
        .find(|agent| agent["id"] == "vexa")
        .expect("Vexa deveria estar no elenco");
    assert_eq!(vexa["name"], "Vexa");
    assert_eq!(vexa["role"], "Frontend Developer");
    assert!(agents.iter().any(|agent| agent["id"] == "axiom"));
}

#[test]
fn save_item_plan_proposto_grava_o_plano_do_card_e_responde_com_o_id() {
    let world = world();
    let project = world.project("a");
    let axiom = world.leader(project);
    let card = world.create(&axiom, "Login");

    let outcome = world.call(
        &axiom,
        "save_item_plan",
        json!({ "item_id": card, "state": "proposto", "proposal": "Dividir em 2 lotes", "agent": "axiom" }),
    );

    assert!(!outcome.is_error, "{}", outcome.text);
    assert_eq!(outcome.text, format!("Plano do card {card} gravado."));
    let plans = world.db().list_backlog(project).expect("backlog").plans;
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].item_id, card);
    assert_eq!(plans[0].state, "proposto");
    assert_eq!(plans[0].proposal.as_deref(), Some("Dividir em 2 lotes"));
    assert_eq!(plans[0].agent.as_deref(), Some("axiom"));
}

#[test]
fn save_item_plan_aceita_agente_do_elenco_alem_de_axiom_e_vexa() {
    let world = world();
    let project = world.project("a");
    let axiom = world.leader(project);
    let card = world.create(&axiom, "Login");

    let outcome = world.call(
        &axiom,
        "save_item_plan",
        json!({ "item_id": card, "state": "proposto", "proposal": "Dividir em 2 lotes", "agent": "sentry" }),
    );

    assert!(!outcome.is_error, "{}", outcome.text);
    assert_eq!(outcome.text, format!("Plano do card {card} gravado."));
    let plans = world.db().list_backlog(project).expect("backlog").plans;
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].agent.as_deref(), Some("sentry"));
}

#[test]
fn save_item_plan_recusa_entrada_invalida_e_nao_grava_plano() {
    let world = world();
    let project = world.project("a");
    let axiom = world.leader(project);
    let card = world.create(&axiom, "Login");
    let invalid = [
        json!({ "item_id": card, "state": "inventado" }),
        json!({ "item_id": card, "state": "proposto" }),
        json!({ "item_id": card, "state": "proposto", "proposal": "   " }),
        json!({ "item_id": card, "state": "vinculado", "plan": { "id": "p1", "name": "Plano" } }),
        json!({ "item_id": card, "state": "proposto", "proposal": "x", "agent": "agente invalido" }),
        json!({ "item_id": "CARD-INEXISTENTE", "state": "proposto", "proposal": "x" }),
    ];

    for arguments in invalid {
        let outcome = world.call(&axiom, "save_item_plan", arguments.clone());
        assert!(outcome.is_error, "{arguments}: {}", outcome.text);
    }

    assert!(world.db().list_backlog(project).expect("backlog").plans.is_empty());
}

#[test]
fn save_item_plan_sem_campo_obrigatorio_volta_como_argumentos_invalidos() {
    let world = world();
    let project = world.project("a");
    let axiom = world.leader(project);

    let outcome = world.call(&axiom, "save_item_plan", json!({ "state": "proposto" }));

    assert!(outcome.is_error);
    assert_eq!(outcome.text, SAVE_ITEM_PLAN_BAD_ARGUMENTS);
}

#[test]
fn list_movements_devolve_as_passagens_gravadas_do_projeto_e_nao_as_de_outro() {
    let world = world();
    let project = world.project("a");
    let other = world.project("b");
    let axiom = world.leader(project);
    let handoff = |from: &str, to: &str| HandoffDraft {
        run_id: None,
        from_agent: from.to_string(),
        to_agent: to.to_string(),
        from_layer: Some("lideranca".to_string()),
        to_layer: Some("implementacao".to_string()),
        action: "delegar".to_string(),
        kind: "implementar".to_string(),
        item_id: None,
        return_count: 0,
        message_id: None,
        event: None,
        edge_id: None,
    };
    world.db().record_handoff(project, &handoff("axiom", "vexa")).expect("passagem");
    world.db().record_handoff(other, &handoff("axiom", "kairo")).expect("passagem de outro projeto");

    let outcome = world.call(&axiom, "list_movements", json!({}));

    assert!(!outcome.is_error, "{}", outcome.text);
    let moves = json_of(&outcome.text);
    let moves = moves.as_array().expect("lista");
    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0]["fromAgent"], "axiom");
    assert_eq!(moves[0]["toAgent"], "vexa");
    assert_eq!(moves[0]["fromLayer"], "lideranca");
    assert_eq!(moves[0]["action"], "delegar");
}
