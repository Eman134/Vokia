use crate::agents::{join_items, Agent, Team};
use crate::db::{Layer, OrchestrationSnapshot, Rule};

use super::counters::TaskCounters;
use super::snapshot::{
    delegation_targets, enforced, event_phrase, placement_of, ACTION_DELEGATE, ACTION_ESCALATE,
    ACTION_RETURN,
};

pub(crate) const LIMIT_TITLE: &str = "Limite do mapa atingido";
const REVIEW_BATCH_TITLE: &str = "Revisão do lote";
const REVIEW_TASK_TITLE: &str = "Revisão da tarefa";
const HUMAN_GATE_NEVER: &str = "nunca";
const HUMAN_GATE_END: &str = "fim";
const DEFAULT_KIND: &str = "implementar";
const EXEC_MODE_PARALLEL: &str = "paralelo";

pub(crate) struct Passage {
    pub(crate) from_layer: Option<String>,
    pub(crate) to_layer: Option<String>,
    pub(crate) action: String,
    pub(crate) kind: String,
    pub(crate) return_count: i64,
}

pub(crate) enum Verdict {
    Deliver(Passage),
    Refuse(String),
    Escalate(String),
}

pub(crate) struct Review {
    pub(crate) title: &'static str,
    pub(crate) detail: String,
}

pub(crate) fn check(
    snapshot: Option<&OrchestrationSnapshot>,
    team: &Team,
    from: &Agent,
    to: &Agent,
    counters: &TaskCounters,
) -> Verdict {
    let Some(snapshot) = snapshot else {
        return Verdict::Deliver(passage(None, None, None, 0));
    };
    let from_layer = layer_of(snapshot, from);
    let to_layer = layer_of(snapshot, to);
    let rule = pair_rule(snapshot, from, to);
    if !enforced(Some(snapshot)) {
        return Verdict::Deliver(passage(from_layer, to_layer, rule, 0));
    }
    let Some(from_layer) = from_layer else {
        return Verdict::Refuse(format!(
            "{} não tem posição no mapa e não pode passar trabalho.",
            team.name_of(from)
        ));
    };
    let Some(to_layer) = to_layer else {
        return Verdict::Refuse(format!("{} não tem posição no mapa.", team.name_of(to)));
    };
    let Some(rule) = rule else {
        return Verdict::Refuse(not_allowed(snapshot, team, from, to, from_layer));
    };
    let pair_limit = snapshot.settings.pair_turn_limit;
    if counters.passages(from, to) >= pair_limit {
        return Verdict::Escalate(format!(
            "{} passou trabalho a {} mais de {pair_limit} vezes nesta tarefa.",
            team.name_of(from),
            team.name_of(to)
        ));
    }
    let mut return_count = 0;
    if rule.action == ACTION_RETURN {
        let returns = counters.returns(from, to);
        if returns >= rule.return_limit {
            return Verdict::Escalate(format!(
                "{} devolveu trabalho a {} mais de {} vezes nesta tarefa.",
                team.name_of(from),
                team.name_of(to),
                rule.return_limit
            ));
        }
        return_count = returns + 1;
    }
    Verdict::Deliver(passage(
        Some(from_layer),
        Some(to_layer),
        Some(rule),
        return_count,
    ))
}

// Usado pela ferramenta MCP: o tipo e o evento informados precisam bater com a seta do par, além da política.
// Os limites de turno continuam sendo contados na troca; a chamada não recebe os contadores.
pub(crate) fn check_intent(
    snapshot: Option<&OrchestrationSnapshot>,
    team: &Team,
    from: &Agent,
    to: &Agent,
    kind: Option<&str>,
    event: Option<&str>,
) -> Result<(), String> {
    let passage = match check(snapshot, team, from, to, &TaskCounters::default()) {
        Verdict::Deliver(passage) => passage,
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
    if let Some(event) = event {
        let allowed = pair_rule(snapshot, from, to).map(|rule| rule.when_event.as_str());
        if allowed != Some(event) {
            return Err(format!(
                "O evento {event} não é permitido de {} para {}.",
                team.name_of(from),
                team.name_of(to)
            ));
        }
    }
    Ok(())
}

// Várias delegações no mesmo turno só valem quando a seta do par tem execução em paralelo.
pub(crate) fn parallel_pair(
    snapshot: Option<&OrchestrationSnapshot>,
    from: &Agent,
    to: &Agent,
) -> bool {
    snapshot
        .filter(|snapshot| enforced(Some(snapshot)))
        .and_then(|snapshot| pair_rule(snapshot, from, to))
        .is_some_and(|rule| rule.exec_mode == EXEC_MODE_PARALLEL)
}

fn pair_rule<'a>(
    snapshot: &'a OrchestrationSnapshot,
    from: &Agent,
    to: &Agent,
) -> Option<&'a Rule> {
    let from_layer = layer_of(snapshot, from)?;
    let to_layer = layer_of(snapshot, to)?;
    snapshot
        .rules
        .iter()
        .find(|rule| rule.from_layer == from_layer.id && rule.to_layer == to_layer.id)
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

// Revisão do fim da cadeia: só com `on`, gate diferente de `nunca` e uma seta escalar saindo da camada de quem falou.
pub(crate) fn chain_review(
    snapshot: Option<&OrchestrationSnapshot>,
    speaker: &Agent,
) -> Option<Review> {
    let snapshot = snapshot.filter(|snapshot| enforced(Some(snapshot)))?;
    let human_gate = snapshot.settings.human_gate.as_str();
    if human_gate == HUMAN_GATE_NEVER {
        return None;
    }
    let layer = layer_of(snapshot, speaker)?;
    let rule = snapshot
        .rules
        .iter()
        .find(|rule| rule.from_layer == layer.id && rule.action == ACTION_ESCALATE)?;
    let event = event_phrase(&rule.when_event)?;
    let title = if human_gate == HUMAN_GATE_END {
        REVIEW_TASK_TITLE
    } else {
        REVIEW_BATCH_TITLE
    };
    Some(Review {
        title,
        detail: format!("Quando {event}, a camada {} chama você.", layer.name),
    })
}

fn layer_of<'a>(snapshot: &'a OrchestrationSnapshot, agent: &Agent) -> Option<&'a Layer> {
    let placement = placement_of(snapshot, agent)?;
    snapshot
        .layers
        .iter()
        .find(|layer| layer.id == placement.layer_id)
}

fn not_allowed(
    snapshot: &OrchestrationSnapshot,
    team: &Team,
    from: &Agent,
    to: &Agent,
    from_layer: &Layer,
) -> String {
    let allowed = delegation_targets(snapshot, team, from, &from_layer.id);
    if allowed.is_empty() {
        return format!("A camada {} não passa trabalho a ninguém.", from_layer.name);
    }
    let names: Vec<String> = allowed
        .iter()
        .map(|agent| team.name_of(agent).to_string())
        .collect();
    format!(
        "{} não é um destino permitido para a camada {}. Destinos permitidos: {}.",
        team.name_of(to),
        from_layer.name,
        join_items(&names)
    )
}

fn passage(
    from_layer: Option<&Layer>,
    to_layer: Option<&Layer>,
    rule: Option<&Rule>,
    return_count: i64,
) -> Passage {
    Passage {
        from_layer: from_layer.map(|layer| layer.id.clone()),
        to_layer: to_layer.map(|layer| layer.id.clone()),
        action: rule.map_or_else(|| ACTION_DELEGATE.to_string(), |rule| rule.action.clone()),
        kind: rule.map_or_else(|| DEFAULT_KIND.to_string(), |rule| rule.kind.clone()),
        return_count,
    }
}
