use std::collections::{HashMap, HashSet};

use crate::db::{
    FlowEdgeDraft, FlowProblem, TeamFlowDraft, FLOW_END, FLOW_START, GATEWAY_EXCLUSIVE,
    GATEWAY_PARALLEL, LINK_COMMUNICATION, LINK_FLOW,
};

// Metade da altura do nó na tela: dois agentes mais próximos que isso na vertical estão na mesma linha.
const SAME_LEVEL_TOLERANCE: f64 = 38.0;
const GATEWAY_MIN_BRANCHES: usize = 2;

const START_NEEDS_ONE_EXIT: &str = "O início precisa de exatamente uma ligação de saída.";
const END_NEEDS_ENTRY: &str = "O fim precisa de ao menos uma ligação de entrada.";
const COMMUNICATION_SAME_ROW: &str = "A comunicação liga só agentes da mesma linha.";

struct Graph<'a> {
    flow: Vec<&'a FlowEdgeDraft>,
}

impl<'a> Graph<'a> {
    fn entries(&self, node: &str) -> Vec<&'a FlowEdgeDraft> {
        self.flow
            .iter()
            .copied()
            .filter(|edge| edge.to_node == node)
            .collect()
    }

    fn exits(&self, node: &str) -> Vec<&'a FlowEdgeDraft> {
        self.flow
            .iter()
            .copied()
            .filter(|edge| edge.from_node == node)
            .collect()
    }

    fn reachable(&self, from: &str, backwards: bool) -> HashSet<&'a str> {
        let mut seen: HashSet<&'a str> = HashSet::new();
        let mut pending: Vec<&str> = vec![from];
        while let Some(node) = pending.pop() {
            for edge in &self.flow {
                let (near, far) = if backwards {
                    (edge.to_node.as_str(), edge.from_node.as_str())
                } else {
                    (edge.from_node.as_str(), edge.to_node.as_str())
                };
                if near == node && seen.insert(far) {
                    pending.push(far);
                }
            }
        }
        seen
    }
}

fn problem(code: &'static str, node: Option<&str>, message: String) -> FlowProblem {
    FlowProblem {
        code,
        node: node.map(str::to_string),
        message,
    }
}

// Aplica V1 a V7 do fluxo de um time, nesta ordem; fluxo vazio é válido. `name_of` dá o nome de exibição de um agente.
pub(crate) fn validate_flow(
    draft: &TeamFlowDraft,
    name_of: &dyn Fn(&str) -> String,
) -> Vec<FlowProblem> {
    if draft.nodes.is_empty() && draft.gateways.is_empty() && draft.edges.is_empty() {
        return Vec::new();
    }
    let graph = Graph {
        flow: draft
            .edges
            .iter()
            .filter(|edge| edge.link == LINK_FLOW)
            .collect(),
    };
    let gateways: HashMap<&str, &str> = draft
        .gateways
        .iter()
        .map(|gateway| (gateway.id.as_str(), gateway.kind.as_str()))
        .collect();
    let positions: HashMap<&str, f64> = draft
        .nodes
        .iter()
        .map(|node| (node.agent_id.as_str(), node.pos_y))
        .collect();
    let label = |id: &str| match gateways.get(id) {
        Some(kind) => format!("gateway {kind}"),
        None => name_of(id),
    };
    let mut problems = Vec::new();

    if graph.exits(FLOW_START).len() != 1 {
        problems.push(problem(
            "V1",
            Some(FLOW_START),
            START_NEEDS_ONE_EXIT.to_string(),
        ));
    }
    if graph.entries(FLOW_END).is_empty() {
        problems.push(problem("V2", Some(FLOW_END), END_NEEDS_ENTRY.to_string()));
    }
    for node in &draft.nodes {
        let id = node.agent_id.as_str();
        if graph.entries(id).is_empty() || graph.exits(id).is_empty() {
            problems.push(problem(
                "V3",
                Some(id),
                format!(
                    "{} precisa de uma ligação de entrada e uma de saída.",
                    label(id)
                ),
            ));
        }
    }
    let from_start = graph.reachable(FLOW_START, false);
    let to_end = graph.reachable(FLOW_END, true);
    let in_flow = draft
        .nodes
        .iter()
        .map(|node| node.agent_id.as_str())
        .chain(draft.gateways.iter().map(|gateway| gateway.id.as_str()));
    for id in in_flow {
        if !from_start.contains(id) || !to_end.contains(id) {
            problems.push(problem(
                "V4",
                Some(id),
                format!("{} não está no caminho do início ao fim.", label(id)),
            ));
        }
    }
    for gateway in &draft.gateways {
        if !gateway_is_valid(&graph, &gateway.id, &gateway.kind) {
            problems.push(problem(
                "V5",
                Some(&gateway.id),
                format!("O gateway {} está com ligações inválidas.", gateway.kind),
            ));
        }
    }
    for edge in draft
        .edges
        .iter()
        .filter(|edge| edge.link == LINK_COMMUNICATION)
    {
        let same_row = match (
            positions.get(edge.from_node.as_str()),
            positions.get(edge.to_node.as_str()),
        ) {
            (Some(from), Some(to)) => (from - to).abs() <= SAME_LEVEL_TOLERANCE,
            _ => false,
        };
        if !same_row {
            problems.push(problem(
                "V6",
                Some(&edge.from_node),
                COMMUNICATION_SAME_ROW.to_string(),
            ));
        }
    }
    let known = |id: &str| {
        id == FLOW_START
            || id == FLOW_END
            || positions.contains_key(id)
            || gateways.contains_key(id)
    };
    let mut reported: HashSet<&str> = HashSet::new();
    for edge in &draft.edges {
        for id in [edge.from_node.as_str(), edge.to_node.as_str()] {
            if !known(id) && reported.insert(id) {
                problems.push(problem(
                    "V7",
                    Some(id),
                    format!("{} não é deste time.", name_of(id)),
                ));
            }
        }
    }
    problems
}

// Paralelo abre (1 entrada, 2 ou mais saídas) ou junta (2 ou mais entradas, 1 saída); exclusivo escolhe um ramo:
// 1 entrada, 2 ou mais saídas com eventos distintos e exatamente uma delas como padrão.
fn gateway_is_valid(graph: &Graph, id: &str, kind: &str) -> bool {
    let entries = graph.entries(id);
    let exits = graph.exits(id);
    match kind {
        GATEWAY_PARALLEL => {
            (entries.len() == 1 && exits.len() >= GATEWAY_MIN_BRANCHES)
                || (entries.len() >= GATEWAY_MIN_BRANCHES && exits.len() == 1)
        }
        GATEWAY_EXCLUSIVE => {
            let events: HashSet<&str> = exits.iter().map(|edge| edge.when_event.as_str()).collect();
            entries.len() == 1
                && exits.len() >= GATEWAY_MIN_BRANCHES
                && events.len() == exits.len()
                && exits.iter().filter(|edge| edge.is_default).count() == 1
        }
        _ => false,
    }
}
