use tauri::{AppHandle, State};

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

#[tauri::command]
pub fn list_agents(
    registry: State<'_, AgentRegistry>,
    providers: State<'_, ProviderRegistry>,
) -> Result<TeamListing, String> {
    registry.listing(&providers).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn available_providers(providers: State<'_, ProviderRegistry>) -> Vec<String> {
    providers
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

#[tauri::command]
pub fn create_agent(
    registry: State<'_, AgentRegistry>,
    providers: State<'_, ProviderRegistry>,
    roles: State<'_, RoleRegistry>,
    draft: AgentDraft,
) -> Result<AgentProfile, String> {
    roles
        .ensure_exists(&draft.role)
        .map_err(|error| error.to_string())?;
    registry
        .create(&draft, &providers)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_agent(
    registry: State<'_, AgentRegistry>,
    providers: State<'_, ProviderRegistry>,
    roles: State<'_, RoleRegistry>,
    id: Agent,
    draft: AgentDraft,
) -> Result<AgentProfile, String> {
    roles
        .ensure_exists(&draft.role)
        .map_err(|error| error.to_string())?;
    registry
        .update(&id, &draft, &providers)
        .map_err(|error| error.to_string())
}

// O Ember assume os cards do agente excluído; o aviso de cada troca entra no feed do projeto.
#[tauri::command]
pub fn delete_agent(
    app: AppHandle,
    registry: State<'_, AgentRegistry>,
    state: State<'_, DbState>,
    id: Agent,
) -> Result<(), String> {
    let team = registry.team().map_err(|error| error.to_string())?;
    let agent_name = team.name_of(&id).to_string();
    let successor = Agent::ember();
    let successor_name = team.name_of(&successor).to_string();
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
            &app,
            note.project_id,
            Speaker::Agent(successor.clone()),
            &note.text,
            time,
        );
        let _ = emit_backlog_changed(&app, note.project_id);
    }
    Ok(())
}

#[tauri::command]
pub async fn set_agent_instructions(
    registry: State<'_, AgentRegistry>,
    app_state: State<'_, AppState>,
    id: Agent,
    text: String,
) -> Result<AgentProfile, String> {
    let profile = registry
        .set_instructions(&id, &text)
        .map_err(|error| error.to_string())?;
    // Só depois de gravada: a sessão que reabrir já lê o prompt com o texto novo.
    app_state.sessions.retire_agent(&id).await;
    Ok(profile)
}

#[tauri::command]
pub fn list_temporary_overrides(app_state: State<'_, AppState>) -> Vec<TemporaryOverride> {
    let now = now_millis_u64();
    let mut map = app_state
        .temporary_overrides
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    map.retain(|_, ov| ov.expires_at.is_none_or(|exp| exp > now));
    map.values().cloned().collect()
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn set_temporary_override(
    app: AppHandle,
    registry: State<'_, AgentRegistry>,
    providers: State<'_, ProviderRegistry>,
    app_state: State<'_, AppState>,
    agent: Agent,
    cli: String,
    model_tier: Option<ModelTier>,
    model: Option<String>,
    reason: Option<OverrideReason>,
    expires_in_ms: Option<u64>,
) -> Result<TemporaryOverride, String> {
    let team = registry.team().map_err(|error| error.to_string())?;
    let profile = team
        .profile(&agent)
        .ok_or_else(|| format!("Agente desconhecido: {agent}"))?;
    let provider = providers.get(&cli).map_err(|error| error.to_string())?;
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
    {
        let mut map = app_state
            .temporary_overrides
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.insert(agent.clone(), ov.clone());
    }
    app_state.sessions.retire_agent(&agent).await;
    emit_overrides_changed(&app, &app_state);
    Ok(ov)
}

#[tauri::command]
pub async fn clear_temporary_override(
    app: AppHandle,
    app_state: State<'_, AppState>,
    agent: Agent,
) -> Result<(), String> {
    let removed = {
        let mut map = app_state
            .temporary_overrides
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.remove(&agent).is_some()
    };
    if removed {
        app_state.sessions.retire_agent(&agent).await;
        emit_overrides_changed(&app, &app_state);
    }
    Ok(())
}

#[tauri::command]
pub async fn clear_all_temporary_overrides(
    app: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<(), String> {
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
        emit_overrides_changed(&app, &app_state);
    }
    Ok(())
}

#[tauri::command]
pub async fn update_ember_llm(
    registry: State<'_, AgentRegistry>,
    providers: State<'_, ProviderRegistry>,
    app_state: State<'_, AppState>,
    cli: Option<String>,
    model_tier: Option<ModelTier>,
    model: Option<String>,
    fallback: Option<Vec<LlmTarget>>,
) -> Result<AgentProfile, String> {
    let profile = registry
        .update_ember_llm(
            cli,
            model_tier,
            model,
            fallback.unwrap_or_default(),
            &providers,
        )
        .map_err(|error| error.to_string())?;
    app_state.sessions.retire_agent(&Agent::ember()).await;
    Ok(profile)
}
