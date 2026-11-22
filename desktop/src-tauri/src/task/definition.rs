use serde::Serialize;
use serde_json::Value as Json;

use crate::agents::{Agent, Team};

use super::cache::{
    truncate, validate_name, validate_value, Cache, Rule, VarValue, Variable, CONVERSATION_VAR,
};

const NUMBER_TYPE: &str = "numero";
const TEXT_TYPE: &str = "texto";
const LIST_TYPE: &str = "lista";
const FREE_RULE: &str = "livre";
const INCREMENT_RULE: &str = "mais_um";
const READ_ONLY_RULE: &str = "so_leitura";
const EXISTS_LIMIT: &str = "existe";
const MULTI_AGENT_MIN_PARTICIPANTS: usize = 2;
const LONG_REQUEST_CHARS: usize = 1200;
const REQUEST_PROMPT_MAX: usize = 8000;
const REQUEST_TRUNCATED: &str = " (pedido truncado)";
const CRITERION_MAX_CHARS: usize = 300;
const DEMAND_PREFIXES: &[&str] = &["/plano", "/revisar"];
const CONVERSATION_GREETINGS: &[&str] = &[
    "oi", "olá", "ola", "bom dia", "boa tarde", "boa noite", "e ai", "e aí", "tudo bem", "obrigado", "obrigada",
    "valeu", "ok", "entendi", "beleza", "teste",
];
const DEMAND_VERBS: &[&str] = &[
    "crie", "criar", "implemente", "implementar", "corrija", "corrigir", "arrume", "arrumar", "refatore",
    "refatorar", "adicione", "adicionar", "remova", "remover", "altere", "alterar", "edite", "editar", "atualize",
    "atualizar", "planeje", "planejar", "investigue", "investigar", "pesquise", "pesquisar", "revise", "revisar",
    "execute", "executar", "rode", "rodar", "teste", "testar", "commite", "commit", "gere", "gerar", "construa",
    "construir", "faça", "faca", "card", "backlog", "pr", "pull request", "issue", "bug", "branch", "worktree",
];
// Pedidos com revezamento, variáveis ou condição de parada precisam do interpretador; o resto a definição local resolve.
const INTERPRETER_TRIGGERS: &[&str] = &[
    "alternadamente", "revezando", "um de cada vez", "variável", "variavel", "variáveis", "variaveis",
    "pronto quando", "até alcançar", "ate alcancar", "condição de parada", "condicao de parada",
];
const VARIABLE_SIGIL: char = '$';
const MENTION_PREFIX: char = '@';
const KIND_FIELD: &str = "tipo";
const DEMAND_KIND_VALUE: &str = "demanda";
const CONVERSATION_KIND_VALUE: &str = "conversa";

// Conversa (saudação, pergunta direta) termina sem cartão de conclusão; demanda é trabalho de verdade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskKind {
    #[default]
    Conversa,
    Demanda,
}

#[derive(Debug)]
pub struct TaskDefinition {
    pub kind: TaskKind,
    pub nova_tarefa: bool,
    pub objetivo: String,
    pub criterion: String,
    pub verify: bool,
    pub participants: Vec<Agent>,
    pub alternate: bool,
    pub variables: Vec<Variable>,
    pub stop: Option<StopCondition>,
}

impl TaskDefinition {
    // Alternância, verificação, parada, variáveis ou mais de um agente só existem em trabalho de verdade.
    pub fn has_task_structure(&self) -> bool {
        self.alternate
            || self.verify
            || self.stop.is_some()
            || !self.variables.is_empty()
            || self.participants.len() >= MULTI_AGENT_MIN_PARTICIPANTS
    }

    pub fn promote_to_demand(&mut self) {
        self.kind = TaskKind::Demanda;
    }
}

