use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::agents::{Agent, AgentDraft, AgentProfile, AgentRegistry, TeamListing};
use crate::db::DbState;
use crate::events::Speaker;
use crate::orchestrator::fallback::{
    emit_overrides_changed, now_millis_u64, LlmTarget, OverrideReason, TemporaryOverride,
};
use crate::orchestrator::{emit_backlog_changed, emit_feed_note, now_millis};
use crate::providers::{ModelTier, ProviderRegistry};
use crate::roles::RoleRegistry;
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateAgentArgs {
    pub draft: AgentDraft,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateAgentArgs {
    pub id: Agent,
    pub draft: AgentDraft,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteAgentArgs {
    pub id: Agent,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetAgentInstructionsArgs {
    pub id: Agent,
    pub text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetTemporaryOverrideArgs {
    pub agent: Agent,
    pub cli: String,
    pub model_tier: Option<ModelTier>,
    pub model: Option<String>,
    pub reason: Option<OverrideReason>,
    pub expires_in_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClearTemporaryOverrideArgs {
    pub agent: Agent,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateEmberLlmArgs {
    pub cli: Option<String>,
    pub model_tier: Option<ModelTier>,
    pub model: Option<String>,
    pub fallback: Option<Vec<LlmTarget>>,
}

pub(crate) fn list_agents_exec(app: &AppHandle) -> Result<TeamListing, String> {
    app.state::<AgentRegistry>()
        .listing(&app.state::<ProviderRegistry>())
        .map_err(|error| error.to_string())
}

pub(crate) fn available_providers_exec(app: &AppHandle) -> Vec<String> {
    app.state::<ProviderRegistry>()
        .list()
        .unwrap_or_default()
        .into_iter()
        .filter(|provider| {
            shlex::split(provider.default_command.trim())
                .and_then(|parts| parts.into_iter().next())
                .is_some_and(|bin| which::which(bin).is_ok())
        })
        .map(|provider| provider.id)
        .collect()
}

pub(crate) fn create_agent_exec(
    app: &AppHandle,
    args: CreateAgentArgs,
) -> Result<AgentProfile, String> {
    app.state::<RoleRegistry>()
        .ensure_exists(&args.draft.role)
        .map_err(|error| error.to_string())?;
    app.state::<AgentRegistry>()
        .create(&args.draft, &app.state::<ProviderRegistry>())
        .map_err(|error| error.to_string())
}

pub(crate) async fn update_agent_exec(
    app: &AppHandle,
    args: UpdateAgentArgs,
) -> Result<AgentProfile, String> {
    app.state::<RoleRegistry>()
        .ensure_exists(&args.draft.role)
        .map_err(|error| error.to_string())?;
    let profile = app
        .state::<AgentRegistry>()
        .update(&args.id, &args.draft, &app.state::<ProviderRegistry>())
        .map_err(|error| error.to_string())?;
    // Papel, permissões e skills entram no prompt de sistema; a sessão viva seguiria com o perfil antigo.
    app.state::<AppState>().sessions.retire_agent(&args.id).await;
    Ok(profile)
}

// O Ember assume os cards do agente excluído; o aviso de cada troca entra no feed do projeto.
pub(crate) async fn delete_agent_exec(app: &AppHandle, args: DeleteAgentArgs) -> Result<(), String> {
    let id = args.id;
    let registry = app.state::<AgentRegistry>();
    let team = registry.team().map_err(|error| error.to_string())?;
    let agent_name = team.name_of(&id).to_string();
    let successor = Agent::ember();
    let successor_name = team.name_of(&successor).to_string();
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let time = now_millis().map_err(|error| error.to_string())?;
    let notes = db
        .remove_agent(&id, &agent_name, &successor, &successor_name, time, || {
            registry.remove(&id)
        })
        .map_err(|error| error.to_string())?;
    for note in notes {
        // A troca já está gravada; falha ao avisar o feed não vira erro para quem excluiu.
        let _ = emit_feed_note(
            app,
            note.project_id,
            Speaker::Agent(successor.clone()),
            &note.text,
            time,
        );
        let _ = emit_backlog_changed(app, note.project_id);
    }
    app.state::<AppState>()
        .sessions
        .close_agent(&db, &id)
        .await
        .map_err(|error| error.to_string())
}

pub(crate) async fn set_agent_instructions_exec(
    app: &AppHandle,
    args: SetAgentInstructionsArgs,
) -> Result<AgentProfile, String> {
    let profile = app
        .state::<AgentRegistry>()
        .set_instructions(&args.id, &args.text)
        .map_err(|error| error.to_string())?;
    // Só depois de gravada: a sessão que reabrir já lê o prompt com o texto novo.
    app.state::<AppState>()
        .sessions
        .retire_agent(&args.id)
        .await;
    Ok(profile)
}

pub(crate) fn list_temporary_overrides_exec(app: &AppHandle) -> Vec<TemporaryOverride> {
    let now = now_millis_u64();
    let app_state = app.state::<AppState>();
    let mut map = app_state
        .temporary_overrides
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    map.retain(|_, ov| ov.expires_at.is_none_or(|exp| exp > now));
    map.values().cloned().collect()
}

pub(crate) async fn set_temporary_override_exec(
    app: &AppHandle,
    args: SetTemporaryOverrideArgs,
) -> Result<TemporaryOverride, String> {
    let SetTemporaryOverrideArgs {
        agent,
        cli,
        model_tier,
        model,
        reason,
        expires_in_ms,
    } = args;
    let team = app
        .state::<AgentRegistry>()
        .team()
        .map_err(|error| error.to_string())?;
    let profile = team
        .profile(&agent)
        .ok_or_else(|| format!("Agente desconhecido: {agent}"))?;
    let provider = app
        .state::<ProviderRegistry>()
        .get(&cli)
        .map_err(|error| error.to_string())?;
    if agent == Agent::ember() && !provider.has_tools() {
        return Err(
            "O agente Ember exige um provedor com suporte a ferramentas (MCP ou Vokia CLI)."
                .to_string(),
        );
    }
    let resolved_model = match model {
        Some(m) if !m.trim().is_empty() => m.trim().to_string(),
        _ => model_tier
            .map(|tier| provider.tiers.get(tier).trim().to_string())
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| provider.default_model.clone()),
    };
    let now = now_millis_u64();
    let ov = TemporaryOverride {
        agent: agent.clone(),
        cli,
        model: resolved_model,
        model_tier,
        command: provider.default_command.clone(),
        original_cli: profile.cli.clone(),
        original_model: profile.model.clone(),
        original_tier: profile.model_tier,
        reason: reason.unwrap_or(OverrideReason::ManualTemp),
        applied_at: now,
        expires_at: expires_in_ms.map(|ms| now.saturating_add(ms)),
    };
    let app_state = app.state::<AppState>();
    {
        let mut map = app_state
            .temporary_overrides
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.insert(agent.clone(), ov.clone());
    }
    app_state.sessions.retire_agent(&agent).await;
    emit_overrides_changed(app, &app_state);
    Ok(ov)
}

pub(crate) async fn clear_temporary_override_exec(
    app: &AppHandle,
    args: ClearTemporaryOverrideArgs,
) -> Result<(), String> {
    let app_state = app.state::<AppState>();
    let removed = {
        let mut map = app_state
            .temporary_overrides
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.remove(&args.agent).is_some()
    };
    if removed {
        app_state.sessions.retire_agent(&args.agent).await;
        emit_overrides_changed(app, &app_state);
    }
    Ok(())
}

pub(crate) async fn clear_all_temporary_overrides_exec(app: &AppHandle) -> Result<(), String> {
    let app_state = app.state::<AppState>();
    let agents: Vec<Agent> = {
        let mut map = app_state
            .temporary_overrides
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let keys: Vec<Agent> = map.keys().cloned().collect();
        map.clear();
        keys
    };
    for agent in &agents {
        app_state.sessions.retire_agent(agent).await;
    }
    if !agents.is_empty() {
        emit_overrides_changed(app, &app_state);
    }
    Ok(())
}

pub(crate) async fn update_ember_llm_exec(
    app: &AppHandle,
    args: UpdateEmberLlmArgs,
) -> Result<AgentProfile, String> {
    let profile = app
        .state::<AgentRegistry>()
        .update_ember_llm(
            args.cli,
            args.model_tier,
            args.model,
            args.fallback.unwrap_or_default(),
            &app.state::<ProviderRegistry>(),
        )
        .map_err(|error| error.to_string())?;
    app.state::<AppState>()
        .sessions
        .retire_agent(&Agent::ember())
        .await;
    Ok(profile)
}

#[tauri::command]
pub fn list_agents(app: AppHandle) -> Result<TeamListing, String> {
    list_agents_exec(&app)
}

#[tauri::command]
pub fn available_providers(app: AppHandle) -> Vec<String> {
    available_providers_exec(&app)
}

#[tauri::command]
pub fn create_agent(app: AppHandle, draft: AgentDraft) -> Result<AgentProfile, String> {
    create_agent_exec(&app, CreateAgentArgs { draft })
}

#[tauri::command]
pub async fn update_agent(app: AppHandle, id: Agent, draft: AgentDraft) -> Result<AgentProfile, String> {
    update_agent_exec(&app, UpdateAgentArgs { id, draft }).await
}

#[tauri::command]
pub async fn delete_agent(app: AppHandle, id: Agent) -> Result<(), String> {
    delete_agent_exec(&app, DeleteAgentArgs { id }).await
}

#[tauri::command]
pub async fn set_agent_instructions(
    app: AppHandle,
    id: Agent,
    text: String,
) -> Result<AgentProfile, String> {
    set_agent_instructions_exec(&app, SetAgentInstructionsArgs { id, text }).await
}

#[tauri::command]
pub fn list_temporary_overrides(app: AppHandle) -> Vec<TemporaryOverride> {
    list_temporary_overrides_exec(&app)
}

#[tauri::command]
pub async fn set_temporary_override(
    app: AppHandle,
    agent: Agent,
    cli: String,
    model_tier: Option<ModelTier>,
    model: Option<String>,
    reason: Option<OverrideReason>,
    expires_in_ms: Option<u64>,
) -> Result<TemporaryOverride, String> {
    set_temporary_override_exec(
        &app,
        SetTemporaryOverrideArgs {
            agent,
            cli,
            model_tier,
            model,
            reason,
            expires_in_ms,
        },
    )
    .await
}

#[tauri::command]
pub async fn clear_temporary_override(app: AppHandle, agent: Agent) -> Result<(), String> {
    clear_temporary_override_exec(&app, ClearTemporaryOverrideArgs { agent }).await
}

#[tauri::command]
pub async fn clear_all_temporary_overrides(app: AppHandle) -> Result<(), String> {
    clear_all_temporary_overrides_exec(&app).await
}

#[tauri::command]
pub async fn update_ember_llm(
    app: AppHandle,
    cli: Option<String>,
    model_tier: Option<ModelTier>,
    model: Option<String>,
    fallback: Option<Vec<LlmTarget>>,
) -> Result<AgentProfile, String> {
    update_ember_llm_exec(
        &app,
        UpdateEmberLlmArgs {
            cli,
            model_tier,
            model,
            fallback,
        },
    )
    .await
}
