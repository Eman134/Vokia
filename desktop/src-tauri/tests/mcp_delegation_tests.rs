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

mod map_rules {
    use super::super::backlog_tests::World;
    use super::*;
    use crate::db::{
        LayerDraft, OrchestrationSettings, PlacementDraft, RuleDraft, TeamDraft,
    };

    const SEQUENTIAL: &str = "sequencia";
    const PARALLEL: &str = "paralelo";
    const EVENT_NOT_ALLOWED: &str = "O evento testes_falharam não é permitido de Axiom para Vexa.";

    fn layer(world: &World, project: i64, name: &str, kind: &str) -> String {
        world
            .db()
            .save_layer(
                project,
                &LayerDraft {
                    id: None,
                    name: name.to_string(),
                    description: String::new(),
                    kind: kind.to_string(),
                    instructions: String::new(),
                },
            )
            .expect("camada")
            .id
    }

    fn place(world: &World, project: i64, team: &str, agent: &str, layer: &str) {
        world
            .db()
            .place_agent(
                project,
                &PlacementDraft {
                    agent_id: agent.to_string(),
                    team_id: team.to_string(),
                    layer_id: layer.to_string(),
                    instructions: String::new(),
                    max_instances: 1,
                },
            )
            .expect("posição");
    }

    // Axiom (planejar) → Vexa (implementar) é a única seta, e só no evento plano_aprovado; Sentry (revisar) não tem seta de entrada.
    fn enforced_map(world: &World, project: i64, exec_mode: &str) {
        let db = world.db();
        db.save_settings(
            project,
            &OrchestrationSettings {
                enforcement: "on".to_string(),
                ..OrchestrationSettings::default()
            },
        )
        .expect("configuração");
        let team = db
            .save_team(
                project,
                &TeamDraft {
                    id: None,
                    name: "Time".to_string(),
                },
            )
            .expect("time")
            .id;
        let lead = layer(world, project, "Liderança", "planejar");
        let build = layer(world, project, "Implementação", "implementar");
        let review = layer(world, project, "Revisão", "revisar");
        place(world, project, &team, "axiom", &lead);
        place(world, project, &team, "vexa", &build);
        place(world, project, &team, "sentry", &review);
        db.save_rule(
            project,
            &RuleDraft {
                id: None,
                from_layer: lead,
                to_layer: build,
                when_event: "plano_aprovado".to_string(),
                action: "delegar".to_string(),
                kind: "implementar".to_string(),
                exec_mode: exec_mode.to_string(),
                return_limit: 1,
            },
        )
        .expect("regra");
    }

    #[test]
    fn destino_sem_regra_a_partir_da_camada_e_recusado_e_nada_e_gravado() {
        let world = world();
        let project = world.project("a");
        enforced_map(&world, project, SEQUENTIAL);
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
            "Sentry não é um destino permitido para a camada Liderança. Destinos permitidos: Vexa."
        );
        assert!(collected.delegation.is_none());
    }

    #[test]
    fn evento_fora_do_par_e_recusado_e_nada_e_gravado() {
        let world = world();
        let project = world.project("a");
        enforced_map(&world, project, SEQUENTIAL);
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
        assert!(collected.delegation.is_none());
    }

    #[test]
    fn evento_do_par_e_aceito_e_a_delegacao_e_registrada() {
        let world = world();
        let project = world.project("a");
        enforced_map(&world, project, SEQUENTIAL);
        let axiom = world.leader(project);

        axiom.slot.begin(None, None);
        let outcome = world.call(
            &axiom,
            "delegate",
            json!({ "agent": "Vexa", "request": "Implemente", "event": "plano_aprovado" }),
        );
        let collected = axiom.slot.end();

        assert!(!outcome.is_error, "{}", outcome.text);
        assert_eq!(
            collected.delegation,
            Some((agent("vexa"), "Implemente".to_string()))
        );
    }

    #[test]
    fn tipo_diferente_do_da_seta_e_recusado_e_nada_e_gravado() {
        let world = world();
        let project = world.project("a");
        enforced_map(&world, project, SEQUENTIAL);
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
        assert!(collected.delegation.is_none());
    }

    #[test]
    fn segunda_delegacao_ao_mesmo_par_sem_execucao_paralela_e_recusada() {
        let world = world();
        let project = world.project("a");
        enforced_map(&world, project, SEQUENTIAL);
        let axiom = world.leader(project);

        axiom.slot.begin(None, None);
        world.call(&axiom, "delegate", json!({ "agent": "Vexa", "request": "Um" }));
        let second = world.call(&axiom, "delegate", json!({ "agent": "Vexa", "request": "Dois" }));
        let collected = axiom.slot.end();

        assert!(second.is_error);
        assert_eq!(second.text, ALREADY);
        assert_eq!(request_of(&collected.delegation), Some("Um"));
        assert!(collected.parallel_delegations.is_empty());
    }

    #[test]
    fn par_com_execucao_paralela_aceita_varias_delegacoes_na_mesma_resposta() {
        let world = world();
        let project = world.project("a");
        enforced_map(&world, project, PARALLEL);
        let axiom = world.leader(project);

        axiom.slot.begin(None, None);
        let first = world.call(&axiom, "delegate", json!({ "agent": "Vexa", "request": "Um" }));
        let second = world.call(&axiom, "delegate", json!({ "agent": "Vexa", "request": "Dois" }));
        let collected = axiom.slot.end();

        assert!(!first.is_error && !second.is_error, "{} {}", first.text, second.text);
        assert_eq!(
            collected.parallel_delegations,
            vec![
                (agent("vexa"), "Um".to_string()),
                (agent("vexa"), "Dois".to_string())
            ]
        );
        assert!(collected.delegation.is_none());
    }

    #[test]
    fn item_de_outro_projeto_e_recusado_e_o_do_proprio_projeto_e_aceito() {
        let world = world();
        let mine = world.project("minha");
        let other = world.project("outra");
        enforced_map(&world, mine, SEQUENTIAL);
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
        assert!(after_refusal.delegation.is_none());
        assert!(!accepted.is_error, "{}", accepted.text);
        assert_eq!(request_of(&collected.delegation), Some("Faça"));
    }
}