pub fn classify_request(request: &str) -> TaskKind {
    let lower = request.trim().to_lowercase();
    if lower.is_empty() {
        return TaskKind::Conversa;
    }
    if DEMAND_PREFIXES.iter().any(|prefix| lower.starts_with(prefix)) {
        return TaskKind::Demanda;
    }
    let clean = lower.trim_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace());
    if CONVERSATION_GREETINGS.contains(&clean) {
        return TaskKind::Conversa;
    }
    let asks_for_work = DEMAND_VERBS.iter().any(|keyword| {
        lower
            .split(|c: char| !c.is_alphanumeric() && c != '/')
            .any(|word| word == *keyword)
            || (keyword.contains(' ') && lower.contains(keyword))
    });
    if asks_for_work {
        TaskKind::Demanda
    } else {
        TaskKind::Conversa
    }
}

// Estrutura de tarefa sempre vence; senão vale o "tipo" que o interpretador escreveu, e por fim a heurística do pedido.
fn kind_of(definition: &TaskDefinition, declared: Option<&str>, request: &str) -> TaskKind {
    if definition.has_task_structure() {
        return TaskKind::Demanda;
    }
    match declared {
        Some(DEMAND_KIND_VALUE) => TaskKind::Demanda,
        Some(CONVERSATION_KIND_VALUE) => TaskKind::Conversa,
        _ => classify_request(request),
    }
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
    Exists,
}

impl StopCondition {
    pub fn reached(&self, cache: &Cache) -> bool {
        match &self.limit {
            StopLimit::Exists => cache.exists(&self.variable),
            StopLimit::Value(value) => cache
                .number(&self.variable)
                .is_some_and(|current| current >= *value),
            StopLimit::Variable(name) => match (cache.number(&self.variable), cache.number(name)) {
                (Some(current), Some(limit)) => current >= limit,
                _ => false,
            },
        }
    }
}

