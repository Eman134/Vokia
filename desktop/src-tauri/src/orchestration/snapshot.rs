use crate::agents::{join_items, replica_of, Agent, Team};
use super::stages::{judging_team, JUDGMENT_RULE};
use crate::db::{
    FlowEdge, FlowGateway, Layer, OrchestrationSnapshot, Placement, FLOW_END, FLOW_START,
    GATEWAY_PARALLEL, LINK_COMMUNICATION, LINK_FLOW,
};

const ENFORCEMENT_ON: &str = "on";
const LEADER_LAYER_KIND: &str = "planejar";
pub(crate) const ACTION_DELEGATE: &str = "delegar";
pub(crate) const ACTION_RETURN: &str = "devolver";
pub(crate) const ACTION_ESCALATE: &str = "escalar";
pub(crate) const ACTION_CONSULT: &str = "consultar";
const NO_DESTINATION_ROSTER: &str = "Time desta sessão: nenhum agente para passar trabalho.";
const NO_OUTGOING_RULES: &str =
    "Você não passa trabalho a ninguém: termine a resposta quando concluir.";
const NO_OTHER_AGENT_CLAUSE: &str = "Não passe trabalho a nenhum outro agente.";
const EVENT_PHRASES: &[(&str, &str)] = &[
    ("plano_aprovado", "o plano for aprovado"),
    ("tarefa_concluida", "a tarefa for concluída"),
    ("revisao_reprovou", "a revisão reprovar"),
    ("testes_falharam", "os testes falharem"),
    ("revisao_aprovou", "a revisão aprovar"),
    ("pedido_de_ajuda", "o agente pedir ajuda"),
];
const KIND_PHRASES: &[(&str, &str)] = &[
    ("planejar", "planejamento"),
    ("implementar", "implementação"),
    ("testar", "testes"),
    ("revisar", "revisão"),
    ("pesquisar", "pesquisa"),
];

pub(crate) struct LayerContext {
    pub(crate) layer_name: String,
    pub(crate) layer_instructions: String,
    pub(crate) agent_instructions: String,
    pub(crate) agent_criteria: String,
    pub(crate) map_rules: Option<String>,
}

pub(crate) struct AgentPrompt {
    pub(crate) roster: String,
    pub(crate) layer: Option<LayerContext>,
}

// Agente sem posição, sem snapshot ou com a posição apontando para camada inexistente: prompt de hoje.
pub(crate) fn agent_prompt(
    snapshot: Option<&OrchestrationSnapshot>,
    team: &Team,
    agent: &Agent,
) -> AgentPrompt {
    let Some(snapshot) = snapshot else {
        return without_layer(team);
    };
    let Some(placement) = placement_of(snapshot, agent) else {
        return without_layer(team);
    };
    let Some(layer) = snapshot
        .layers
        .iter()
        .find(|layer| layer.id == placement.layer_id)
    else {
        return without_layer(team);
    };
    let enforced = snapshot.settings.enforcement == ENFORCEMENT_ON;
    let destinations = if enforced {
        delegation_targets(snapshot, team, agent)
    } else {
        Vec::new()
    };
    let roster = if !enforced {
        team.roster_line()
    } else if destinations.is_empty() {
        NO_DESTINATION_ROSTER.to_string()
    } else {
        team.roster_line_among(&destinations)
    };
    let map_rules = enforced.then(|| map_rules_text(snapshot, team, agent, &placement.team_id));
    // A resposta do líder que julga é lida mesmo sem as regras do mapa ligadas (turns.rs), então a regra também vem.
    let judgment = judging_team(snapshot, agent).map(|_| JUDGMENT_RULE);
    let map_rules = match (map_rules, judgment) {
        (Some(rules), Some(rule)) => Some(format!("{rules} {rule}")),
        (rules, rule) => rules.or_else(|| rule.map(str::to_string)),
    };
    AgentPrompt {
        roster,
        layer: Some(LayerContext {
            layer_name: layer.name.clone(),
            layer_instructions: layer.instructions.trim().to_string(),
            agent_instructions: placement.instructions.trim().to_string(),
            agent_criteria: placement.criteria.trim().to_string(),
            map_rules,
        }),
    }
}

