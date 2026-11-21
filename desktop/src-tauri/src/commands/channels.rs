use tauri::{AppHandle, Manager, State};

use crate::agents::{Agent, AgentRegistry};
use crate::db::{Channel, DbState};
use crate::events::Speaker;
use crate::orchestrator::emit_message;

const GENERAL_CHANNEL: &str = "geral";
const NAME_MIN_CHARS: usize = 2;
const NAME_MAX_CHARS: usize = 32;
const MIN_MEMBERS: usize = 2;
const INVALID_NAME_MESSAGE: &str =
    "Nome de canal inválido: use de 2 a 32 letras minúsculas, números ou hífen.";
const GENERAL_TAKEN_MESSAGE: &str = "O canal #geral já existe.";
const TOO_FEW_MEMBERS_MESSAGE: &str = "Um canal precisa de pelo menos 2 agentes.";
const EMPTY_MESSAGE: &str = "O texto da mensagem não pode ser vazio.";
const NOT_MEMBER_MESSAGE: &str = "Você não é membro deste canal.";
const POST_FAILED_MESSAGE: &str = "Não foi possível gravar a mensagem agora. Tente de novo mais tarde.";

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

// Aceita "#Nome" e "Nome" e devolve o nome do canal como é gravado.
fn plain_name(raw: &str) -> String {
    let trimmed = raw.trim();
    trimmed.strip_prefix('#').unwrap_or(trimmed).to_lowercase()
}

// Agente escreve no canal do projeto como o usuário: a mensagem vai ao feed com o canal; só membros escrevem.
pub(crate) fn post_agent_message(
    app: &AppHandle,
    project_id: i64,
    agent: &Agent,
    channel: &str,
    text: &str,
) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err(EMPTY_MESSAGE.to_string());
    }
    let name = plain_name(channel);
    let state = app.state::<DbState>();
    let db = state.require().map_err(|_| POST_FAILED_MESSAGE.to_string())?;
    let channels = db
        .list_channels(project_id)
        .map_err(|_| POST_FAILED_MESSAGE.to_string())?;
    let found = channels
        .into_iter()
        .find(|candidate| candidate.name == name)
        .ok_or_else(|| format!("O canal #{name} não existe neste projeto."))?;
    if !found.members.iter().any(|member| member == agent.as_str()) {
        return Err(NOT_MEMBER_MESSAGE.to_string());
    }
    emit_message(
        app,
        project_id,
        Speaker::from(agent.clone()),
        Speaker::User,
        text,
        Some(found.id),
        None,
        None,
        None,
    )
    .map_err(|_| POST_FAILED_MESSAGE.to_string())
}

fn normalize_name(raw: &str) -> Result<String, String> {
    let name = plain_name(raw);
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
