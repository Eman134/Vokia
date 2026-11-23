use super::*;
use crate::db::{Layer, OrchestrationSettings, Placement};

const LEADER_LAYER: &str = "lideranca";
const IMPLEMENT_LAYER: &str = "implementacao";
const REVIEW_LAYER: &str = "revisao";
const CARD: Option<&str> = Some("card-1");
const NO_CARD: Option<&str> = None;

fn layer(id: &str, kind: &str) -> Layer {
    Layer {
        id: id.to_string(),
        name: id.to_string(),
        description: String::new(),
        position: 0,
        kind: kind.to_string(),
        instructions: String::new(),
    }
}

fn placement(agent: &str, layer: &str, max_instances: i64) -> Placement {
    Placement {
        agent_id: agent.to_string(),
        team_id: "time".to_string(),
        layer_id: layer.to_string(),
        instructions: String::new(),
        max_instances,
        pos_x: 0.0,
        pos_y: 0.0,
        criteria: String::new(),
        max_worktrees: None,
    }
}

// Líder e revisor sempre existem e nunca contam como implementadores.
fn snapshot_with(implementer_instances: &[i64]) -> OrchestrationSnapshot {
    let mut placements = vec![
        placement("axiom", LEADER_LAYER, 4),
        placement("sentry", REVIEW_LAYER, 4),
    ];
    placements.extend(
        implementer_instances
            .iter()
            .enumerate()
            .map(|(index, max)| placement(&format!("dev{index}"), IMPLEMENT_LAYER, *max)),
    );
    OrchestrationSnapshot {
        settings: OrchestrationSettings::default(),
        layers: vec![
            layer(LEADER_LAYER, "planejar"),
            layer(IMPLEMENT_LAYER, "implementar"),
            layer(REVIEW_LAYER, "revisar"),
        ],
        teams: Vec::new(),
        placements,
        edges: Vec::new(),
        gateways: Vec::new(),
        task_stages: Vec::new(),
    }
}

#[test]
fn sempre_usa_worktree_em_pedido_com_card_com_qualquer_numero_de_implementadores() {
    for implementers in [&[][..], &[1], &[1, 1]] {
        assert!(
            should_use("sempre", &snapshot_with(implementers), CARD),
            "{implementers:?}"
        );
    }
}

#[test]
fn paralelo_so_usa_worktree_a_partir_de_dois_implementadores() {
    let expected = [(&[][..], false), (&[1], false), (&[1, 1], true)];
    for (implementers, want) in expected {
        assert_eq!(
            should_use("paralelo", &snapshot_with(implementers), CARD),
            want,
            "{implementers:?}"
        );
    }
}

#[test]
fn paralelo_conta_instancias_de_um_unico_implementador() {
    assert!(should_use("paralelo", &snapshot_with(&[2]), CARD));
}

#[test]
fn paralelo_ignora_instancias_de_camadas_que_nao_implementam() {
    assert!(!should_use("paralelo", &snapshot_with(&[1]), CARD));
}

#[test]
fn nunca_nao_usa_worktree_mesmo_com_card_e_varios_implementadores() {
    for implementers in [&[][..], &[1], &[1, 1]] {
        assert!(
            !should_use("nunca", &snapshot_with(implementers), CARD),
            "{implementers:?}"
        );
    }
}

#[test]
fn pedido_sem_card_nunca_usa_worktree_em_nenhuma_politica() {
    for policy in ["sempre", "paralelo", "nunca"] {
        for implementers in [&[][..], &[1], &[1, 1]] {
            assert!(
                !should_use(policy, &snapshot_with(implementers), NO_CARD),
                "{policy} {implementers:?}"
            );
        }
    }
}
