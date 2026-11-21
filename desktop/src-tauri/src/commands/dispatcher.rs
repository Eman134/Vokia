use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::projects::ProjectError;

use super::{
    agents, artifacts, attachments, backlog, channels, orchestration, projects, providers, roles,
    settings, team,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exposure {
    AppAndCli,
    UiOnly,
}

pub struct CommandMeta {
    pub name: &'static str,
    pub description: &'static str,
    pub exposure: Exposure,
}

const UI_ONLY_ERROR: &str = "Comando exclusivo da interface gráfica (não disponível via MCP/CLI).";
const APP_PREFIX: &str = "app:";

const fn exposed(name: &'static str, description: &'static str) -> CommandMeta {
    CommandMeta {
        name,
        description,
        exposure: Exposure::AppAndCli,
    }
}

const fn ui_only(name: &'static str, description: &'static str) -> CommandMeta {
    CommandMeta {
        name,
        description,
        exposure: Exposure::UiOnly,
    }
}

const COMMANDS: &[CommandMeta] = &[
    exposed(
        "cli_problem",
        "Problema detectado ao localizar a CLI do Claude, se houver.",
    ),
    exposed(
        "send_request",
        "Envia um pedido de texto a um agente de um projeto.",
    ),
    exposed(
        "answer_permission",
        "Responde a um pedido de permissão pendente.",
    ),
    exposed(
        "answer_decision",
        "Responde a uma decisão pendente do fluxo.",
    ),
    exposed(
        "list_agent_pauses",
        "Lista os agentes pausados por projeto.",
    ),
    exposed("list_interrupted_runs", "Lista as execuções interrompidas."),
    exposed(
        "resume_interrupted_run",
        "Retoma uma execução interrompida.",
    ),
    exposed(
        "discard_interrupted_run",
        "Descarta uma execução interrompida.",
    ),
    exposed(
        "set_agent_paused",
        "Pausa ou retoma um agente em um projeto.",
    ),
    exposed(
        "check_connection",
        "Verifica se o provedor padrão está acessível.",
    ),
    exposed(
        "agent_console",
        "Devolve o console recente de um agente em um projeto.",
    ),
    exposed("list_projects", "Lista os projetos."),
    exposed("select_project", "Seleciona o projeto ativo."),
    ui_only("pick_folder", "Abre o seletor de pasta do sistema."),
    exposed(
        "inspect_folder",
        "Inspeciona uma pasta local para criar um projeto.",
    ),
    exposed(
        "inspect_github_repo",
        "Inspeciona um repositório do GitHub para criar um projeto.",
    ),
    exposed("create_project", "Cria um projeto."),
    exposed("update_project", "Edita um projeto."),
    exposed("delete_project", "Exclui um projeto."),
    exposed("pin_project", "Fixa ou desafixa um projeto."),
    exposed("clear_conversation", "Apaga a conversa de um projeto."),
    exposed(
        "load_history",
        "Carrega o histórico de eventos de um projeto.",
    ),
    exposed(
        "set_permission_level",
        "Define o nível de permissão de um projeto.",
    ),
    exposed(
        "list_permission_rules",
        "Lista as regras de permissão de um projeto.",
    ),
    exposed(
        "delete_permission_rule",
        "Remove uma regra de permissão de um projeto.",
    ),
    exposed("list_channels", "Lista os canais de um projeto."),
    exposed("create_channel", "Cria um canal em um projeto."),
    exposed("update_channel", "Edita o nome e os agentes de um canal."),
    exposed("delete_channel", "Exclui um canal de um projeto."),
    exposed("list_backlog", "Lista o backlog de um projeto."),
    exposed("save_backlog_item", "Grava um item do backlog."),
    exposed("create_backlog_item", "Cria um card no backlog."),
    exposed(
        "create_request_from_proposal",
        "Cria um card de origem agente a partir de uma proposta.",
    ),
    exposed(
        "move_backlog_item",
        "Move um item do backlog para outra coluna.",
    ),
    exposed(
        "return_task",
        "Devolve uma task concluída ao líder com um motivo.",
    ),
    exposed("save_item_plan", "Grava o plano de um item do backlog."),
    exposed(
        "add_backlog_label",
        "Adiciona um rótulo a um item do backlog.",
    ),
    exposed(
        "remove_backlog_label",
        "Remove um rótulo de um item do backlog.",
    ),
    exposed(
        "set_backlog_responsible",
        "Define o agente responsável por um item do backlog.",
    ),
    exposed("archive_backlog_item", "Arquiva um item do backlog."),
    exposed(
        "restore_backlog_item",
        "Restaura um item arquivado do backlog.",
    ),
    exposed("delete_backlog_item", "Exclui um item do backlog."),
    exposed("list_artifacts", "Lista os artefatos de um projeto."),
    exposed(
        "set_artifact_criterion",
        "Marca ou desmarca um critério de um artefato.",
    ),
    ui_only(
        "pick_attachment_files",
        "Abre o seletor de arquivos para anexos.",
    ),
    exposed(
        "save_attachment_files",
        "Salva arquivos locais como anexos de um projeto.",
    ),
    ui_only(
        "save_attachment_bytes",
        "Salva anexo enviado como bytes pela interface.",
    ),
    exposed("discard_attachment", "Descarta um anexo de um projeto."),
    exposed("list_agents", "Lista o elenco de agentes."),
    exposed(
        "available_providers",
        "Lista os provedores instalados na máquina.",
    ),
    exposed("create_agent", "Cria um agente."),
    exposed("update_agent", "Edita um agente."),
    exposed("delete_agent", "Exclui um agente."),
    exposed(
        "set_agent_instructions",
        "Define as instruções de um agente.",
    ),
    exposed(
        "list_temporary_overrides",
        "Lista as trocas temporárias de LLM.",
    ),
    exposed(
        "set_temporary_override",
        "Aplica uma troca temporária de LLM a um agente.",
    ),
    exposed(
        "clear_temporary_override",
        "Remove a troca temporária de LLM de um agente.",
    ),
    exposed(
        "clear_all_temporary_overrides",
        "Remove todas as trocas temporárias de LLM.",
    ),
    exposed("update_ember_llm", "Atualiza o LLM do agente Ember."),
    exposed("role_list", "Lista os papéis."),
    exposed("role_create", "Cria um papel."),
    exposed("role_update", "Edita um papel."),
    exposed("role_delete", "Exclui um papel."),
    exposed(
        "get_orchestration",
        "Devolve a orquestração (camadas, times e posições).",
    ),
    exposed("save_layer", "Grava uma camada da orquestração."),
    exposed("delete_layer", "Exclui uma camada da orquestração."),
    exposed("reorder_layers", "Reordena as camadas da orquestração."),
    exposed("save_team", "Grava um time da orquestração."),
    exposed("delete_team", "Exclui um time da orquestração."),
    exposed("place_agent", "Posiciona um agente na orquestração."),
    exposed(
        "unplace_agent",
        "Remove a posição de um agente na orquestração.",
    ),
    exposed("save_team_flow", "Grava o fluxo de um time."),
    exposed(
        "set_final_validation",
        "Define o modo de validação final de um time.",
    ),
    exposed(
        "save_orchestration_settings",
        "Grava as configurações da orquestração.",
    ),
    exposed(
        "list_agent_activity",
        "Lista as execuções em andamento por agente.",
    ),
    exposed(
        "list_agent_copies",
        "Lista as réplicas de agentes em execução em um projeto.",
    ),
    exposed("list_providers", "Lista os provedores configurados."),
    exposed("save_provider", "Grava a configuração de um provedor."),
    exposed("delete_provider", "Remove um provedor."),
    exposed(
        "discover_provider_models",
        "Descobre os modelos disponíveis de um provedor.",
    ),
    exposed(
        "inspect_cli",
        "Inspeciona uma CLI e devolve seu texto de ajuda.",
    ),
    exposed(
        "build_setup_prompt",
        "Monta o prompt para configurar uma CLI como provedor.",
    ),
    exposed(
        "test_provider_config",
        "Testa a configuração de um provedor.",
    ),
    exposed(
        "auto_configure_provider",
        "Configura um provedor automaticamente com um agente.",
    ),
    exposed(
        "db_problem",
        "Problema detectado ao abrir o banco, se houver.",
    ),
    exposed(
        "erase_all_data",
        "Apaga todos os dados; exige { \"confirm\": true }.",
    ),
    exposed("get_settings", "Devolve as configurações."),
    exposed("set_setting", "Grava uma configuração."),
];

#[derive(Deserialize, Default)]
struct ConfirmArgs {
    #[serde(default)]
    confirm: bool,
}

pub fn list_commands() -> Vec<Value> {
    COMMANDS
        .iter()
        .filter(|command| command.exposure == Exposure::AppAndCli)
        .map(|command| {
            json!({
                "name": command.name,
                "description": command.description,
                "inputSchema": { "type": "object" },
            })
        })
        .collect()
}

pub async fn dispatch(
    app: &AppHandle,
    name: &str,
    arguments: Value,
) -> Option<Result<Value, String>> {
    let canonical = name.strip_prefix(APP_PREFIX).unwrap_or(name);
    let command = COMMANDS.iter().find(|command| command.name == canonical)?;
    if command.exposure == Exposure::UiOnly {
        return Some(Err(UI_ONLY_ERROR.to_string()));
    }
    Some(run(app, canonical, arguments).await)
}

fn parse_args<T: DeserializeOwned>(arguments: Value) -> Result<T, String> {
    let normalized = if arguments.is_null() {
        json!({})
    } else {
        arguments
    };
    serde_json::from_value(normalized).map_err(|error| format!("Argumentos inválidos: {error}"))
}

fn json_ok<T: Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| error.to_string())
}

