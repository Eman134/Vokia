use serde::Serialize;

use crate::events::Agent;
use crate::task::{ActiveTask, TaskOutcome, TaskRecord};

pub const VAR_NAME_MAX: usize = 32;
pub const VAR_VALUE_MAX: usize = 500;
pub const LIST_MAX_ITEMS: usize = 50;
pub const CACHE_MAX_VARS: usize = 20;
pub const CONVERSATION_ITEMS: usize = 6;
pub const CONVERSATION_VAR: &str = "conversa";
pub const CONVERSATION_ITEM_MAX_CHARS: usize = 300;
pub const RECOVERY_CONVERSATION_ITEMS: usize = 3;
const MARKER: char = '$';
const NAME_DELIMITERS: [char; 3] = ['=', '+', '@'];
const ASSIGN_MARK: char = '=';
const APPEND_MARK: &str = "+=";
const SHARE_MARK: &str = "@time";

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum VarValue {
    Number(i64),
    Text(String),
    List(Vec<String>),
}

impl VarValue {
    fn from_text(text: &str) -> Self {
        text.parse()
            .map_or_else(|_| VarValue::Text(text.to_string()), VarValue::Number)
    }

    pub fn type_label(&self) -> &'static str {
        match self {
            VarValue::Number(_) => "número",
            VarValue::Text(_) => "texto",
            VarValue::List(_) => "lista",
        }
    }

    pub fn display(&self) -> String {
        match self {
            VarValue::Number(number) => number.to_string(),
            VarValue::Text(text) => text.clone(),
            VarValue::List(items) => items.join(", "),
        }
    }

    pub fn as_number(&self) -> Option<i64> {
        match self {
            VarValue::Number(number) => Some(*number),
            _ => None,
        }
    }

    fn same_kind(&self, other: &VarValue) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Private,
    Team,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    Free,
    Increment,
    Append,
    ReadOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Creator {
    System,
    Agent(Agent),
}

#[derive(Clone, Debug)]
pub struct Variable {
    pub name: String,
    pub value: VarValue,
    pub scope: Scope,
    pub rule: Rule,
    pub creator: Creator,
}

