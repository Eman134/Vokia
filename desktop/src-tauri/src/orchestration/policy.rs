use crate::agents::{join_items, Agent, Team};
use crate::db::{
    FlowEdge, Layer, OrchestrationSnapshot, FLOW_END, GATEWAY_EXCLUSIVE, GATEWAY_PARALLEL,
    LINK_COMMUNICATION,
};

use super::counters::TaskCounters;
use super::snapshot::{
    delegation_targets, enforced, event_phrase, flow_target, gateway_exits, node_id,
    outgoing_edges, placement_of, ACTION_CONSULT, ACTION_DELEGATE, ACTION_RETURN,
};

pub(crate) const LIMIT_TITLE: &str = "Limite do mapa atingido";
pub(crate) const REVIEW_BATCH_TITLE: &str = "Revisão do lote";
pub(crate) const REVIEW_TASK_TITLE: &str = "Revisão da tarefa";
pub(crate) const HUMAN_GATE_NEVER: &str = "nunca";
pub(crate) const HUMAN_GATE_END: &str = "fim";
const DEFAULT_KIND: &str = "implementar";
const CONSULT_KIND: &str = "pesquisar";
const SYNC_MODE_ASYNC: &str = "assincrona";

// Quando um gateway escolheu o caminho: o destino real e o evento que o escolheu.
pub(crate) struct Routed {
    pub(crate) to: Agent,
    pub(crate) event: String,
}

pub(crate) struct Passage {
    pub(crate) from_layer: Option<String>,
    pub(crate) to_layer: Option<String>,
    pub(crate) action: String,
    pub(crate) kind: String,
    pub(crate) return_count: i64,
    pub(crate) edge_id: Option<i64>,
    pub(crate) routed: Option<Routed>,
    // Linha assíncrona: o pedido segue como ramo separado e o turno de quem enviou termina na entrega.
    pub(crate) asynchronous: bool,
}

pub(crate) enum Verdict {
    Deliver(Passage),
    // Gateway paralelo: uma passagem por saída, todas entregues ao mesmo tempo.
    Fork(Vec<Passage>),
    Refuse(String),
    Escalate(String),
}

// Como a delegação de `from` a `to` chega ao destino: direto, por um gateway exclusivo (uma saída escolhida pelo
// evento) ou por um gateway paralelo (todas as saídas).
enum Line<'a> {
    Direct(&'a FlowEdge),
    Chosen(&'a FlowEdge),
    Fork(Vec<&'a FlowEdge>),
}

pub(crate) struct Review {
    pub(crate) title: &'static str,
    pub(crate) detail: String,
}

// `edge_passages` diz quantas passagens a ligação já teve na task (por card, ou pelos contadores da troca).
pub(crate) fn check(
    snapshot: Option<&OrchestrationSnapshot>,
    team: &Team,
    from: &Agent,
    to: &Agent,
    counters: &TaskCounters,
    edge_passages: &dyn Fn(i64) -> i64,
    event: Option<&str>,
) -> Verdict {
    let Some(snapshot) = snapshot else {
        return Verdict::Deliver(passage(None, None, None, 0));
    };
    let from_layer = layer_of(snapshot, from);
    let to_layer = layer_of(snapshot, to);
    if !enforced(Some(snapshot)) {
        return Verdict::Deliver(passage(
            from_layer,
            to_layer,
            pair_edge(snapshot, from, to),
            0,
        ));
    }
    if from_layer.is_none() {
        return Verdict::Refuse(format!(
            "{} não tem posição no mapa e não pode passar trabalho.",
            team.name_of(from)
        ));
    }
    if to_layer.is_none() {
        return Verdict::Refuse(format!("{} não tem posição no mapa.", team.name_of(to)));
    }
    let delivery = Delivery {
        snapshot,
        team,
        counters,
        edge_passages,
        from,
    };
    match resolve_line(snapshot, team, from, to, event) {
        None => Verdict::Refuse(not_allowed(snapshot, team, from, to)),
        Some(Line::Direct(edge)) => match delivery.deliver(to, edge, false) {
            Ok(found) => Verdict::Deliver(found),
            Err(motive) => Verdict::Escalate(motive),
        },
        Some(Line::Chosen(edge)) => match flow_target(team, &edge.to_node) {
            Some(destination) => match delivery.deliver(destination, edge, true) {
                Ok(found) => Verdict::Deliver(found),
                Err(motive) => Verdict::Escalate(motive),
            },
            None => Verdict::Refuse(not_allowed(snapshot, team, from, to)),
        },
        Some(Line::Fork(exits)) => {
            let mut passages = Vec::new();
            for exit in exits {
                let Some(destination) = flow_target(team, &exit.to_node) else {
                    continue;
                };
                match delivery.deliver(destination, exit, true) {
                    Ok(mut found) => {
                        found.asynchronous = false;
                        passages.push(found);
                    }
                    Err(motive) => return Verdict::Escalate(motive),
                }
            }
            Verdict::Fork(passages)
        }
    }
}

struct Delivery<'a> {
    snapshot: &'a OrchestrationSnapshot,
    team: &'a Team,
    counters: &'a TaskCounters,
    edge_passages: &'a dyn Fn(i64) -> i64,
    from: &'a Agent,
}

impl Delivery<'_> {
    // Confere o limite da linha e monta a passagem; o Err é o motivo da escalada.
    fn deliver(
        &self,
        destination: &Agent,
        edge: &FlowEdge,
        routed: bool,
    ) -> Result<Passage, String> {
        let limit = edge
            .turn_limit
            .unwrap_or(self.snapshot.settings.pair_turn_limit);
        let passed = (self.edge_passages)(edge.id);
        if passed >= limit {
            return Err(format!(
                "{} passou trabalho a {} {passed} vezes nesta task (limite da linha: {limit}).",
                self.team.name_of(self.from),
                self.team.name_of(destination)
            ));
        }
        let return_count = if edge.action == ACTION_RETURN {
            self.counters.returns(self.from, destination) + 1
        } else {
            0
        };
        let mut found = passage(
            layer_of(self.snapshot, self.from),
            layer_of(self.snapshot, destination),
            Some(edge),
            return_count,
        );
        if routed {
            found.routed = Some(Routed {
                to: destination.clone(),
                event: edge.when_event.clone(),
            });
        }
        Ok(found)
    }
}

