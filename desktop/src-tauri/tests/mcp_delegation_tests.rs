use serde_json::json;

use super::backlog_tests::world;
use super::*;
use crate::agents::Agent;

const REGISTERED: &str = "Delegação registrada.";
const DUPLICATE: &str =
    "Delegação repetida ignorada: esse agente já recebeu este tipo de pedido neste item nesta resposta.";
const LIMIT: &str = "Limite de delegações por resposta atingido.";
const EMPTY_REQUEST: &str = "O pedido da delegação não pode ser vazio.";
const TO_SELF: &str = "Você não pode delegar a si mesmo.";
const MAX_PER_REPLY: usize = 5;

fn agent(id: &str) -> Agent {
    Agent::from_stored(id).expect("agente")
}

fn requests(collected: &ExchangeOutcome) -> Vec<&str> {
    collected
        .delegations
        .iter()
        .map(|delegation| delegation.request.as_str())
        .collect()
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
    assert!(outcome.text.starts_with(REGISTERED), "{}", outcome.text);
    assert_eq!(collected.delegations.len(), 1);
    assert_eq!(collected.delegations[0].agent, agent("vexa"));
    assert_eq!(collected.delegations[0].request, "Revise o login");
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
    assert!(axiom.slot.end().delegations.is_empty());
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
    assert!(axiom.slot.end().delegations.is_empty());
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

    assert!(axiom.slot.end().delegations.is_empty());
}

#[test]
fn delegacao_igual_na_mesma_resposta_e_ignorada_com_aviso_e_a_primeira_fica() {
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

    assert!(!second.is_error, "{}", second.text);
    assert_eq!(second.text, DUPLICATE);
    assert_eq!(requests(&collected), vec!["Primeiro"]);
}

#[test]
fn varias_delegacoes_na_mesma_resposta_ficam_registradas_com_contexto() {
    let world = world();
    let project = world.project("a");
    let axiom = world.leader(project);
    let first_card = world.create(&axiom, "Lote 1");
    let second_card = world.create(&axiom, "Lote 2");

    axiom.slot.begin(None, None);
    world.call(
        &axiom,
        "delegate",
        json!({ "agent": "Ember", "request": "Começamos o item", "notify": true }),
    );
    world.call(
        &axiom,
        "delegate",
        json!({ "agent": "Vexa", "request": "Lote 1", "item_id": first_card }),
    );
    let last = world.call(
        &axiom,
        "delegate",
        json!({ "agent": "Vexa", "request": "Lote 2", "item_id": second_card }),
    );
    let collected = axiom.slot.end();

    assert!(!last.is_error, "{}", last.text);
    assert!(
        last.text.contains("Ember · geral · sem item · aviso enviado"),
        "{}",
        last.text
    );
    assert!(
        last.text.contains(&format!("Vexa · geral · {first_card} · pendente")),
        "{}",
        last.text
    );
    assert!(
        last.text.contains(&format!("Vexa · geral · {second_card} · pendente")),
        "{}",
        last.text
    );
    assert_eq!(collected.delegations.len(), 3);
    assert!(collected.delegations[0].intent.notify);
    assert_eq!(
        requests(&collected),
        vec!["Começamos o item", "Lote 1", "Lote 2"]
    );
}

#[test]
fn aviso_nao_ocupa_vaga_e_o_teto_recusa_a_delegacao_alem_de_cinco() {
    let world = world();
    let project = world.project("a");
    let axiom = world.leader(project);
    let cards: Vec<String> = (1..=MAX_PER_REPLY + 1)
        .map(|n| world.create(&axiom, &format!("Card {n}")))
        .collect();

    axiom.slot.begin(None, None);
    world.call(
        &axiom,
        "delegate",
        json!({ "agent": "Ember", "request": "Aviso", "notify": true }),
    );
    for card in &cards[..MAX_PER_REPLY] {
        let outcome = world.call(
            &axiom,
            "delegate",
            json!({ "agent": "Vexa", "request": "Faça", "item_id": card }),
        );
        assert!(!outcome.is_error, "{}", outcome.text);
    }
    let refused = world.call(
        &axiom,
        "delegate",
        json!({ "agent": "Vexa", "request": "Faça", "item_id": cards[MAX_PER_REPLY] }),
    );
    let collected = axiom.slot.end();

    assert!(refused.is_error);
    assert_eq!(refused.text, LIMIT);
    assert_eq!(collected.delegations.len(), 1 + MAX_PER_REPLY);
}