impl Variable {
    pub fn system(name: String, value: VarValue, rule: Rule) -> Self {
        Self {
            name,
            value,
            scope: Scope::Team,
            rule,
            creator: Creator::System,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Change {
    Set {
        name: String,
        value: VarValue,
        share: bool,
    },
    Append {
        name: String,
        item: String,
    },
    Share {
        name: String,
    },
}

impl Change {
    pub fn feed(&self) -> Option<String> {
        match self {
            Change::Set { value, .. } => Some(value.display()),
            Change::Append { item, .. } => Some(item.clone()),
            Change::Share { .. } => None,
        }
    }
}

#[derive(Default)]
pub struct Applied {
    pub accepted: Vec<Change>,
    pub rejected: Vec<String>,
}

#[derive(Default)]
pub struct Cache {
    variables: Vec<Variable>,
    task: Option<ActiveTask>,
}

impl Cache {
    pub fn names(&self) -> Vec<String> {
        self.variables
            .iter()
            .map(|variable| variable.name.clone())
            .collect()
    }

    pub fn number(&self, name: &str) -> Option<i64> {
        self.find(name)?.value.as_number()
    }

    pub fn visible_to(&self, agent: Agent) -> Vec<&Variable> {
        self.variables
            .iter()
            .filter(|variable| {
                variable.scope == Scope::Team || variable.creator == Creator::Agent(agent)
            })
            .collect()
    }

    pub fn long_request(&self) -> Option<&str> {
        self.task.as_ref()?.long_request.as_deref()
    }

    pub fn recent_conversation(&self, count: usize) -> Vec<String> {
        let Some(VarValue::List(items)) =
            self.find(CONVERSATION_VAR).map(|variable| &variable.value)
        else {
            return Vec::new();
        };
        let skip = items.len().saturating_sub(count);
        items[skip..]
            .iter()
            .map(|item| truncate(item, CONVERSATION_ITEM_MAX_CHARS))
            .collect()
    }

    pub fn start_task(&mut self, task: ActiveTask, variables: Vec<Variable>) -> Result<(), String> {
        let new: Vec<Variable> = variables
            .into_iter()
            .filter(|variable| self.find(&variable.name).is_none())
            .collect();
        let conversation_missing = self.find(CONVERSATION_VAR).is_none();
        self.ensure_room_for(new.len() + usize::from(conversation_missing))?;
        if conversation_missing {
            self.variables.push(Variable::system(
                CONVERSATION_VAR.to_string(),
                VarValue::List(Vec::new()),
                Rule::ReadOnly,
            ));
        }
        self.variables.extend(new);
        self.task = Some(task);
        Ok(())
    }

    pub fn archive(&mut self) -> Option<TaskRecord> {
        let record = self.task.take().map(|task| TaskRecord {
            objetivo: task.objetivo,
            participants: task.participants,
            variables: self
                .variables
                .iter()
                .filter(|variable| variable.scope == Scope::Team)
                .map(|variable| (variable.name.clone(), variable.value.clone()))
                .collect(),
            outcome: task.outcome.unwrap_or(TaskOutcome::Failed),
        });
        self.variables.clear();
        record
    }

    pub fn finish_task(&mut self, outcome: TaskOutcome) {
        if let Some(task) = self.task.as_mut() {
            task.outcome = Some(outcome);
        }
    }

    pub fn push_conversation(&mut self, lines: Vec<String>) {
        let Some(VarValue::List(items)) = self
            .find_mut(CONVERSATION_VAR)
            .map(|variable| &mut variable.value)
        else {
            return;
        };
        items.extend(lines.into_iter().map(|line| truncate(&line, VAR_VALUE_MAX)));
        let excess = items.len().saturating_sub(CONVERSATION_ITEMS);
        items.drain(..excess);
    }

    pub fn apply_reply(&mut self, author: Agent, updates: Vec<Result<Change, String>>) -> Applied {
        let backup = self.variables.clone();
        let mut applied = Applied::default();
        for update in updates {
            match update.and_then(|change| self.apply(author, &change).map(|()| change)) {
                Ok(change) => applied.accepted.push(change),
                Err(motive) => applied.rejected.push(motive),
            }
        }
        if !applied.rejected.is_empty() {
            self.variables = backup;
            applied.accepted.clear();
        }
        applied
    }

    fn apply(&mut self, author: Agent, change: &Change) -> Result<(), String> {
        match change {
            Change::Set { name, value, share } => self.set(author, name, value, *share),
            Change::Append { name, item } => self.append(author, name, item),
            Change::Share { name } => self.share(author, name),
        }
    }

    fn set(
        &mut self,
        author: Agent,
        name: &str,
        value: &VarValue,
        share: bool,
    ) -> Result<(), String> {
        validate_value(value)?;
        let Some(variable) = self.find_mut(name) else {
            validate_name(name)?;
            self.ensure_room_for(1)?;
            self.variables.push(Variable {
                name: name.to_string(),
                value: value.clone(),
                scope: if share { Scope::Team } else { Scope::Private },
                rule: Rule::Free,
                creator: Creator::Agent(author),
            });
            return Ok(());
        };
        ensure_visible(variable, author)?;
        if variable.rule == Rule::ReadOnly {
            return Err(format!("a variável {name} é somente leitura"));
        }
        if variable.rule == Rule::Append {
            return Err(format!("a variável {name} é uma lista: use {name} += item"));
        }
        if !variable.value.same_kind(value) {
            return Err(format!(
                "a variável {name} é {}; o valor enviado é {}",
                variable.value.type_label(),
                value.type_label()
            ));
        }
        if variable.rule == Rule::Increment {
            let current = variable.value.as_number().unwrap_or_default();
            let expected = current.saturating_add(1);
            if value.as_number() != Some(expected) {
                return Err(format!(
                    "a variável {name} vale {current}; o próximo valor é {expected}"
                ));
            }
        }
        if share && variable.creator != Creator::Agent(author) {
            return Err(format!("só quem criou {name} pode compartilhá-la"));
        }
        variable.value = value.clone();
        if share {
            variable.scope = Scope::Team;
        }
        Ok(())
    }

    fn append(&mut self, author: Agent, name: &str, item: &str) -> Result<(), String> {
        check_text(item)?;
        let Some(variable) = self.find_mut(name) else {
            validate_name(name)?;
            self.ensure_room_for(1)?;
            self.variables.push(Variable {
                name: name.to_string(),
                value: VarValue::List(vec![item.to_string()]),
                scope: Scope::Private,
                rule: Rule::Append,
                creator: Creator::Agent(author),
            });
            return Ok(());
        };
        ensure_visible(variable, author)?;
        if variable.rule == Rule::ReadOnly {
            return Err(format!("a variável {name} é somente leitura"));
        }
        let items = match &mut variable.value {
            VarValue::List(items) => items,
            other => {
                return Err(format!(
                    "a variável {name} é {}; += só vale para listas",
                    other.type_label()
                ))
            }
        };
        if items.len() >= LIST_MAX_ITEMS {
            return Err(format!("a lista {name} aceita até {LIST_MAX_ITEMS} itens"));
        }
        items.push(item.to_string());
        Ok(())
    }

    fn share(&mut self, author: Agent, name: &str) -> Result<(), String> {
        let Some(variable) = self.find_mut(name) else {
            return Err(format!("a variável {name} não existe"));
        };
        ensure_visible(variable, author)?;
        if variable.creator != Creator::Agent(author) {
            return Err(format!("só quem criou {name} pode compartilhá-la"));
        }
        variable.scope = Scope::Team;
        Ok(())
    }

    fn find(&self, name: &str) -> Option<&Variable> {
        self.variables.iter().find(|variable| variable.name == name)
    }

    fn find_mut(&mut self, name: &str) -> Option<&mut Variable> {
        self.variables
            .iter_mut()
            .find(|variable| variable.name == name)
    }

    fn ensure_room_for(&self, extra: usize) -> Result<(), String> {
        if self.variables.len() + extra <= CACHE_MAX_VARS {
            Ok(())
        } else {
            Err(format!("o cache aceita até {CACHE_MAX_VARS} variáveis"))
        }
    }
}

fn ensure_visible(variable: &Variable, author: Agent) -> Result<(), String> {
    match variable.creator {
        Creator::Agent(owner) if variable.scope == Scope::Private && owner != author => Err(
            format!("a variável {} é privada de {}", variable.name, owner.name()),
        ),
        _ => Ok(()),
    }
}

pub fn validate_name(name: &str) -> Result<(), String> {
    let starts_with_letter = name.chars().next().is_some_and(|c| c.is_ascii_lowercase());
    let valid = starts_with_letter
        && name.len() <= VAR_NAME_MAX
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if valid {
        Ok(())
    } else {
        Err(format!("nome de variável inválido: {name}"))
    }
}

pub fn validate_value(value: &VarValue) -> Result<(), String> {
    match value {
        VarValue::Number(_) => Ok(()),
        VarValue::Text(text) => check_text(text),
        VarValue::List(items) => {
            if items.len() > LIST_MAX_ITEMS {
                return Err(format!("a lista aceita até {LIST_MAX_ITEMS} itens"));
            }
            items.iter().try_for_each(|item| check_text(item))
        }
    }
}

fn truncate(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn check_text(text: &str) -> Result<(), String> {
    if text.chars().count() <= VAR_VALUE_MAX {
        Ok(())
    } else {
        Err(format!("o valor passa de {VAR_VALUE_MAX} caracteres"))
    }
}

pub fn parse_update_line(line: &str) -> Option<Result<Change, String>> {
    let body = line.trim().strip_prefix(MARKER)?;
    let name_end = body
        .find(|c: char| c.is_whitespace() || NAME_DELIMITERS.contains(&c))
        .unwrap_or(body.len());
    let (name, action) = body.split_at(name_end);
    Some(parse_action(name, action.trim()))
}

fn parse_action(name: &str, action: &str) -> Result<Change, String> {
    if name.is_empty() {
        return Err("marcador sem nome de variável".to_string());
    }
    if action == SHARE_MARK {
        return Ok(Change::Share {
            name: name.to_string(),
        });
    }
    if let Some(item) = action.strip_prefix(APPEND_MARK) {
        let item = item.trim();
        if item.is_empty() {
            return Err(format!("{name} += sem item"));
        }
        return Ok(Change::Append {
            name: name.to_string(),
            item: item.to_string(),
        });
    }
    if let Some(value) = action.strip_prefix(ASSIGN_MARK) {
        let (text, share) = split_share(value.trim())?;
        return Ok(Change::Set {
            name: name.to_string(),
            value: VarValue::from_text(text),
            share,
        });
    }
    Err(format!("marcador inválido para {name}: {action}"))
}

fn split_share(value: &str) -> Result<(&str, bool), String> {
    let (text, share) = match value.strip_suffix(SHARE_MARK) {
        Some(rest) if rest.is_empty() || rest.ends_with(char::is_whitespace) => {
            (rest.trim_end(), true)
        }
        _ => (value, false),
    };
    if text.is_empty() {
        return Err("valor vazio".to_string());
    }
    match text.split_whitespace().last() {
        Some(word) if word.starts_with('@') => Err(format!("marcador desconhecido: {word}")),
        _ => Ok((text, share)),
    }
}