fn resolve_line<'a>(
    snapshot: &'a OrchestrationSnapshot,
    team: &Team,
    from: &Agent,
    to: &Agent,
    event: Option<&str>,
) -> Option<Line<'a>> {
    if let Some(edge) = pair_edge(snapshot, from, to) {
        return Some(Line::Direct(edge));
    }
    for entry in outgoing_edges(snapshot, from) {
        let Some((gateway, exits)) = gateway_exits(snapshot, entry) else {
            continue;
        };
        if !exits.iter().any(|exit| exit.to_node == node_id(to)) {
            continue;
        }
        return match gateway.kind.as_str() {
            GATEWAY_PARALLEL => Some(Line::Fork(
                exits
                    .into_iter()
                    .filter(|exit| flow_target(team, &exit.to_node).is_some())
                    .collect(),
            )),
            GATEWAY_EXCLUSIVE => choose_exit(&exits, event).map(Line::Chosen),
            _ => None,
        };
    }
    None
}

// A saída do evento declarado; sem saída igual, a padrão.
fn choose_exit<'a>(exits: &[&'a FlowEdge], event: Option<&str>) -> Option<&'a FlowEdge> {
    event
        .and_then(|event| exits.iter().copied().find(|exit| exit.when_event == event))
        .or_else(|| exits.iter().copied().find(|exit| exit.is_default))
}

// Usado pela ferramenta MCP: o tipo e o evento informados precisam bater com a ligação do par (ou com o gateway
// que a recebe), além da política. Os limites de turno continuam sendo contados na troca.
pub(crate) fn check_intent(
    snapshot: Option<&OrchestrationSnapshot>,
    team: &Team,
    from: &Agent,
    to: &Agent,
    kind: Option<&str>,
    event: Option<&str>,
) -> Result<(), String> {
    let verdict = check(
        snapshot,
        team,
        from,
        to,
        &TaskCounters::default(),
        &|_| 0,
        event,
    );
    let passage = match verdict {
        Verdict::Deliver(passage) => passage,
        Verdict::Fork(passages) => match passages.into_iter().next() {
            Some(passage) => passage,
            None => return Err(not_allowed_text(team, from, to)),
        },
        Verdict::Refuse(motive) | Verdict::Escalate(motive) => return Err(motive),
    };
    let Some(snapshot) = snapshot.filter(|snapshot| enforced(Some(snapshot))) else {
        return Ok(());
    };
    if let Some(kind) = kind.filter(|kind| *kind != passage.kind) {
        return Err(format!(
            "O tipo {kind} não é o da seta de {} para {}: use {}.",
            team.name_of(from),
            team.name_of(to),
            passage.kind
        ));
    }
    if let (Some(event), Some(edge)) = (event, pair_edge(snapshot, from, to)) {
        if edge.when_event != event {
            return Err(format!(
                "O evento {event} não é permitido de {} para {}.",
                team.name_of(from),
                team.name_of(to)
            ));
        }
    }
    Ok(())
}

// A ligação que o time do remetente desenhou entre os dois: de fluxo (do remetente ao destinatário) ou de
// comunicação, que não tem direção e vale nos dois sentidos.
fn pair_edge<'a>(
    snapshot: &'a OrchestrationSnapshot,
    from: &Agent,
    to: &Agent,
) -> Option<&'a FlowEdge> {
    outgoing_edges(snapshot, from)
        .find(|edge| edge.to_node == node_id(to))
        .or_else(|| communication_edge(snapshot, from, to))
}

