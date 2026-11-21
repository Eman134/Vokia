use serde::Deserialize;
use tauri::{AppHandle, Manager};

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
const POST_FAILED_MESSAGE: &str =
    "Não foi possível gravar a mensagem agora. Tente de novo mais tarde.";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListChannelsArgs {
    pub project_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateChannelArgs {
    pub project_id: i64,
    pub name: String,
    pub members: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateChannelArgs {
    pub project_id: i64,
    pub id: i64,
    pub name: String,
    pub members: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteChannelArgs {
    pub project_id: i64,
    pub id: i64,
}

pub(crate) fn list_channels_exec(
    app: &AppHandle,
    args: ListChannelsArgs,
) -> Result<Vec<Channel>, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.list_channels(args.project_id)
        .map_err(|error| error.to_string())
}

pub(crate) fn create_channel_exec(
    app: &AppHandle,
    args: CreateChannelArgs,
) -> Result<Channel, String> {
    let team = app
        .state::<AgentRegistry>()
        .team()
        .map_err(|error| error.to_string())?;
    let (name, members) = normalize_channel(&args.name, &args.members, |id| {
        Agent::from_stored(id).is_some_and(|agent| team.profile(&agent).is_some())
    })?;
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.create_channel(args.project_id, &name, &members)
        .map_err(|error| error.to_string())
}

pub(crate) fn update_channel_exec(
    app: &AppHandle,
    args: UpdateChannelArgs,
) -> Result<Channel, String> {
    let team = app
        .state::<AgentRegistry>()
        .team()
        .map_err(|error| error.to_string())?;
    let (name, members) = normalize_channel(&args.name, &args.members, |id| {
        Agent::from_stored(id).is_some_and(|agent| team.profile(&agent).is_some())
    })?;
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.update_channel(args.project_id, args.id, &name, &members)
        .map_err(|error| error.to_string())
}

pub(crate) fn delete_channel_exec(app: &AppHandle, args: DeleteChannelArgs) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.delete_channel(args.project_id, args.id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_channels(app: AppHandle, project_id: i64) -> Result<Vec<Channel>, String> {
    list_channels_exec(&app, ListChannelsArgs { project_id })
}

#[tauri::command]
pub fn create_channel(
    app: AppHandle,
    project_id: i64,
    name: String,
    members: Vec<String>,
) -> Result<Channel, String> {
    create_channel_exec(
        &app,
        CreateChannelArgs {
            project_id,
            name,
            members,
        },
    )
}

#[tauri::command]
pub fn update_channel(
    app: AppHandle,
    project_id: i64,
    id: i64,
    name: String,
    members: Vec<String>,
) -> Result<Channel, String> {
    update_channel_exec(
        &app,
        UpdateChannelArgs {
            project_id,
            id,
            name,
            members,
        },
    )
}

#[tauri::command]
pub fn delete_channel(app: AppHandle, project_id: i64, id: i64) -> Result<(), String> {
    delete_channel_exec(&app, DeleteChannelArgs { project_id, id })
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
    let db = state
        .require()
        .map_err(|_| POST_FAILED_MESSAGE.to_string())?;
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
