use tauri::State;

use crate::agents::{Agent, AgentRegistry};
use crate::db::{Channel, DbState};

const GENERAL_CHANNEL: &str = "geral";
const NAME_MIN_CHARS: usize = 2;
const NAME_MAX_CHARS: usize = 32;
const MIN_MEMBERS: usize = 2;
const INVALID_NAME_MESSAGE: &str =
    "Nome de canal inválido: use de 2 a 32 letras minúsculas, números ou hífen.";
const GENERAL_TAKEN_MESSAGE: &str = "O canal #geral já existe.";
const TOO_FEW_MEMBERS_MESSAGE: &str = "Um canal precisa de pelo menos 2 agentes.";

#[tauri::command]
pub fn list_channels(state: State<'_, DbState>, project_id: i64) -> Result<Vec<Channel>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.list_channels(project_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_channel(
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    project_id: i64,
    name: String,
    members: Vec<String>,
) -> Result<Channel, String> {
    let team = registry.team().map_err(|error| error.to_string())?;
    let (name, members) = normalize_channel(&name, &members, |id| {
        Agent::from_stored(id).is_some_and(|agent| team.profile(&agent).is_some())
    })?;
    let db = state.require().map_err(|error| error.to_string())?;
    db.create_channel(project_id, &name, &members)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_channel(state: State<'_, DbState>, project_id: i64, id: i64) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.delete_channel(project_id, id)
        .map_err(|error| error.to_string())
}

pub(crate) fn normalize_channel(
    name: &str,
    members: &[String],
    is_agent: impl Fn(&str) -> bool,
) -> Result<(String, Vec<String>), String> {
    let name = normalize_name(name)?;
    let mut distinct: Vec<String> = Vec::with_capacity(members.len());
    for id in members {
        if !is_agent(id) {
            return Err(format!("Agente desconhecido: {id}."));
        }
        if !distinct.contains(id) {
            distinct.push(id.clone());
        }
    }
    if distinct.len() < MIN_MEMBERS {
        return Err(TOO_FEW_MEMBERS_MESSAGE.to_string());
    }
    Ok((name, distinct))
}

fn normalize_name(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    let name = trimmed.strip_prefix('#').unwrap_or(trimmed).to_lowercase();
    let length = name.chars().count();
    let valid_chars = name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !(NAME_MIN_CHARS..=NAME_MAX_CHARS).contains(&length) || !valid_chars || name.starts_with('-')
    {
        return Err(INVALID_NAME_MESSAGE.to_string());
    }
    if name == GENERAL_CHANNEL {
        return Err(GENERAL_TAKEN_MESSAGE.to_string());
    }
    Ok(name)
}

#[cfg(test)]
#[path = "../../tests/commands_channels_tests.rs"]
mod tests;