fn without_layer(team: &Team) -> AgentPrompt {
    AgentPrompt {
        roster: team.roster_line(),
        layer: None,
    }
}

// Id do nó do agente no fluxo: a réplica usa o nó do agente base.
pub(crate) fn node_id(agent: &Agent) -> &str {
    replica_of(agent.as_str()).map_or(agent.as_str(), |(base, _)| base)
}

pub(crate) fn flow_target<'a>(team: &'a Team, node: &str) -> Option<&'a Agent> {
    team.ids().find(|id| id.as_str() == node)
}

// Ligações de fluxo que saem do agente, na ordem em que foram desenhadas, dentro do time dele.
pub(crate) fn outgoing_edges<'a>(
    snapshot: &'a OrchestrationSnapshot,
    agent: &Agent,
) -> impl Iterator<Item = &'a FlowEdge> {
    let team_id = placement_of(snapshot, agent).map(|placement| placement.team_id.as_str());
    let node = node_id(agent).to_string();
    snapshot.edges.iter().filter(move |edge| {
        Some(edge.team_id.as_str()) == team_id && edge.link == LINK_FLOW && edge.from_node == node
    })
}

// Gateway para o qual a ligação aponta, com as ligações de saída dele; None se ela aponta para um agente ou o fim.
pub(crate) fn gateway_exits<'a>(
    snapshot: &'a OrchestrationSnapshot,
    entry: &FlowEdge,
) -> Option<(&'a FlowGateway, Vec<&'a FlowEdge>)> {
    let gateway = snapshot
        .gateways
        .iter()
        .find(|gateway| gateway.team_id == entry.team_id && gateway.id == entry.to_node)?;
    let exits = snapshot
        .edges
        .iter()
        .filter(|edge| {
            edge.team_id == gateway.team_id
                && edge.link == LINK_FLOW
                && edge.from_node == gateway.id
        })
        .collect();
    Some((gateway, exits))
}

// Gateway paralelo de junção: 2 ou mais entradas e uma só saída. Os ramos que chegam nele terminam sem delegar, e o app
// entrega o resultado ao destino uma única vez, quando o último ramo volta.
pub(crate) fn is_join(snapshot: &OrchestrationSnapshot, gateway: &FlowGateway) -> bool {
    if gateway.kind != GATEWAY_PARALLEL {
        return false;
    }
    let flow_edges = || {
        snapshot
            .edges
            .iter()
            .filter(|edge| edge.team_id == gateway.team_id && edge.link == LINK_FLOW)
    };
    let entries = flow_edges()
        .filter(|edge| edge.to_node == gateway.id)
        .count();
    let exits = flow_edges()
        .filter(|edge| edge.from_node == gateway.id)
        .count();
    entries >= 2 && exits == 1
}

// A ligação que leva o agente ao fim do fluxo: direta ou pela saída de um gateway (gateways encadeados também).
pub(crate) fn end_edge<'a>(
    snapshot: &'a OrchestrationSnapshot,
    agent: &Agent,
) -> Option<&'a FlowEdge> {
    outgoing_edges(snapshot, agent).find_map(|edge| end_through(snapshot, edge, &mut Vec::new()))
}

fn end_through<'a>(
    snapshot: &'a OrchestrationSnapshot,
    edge: &'a FlowEdge,
    visited: &mut Vec<&'a str>,
) -> Option<&'a FlowEdge> {
    if edge.to_node == FLOW_END {
        return Some(edge);
    }
    let (gateway, exits) = gateway_exits(snapshot, edge)?;
    if visited.contains(&gateway.id.as_str()) {
        return None;
    }
    visited.push(&gateway.id);
    exits
        .into_iter()
        .find_map(|exit| end_through(snapshot, exit, visited))
}

