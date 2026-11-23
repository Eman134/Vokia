use std::error::Error;

use tauri::{App, Manager};

use crate::db::DbState;
use crate::{agents, attachments, mcp, orchestration, providers, roles, shell};

pub(crate) fn setup(app: &mut App) -> Result<(), Box<dyn Error>> {
    let data_dir = app.path().app_data_dir()?;
    let providers = providers::ProviderRegistry::load(&data_dir);
    let agents = agents::AgentRegistry::load(&data_dir);
    app.manage(roles::RoleRegistry::load(&data_dir, &agents));
    app.manage(providers);
    app.manage(agents);
    app.manage(DbState::open(&data_dir));
    {
        let team = app
            .state::<agents::AgentRegistry>()
            .team()
            .map_err(|error| error.to_string())?;
        if let Some(db) = app.state::<DbState>().db() {
            orchestration::seed::fill_missing_leaders(db, &team)
                .map_err(|error| error.to_string())?;
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
    shell::setup_tray(app)
}