#[test]
fn absorb_fica_com_as_delegacoes_mais_recentes() {
    let mut collected = ExchangeOutcome {
        delegations: vec![Delegation {
            agent: agent("vexa"),
            request: "Antiga".to_string(),
            intent: DelegationIntent::default(),
        }],
        ..ExchangeOutcome::default()
    };

    collected.absorb(ExchangeOutcome::default());
    assert_eq!(requests(&collected), vec!["Antiga"]);

    collected.absorb(ExchangeOutcome {
        delegations: vec![Delegation {
            agent: agent("vexa"),
            request: "Nova".to_string(),
            intent: DelegationIntent::default(),
        }],
        ..ExchangeOutcome::default()
    });
    assert_eq!(requests(&collected), vec!["Nova"]);
}

mod map_rules {
    use super::super::backlog_tests::World;
    use super::*;
    use crate::db::{LayerDraft, OrchestrationSettings, PlacementDraft, TeamDraft};

    const SEQUENTIAL: &str = "sincrona";
    const EVENT_NOT_ALLOWED: &str = "O evento testes_falharam não é permitido de Axiom para Vexa.";

    fn layer(world: &World, name: &str, kind: &str) -> String {
        world
            .db()
            .save_layer(&LayerDraft {
                id: None,
                name: name.to_string(),
                description: String::new(),
                kind: kind.to_string(),
                instructions: String::new(),
            })
            .expect("camada")
            .id
    }

    fn place(world: &World, team: &str, agent: &str, layer: &str) {
        world
            .db()
            .place_agent(&PlacementDraft {
                agent_id: agent.to_string(),
                team_id: team.to_string(),
                layer_id: layer.to_string(),
                instructions: String::new(),
                max_instances: 1,
                pos_x: None,
                pos_y: None,
                criteria: String::new(),
                max_worktrees: None,
            })
            .expect("posição");
    }

    // Axiom → Vexa é a única ligação, e só no evento plano_aprovado; Sentry não tem ligação de entrada.
    fn enforced_map(world: &World, sync_mode: &str) {
        let db = world.db();
        db.save_settings(&OrchestrationSettings {
            enforcement: "on".to_string(),
            ..OrchestrationSettings::default()
        })
        .expect("configuração");
        let team = db
            .save_team(&TeamDraft {
                id: None,
                name: "Time".to_string(),
                instructions: None,
            })
            .expect("time")
            .id;
        let lead = layer(world, "Liderança", "planejar");
        let build = layer(world, "Implementação", "implementar");
        let review = layer(world, "Revisão", "revisar");
        place(world, &team, "axiom", &lead);
        place(world, &team, "vexa", &build);
        place(world, &team, "sentry", &review);
        db.conn()
            .expect("conexão")
            .execute(
                "INSERT INTO orch_flow_edges (team_id, from_node, to_node, sync_mode, when_event, action, kind, position) \
                 VALUES (?1, 'axiom', 'vexa', ?2, 'plano_aprovado', 'delegar', 'implementar', 0)",
                rusqlite::params![team, sync_mode],
            )
            .expect("ligação");
    }

    #[test]
    fn destino_sem_regra_a_partir_da_camada_e_recusado_e_nada_e_gravado() {
        let world = world();
        let project = world.project("a");
        enforced_map(&world, SEQUENTIAL);
        let axiom = world.leader(project);

        axiom.slot.begin(None, None);
        let outcome = world.call(
            &axiom,
            "delegate",
            json!({ "agent": "Sentry", "request": "Revise" }),
        );
        let collected = axiom.slot.end();

        assert!(outcome.is_error);
        assert_eq!(
            outcome.text,
            "Sentry não é um destino permitido para Axiom. Destinos permitidos: Vexa."
        );
        assert!(collected.delegations.is_empty());
    }

