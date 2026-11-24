use super::*;
use crate::db::{FlowEdge, OrchestrationSettings};

fn edge(from: &str, to: &str) -> FlowEdge {
    FlowEdge {
        id: 7,
        team_id: "time".to_string(),
        from_node: from.to_string(),
        to_node: to.to_string(),
        link: String::new(),
        sync_mode: String::new(),
        when_event: TASK_DONE_EVENT.to_string(),
        is_default: false,
        action: String::new(),
        kind: String::new(),
        turn_limit: None,
        curve: 0.0,
        instructions: String::new(),
        position: 0,
    }
}

fn snapshot(edges: Vec<FlowEdge>) -> OrchestrationSnapshot {
    OrchestrationSnapshot {
        settings: OrchestrationSettings {
            enforcement: String::new(),
            leader_agent: "axiom".to_string(),
            step_mode: String::new(),
            human_gate: String::new(),
            max_parallel: 1,
            pair_turn_limit: 1,
            task_turn_limit: 1,
            worktree_policy: String::new(),
        },
        layers: Vec::new(),
        teams: Vec::new(),
        placements: Vec::new(),
        edges,
        gateways: Vec::new(),
        task_stages: Vec::new(),
    }
}

#[test]
fn passagem_sem_aresta_avanca_sem_id_de_aresta() {
    assert_eq!(handoff_edge(&snapshot(Vec::new()), "time", "kairo", "vexa"), None);
}

#[test]
fn passagem_com_aresta_devolve_o_id_dela() {
    let snapshot = snapshot(vec![edge("kairo", "vexa")]);
    assert_eq!(handoff_edge(&snapshot, "time", "kairo", "vexa"), Some(7));
}

#[test]
fn mesmo_agente_em_etapas_seguidas_nao_usa_aresta() {
    let snapshot = snapshot(vec![edge("kairo", "kairo")]);
    assert_eq!(handoff_edge(&snapshot, "time", "kairo", "kairo"), None);
}
