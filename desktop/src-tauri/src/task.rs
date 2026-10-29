use serde_json::Value as Json;

use crate::cache::{
    validate_name, validate_value, Cache, Rule, VarValue, Variable, CONVERSATION_VAR,
};
use crate::events::Agent;

const NUMBER_TYPE: &str = "numero";
const TEXT_TYPE: &str = "texto";
const LIST_TYPE: &str = "lista";
const FREE_RULE: &str = "livre";
const INCREMENT_RULE: &str = "mais_um";
const READ_ONLY_RULE: &str = "so_leitura";

#[derive(Debug)]
pub struct TaskDefinition {
    pub nova_tarefa: bool,
    pub objetivo: String,
    pub participants: Vec<Agent>,
    pub alternate: bool,
    pub variables: Vec<Variable>,
    pub stop: Option<StopCondition>,
}

#[derive(Debug)]
pub struct StopCondition {
    pub variable: String,
    pub limit: StopLimit,
}

#[derive(Debug)]
pub enum StopLimit {
    Value(i64),
    Variable(String),
}

impl StopCondition {
    pub fn reached(&self, cache: &Cache) -> bool {
        let Some(current) = cache.number(&self.variable) else {
            return false;
        };
        let limit = match &self.limit {
            StopLimit::Value(value) => Some(*value),
            StopLimit::Variable(name) => cache.number(name),
        };
        limit.is_some_and(|limit| current >= limit)
    }
}

#[derive(Debug)]
pub struct ActiveTask {
    pub objetivo: String,
    pub participants: Vec<Agent>,
    pub outcome: Option<TaskOutcome>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskOutcome {
    Finished,
    Stalled,
    Failed,
}

// Só a escrita existe por enquanto: nenhuma tela lê o arquivo, então os campos ficariam sem leitor.
#[allow(dead_code)]
pub struct TaskRecord {
    pub objetivo: String,
    pub participants: Vec<Agent>,
    pub variables: Vec<(String, VarValue)>,
    pub outcome: TaskOutcome,
}

pub fn agent_named(name: &str) -> Option<Agent> {
    Agent::ALL
        .into_iter()
        .find(|agent| agent.name().eq_ignore_ascii_case(name))
}

pub fn parse_definition(reply: &str, origin: Agent, request: &str) -> TaskDefinition {
    definition_from_json(reply, origin).unwrap_or_else(|| safe_definition(origin, request))
}

fn safe_definition(origin: Agent, request: &str) -> TaskDefinition {
    TaskDefinition {
        nova_tarefa: false,
        objetivo: request.to_string(),
        participants: vec![origin],
        alternate: false,
        variables: Vec::new(),
        stop: None,
    }
}

fn definition_from_json(reply: &str, origin: Agent) -> Option<TaskDefinition> {
    let start = reply.find('{')?;
    let end = reply.rfind('}')?;
    let json: Json = serde_json::from_str(reply.get(start..=end)?).ok()?;
    let objetivo = json.get("objetivo")?.as_str()?.trim();
    if objetivo.is_empty() {
        return None;
    }
    let variables = variables_from_json(json.get("variaveis")?.as_array()?);
    let participants = participants_from_json(json.get("participantes")?.as_array()?, origin);
    let stop = stop_from_json(json.get("parada"), &variables);
    Some(TaskDefinition {
        nova_tarefa: json.get("nova_tarefa")?.as_bool()?,
        objetivo: objetivo.to_string(),
        participants,
        alternate: json.get("alternar")?.as_bool()?,
        variables,
        stop,
    })
}

fn participants_from_json(names: &[Json], origin: Agent) -> Vec<Agent> {
    let mut participants: Vec<Agent> = Vec::new();
    for agent in names
        .iter()
        .filter_map(|name| name.as_str().and_then(agent_named))
    {
        if !participants.contains(&agent) {
            participants.push(agent);
        }
    }
    if let Some(index) = participants.iter().position(|agent| *agent == origin) {
        let agent = participants.remove(index);
        participants.insert(0, agent);
    }
    if participants.is_empty() {
        participants.push(origin);
    }
    participants
}

fn variables_from_json(values: &[Json]) -> Vec<Variable> {
    let mut variables: Vec<Variable> = Vec::new();
    for variable in values.iter().filter_map(variable_from_json) {
        if variables
            .iter()
            .all(|existing| existing.name != variable.name)
        {
            variables.push(variable);
        }
    }
    variables
}

fn variable_from_json(json: &Json) -> Option<Variable> {
    let name = json.get("nome")?.as_str()?;
    let rule = match json.get("regra")?.as_str()? {
        FREE_RULE => Rule::Free,
        INCREMENT_RULE => Rule::Increment,
        READ_ONLY_RULE => Rule::ReadOnly,
        _ => return None,
    };
    let value = value_from_json(json.get("tipo")?.as_str()?, json.get("valor")?)?;
    let valid = name != CONVERSATION_VAR
        && validate_name(name).is_ok()
        && validate_value(&value).is_ok()
        && (rule != Rule::Increment || matches!(value, VarValue::Number(_)));
    valid.then(|| Variable::system(name.to_string(), value, rule))
}

fn value_from_json(kind: &str, value: &Json) -> Option<VarValue> {
    match kind {
        NUMBER_TYPE => value.as_i64().map(VarValue::Number),
        TEXT_TYPE => value.as_str().map(|text| VarValue::Text(text.to_string())),
        LIST_TYPE => value
            .as_array()?
            .iter()
            .map(|item| item.as_str().map(str::to_string))
            .collect::<Option<Vec<String>>>()
            .map(VarValue::List),
        _ => None,
    }
}

fn stop_from_json(json: Option<&Json>, variables: &[Variable]) -> Option<StopCondition> {
    let json = json?;
    let variable = json.get("variavel")?.as_str()?;
    if !is_number_variable(variables, variable) {
        return None;
    }
    let limit = match json.get("alcanca")? {
        Json::String(name) => {
            if !is_number_variable(variables, name) {
                return None;
            }
            StopLimit::Variable(name.clone())
        }
        other => StopLimit::Value(other.as_i64()?),
    };
    Some(StopCondition {
        variable: variable.to_string(),
        limit,
    })
}

fn is_number_variable(variables: &[Variable], name: &str) -> bool {
    variables
        .iter()
        .any(|variable| variable.name == name && matches!(variable.value, VarValue::Number(_)))
}