    #[test]
    fn evento_fora_do_par_e_recusado_e_nada_e_gravado() {
        let world = world();
        let project = world.project("a");
        enforced_map(&world, SEQUENTIAL);
        let axiom = world.leader(project);

        axiom.slot.begin(None, None);
        let outcome = world.call(
            &axiom,
            "delegate",
            json!({ "agent": "Vexa", "request": "Corrija", "event": "testes_falharam" }),
        );
        let collected = axiom.slot.end();

        assert!(outcome.is_error);
        assert_eq!(outcome.text, EVENT_NOT_ALLOWED);
        assert!(collected.delegations.is_empty());
    }

    #[test]
    fn evento_do_par_e_aceito_e_a_delegacao_e_registrada() {
        let world = world();
        let project = world.project("a");
        enforced_map(&world, SEQUENTIAL);
        let axiom = world.leader(project);

        axiom.slot.begin(None, None);
        let outcome = world.call(
            &axiom,
            "delegate",
            json!({ "agent": "Vexa", "request": "Implemente", "event": "plano_aprovado" }),
        );
        let collected = axiom.slot.end();

        assert!(!outcome.is_error, "{}", outcome.text);
        assert_eq!(collected.delegations.len(), 1);
        assert_eq!(collected.delegations[0].agent, agent("vexa"));
        assert_eq!(collected.delegations[0].request, "Implemente");
    }

    #[test]
    fn tipo_diferente_do_da_seta_e_recusado_e_nada_e_gravado() {
        let world = world();
        let project = world.project("a");
        enforced_map(&world, SEQUENTIAL);
        let axiom = world.leader(project);

        axiom.slot.begin(None, None);
        let outcome = world.call(
            &axiom,
            "delegate",
            json!({ "agent": "Vexa", "request": "Teste", "kind": "testar" }),
        );
        let collected = axiom.slot.end();

        assert!(outcome.is_error);
        assert_eq!(
            outcome.text,
            "O tipo testar não é o da seta de Axiom para Vexa: use implementar."
        );
        assert!(collected.delegations.is_empty());
    }

    #[test]
    fn segunda_delegacao_ao_mesmo_par_e_ignorada_e_a_primeira_fica() {
        let world = world();
        let project = world.project("a");
        enforced_map(&world, SEQUENTIAL);
        let axiom = world.leader(project);

        axiom.slot.begin(None, None);
        world.call(
            &axiom,
            "delegate",
            json!({ "agent": "Vexa", "request": "Um" }),
        );
        let second = world.call(
            &axiom,
            "delegate",
            json!({ "agent": "Vexa", "request": "Dois" }),
        );
        let collected = axiom.slot.end();

        assert!(!second.is_error, "{}", second.text);
        assert_eq!(second.text, DUPLICATE);
        assert_eq!(requests(&collected), vec!["Um"]);
    }

    #[test]
    fn item_de_outro_projeto_e_recusado_e_o_do_proprio_projeto_e_aceito() {
        let world = world();
        let mine = world.project("minha");
        let other = world.project("outra");
        enforced_map(&world, SEQUENTIAL);
        // Ids são sequenciais por projeto: o segundo card de "outra" não existe em "minha", que só terá um.
        let other_leader = world.leader(other);
        world.create(&other_leader, "Primeiro alheio");
        let foreign = world.create(&other_leader, "Card alheio");
        let own = world.create(&world.leader(mine), "Card meu");
        let axiom = world.leader(mine);

        axiom.slot.begin(None, None);
        let refused = world.call(
            &axiom,
            "delegate",
            json!({ "agent": "Vexa", "request": "Faça", "item_id": foreign }),
        );
        let after_refusal = axiom.slot.end();
        axiom.slot.begin(None, None);
        let accepted = world.call(
            &axiom,
            "delegate",
            json!({ "agent": "Vexa", "request": "Faça", "item_id": own }),
        );
        let collected = axiom.slot.end();

        assert!(refused.is_error);
        assert_eq!(
            refused.text,
            format!("O item {foreign} não existe no backlog deste projeto.")
        );
        assert!(after_refusal.delegations.is_empty());
        assert!(!accepted.is_error, "{}", accepted.text);
        assert_eq!(requests(&collected), vec!["Faça"]);
    }
}