fn communication_edge<'a>(
    snapshot: &'a OrchestrationSnapshot,
    from: &Agent,
    to: &Agent,
) -> Option<&'a FlowEdge> {
    let team_id = &placement_of(snapshot, from)?.team_id;
    let (from, to) = (node_id(from), node_id(to));
    snapshot.edges.iter().find(|edge| {
        edge.team_id == *team_id
            && edge.link == LINK_COMMUNICATION
            && ((edge.from_node == from && edge.to_node == to)
                || (edge.from_node == to && edge.to_node == from))
    })
}

// Sem setas, a delegação só registra; com `on`, participantes sem posição saem da tarefa e, se nenhum sobra, vale quem recebeu o pedido.
pub(crate) fn placed_participants(
    snapshot: Option<&OrchestrationSnapshot>,
    participants: Vec<Agent>,
    origin: &Agent,
) -> Vec<Agent> {
    let Some(snapshot) = snapshot.filter(|snapshot| enforced(Some(snapshot))) else {
        return participants;
    };
    let placed: Vec<Agent> = participants
        .into_iter()
        .filter(|agent| placement_of(snapshot, agent).is_some())
        .collect();
    if placed.is_empty() {
        vec![origin.clone()]
    } else {
        placed
    }
}

pub(crate) fn task_limit_reached(
    snapshot: Option<&OrchestrationSnapshot>,
    counters: &TaskCounters,
) -> Option<String> {
    let snapshot = snapshot.filter(|snapshot| enforced(Some(snapshot)))?;
    let limit = snapshot.settings.task_turn_limit;
    (counters.turns() >= limit).then(|| format!("A tarefa passou de {limit} turnos."))
}

// Quem tem ligação até o fim do fluxo chega a ele ao terminar sem delegar.
pub(crate) fn reaches_flow_end(snapshot: Option<&OrchestrationSnapshot>, speaker: &Agent) -> bool {
    snapshot
        .filter(|snapshot| enforced(Some(snapshot)))
        .is_some_and(|snapshot| {
            outgoing_edges(snapshot, speaker).any(|edge| edge.to_node == FLOW_END)
        })
}

// Revisão do fim da cadeia: só com `on`, gate diferente de `nunca` e uma ligação de quem falou até o fim do fluxo.
pub(crate) fn chain_review(
    snapshot: Option<&OrchestrationSnapshot>,
    team: &Team,
    speaker: &Agent,
) -> Option<Review> {
    let snapshot = snapshot.filter(|snapshot| enforced(Some(snapshot)))?;
    let human_gate = snapshot.settings.human_gate.as_str();
    if human_gate == HUMAN_GATE_NEVER {
        return None;
    }
    let edge = outgoing_edges(snapshot, speaker).find(|edge| edge.to_node == FLOW_END)?;
    let event = event_phrase(&edge.when_event)?;
    let title = if human_gate == HUMAN_GATE_END {
        REVIEW_TASK_TITLE
    } else {
        REVIEW_BATCH_TITLE
    };
    Some(Review {
        title,
        detail: format!("Quando {event}, {} chama você.", team.name_of(speaker)),
    })
}

fn layer_of<'a>(snapshot: &'a OrchestrationSnapshot, agent: &Agent) -> Option<&'a Layer> {
    let placement = placement_of(snapshot, agent)?;
    snapshot
        .layers
        .iter()
        .find(|layer| layer.id == placement.layer_id)
}

fn not_allowed(snapshot: &OrchestrationSnapshot, team: &Team, from: &Agent, to: &Agent) -> String {
    let allowed = delegation_targets(snapshot, team, from);
    if allowed.is_empty() {
        return format!("{} não passa trabalho a ninguém.", team.name_of(from));
    }
    let names: Vec<String> = allowed
        .iter()
        .map(|agent| team.name_of(agent).to_string())
        .collect();
    format!(
        "{} não é um destino permitido para {}. Destinos permitidos: {}.",
        team.name_of(to),
        team.name_of(from),
        join_items(&names)
    )
}

fn not_allowed_text(team: &Team, from: &Agent, to: &Agent) -> String {
    format!(
        "{} não é um destino permitido para {}.",
        team.name_of(to),
        team.name_of(from)
    )
}

fn passage(
    from_layer: Option<&Layer>,
    to_layer: Option<&Layer>,
    edge: Option<&FlowEdge>,
    return_count: i64,
) -> Passage {
    // Consulta pela ligação de comunicação: pedido de pesquisa que não muda a etapa.
    let consults = edge.is_some_and(|edge| edge.link == LINK_COMMUNICATION);
    let (action, kind) = match edge {
        Some(_) if consults => (ACTION_CONSULT.to_string(), CONSULT_KIND.to_string()),
        Some(edge) => (edge.action.clone(), edge.kind.clone()),
        None => (ACTION_DELEGATE.to_string(), DEFAULT_KIND.to_string()),
    };
    Passage {
        from_layer: from_layer.map(|layer| layer.id.clone()),
        to_layer: to_layer.map(|layer| layer.id.clone()),
        action,
        kind,
        return_count,
        edge_id: edge.map(|edge| edge.id),
        routed: None,
        asynchronous: edge.is_some_and(|edge| edge.sync_mode == SYNC_MODE_ASYNC),
    }
}