pub(crate) fn delegation_targets(
    snapshot: &OrchestrationSnapshot,
    team: &Team,
    agent: &Agent,
) -> Vec<Agent> {
    let mut targets: Vec<Agent> = Vec::new();
    let delegating = outgoing_edges(snapshot, agent)
        .filter(|edge| edge.action == ACTION_DELEGATE || edge.action == ACTION_RETURN);
    for edge in delegating {
        let destinations: Vec<&str> = match gateway_exits(snapshot, edge) {
            // Quem entra numa junção termina sem delegar: o app entrega ao destino quando todos os ramos voltam.
            Some((gateway, _)) if is_join(snapshot, gateway) => continue,
            Some((_, exits)) => exits.iter().map(|exit| exit.to_node.as_str()).collect(),
            None => vec![edge.to_node.as_str()],
        };
        for node in destinations {
            if let Some(target) = flow_target(team, node) {
                if target != agent && !targets.contains(target) {
                    targets.push(target.clone());
                }
            }
        }
    }
    for partner in communication_partners(snapshot, team, agent) {
        if !targets.contains(&partner) {
            targets.push(partner);
        }
    }
    targets
}

// Quem pode ser consultado pelo agente (e consultá-lo): a ligação de comunicação não tem direção.
fn communication_partners(
    snapshot: &OrchestrationSnapshot,
    team: &Team,
    agent: &Agent,
) -> Vec<Agent> {
    let Some(placement) = placement_of(snapshot, agent) else {
        return Vec::new();
    };
    let node = node_id(agent);
    snapshot
        .edges
        .iter()
        .filter(|edge| edge.team_id == placement.team_id && edge.link == LINK_COMMUNICATION)
        .filter_map(
            |edge| match (edge.from_node == node, edge.to_node == node) {
                (true, _) => flow_target(team, &edge.to_node),
                (_, true) => flow_target(team, &edge.from_node),
                _ => None,
            },
        )
        .cloned()
        .collect()
}

fn map_rules_text(
    snapshot: &OrchestrationSnapshot,
    team: &Team,
    agent: &Agent,
    team_id: &str,
) -> String {
    let consultations = communication_partners(snapshot, team, agent)
        .into_iter()
        .map(|partner| {
            format!(
                "Consulte {} quando precisar de uma pesquisa: a consulta não muda a etapa.",
                team.name_of(&partner)
            )
        });
    let sentences: Vec<String> = outgoing_edges(snapshot, agent)
        .filter_map(|edge| edge_sentence(snapshot, team, edge))
        .chain(consultations)
        .collect();
    let team_name = snapshot
        .teams
        .iter()
        .find(|orch_team| orch_team.id == team_id)
        .map_or(team_id, |orch_team| orch_team.name.as_str());
    let role = team
        .profile(agent)
        .map_or("", |profile| profile.role.as_str());
    let header = format!("Regras do fluxo do time {team_name}: você é {role}.");
    if sentences.is_empty() {
        format!("{header} {NO_OUTGOING_RULES}")
    } else {
        format!("{header} {} {NO_OTHER_AGENT_CLAUSE}", sentences.join(" "))
    }
}

// Ligação que entra num gateway: o agente delega a qualquer destino das saídas e o app escolhe o caminho.
fn gateway_sentence(
    team: &Team,
    entry: &FlowEdge,
    gateway_kind: &str,
    exits: &[&FlowEdge],
    event: &str,
) -> Option<String> {
    let kind = phrase(KIND_PHRASES, &entry.kind)?;
    let destinations: Vec<(&str, &str)> = exits
        .iter()
        .filter_map(|exit| {
            let name = team.name_of(flow_target(team, &exit.to_node)?);
            Some((name, phrase(EVENT_PHRASES, &exit.when_event)?))
        })
        .collect();
    if destinations.is_empty() {
        return None;
    }
    if gateway_kind == GATEWAY_PARALLEL {
        let names: Vec<String> = destinations
            .iter()
            .map(|(name, _)| name.to_string())
            .collect();
        return Some(format!(
            "Quando {event}, passe trabalho a {} ao mesmo tempo, como pedido de {kind}: uma só delegação vai a todos.",
            join_items(&names)
        ));
    }
    let paths: Vec<String> = destinations
        .iter()
        .map(|(name, when)| format!("{name} quando {when}"))
        .collect();
    Some(format!(
        "Quando {event}, passe trabalho por um dos caminhos, informando o evento ao delegar: {}.",
        join_items(&paths)
    ))
}

