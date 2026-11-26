use std::error::Error;

use tauri::{App, Manager};

use crate::db::DbState;
use crate::{agents, attachments, mcp, orchestration, provider_auth, provider_env, providers, roles, shell};

pub(crate) fn setup(app: &mut App) -> Result<(), Box<dyn Error>> {
    let data_dir = app.path().app_data_dir()?;
    let providers = providers::ProviderRegistry::load(&data_dir);
    let provider_env = provider_env::ProviderEnvRegistry::load(&data_dir);
    let agents = agents::AgentRegistry::load(&data_dir);
    app.manage(roles::RoleRegistry::load(&data_dir, &agents));
    app.manage(providers);
    app.manage(provider_env);
    app.manage(provider_auth::ProviderLoginManager::default());
    app.manage(agents);
    if let Some(state) = app.try_state::<crate::state::AppState>() {
        let handle = app.handle().clone();
        let codex_command = std::sync::Arc::new(move || {
            handle
                .try_state::<providers::ProviderRegistry>()
                .and_then(|registry| registry.get("codex").ok())
                .map(|provider| provider.default_command)
        });
        crate::usage::readers::register_default_readers(&state.usage, Some(codex_command));
        // Sobrescreve o leitor do Codex para reaproveitar o codex app-server compartilhado quando ativo.
        let handle_rpc = app.handle().clone();
        let shared_codex_reader: crate::usage::OptionalUsageReaderFn =
            std::sync::Arc::new(move || {
                let handle = handle_rpc.clone();
                Box::pin(async move {
                    let now_ms = crate::usage::now_millis();
                    let provider_cfg = handle
                        .try_state::<providers::ProviderRegistry>()
                        .and_then(|registry| registry.get("codex").ok());
                    if let (Some(cfg), Some(app_state)) =
                        (provider_cfg.as_ref(), handle.try_state::<crate::state::AppState>())
                    {
                        if let Some(tokens) = shlex::split(cfg.default_command.trim()) {
                            if let Some((bin, args)) = tokens.split_first() {
                                if let Ok(program) = which::which(bin) {
                                    if let Ok(Some(usage)) = app_state
                                        .sessions
                                        .adapters()
                                        .read_rate_limits(cfg.protocol, &program, args, now_ms)
                                        .await
                                    {
                                        return Ok(Some(usage));
                                    }
                                }
                            }
                        }
                    }
                    let config = crate::usage::readers::codex::CodexReaderConfig {
                        command: provider_cfg.map(|p| p.default_command),
                        ..crate::usage::readers::codex::CodexReaderConfig::default()
                    };
                    crate::usage::readers::codex::read_codex_usage(&config, now_ms).await
                })
            });
        state
            .usage
            .register_optional_reader("codex", shared_codex_reader);
    }
    app.manage(DbState::open(&data_dir));
    {
        let team = app
            .state::<agents::AgentRegistry>()
            .team()
            .map_err(|error| error.to_string())?;
        if let Some(db) = app.state::<DbState>().db() {
            orchestration::seed::fill_missing_leaders(db, &team)
                .map_err(|error| error.to_string())?;
            if let (Ok(threshold), Some(state)) = (
                db.usage_pause_threshold(),
                app.try_state::<crate::state::AppState>(),
            ) {
                state.usage.set_pause_threshold(threshold);
            }
        }
    }
    // Um anexo nunca enviado é sobra de rascunho; falha na limpeza não pode impedir o app de abrir.
    if let Some(db) = app.state::<DbState>().db() {
        let _ = attachments::purge_unsent(db, &data_dir);
    }
    mcp::clear_stale_configs(&data_dir);
    app.manage(mcp::McpState::start(app.handle().clone()));
    tauri::async_runtime::spawn(crate::orchestrator::restore_flow_queue(
        app.handle().clone(),
    ));
    crate::orchestrator::spawn_idle_reaper(app.handle().clone());
    shell::setup_tray(app)
}