#[derive(Debug)]
pub struct ActiveTask {
    pub objetivo: String,
    pub participants: Vec<Agent>,
    pub long_request: Option<String>,
    pub outcome: Option<TaskOutcome>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum TaskOutcome {
    Finished,
    Stalled,
    Failed,
    Paused,
}

#[derive(Serialize)]
pub struct TaskRecord {
    pub objetivo: String,
    pub participants: Vec<Agent>,
    pub variables: Vec<(String, VarValue)>,
    pub outcome: TaskOutcome,
}

fn is_long_request(request: &str) -> bool {
    request.chars().count() > LONG_REQUEST_CHARS
}

pub fn long_request_text(request: &str) -> Option<String> {
    if !is_long_request(request) {
        return None;
    }
    if request.chars().count() <= REQUEST_PROMPT_MAX {
        return Some(request.to_string());
    }
    let head: String = request.chars().take(REQUEST_PROMPT_MAX).collect();
    Some(format!("{head}{REQUEST_TRUNCATED}"))
}

pub fn parse_definition(reply: &str, origin: &Agent, request: &str, team: &Team) -> TaskDefinition {
    definition_from_json(reply, origin, team, request)
        .unwrap_or_else(|| safe_definition(origin, request))
}

pub fn needs_llm_interpreter(request: &str, has_active_variables: bool) -> bool {
    if has_active_variables {
        return true;
    }
    let trimmed = request.trim();
    if trimmed.contains(VARIABLE_SIGIL) {
        return true;
    }
    let lower = trimmed.to_lowercase();
    INTERPRETER_TRIGGERS.iter().any(|trigger| lower.contains(trigger))
}

// Definição sem chamada ao modelo: o agente do pedido e quem o pedido cita com @Nome.
pub fn fast_definition(origin: &Agent, request: &str, team: &Team, nova_tarefa: bool) -> TaskDefinition {
    let trimmed = request.trim();
    let objective = if trimmed.is_empty() {
        request.to_string()
    } else {
        truncate(trimmed, CRITERION_MAX_CHARS)
    };
    let mut participants = vec![origin.clone()];
    for mentioned in mentioned_agents(trimmed, team) {
        if !participants.contains(&mentioned) {
            participants.push(mentioned);
        }
    }
    let mut definition = TaskDefinition {
        kind: TaskKind::Conversa,
        nova_tarefa,
        criterion: objective.clone(),
        objetivo: objective,
        verify: false,
        participants,
        alternate: false,
        variables: Vec::new(),
        stop: None,
    };
    definition.kind = kind_of(&definition, None, request);
    definition
}

fn mentioned_agents(request: &str, team: &Team) -> Vec<Agent> {
    request
        .split_whitespace()
        .filter_map(|token| token.strip_prefix(MENTION_PREFIX))
        .map(|name| name.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|name| !name.is_empty())
        .filter_map(|name| team.named(name))
        .collect()
}

fn safe_definition(origin: &Agent, request: &str) -> TaskDefinition {
    let mut definition = TaskDefinition {
        kind: TaskKind::Conversa,
        nova_tarefa: false,
        objetivo: request.to_string(),
        criterion: request.to_string(),
        verify: false,
        participants: vec![origin.clone()],
        alternate: false,
        variables: Vec::new(),
        stop: None,
    };
    definition.kind = kind_of(&definition, None, request);
    definition
}

fn definition_from_json(
    reply: &str,
    origin: &Agent,
    team: &Team,
    request: &str,
) -> Option<TaskDefinition> {
    let start = reply.find('{')?;
    let end = reply.rfind('}')?;
    let json: Json = serde_json::from_str(reply.get(start..=end)?).ok()?;
    let objetivo = json.get("objetivo")?.as_str()?.trim();
    if objetivo.is_empty() {
        return None;
    }
    let variables = variables_from_json(json.get("variaveis")?.as_array()?);
    let participants = participants_from_json(json.get("participantes")?.as_array()?, origin, team);
    let alternate =
        json.get("alternar")?.as_bool()? && participants.len() >= MULTI_AGENT_MIN_PARTICIPANTS;
    let stop = stop_from_json(json.get("parada"), &variables);
    let criterion = json
        .get("criterio")
        .and_then(Json::as_str)
        .map(str::trim)
        .filter(|criterion| !criterion.is_empty())
        .map_or_else(
            || objetivo.to_string(),
            |criterion| truncate(criterion, CRITERION_MAX_CHARS),
        );
    let verify = json
        .get("verificar")
        .and_then(Json::as_bool)
        .unwrap_or(false);
    let mut definition = TaskDefinition {
        kind: TaskKind::Conversa,
        nova_tarefa: json.get("nova_tarefa")?.as_bool()?,
        objetivo: objetivo.to_string(),
        criterion,
        verify,
        participants,
        alternate,
        variables,
        stop,
    };
    let declared = json.get(KIND_FIELD).and_then(Json::as_str);
    definition.kind = kind_of(&definition, declared, request);
    Some(definition)
}

fn participants_from_json(names: &[Json], origin: &Agent, team: &Team) -> Vec<Agent> {
    let mut participants: Vec<Agent> = Vec::new();
    for agent in names
        .iter()
        .filter_map(|name| name.as_str().and_then(|name| team.named(name)))
    {
        if !participants.contains(&agent) {
            participants.push(agent);
        }
    }
    if let Some(index) = participants.iter().position(|agent| agent == origin) {
        let agent = participants.remove(index);
        participants.insert(0, agent);
    }
    if participants.is_empty() {
        participants.push(origin.clone());
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
    if !has_variable(variables, variable) {
        return None;
    }
    let limit = match json.get("alcanca")? {
        Json::String(name) if name == EXISTS_LIMIT => StopLimit::Exists,
        Json::String(name) => {
            if !is_number_variable(variables, variable) || !is_number_variable(variables, name) {
                return None;
            }
            StopLimit::Variable(name.clone())
        }
        other => {
            if !is_number_variable(variables, variable) {
                return None;
            }
            StopLimit::Value(other.as_i64()?)
        }
    };
    Some(StopCondition {
        variable: variable.to_string(),
        limit,
    })
}

fn has_variable(variables: &[Variable], name: &str) -> bool {
    variables.iter().any(|variable| variable.name == name)
}

fn is_number_variable(variables: &[Variable], name: &str) -> bool {
    variables
        .iter()
        .any(|variable| variable.name == name && matches!(variable.value, VarValue::Number(_)))
}

#[cfg(test)]
#[path = "definition_tests.rs"]
mod tests;
