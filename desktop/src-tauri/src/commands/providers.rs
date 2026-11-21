use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

use crate::agents::{Agent, AgentRegistry};
use crate::claude::{
    ask as claude_ask, base_command, classify_provider_error, kill_process_tree, ClaudeError,
};
use crate::orchestrator::fallback::ask_with_fallback;
use crate::providers::{
    parse_codex_models_cache, parse_json_models, parse_models_lines, ProviderConfig,
    ProviderProtocol, ProviderRegistry,
};
use crate::state::AppState;

const INSPECT_TIMEOUT_SECS: u64 = 5;
const DISCOVER_TIMEOUT_SECS: u64 = 5;
const TEST_TIMEOUT_SECS: u64 = 20;
const MAX_HELP_BYTES: usize = 8192;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderWithStatus {
    #[serde(flatten)]
    pub config: ProviderConfig,
    pub installed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverModelsResult {
    pub models: Vec<String>,
    pub source: String,
    pub warning: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectCliResult {
    pub found: bool,
    pub program_path: Option<String>,
    pub help_text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestProviderResult {
    pub ok: bool,
    pub latency_ms: u64,
    pub reply: String,
    pub error_kind: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveProviderArgs {
    pub config: ProviderConfig,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderIdArgs {
    pub id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InspectCliArgs {
    pub command: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BuildSetupPromptArgs {
    pub cli_name: String,
    pub command: String,
    pub help_text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestProviderConfigArgs {
    pub config: ProviderConfig,
    pub model: Option<String>,
}

#[tauri::command]
pub fn list_providers(app: AppHandle) -> Result<Vec<ProviderWithStatus>, String> {
    list_providers_exec(&app)
}

#[tauri::command]
pub fn save_provider(app: AppHandle, config: ProviderConfig) -> Result<ProviderConfig, String> {
    save_provider_exec(&app, SaveProviderArgs { config })
}

#[tauri::command]
pub fn delete_provider(
    providers: State<'_, ProviderRegistry>,
    agents: State<'_, AgentRegistry>,
    id: String,
) -> Result<(), String> {
    delete_provider_impl(&providers, &agents, &id)
}

#[tauri::command]
pub async fn discover_provider_models(
    app: AppHandle,
    id: String,
) -> Result<DiscoverModelsResult, String> {
    discover_provider_models_exec(&app, ProviderIdArgs { id }).await
}

#[tauri::command]
pub async fn inspect_cli(command: String) -> Result<InspectCliResult, String> {
    inspect_cli_exec(InspectCliArgs { command }).await
}

#[tauri::command]
pub fn build_setup_prompt(cli_name: String, command: String, help_text: String) -> String {
    build_setup_prompt_exec(BuildSetupPromptArgs {
        cli_name,
        command,
        help_text,
    })
}

#[tauri::command]
pub async fn test_provider_config(
    config: ProviderConfig,
    model: Option<String>,
) -> Result<TestProviderResult, String> {
    test_provider_config_exec(TestProviderConfigArgs { config, model }).await
}

#[tauri::command]
pub async fn auto_configure_provider(
    app: AppHandle,
    cli_name: String,
    command: String,
    help_text: String,
) -> Result<ProviderConfig, String> {
    auto_configure_provider_exec(
        &app,
        BuildSetupPromptArgs {
            cli_name,
            command,
            help_text,
        },
    )
    .await
}

pub(crate) fn list_providers_exec(app: &AppHandle) -> Result<Vec<ProviderWithStatus>, String> {
    let list = app
        .state::<ProviderRegistry>()
        .list()
        .map_err(|error| error.to_string())?;
    Ok(list
        .into_iter()
        .map(|config| {
            let installed = resolve_program(&config.default_command).is_ok();
            ProviderWithStatus { config, installed }
        })
        .collect())
}

pub(crate) fn save_provider_exec(
    app: &AppHandle,
    args: SaveProviderArgs,
) -> Result<ProviderConfig, String> {
    validate_provider_config(&args.config)?;
    app.state::<ProviderRegistry>()
        .save_provider(args.config)
        .map_err(|error| error.to_string())
}

pub(crate) fn delete_provider_exec(app: &AppHandle, args: ProviderIdArgs) -> Result<(), String> {
    delete_provider_impl(
        &app.state::<ProviderRegistry>(),
        &app.state::<AgentRegistry>(),
        &args.id,
    )
}

fn delete_provider_impl(
    providers: &ProviderRegistry,
    agents: &AgentRegistry,
    id: &str,
) -> Result<(), String> {
    if id == "claude" {
        return Err("O provedor padrão 'claude' não pode ser removido.".to_string());
    }
    let team = agents.team().map_err(|error| error.to_string())?;
    if let Some(in_use) = team.profiles().iter().find(|agent| agent.cli == id) {
        return Err(format!(
            "Não é possível remover '{id}': está em uso pelo agente {}.",
            in_use.name
        ));
    }
    providers
        .remove_provider(id)
        .map_err(|error| error.to_string())
}

pub(crate) async fn discover_provider_models_exec(
    app: &AppHandle,
    args: ProviderIdArgs,
) -> Result<DiscoverModelsResult, String> {
    let id = args.id;
    let providers = app.state::<ProviderRegistry>();
    let provider = providers.get(&id).map_err(|error| error.to_string())?;
    let cached = provider.models.clone();

    let Some(discovery) = provider.discovery.as_ref() else {
        return Ok(DiscoverModelsResult {
            models: cached,
            source: "cache".to_string(),
            warning: None,
        });
    };

    let live_result: Result<Vec<String>, String> = if let Some(cache_file) = discovery
        .cache_file
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    {
        discover_from_cache_file(cache_file, discovery.json_path.as_deref())
    } else if !discovery.args.is_empty() {
        discover_from_command(
            &provider.default_command,
            &discovery.args,
            discovery.json_path.as_deref(),
        )
        .await
    } else {
        Ok(Vec::new())
    };

    match live_result {
        Ok(models) if !models.is_empty() => {
            let _ = providers.update_cached_models(&id, models.clone());
            Ok(DiscoverModelsResult {
                models,
                source: "live".to_string(),
                warning: None,
            })
        }
        Ok(_) => Ok(DiscoverModelsResult {
            models: cached,
            source: "cache".to_string(),
            warning: Some(
                "Nenhum modelo retornado pela descoberta; usando cache local.".to_string(),
            ),
        }),
        Err(reason) => Ok(DiscoverModelsResult {
            models: cached,
            source: "cache".to_string(),
            warning: Some(format!(
                "Falha ao atualizar modelos ({reason}); mantendo cache local."
            )),
        }),
    }
}

pub(crate) async fn inspect_cli_exec(args: InspectCliArgs) -> Result<InspectCliResult, String> {
    let (program, mut base_args) = split_command(&args.command)?;
    let Ok(program_path) = which::which(&program) else {
        return Ok(InspectCliResult {
            found: false,
            program_path: None,
            help_text: String::new(),
        });
    };

    base_args.push("--help".to_string());
    let output = run_bounded_process(
        &program_path,
        &base_args,
        Duration::from_secs(INSPECT_TIMEOUT_SECS),
    )
    .await?;

    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&stderr);
    }

    Ok(InspectCliResult {
        found: true,
        program_path: Some(program_path.display().to_string()),
        help_text: truncate_utf8(&combined, MAX_HELP_BYTES),
    })
}

pub(crate) fn build_setup_prompt_exec(args: BuildSetupPromptArgs) -> String {
    let BuildSetupPromptArgs {
        cli_name,
        command,
        help_text,
    } = args;
    let clipped_help = truncate_utf8(&help_text, MAX_HELP_BYTES);
    format!(
        r#"Gere APENAS um objeto JSON válido seguindo o schema `ProviderConfig` do Vokia para configurar a CLI "{cli_name}" (comando base: `{command}`).

Regras obrigatórias:
- Nunca use shell (`sh -c`, `cmd /C`) nem pipe. Todas as flags devem ser explícitas em arrays.
- Use `"protocol": "generic-cli"` para novas CLIs (ou `"claude-live"`, `"codex-jsonl"`, `"open-code-json"`, `"antigravity-stream"` se for compatível).
- Defina `"toolTransport"`: `"mcp-native"` (se a CLI aceitar MCP por flag de sessão), `"vokia-cli"` (padrão para novas CLIs sem flag MCP por sessão) ou `"none"`.
- Preencha `tiers` (`basic`, `intermediate`, `advanced`) com modelos válidos sugeridos ou compatíveis com a CLI.
- Preencha `rateLimitPatterns` e `modelUnavailablePatterns` com trechos de erro esperados.

Schema JSON esperado:
{{
  "id": "string-kebab-case",
  "label": "Nome de exibição",
  "protocol": "generic-cli",
  "defaultCommand": "{command}",
  "defaultModel": "modelo-padrao",
  "models": ["modelo-1", "modelo-2"],
  "tiers": {{
    "basic": "modelo-rapido",
    "intermediate": "modelo-equilibrado",
    "advanced": "modelo-avancado"
  }},
  "supportsMcp": false,
  "toolTransport": "vokia-cli",
  "supportsSandbox": false,
  "reservedFlags": [],
  "rateLimitPatterns": ["rate_limit", "usage_limit", "quota"],
  "modelUnavailablePatterns": ["model_not_found", "not available"],
  "discovery": {{
    "args": ["models"],
    "jsonPath": null,
    "cacheFile": null
  }},
  "generic": {{
    "promptArgs": ["-p", "{{prompt}}"],
    "promptViaStdin": false,
    "modelFlag": "--model",
    "dirFlag": null,
    "jsonOutputArgs": [],
    "textJsonPaths": [],
    "errorJsonPaths": [],
    "sandboxArgs": {{}}
  }},
  "builtinSeed": false
}}

Saída de `{command} --help`:
```
{clipped_help}
```"#
    )
}

pub(crate) async fn test_provider_config_exec(
    args: TestProviderConfigArgs,
) -> Result<TestProviderResult, String> {
    let TestProviderConfigArgs { config, model } = args;
    validate_provider_config(&config)?;
    let (program_path, base_args) = match resolve_program(&config.default_command) {
        Ok(resolved) => resolved,
        Err(message) => {
            return Ok(TestProviderResult {
                ok: false,
                latency_ms: 0,
                reply: String::new(),
                error_kind: Some("other".to_string()),
                error_message: Some(message),
            });
        }
    };

    let target_model = model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .unwrap_or(&config.default_model)
        .to_string();

    let started = Instant::now();
    let ping_prompt = "Responda apenas OK.";

    let result = match config.protocol {
        ProviderProtocol::ClaudeLive => timeout(
            Duration::from_secs(TEST_TIMEOUT_SECS),
            claude_ask(&program_path, ping_prompt, "", &target_model),
        )
        .await
        .map_err(|_| ClaudeError::Timeout(Duration::from_secs(TEST_TIMEOUT_SECS)))
        .and_then(|inner| inner)
        .map(|reply| reply.text),
        _ => {
            let args = build_test_args(&config, &base_args, &target_model, ping_prompt);
            let stdin_prompt = match (&config.protocol, &config.generic) {
                (ProviderProtocol::GenericCli, Some(spec)) if spec.prompt_via_stdin => {
                    Some(ping_prompt)
                }
                _ => None,
            };
            let output = run_bounded_process_with_stdin(
                &program_path,
                &args,
                stdin_prompt,
                Duration::from_secs(TEST_TIMEOUT_SECS),
            )
            .await;

            match output {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
                    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
                    let combined = if stderr.trim().is_empty() {
                        stdout.clone()
                    } else if stdout.trim().is_empty() {
                        stderr.clone()
                    } else {
                        format!("{stdout}\n{stderr}")
                    };

                    if let Some(classified) = classify_provider_error(
                        &config.id,
                        &target_model,
                        &combined,
                        &config.rate_limit_patterns,
                        &config.model_unavailable_patterns,
                    ) {
                        Err(classified)
                    } else if !out.status.success() {
                        Err(ClaudeError::Exit {
                            code: out.status.code(),
                            message: combined,
                        })
                    } else {
                        Ok(stdout.trim().to_string())
                    }
                }
                Err(message) => Err(ClaudeError::Reported(message)),
            }
        }
    };

    let latency_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(reply) => Ok(TestProviderResult {
            ok: true,
            latency_ms,
            reply,
            error_kind: None,
            error_message: None,
        }),
        Err(err) => {
            let (kind, message) = classify_test_error(&err, &config, &target_model);
            Ok(TestProviderResult {
                ok: false,
                latency_ms,
                reply: String::new(),
                error_kind: Some(kind.to_string()),
                error_message: Some(message),
            })
        }
    }
}

pub(crate) async fn auto_configure_provider_exec(
    app: &AppHandle,
    args: BuildSetupPromptArgs,
) -> Result<ProviderConfig, String> {
    let state = app.state::<AppState>();
    let prompt = build_setup_prompt_exec(args);
    let team = app
        .state::<AgentRegistry>()
        .team()
        .map_err(|error| error.to_string())?;
    let profile = team
        .profile(&Agent::ember())
        .or_else(|| team.profiles().first())
        .ok_or_else(|| "Nenhum agente configurado para auto-configuração.".to_string())?;
    let default_cli = state.cli.as_deref().unwrap_or_else(|_| Path::new("claude"));

    let reply = ask_with_fallback(app, &state, default_cli, &profile.id, profile, &prompt, "")
        .await
        .map_err(|error| format!("Falha ao consultar agente para auto-configuração: {error}"))?;

    let json_slice = extract_json_object(&reply.text)
        .ok_or_else(|| "A resposta do agente não conteve um objeto JSON válido.".to_string())?;
    let config: ProviderConfig = serde_json::from_str(json_slice)
        .map_err(|error| format!("JSON gerado inválido: {error}"))?;
    validate_provider_config(&config)?;
    Ok(config)
}

fn validate_provider_config(config: &ProviderConfig) -> Result<(), String> {
    let id = config.id.trim();
    if id.is_empty()
        || !id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err("ID do provedor inválido: use apenas letras, números, '-' ou '_'.".to_string());
    }
    if config.label.trim().is_empty() {
        return Err("Nome de exibição do provedor é obrigatório.".to_string());
    }
    split_command(&config.default_command)?;
    Ok(())
}

fn split_command(command: &str) -> Result<(String, Vec<String>), String> {
    let tokens = shlex::split(command.trim())
        .ok_or_else(|| "Comando inválido: aspas não fechadas.".to_string())?;
    let Some((program, rest)) = tokens.split_first() else {
        return Err("Comando do provedor não pode ficar vazio.".to_string());
    };
    Ok((program.clone(), rest.to_vec()))
}

fn resolve_program(command: &str) -> Result<(PathBuf, Vec<String>), String> {
    let (program, args) = split_command(command)?;
    let path = which::which(&program)
        .map_err(|_| format!("Executável '{program}' não encontrado no PATH."))?;
    Ok((path, args))
}

fn discover_from_cache_file(
    cache_file: &str,
    json_path: Option<&str>,
) -> Result<Vec<String>, String> {
    let path = resolve_user_path(cache_file)?;
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("Não foi possível ler {}: {error}", path.display()))?;
    if json_path.is_some() {
        parse_json_models(&text, json_path)
    } else {
        parse_codex_models_cache(&text)
    }
}

async fn discover_from_command(
    default_command: &str,
    discovery_args: &[String],
    json_path: Option<&str>,
) -> Result<Vec<String>, String> {
    let (program_path, mut args) = resolve_program(default_command)?;
    args.extend(discovery_args.iter().cloned());
    let output = run_bounded_process(
        &program_path,
        &args,
        Duration::from_secs(DISCOVER_TIMEOUT_SECS),
    )
    .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("comando retornou erro: {}", stderr.trim()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if json_path.is_some() {
        parse_json_models(&stdout, json_path)
    } else {
        Ok(parse_models_lines(&stdout))
    }
}

fn resolve_user_path(raw: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() {
        return Ok(candidate);
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| "Diretório HOME do usuário não definido.".to_string())?;
    Ok(home.join(candidate))
}

async fn run_bounded_process(
    program: &PathBuf,
    args: &[String],
    limit: Duration,
) -> Result<std::process::Output, String> {
    run_bounded_process_with_stdin(program, args, None, limit).await
}

async fn run_bounded_process_with_stdin(
    program: &PathBuf,
    args: &[String],
    stdin_text: Option<&str>,
    limit: Duration,
) -> Result<std::process::Output, String> {
    let mut cmd = base_command(program);
    cmd.args(args)
        .stdin(if stdin_text.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|error| format!("Falha ao iniciar processo: {error}"))?;
    let pid = child.id();

    let run = async {
        if let Some(text) = stdin_text {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text.as_bytes()).await?;
                let _ = stdin.shutdown().await;
            }
        }
        child.wait_with_output().await
    };

    match timeout(limit, run).await {
        Ok(result) => result.map_err(|error| format!("Falha ao aguardar processo: {error}")),
        Err(_) => {
            if let Some(pid_num) = pid {
                kill_process_tree(pid_num);
            }
            Err(format!("Tempo limite ({}s) excedido.", limit.as_secs()))
        }
    }
}

fn build_test_args(
    config: &ProviderConfig,
    base_args: &[String],
    model: &str,
    prompt: &str,
) -> Vec<String> {
    let mut args = base_args.to_vec();
    match config.protocol {
        ProviderProtocol::CodexJsonl => {
            args.extend(["exec".to_string(), "--json".to_string()]);
            if !model.is_empty() {
                args.extend(["--model".to_string(), model.to_string()]);
            }
            args.push(prompt.to_string());
        }
        ProviderProtocol::OpenCodeJson => {
            args.extend([
                "run".to_string(),
                "--format".to_string(),
                "json".to_string(),
            ]);
            if !model.is_empty() {
                args.extend(["--model".to_string(), model.to_string()]);
            }
            args.push(prompt.to_string());
        }
        ProviderProtocol::AntigravityStream => {
            args.extend([
                "-p".to_string(),
                prompt.to_string(),
                "--output-format".to_string(),
                "stream-json".to_string(),
            ]);
            if !model.is_empty() {
                args.extend(["--model".to_string(), model.to_string()]);
            }
        }
        ProviderProtocol::GenericCli => {
            if let Some(spec) = &config.generic {
                if let Some(model_flag) = &spec.model_flag {
                    if !model.is_empty() {
                        args.extend([model_flag.clone(), model.to_string()]);
                    }
                }
                args.extend(spec.json_output_args.iter().cloned());
                for item in &spec.prompt_args {
                    args.push(item.replace("{prompt}", prompt));
                }
            } else {
                args.extend(["-p".to_string(), prompt.to_string()]);
            }
        }
        ProviderProtocol::ClaudeLive => {}
    }
    args
}

fn classify_test_error(
    err: &ClaudeError,
    config: &ProviderConfig,
    model: &str,
) -> (&'static str, String) {
    match err {
        ClaudeError::ModelUnavailable { detail, .. } => ("model-unavailable", detail.clone()),
        ClaudeError::UsageLimit(_) | ClaudeError::LimitExhausted => {
            ("limit", "Limite de uso atingido.".to_string())
        }
        ClaudeError::Reported(message) | ClaudeError::Exit { message, .. } => {
            if let Some(classified) = classify_provider_error(
                &config.id,
                model,
                message,
                &config.rate_limit_patterns,
                &config.model_unavailable_patterns,
            ) {
                return classify_test_error(&classified, config, model);
            }
            ("other", message.clone())
        }
        ClaudeError::Timeout(duration) => (
            "other",
            format!("Tempo limite ({}s) excedido.", duration.as_secs()),
        ),
        ClaudeError::Spawn(io_err) => ("other", format!("Falha ao executar CLI: {io_err}")),
        other => ("other", format!("{other:?}")),
    }
}

fn truncate_utf8(input: &str, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    input[..end].to_string()
}

fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end > start {
        Some(&text[start..=end])
    } else {
        None
    }
}