#[test]
fn delegate_com_item_id_recusa_agente_sem_suporte_a_mcp_conforme_d2() {
    use crate::agents::{AgentDraft, AgentRegistry};
    use tauri::Manager;

    let world = world();
    let project = world.project("a");
    let axiom = world.leader(project);
    let card = world.create(&axiom, "Login");

    let registry = world.app.state::<AgentRegistry>();
    let providers = crate::providers::ProviderRegistry::seeded();

    for (cli, model, cmd) in [
        ("codex", "gpt-6-luna", "codex"),
        ("opencode", "openai/gpt-6-astra", "opencode"),
        ("antigravity", "gemini-4-hydrogen-medium", "agy"),
    ] {
        let draft = AgentDraft {
            name: "Vexa".to_string(),
            role: "Frontend Developer".to_string(),
            avatar: "lume".to_string(),
            cli: cli.to_string(),
            model: model.to_string(),
            model_tier: None,
            reasoning_effort: None,
            command: cmd.to_string(),
            follow_default_llm: false,
            fallback: Vec::new(),
            ..Default::default()
        };
        registry
            .update(&agent("vexa"), &draft, &providers)
            .expect("update vexa");

        // 1. Com item_id (delegação de card) -> MCP-1: provedores com MCP nativo ou Vokia CLI aceitam delegação de card
        axiom.slot.begin(None, None);
        let outcome_card = world.call(
            &axiom,
            "delegate",
            json!({ "agent": "Vexa", "request": "Faça o card", "item_id": card }),
        );
        let after = axiom.slot.end();
        assert!(!outcome_card.is_error, "deveria aceitar delegação de card para {cli}: {}", outcome_card.text);
        assert!(outcome_card.text.starts_with(REGISTERED), "{}", outcome_card.text);
        assert_eq!(requests(&after), vec!["Faça o card"]);

        // 2. Sem item_id (conversa pura de texto) -> DEVE ACEITAR
        axiom.slot.begin(None, None);
        let outcome_text = world.call(
            &axiom,
            "delegate",
            json!({ "agent": "Vexa", "request": "Responda em texto" }),
        );
        let after_text = axiom.slot.end();
        assert!(!outcome_text.is_error, "deveria aceitar texto para {cli}: {}", outcome_text.text);
        assert!(outcome_text.text.starts_with(REGISTERED), "{}", outcome_text.text);
        assert_eq!(requests(&after_text), vec!["Responda em texto"]);
    }
}

#[test]
fn delegate_leva_ping_seconds_ao_slot_e_o_schema_publica_os_limites() {
    let world = world();
    let project = world.project("a");
    let axiom = world.leader(project);

    axiom.slot.begin(None, None);
    let outcome = world.call(&axiom, "delegate", json!({ "agent": "vexa", "request": "Revise", "ping_seconds": 45 }));
    let with_ping = axiom.slot.end();
    axiom.slot.begin(None, None);
    world.call(&axiom, "delegate", json!({ "agent": "vexa", "request": "Revise" }));
    let without_ping = axiom.slot.end();

    assert!(!outcome.is_error, "{}", outcome.text);
    assert_eq!(with_ping.delegations[0].intent.ping_seconds, Some(45));
    assert_eq!(without_ping.delegations[0].intent.ping_seconds, None);

    let definitions = crate::mcp::tools::definitions();
    let delegate = definitions.iter().find(|tool| tool["name"] == "delegate").expect("tool delegate");
    let ping = &delegate["inputSchema"]["properties"]["ping_seconds"];
    assert_eq!(ping["minimum"], 5);
    assert_eq!(ping["maximum"], 300);
    assert_eq!(delegate["inputSchema"]["properties"]["notify"]["type"], "boolean");
}