fn join_sentence(team: &Team, event: &str, exits: &[&FlowEdge]) -> Option<String> {
    let exit = exits.first()?;
    let destination = if exit.to_node == FLOW_END {
        "o fim do fluxo".to_string()
    } else {
        team.name_of(flow_target(team, &exit.to_node)?).to_string()
    };
    Some(format!(
        "Quando {event}, termine a resposta sem delegar: o resultado segue para {destination} quando todos os ramos concluírem."
    ))
}

// A instrução da ligação acompanha a frase dela: é como o agente deve agir naquele caminho.
fn edge_sentence(snapshot: &OrchestrationSnapshot, team: &Team, edge: &FlowEdge) -> Option<String> {
    let sentence = edge_rule_sentence(snapshot, team, edge)?;
    let instructions = edge.instructions.trim();
    if instructions.is_empty() {
        Some(sentence)
    } else {
        Some(format!("{sentence} Instrução: {instructions}"))
    }
}

// Ligação sem agente no destino (gateway, por exemplo) ou com evento/tipo fora da tabela não gera frase.
fn edge_rule_sentence(
    snapshot: &OrchestrationSnapshot,
    team: &Team,
    edge: &FlowEdge,
) -> Option<String> {
    let event = phrase(EVENT_PHRASES, &edge.when_event)?;
    if edge.action == ACTION_ESCALATE {
        return Some(format!(
            "Quando {event}, termine a resposta sem delegar: o usuário revisa."
        ));
    }
    if let Some((gateway, exits)) = gateway_exits(snapshot, edge) {
        if is_join(snapshot, gateway) {
            return join_sentence(team, event, &exits);
        }
        return gateway_sentence(team, edge, &gateway.kind, &exits, event);
    }
    let name = team.name_of(flow_target(team, &edge.to_node)?);
    match edge.action.as_str() {
        ACTION_DELEGATE => {
            let kind = phrase(KIND_PHRASES, &edge.kind)?;
            Some(format!(
                "Passe trabalho a {name} quando {event}, como pedido de {kind}."
            ))
        }
        ACTION_RETURN => Some(format!(
            "Devolva a {name} quando {event}, até {} vezes.",
            edge.turn_limit.unwrap_or(snapshot.settings.pair_turn_limit)
        )),
        _ => None,
    }
}

pub(crate) struct MemberOutline {
    pub(crate) name: String,
    pub(crate) role: String,
    pub(crate) layer: String,
    pub(crate) layer_kind: String,
}

pub(crate) struct TeamOutline {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) instructions: Option<String>,
    pub(crate) leader: Option<String>,
    pub(crate) members: Vec<MemberOutline>,
}

pub(crate) struct TeamOutlines {
    pub(crate) teams: Vec<TeamOutline>,
    pub(crate) unassigned: Vec<String>,
}

// Líder do time, nesta ordem: o destino da ligação de início (ou o líder gravado, sem ela) se está no elenco e não pausado; senão o primeiro membro, na ordem
// do elenco, numa camada de tipo planejar e não pausado. Ember orquestra e nunca lidera; pausado é indisponível.
pub(crate) fn available_leader(
    snapshot: &OrchestrationSnapshot,
    orch_team: &crate::db::Team,
    team: &Team,
    paused: &[Agent],
) -> Option<Agent> {
    let usable = |agent: &Agent| *agent != Agent::ember() && !paused.contains(agent);
    let start_destination = snapshot
        .edges
        .iter()
        .find(|edge| edge.team_id == orch_team.id && edge.from_node == FLOW_START)
        .map(|edge| edge.to_node.as_str());
    let explicit = start_destination
        .or(orch_team.leader_agent.as_deref())
        .and_then(Agent::from_stored)
        .filter(|agent| team.profile(agent).is_some() && usable(agent));
    explicit.or_else(|| {
        team.ids()
            .find(|agent| {
                usable(agent)
                    && placement_of(snapshot, agent).is_some_and(|placement| {
                        placement.team_id == orch_team.id
                            && layer_of(snapshot, &placement.layer_id)
                                .is_some_and(|layer| layer.kind == LEADER_LAYER_KIND)
                    })
            })
            .cloned()
    })
}

pub(crate) struct TeamLeader {
    pub(crate) team_name: String,
    pub(crate) leader: Agent,
}