fn json_result<T: Serialize, E: ToString>(result: Result<T, E>) -> Result<Value, String> {
    result.map_err(|error| error.to_string()).and_then(json_ok)
}

fn json_project_result<T: Serialize>(result: Result<T, ProjectError>) -> Result<Value, String> {
    json_result(result.map_err(|error| error.message().to_string()))
}

async fn run(app: &AppHandle, name: &str, arguments: Value) -> Result<Value, String> {
    match name {
        "cli_problem" => json_ok(team::cli_problem_exec(app)),
        "send_request" => json_result(team::send_request_exec(app, parse_args(arguments)?).await),
        "answer_permission" => {
            json_result(team::answer_permission_exec(app, parse_args(arguments)?))
        }
        "answer_decision" => {
            json_result(team::answer_decision_exec(app, parse_args(arguments)?).await)
        }
        "list_agent_pauses" => json_result(team::list_agent_pauses_exec(app)),
        "list_interrupted_runs" => json_result(team::list_interrupted_runs_exec(app)),
        "resume_interrupted_run" => {
            json_result(team::resume_interrupted_run_exec(app, parse_args(arguments)?).await)
        }
        "discard_interrupted_run" => json_result(team::discard_interrupted_run_exec(
            app,
            parse_args(arguments)?,
        )),
        "set_agent_paused" => json_result(team::set_agent_paused_exec(app, parse_args(arguments)?)),
        "check_connection" => json_ok(team::check_connection_exec().await),
        "agent_console" => json_ok(team::agent_console_exec(app, parse_args(arguments)?)),

        "list_projects" => json_result(projects::list_projects_exec(app)),
        "select_project" => json_result(projects::select_project_exec(app, parse_args(arguments)?)),
        "inspect_folder" => {
            json_project_result(projects::inspect_folder_exec(parse_args(arguments)?).await)
        }
        "inspect_github_repo" => {
            json_project_result(projects::inspect_github_repo_exec(parse_args(arguments)?).await)
        }
        "create_project" => {
            json_project_result(projects::create_project_exec(app, parse_args(arguments)?).await)
        }
        "update_project" => {
            json_project_result(projects::update_project_exec(app, parse_args(arguments)?))
        }
        "delete_project" => {
            json_project_result(projects::delete_project_exec(app, parse_args(arguments)?).await)
        }
        "pin_project" => {
            json_project_result(projects::pin_project_exec(app, parse_args(arguments)?))
        }
        "clear_conversation" => {
            json_result(projects::clear_conversation_exec(app, parse_args(arguments)?).await)
        }
        "load_history" => json_result(projects::load_history_exec(app, parse_args(arguments)?)),
        "set_permission_level" => {
            json_result(projects::set_permission_level_exec(app, parse_args(arguments)?).await)
        }
        "list_permission_rules" => json_result(projects::list_permission_rules_exec(
            app,
            parse_args(arguments)?,
        )),
        "delete_permission_rule" => json_result(projects::delete_permission_rule_exec(
            app,
            parse_args(arguments)?,
        )),

        "list_channels" => json_result(channels::list_channels_exec(app, parse_args(arguments)?)),
        "create_channel" => json_result(channels::create_channel_exec(app, parse_args(arguments)?)),
        "update_channel" => json_result(channels::update_channel_exec(app, parse_args(arguments)?)),
        "delete_channel" => json_result(channels::delete_channel_exec(app, parse_args(arguments)?)),

        "list_backlog" => json_result(backlog::list_backlog_exec(app, parse_args(arguments)?)),
        "save_backlog_item" => {
            json_result(backlog::save_backlog_item_exec(app, parse_args(arguments)?))
        }
        "create_backlog_item" => json_result(backlog::create_backlog_item_exec(
            app,
            parse_args(arguments)?,
        )),
        "create_request_from_proposal" => json_result(backlog::create_request_from_proposal_exec(
            app,
            parse_args(arguments)?,
        )),
        "move_backlog_item" => {
            json_result(backlog::move_backlog_item_exec(app, parse_args(arguments)?))
        }
        "return_task" => json_result(backlog::return_task_exec(app, parse_args(arguments)?).await),
        "save_item_plan" => json_result(backlog::save_item_plan_exec(app, parse_args(arguments)?)),
        "add_backlog_label" => {
            json_result(backlog::add_backlog_label_exec(app, parse_args(arguments)?))
        }
        "remove_backlog_label" => json_result(backlog::remove_backlog_label_exec(
            app,
            parse_args(arguments)?,
        )),
        "set_backlog_responsible" => json_result(backlog::set_backlog_responsible_exec(
            app,
            parse_args(arguments)?,
        )),
        "archive_backlog_item" => json_result(backlog::archive_backlog_item_exec(
            app,
            parse_args(arguments)?,
        )),
        "restore_backlog_item" => json_result(backlog::restore_backlog_item_exec(
            app,
            parse_args(arguments)?,
        )),
        "delete_backlog_item" => json_result(backlog::delete_backlog_item_exec(
            app,
            parse_args(arguments)?,
        )),

        "list_artifacts" => {
            json_result(artifacts::list_artifacts_exec(app, parse_args(arguments)?))
        }
        "set_artifact_criterion" => json_result(artifacts::set_artifact_criterion_exec(
            app,
            parse_args(arguments)?,
        )),

        "save_attachment_files" => {
            json_result(attachments::save_attachment_files_exec(app, parse_args(arguments)?).await)
        }
        "discard_attachment" => {
            json_result(attachments::discard_attachment_exec(app, parse_args(arguments)?).await)
        }

        "list_agents" => json_result(agents::list_agents_exec(app)),
        "available_providers" => json_ok(agents::available_providers_exec(app)),
        "create_agent" => json_result(agents::create_agent_exec(app, parse_args(arguments)?)),
        "update_agent" => json_result(agents::update_agent_exec(app, parse_args(arguments)?)),
        "delete_agent" => json_result(agents::delete_agent_exec(app, parse_args(arguments)?)),
        "set_agent_instructions" => {
            json_result(agents::set_agent_instructions_exec(app, parse_args(arguments)?).await)
        }
        "list_temporary_overrides" => json_ok(agents::list_temporary_overrides_exec(app)),
        "set_temporary_override" => {
            json_result(agents::set_temporary_override_exec(app, parse_args(arguments)?).await)
        }
        "clear_temporary_override" => {
            json_result(agents::clear_temporary_override_exec(app, parse_args(arguments)?).await)
        }
        "clear_all_temporary_overrides" => {
            json_result(agents::clear_all_temporary_overrides_exec(app).await)
        }
        "update_ember_llm" => {
            json_result(agents::update_ember_llm_exec(app, parse_args(arguments)?).await)
        }

        "role_list" => json_result(roles::role_list_exec(app)),
        "role_create" => json_result(roles::role_create_exec(app, parse_args(arguments)?)),
        "role_update" => json_result(roles::role_update_exec(app, parse_args(arguments)?)),
        "role_delete" => json_result(roles::role_delete_exec(app, parse_args(arguments)?)),

        "get_orchestration" => json_result(orchestration::get_orchestration_exec(app)),
        "save_layer" => json_result(orchestration::save_layer_exec(app, parse_args(arguments)?)),
        "delete_layer" => json_result(orchestration::delete_layer_exec(
            app,
            parse_args(arguments)?,
        )),
        "reorder_layers" => json_result(orchestration::reorder_layers_exec(
            app,
            parse_args(arguments)?,
        )),
        "save_team" => json_result(orchestration::save_team_exec(app, parse_args(arguments)?)),
        "delete_team" => json_result(orchestration::delete_team_exec(app, parse_args(arguments)?)),
        "place_agent" => json_result(orchestration::place_agent_exec(app, parse_args(arguments)?)),
        "unplace_agent" => json_result(orchestration::unplace_agent_exec(
            app,
            parse_args(arguments)?,
        )),
        "save_team_flow" => json_result(orchestration::save_team_flow_exec(
            app,
            parse_args(arguments)?,
        )),
        "set_final_validation" => json_result(orchestration::set_final_validation_exec(
            app,
            parse_args(arguments)?,
        )),
        "save_orchestration_settings" => json_result(
            orchestration::save_orchestration_settings_exec(app, parse_args(arguments)?),
        ),
        "list_agent_activity" => json_result(orchestration::list_agent_activity_exec(app)),
        "list_agent_copies" => json_result(orchestration::list_agent_copies_exec(
            app,
            parse_args(arguments)?,
        )),

        "list_providers" => json_result(providers::list_providers_exec(app)),
        "save_provider" => json_result(providers::save_provider_exec(app, parse_args(arguments)?)),
        "delete_provider" => {
            json_result(providers::delete_provider_exec(app, parse_args(arguments)?))
        }
        "discover_provider_models" => {
            json_result(providers::discover_provider_models_exec(app, parse_args(arguments)?).await)
        }
        "inspect_cli" => json_result(providers::inspect_cli_exec(parse_args(arguments)?).await),
        "build_setup_prompt" => json_ok(providers::build_setup_prompt_exec(parse_args(arguments)?)),
        "test_provider_config" => {
            json_result(providers::test_provider_config_exec(parse_args(arguments)?).await)
        }
        "auto_configure_provider" => {
            json_result(providers::auto_configure_provider_exec(app, parse_args(arguments)?).await)
        }

        "db_problem" => json_ok(settings::db_problem_exec(app)),
        "erase_all_data" => {
            let args: ConfirmArgs = parse_args(arguments)?;
            json_result(settings::erase_all_data_exec(app, args.confirm).await)
        }
        "get_settings" => json_result(settings::get_settings_exec(app)),
        "set_setting" => json_result(settings::set_setting_exec(app, parse_args(arguments)?).await),

        _ => Err(format!("Comando sem despacho registrado: {name}")),
    }
}
