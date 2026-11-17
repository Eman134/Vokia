use crate::agents::{join_items, Agent, Team};
use crate::db::{Layer, OrchestrationSnapshot, Placement, Rule};

const ENFORCEMENT_ON: &str = "on";
const LEADER_LAYER_KIND: &str = "planejar";
const ACTION_DELEGATE: &str = "delegar";
const ACTION_RETURN: &str = "devolver";
const ACTION_ESCALATE: &str = "escalar";
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
    let Some(placement) = snapshot
        .placements
        .iter()
        .find(|placement| placement.agent_id == agent.as_str())
    else {
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
        delegation_targets(snapshot, team, agent, &layer.id)
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
    let map_rules = enforced.then(|| map_rules_text(snapshot, team, agent, layer));
    AgentPrompt {
        roster,
        layer: Some(LayerContext {
            layer_name: layer.name.clone(),
            layer_instructions: layer.instructions.trim().to_string(),
            agent_instructions: placement.instructions.trim().to_string(),
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

fn agents_in_layer(snapshot: &OrchestrationSnapshot, team: &Team, layer_id: &str) -> Vec<Agent> {
    team.ids()
        .filter(|id| {
            snapshot.placements.iter().any(|placement| {
                placement.agent_id == id.as_str() && placement.layer_id == layer_id
            })
        })
        .cloned()
        .collect()
}

fn delegation_targets(
    snapshot: &OrchestrationSnapshot,
    team: &Team,
    agent: &Agent,
    from_layer: &str,
) -> Vec<Agent> {
    let mut targets: Vec<Agent> = Vec::new();
    let outgoing = snapshot.rules.iter().filter(|rule| {
        rule.from_layer == from_layer
            && (rule.action == ACTION_DELEGATE || rule.action == ACTION_RETURN)
    });
    for rule in outgoing {
        for target in agents_in_layer(snapshot, team, &rule.to_layer) {
            if target != *agent && !targets.contains(&target) {
                targets.push(target);
            }
        }
    }
    targets
}

fn map_rules_text(
    snapshot: &OrchestrationSnapshot,
    team: &Team,
    agent: &Agent,
    layer: &Layer,
) -> String {
    let sentences: Vec<String> = snapshot
        .rules
        .iter()
        .filter(|rule| rule.from_layer == layer.id)
        .filter_map(|rule| rule_sentence(snapshot, team, agent, rule))
        .collect();
    let header = format!("Regras do mapa: você é da camada {}.", layer.name);
    if sentences.is_empty() {
        format!("{header} {NO_OUTGOING_RULES}")
    } else {
        format!("{header} {} {NO_OTHER_AGENT_CLAUSE}", sentences.join(" "))
    }
}

// Seta sem agente no destino (ou com evento/tipo fora da tabela) não gera frase.
fn rule_sentence(
    snapshot: &OrchestrationSnapshot,
    team: &Team,
    agent: &Agent,
    rule: &Rule,
) -> Option<String> {
    let event = phrase(EVENT_PHRASES, &rule.when_event)?;
    match rule.action.as_str() {
        ACTION_DELEGATE => {
            let names = destination_names(snapshot, team, agent, &rule.to_layer);
            if names.is_empty() {
                return None;
            }
            let kind = phrase(KIND_PHRASES, &rule.kind)?;
            Some(format!(
                "Passe trabalho a {} quando {event}, como pedido de {kind}.",
                join_items(&names)
            ))
        }
        ACTION_RETURN => {
            let names = destination_names(snapshot, team, agent, &rule.to_layer);
            if names.is_empty() {
                return None;
            }
            Some(format!(
                "Devolva a {} quando {event}, até {} vezes.",
                join_items(&names),
                rule.return_limit
            ))
        }
        ACTION_ESCALATE => Some(format!(
            "Quando {event}, termine a resposta sem delegar: o usuário revisa."
        )),
        _ => None,
    }
}

fn destination_names(
    snapshot: &OrchestrationSnapshot,
    team: &Team,
    agent: &Agent,
    layer_id: &str,
) -> Vec<String> {
    agents_in_layer(snapshot, team, layer_id)
        .iter()
        .filter(|target| *target != agent)
        .map(|target| team.name_of(target).to_string())
        .collect()
}

pub(crate) struct MemberOutline {
    pub(crate) name: String,
    pub(crate) layer: String,
}

pub(crate) struct TeamOutline {
    pub(crate) name: String,
    pub(crate) leader: Option<String>,
    pub(crate) members: Vec<MemberOutline>,
}

pub(crate) struct TeamOutlines {
    pub(crate) teams: Vec<TeamOutline>,
    pub(crate) unassigned: Vec<String>,
}

// A tabela de times não tem campo de líder: o líder do time é o primeiro membro posicionado numa camada de tipo planejar.
pub(crate) fn team_outlines(snapshot: &OrchestrationSnapshot, team: &Team) -> TeamOutlines {
    let teams = snapshot
        .teams
        .iter()
        .map(|orch_team| {
            let members: Vec<(bool, MemberOutline)> = team
                .ids()
                .filter_map(|agent| {
                    let placement = placement_of(snapshot, agent)?;
                    if placement.team_id != orch_team.id {
                        return None;
                    }
                    let layer = snapshot
                        .layers
                        .iter()
                        .find(|layer| layer.id == placement.layer_id);
                    let is_leader = layer.is_some_and(|layer| layer.kind == LEADER_LAYER_KIND);
                    let layer_name = layer
                        .map_or_else(|| placement.layer_id.clone(), |layer| layer.name.clone());
                    Some((
                        is_leader,
                        MemberOutline {
                            name: team.name_of(agent).to_string(),
                            layer: layer_name,
                        },
                    ))
                })
                .collect();
            let leader = members
                .iter()
                .find(|(is_leader, _)| *is_leader)
                .map(|(_, member)| member.name.clone());
            TeamOutline {
                name: orch_team.name.clone(),
                leader,
                members: members.into_iter().map(|(_, member)| member).collect(),
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

fn placement_of<'a>(snapshot: &'a OrchestrationSnapshot, agent: &Agent) -> Option<&'a Placement> {
    snapshot
        .placements
        .iter()
        .find(|placement| placement.agent_id == agent.as_str())
}

// Sem banco (ou líder inválido) o interpretador usa o agente embutido padrão.
pub(crate) fn leader_of(snapshot: Option<&OrchestrationSnapshot>) -> Agent {
    snapshot
        .and_then(|snapshot| Agent::from_stored(&snapshot.settings.leader_agent))
        .unwrap_or_else(Agent::ember)
}

fn phrase(table: &[(&'static str, &'static str)], key: &str) -> Option<&'static str> {
    table
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, text)| *text)
}