// Id ou nome exato do time, sem diferença de maiúsculas.
pub(crate) fn find_team<'a>(
    snapshot: &'a OrchestrationSnapshot,
    key: &str,
) -> Option<&'a crate::db::Team> {
    let key = key.trim().to_lowercase();
    snapshot
        .teams
        .iter()
        .find(|team| team.id.to_lowercase() == key || team.name.to_lowercase() == key)
}

pub(crate) fn team_leader(
    snapshot: &OrchestrationSnapshot,
    orch_team: &crate::db::Team,
    team: &Team,
    paused: &[Agent],
) -> Option<TeamLeader> {
    available_leader(snapshot, orch_team, team, paused).map(|leader| TeamLeader {
        team_name: orch_team.name.clone(),
        leader,
    })
}

// Só os times que têm líder disponível, na ordem do mapa.
pub(crate) fn team_leaders(
    snapshot: &OrchestrationSnapshot,
    team: &Team,
    paused: &[Agent],
) -> Vec<TeamLeader> {
    snapshot
        .teams
        .iter()
        .filter_map(|orch_team| team_leader(snapshot, orch_team, team, paused))
        .collect()
}

fn layer_of<'a>(snapshot: &'a OrchestrationSnapshot, layer_id: &str) -> Option<&'a Layer> {
    snapshot.layers.iter().find(|layer| layer.id == layer_id)
}

pub(crate) fn team_outlines(
    snapshot: &OrchestrationSnapshot,
    team: &Team,
    paused: &[Agent],
) -> TeamOutlines {
    let teams = snapshot
        .teams
        .iter()
        .map(|orch_team| {
            let members = team
                .ids()
                .filter_map(|agent| {
                    let placement = placement_of(snapshot, agent)?;
                    if placement.team_id != orch_team.id {
                        return None;
                    }
                    let layer = layer_of(snapshot, &placement.layer_id);
                    Some(MemberOutline {
                        name: team.name_of(agent).to_string(),
                        role: team
                            .profile(agent)
                            .map(|profile| profile.role.clone())
                            .unwrap_or_default(),
                        layer: layer
                            .map_or_else(|| placement.layer_id.clone(), |layer| layer.name.clone()),
                        layer_kind: layer.map(|layer| layer.kind.clone()).unwrap_or_default(),
                    })
                })
                .collect();
            let instructions = Some(orch_team.instructions.trim())
                .filter(|text| !text.is_empty())
                .map(str::to_string);
            TeamOutline {
                id: orch_team.id.clone(),
                name: orch_team.name.clone(),
                instructions,
                leader: available_leader(snapshot, orch_team, team, paused)
                    .map(|leader| team.name_of(&leader).to_string()),
                members,
            }
        })
        .collect();
    let unassigned = team
        .ids()
        .filter(|agent| placement_of(snapshot, agent).is_none())
        .map(|agent| team.name_of(agent).to_string())
        .collect();
    TeamOutlines { teams, unassigned }
}

pub(crate) fn placement_of<'a>(
    snapshot: &'a OrchestrationSnapshot,
    agent: &Agent,
) -> Option<&'a Placement> {
    // Réplica herda a posição do agente base: mesma camada e time, sem posição gravada.
    let id = agent.as_str();
    snapshot
        .placements
        .iter()
        .find(|placement| placement.agent_id == id)
        .or_else(|| {
            let (base, _) = replica_of(id)?;
            snapshot
                .placements
                .iter()
                .find(|placement| placement.agent_id == base)
        })
}

pub(crate) fn enforced(snapshot: Option<&OrchestrationSnapshot>) -> bool {
    snapshot.is_some_and(|snapshot| snapshot.settings.enforcement == ENFORCEMENT_ON)
}

pub(crate) fn event_phrase(event: &str) -> Option<&'static str> {
    phrase(EVENT_PHRASES, event)
}

// Sem banco (ou líder inválido) o interpretador usa o agente embutido padrão.
pub(crate) fn leader_of(snapshot: Option<&OrchestrationSnapshot>) -> Agent {
    snapshot
        .and_then(|snapshot| Agent::from_stored(&snapshot.settings.leader_agent))
        .unwrap_or_else(Agent::axiom)
}

fn phrase(table: &[(&'static str, &'static str)], key: &str) -> Option<&'static str> {
    table
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, text)| *text)
}
