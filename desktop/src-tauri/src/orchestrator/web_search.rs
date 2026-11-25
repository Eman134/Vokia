use std::collections::{BTreeMap, HashSet};
use std::process::Stdio;
use std::sync::atomic::AtomicU32;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

use crate::agents::Agent;
use crate::claude::{self, Access, Reply};
use crate::commands::providers::{evaluate_provider_status, ProviderConnectionStatus};
use crate::db::{Db, DbState, SETTING_WEB_SEARCH};
use crate::permissions::{PermissionBroker, PermissionGate, PermissionLevel};
use crate::providers::{ProviderConfig, ProviderProtocol, ProviderRegistry};
use crate::state::AppState;

use super::adapters::ProviderAdapter;
use super::console::ConsoleSink;
use super::error::FlowError;
use super::fallback::resolve_program_for;
use super::step_labels::{label_for_action, redact_secrets, StepAction};

pub const MAX_WEB_SEARCH_QUERY_CHARS: usize = 400;
pub const MAX_WEB_SEARCH_RESPONSE_CHARS: usize = 6_000;
pub const MAX_WEB_SEARCH_SUMMARY_CHARS: usize = 2_400;
pub const MAX_WEB_SEARCH_SOURCES: usize = 8;
const AUX_SEARCH_TIMEOUT_SECS: u64 = 45;
const MCP_CONNECTOR_TIMEOUT_SECS: u64 = 30;

pub const NO_SEARCH_BACKEND_ERROR: &str =
    "Nenhum provedor de pesquisa configurado em Configurações › Pesquisa";
pub const RESTRICTED_NETWORK_ERROR: &str =
    "Pesquisa web bloqueada no modo restrito (Somente leitura) deste projeto.";
pub const CAPTCHA_BLOCKED_ERROR: &str =
    "Pesquisa bloqueada por verificação anti-bot/CAPTCHA; o Vokia não contorna barreiras anti-bot.";

const AUX_SEARCH_SYSTEM_PROMPT: &str = "Você é um assistente de pesquisa web efêmero e sem acesso a arquivos locais. \
Pesquise apenas a consulta informada usando sua busca web nativa. \
Nunca tente contornar CAPTCHAs ou barreiras anti-bot. \
Não invente URLs. Responda APENAS com um objeto JSON válido neste formato exato: \
{\"summary\":\"resumo objetivo em até 1500 caracteres\",\"sources\":[{\"title\":\"título\",\"url\":\"https://...\"}]}";

const CAPTCHA_MARKERS: [&str; 6] = [
    "captcha",
    "verify you are human",
    "verifique que você é humano",
    "cloudflare turnstile",
    "recaptcha",
    "hcaptcha",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebSearchMode {
    #[default]
    AuxiliaryProvider,
    McpConnector,
    Disabled,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSearchConnectorConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    #[serde(default)]
    pub server_name: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl McpSearchConnectorConfig {
    pub fn is_configured(&self) -> bool {
        !self.command.trim().is_empty()
            || self
                .url
                .as_deref()
                .is_some_and(|url| !url.trim().is_empty())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchSettings {
    #[serde(default)]
    pub mode: WebSearchMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auxiliary_provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_connector: Option<McpSearchConnectorConfig>,
}

impl Default for WebSearchSettings {
    fn default() -> Self {
        Self {
            mode: WebSearchMode::AuxiliaryProvider,
            auxiliary_provider_id: None,
            mcp_connector: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchSource {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchResponse {
    pub query: String,
    pub summary: String,
    pub sources: Vec<WebSearchSource>,
    pub backend: String,
    pub step_label: String,
    pub truncated: bool,
}

pub fn load_web_search_settings(db: &Db) -> WebSearchSettings {
    let Ok(settings) = db.load_settings() else {
        return WebSearchSettings::default();
    };
    let Some(raw) = settings.get(SETTING_WEB_SEARCH) else {
        return WebSearchSettings::default();
    };
    serde_json::from_str::<WebSearchSettings>(raw).unwrap_or_default()
}

pub fn select_auxiliary_provider(
    providers: &[ProviderConfig],
    statuses: &[(String, bool, ProviderConnectionStatus)],
    preferred_id: Option<&str>,
) -> Result<ProviderConfig, String> {
    let is_connected = |id: &str| {
        statuses
            .iter()
            .find(|(pid, _, _)| pid == id)
            .is_some_and(|(_, installed, status)| {
                *installed && *status == ProviderConnectionStatus::Connected
            })
    };

    if let Some(id) = preferred_id.map(str::trim).filter(|s| !s.is_empty()) {
        let Some(provider) = providers.iter().find(|p| p.id == id) else {
            return Err(format!(
                "Provedor auxiliar '{id}' não encontrado. Ajuste em Configurações › Pesquisa."
            ));
        };
        if !provider.enabled || !provider.has_native_web_search || !is_connected(&provider.id) {
            return Err(format!(
                "Provedor auxiliar '{}' não está conectado, habilitado ou com busca nativa ativa. Ajuste em Configurações › Pesquisa.",
                provider.label
            ));
        }
        return Ok(provider.clone());
    }

    let mut candidates: Vec<&ProviderConfig> = providers
        .iter()
        .filter(|p| p.enabled && p.has_native_web_search && is_connected(&p.id))
        .collect();
    candidates.sort_by(|a, b| {
        a.display_order
            .unwrap_or(u32::MAX)
            .cmp(&b.display_order.unwrap_or(u32::MAX))
            .then_with(|| a.id.cmp(&b.id))
    });
    candidates
        .into_iter()
        .next()
        .cloned()
        .ok_or_else(|| NO_SEARCH_BACKEND_ERROR.to_string())
}

pub fn has_configured_web_search_backend(
    settings: &WebSearchSettings,
    providers: &[ProviderConfig],
    statuses: &[(String, bool, ProviderConnectionStatus)],
) -> bool {
    match settings.mode {
        WebSearchMode::Disabled => false,
        WebSearchMode::AuxiliaryProvider => select_auxiliary_provider(
            providers,
            statuses,
            settings.auxiliary_provider_id.as_deref(),
        )
        .is_ok(),
        WebSearchMode::McpConnector => settings
            .mcp_connector
            .as_ref()
            .is_some_and(McpSearchConnectorConfig::is_configured),
    }
}

pub fn is_web_search_configured_for_app(app: &AppHandle) -> bool {
    let db_state = app.state::<DbState>();
    let Ok(db) = db_state.require() else {
        return false;
    };
    let settings = load_web_search_settings(db);
    let Ok(providers) = app.state::<ProviderRegistry>().list() else {
        return false;
    };
    let statuses: Vec<(String, bool, ProviderConnectionStatus)> = providers
        .iter()
        .map(|p| {
            let (installed, status, _) = evaluate_provider_status(p);
            (p.id.clone(), installed, status)
        })
        .collect();
    has_configured_web_search_backend(&settings, &providers, &statuses)
}

pub fn sanitize_query(raw: &str) -> Result<String, String> {
    let redacted = redact_secrets(raw);
    let collapsed: String = redacted.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim();
    if trimmed.is_empty() {
        return Err("Informe um termo de pesquisa em `query`.".to_string());
    }
    Ok(truncate_chars(trimmed, MAX_WEB_SEARCH_QUERY_CHARS))
}

pub fn step_label_for_query(query: &str) -> String {
    label_for_action(StepAction::WebSearch { query: Some(query) })
        .unwrap_or_else(|| "Pesquisando na web…".to_string())
}

pub async fn execute_web_search(
    app: &AppHandle,
    project_id: Option<i64>,
    raw_query: &str,
) -> Result<WebSearchResponse, String> {
    let query = sanitize_query(raw_query)?;
    let db_state = app.state::<DbState>();
    let db = db_state
        .require()
        .map_err(|err| err.to_string())?;

    if let Some(pid) = project_id {
        if let Ok(project) = db.project(pid) {
            if project.permission_level == PermissionLevel::Leitura {
                return Err(RESTRICTED_NETWORK_ERROR.to_string());
            }
        }
    }

    let settings = load_web_search_settings(db);
    let step_label = step_label_for_query(&query);

    match settings.mode {
        WebSearchMode::Disabled => Err(NO_SEARCH_BACKEND_ERROR.to_string()),
        WebSearchMode::AuxiliaryProvider => {
            let providers = app
                .state::<ProviderRegistry>()
                .list()
                .map_err(|err| err.to_string())?;
            let statuses: Vec<(String, bool, ProviderConnectionStatus)> = providers
                .iter()
                .map(|p| {
                    let (installed, status, _) = evaluate_provider_status(p);
                    (p.id.clone(), installed, status)
                })
                .collect();
            let provider = select_auxiliary_provider(
                &providers,
                &statuses,
                settings.auxiliary_provider_id.as_deref(),
            )?;
            run_auxiliary_search(app, &provider, &query, &step_label).await
        }
        WebSearchMode::McpConnector => {
            let Some(connector) = settings
                .mcp_connector
                .as_ref()
                .filter(|cfg| cfg.is_configured())
            else {
                return Err(NO_SEARCH_BACKEND_ERROR.to_string());
            };
            run_mcp_connector_search(connector, &query, &step_label).await
        }
    }
}

async fn run_auxiliary_search(
    app: &AppHandle,
    provider: &ProviderConfig,
    query: &str,
    step_label: &str,
) -> Result<WebSearchResponse, String> {
    let state = app.state::<AppState>();
    let default_cli = state
        .cli
        .as_deref()
        .unwrap_or_else(|_| std::path::Path::new(""));
    let search_agent = Agent::from_stored("oracle").unwrap_or_else(Agent::axiom);
    let model = if !provider.tiers.basic.trim().is_empty() {
        provider.tiers.basic.clone()
    } else {
        provider.default_model.clone()
    };
    let user_prompt = format!(
        "Pesquise na web sobre: {query}\n\nRetorne apenas o JSON com summary e sources."
    );

    let reply: Reply = match provider.protocol {
        ProviderProtocol::ClaudeLive => {
            let (program, _) =
                resolve_program_for(&search_agent, &provider.default_command, default_cli)
                    .map_err(|err| err.to_string())?;
            timeout(
                Duration::from_secs(AUX_SEARCH_TIMEOUT_SECS),
                claude::ask(&program, &user_prompt, AUX_SEARCH_SYSTEM_PROMPT, &model),
            )
            .await
            .map_err(|_| "Tempo limite excedido na pesquisa web auxiliar.".to_string())?
            .map_err(|err| err.to_string())?
        }
        _ => {
            let (program, user_args) =
                resolve_program_for(&search_agent, &provider.default_command, default_cli)
                    .map_err(|err| err.to_string())?;
            let adapter = ProviderAdapter::from_config(provider.clone());
            let broker = PermissionBroker::default();
            let granted = std::sync::Arc::new(std::sync::Mutex::new(HashSet::new()));
            let gate = PermissionGate {
                app,
                broker: &broker,
                agent: search_agent.clone(),
                project_id: 0,
                item_id: None,
                granted: &granted,
                grant_per_session: false,
            };
            let pid = AtomicU32::new(0);
            let console = ConsoleSink::new(
                app,
                crate::session_key::SessionKey {
                    project_id: 0,
                    agent: search_agent.clone(),
                },
                None,
            );
            let call = super::agent_session::AgentCall::for_text_ask(
                adapter.clone(),
                program,
                user_args,
                AUX_SEARCH_SYSTEM_PROMPT.to_string(),
                model,
            );
            timeout(
                Duration::from_secs(AUX_SEARCH_TIMEOUT_SECS),
                adapter.run_fresh(
                    Access::TextOnly,
                    gate,
                    &pid,
                    &call,
                    &user_prompt,
                    &console,
                    state.sessions.adapters(),
                ),
            )
            .await
            .map_err(|_| "Tempo limite excedido na pesquisa web auxiliar.".to_string())?
            .map_err(|err| FlowError::Claude(err).to_string())?
        }
    };

    if contains_captcha_marker(&reply.text) {
        return Err(CAPTCHA_BLOCKED_ERROR.to_string());
    }

    Ok(parse_search_response(
        query,
        &reply.text,
        &format!("auxiliary:{}", provider.id),
        step_label,
    ))
}

async fn run_mcp_connector_search(
    connector: &McpSearchConnectorConfig,
    query: &str,
    step_label: &str,
) -> Result<WebSearchResponse, String> {
    let backend = if connector.server_name.trim().is_empty() {
        format!(
            "mcp:{}",
            connector.preset.as_deref().unwrap_or("custom")
        )
    } else {
        format!("mcp:{}", connector.server_name.trim())
    };

    let raw_text = if let Some(url) = connector
        .url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        call_http_mcp_search(url, connector.tool_name.as_deref(), query).await?
    } else {
        call_stdio_mcp_search(connector, query).await?
    };

    if contains_captcha_marker(&raw_text) {
        return Err(CAPTCHA_BLOCKED_ERROR.to_string());
    }

    Ok(parse_search_response(query, &raw_text, &backend, step_label))
}

async fn call_http_mcp_search(
    url: &str,
    tool_name: Option<&str>,
    query: &str,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(MCP_CONNECTOR_TIMEOUT_SECS))
        .build()
        .map_err(|err| format!("Falha ao inicializar cliente HTTP MCP: {err}"))?;

    let target_tool = tool_name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("web_search");

    let payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": target_tool,
            "arguments": { "query": query }
        }
    });

    let response = client
        .post(url)
        .json(&payload)
        .send()
        .await
        .map_err(|err| format!("Falha ao consultar conector MCP HTTP: {err}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Conector MCP HTTP retornou status {}.",
            response.status()
        ));
    }

    let body: Value = response
        .json()
        .await
        .map_err(|err| format!("Resposta inválida do conector MCP HTTP: {err}"))?;
    extract_mcp_text_from_rpc(&body)
}

async fn call_stdio_mcp_search(
    connector: &McpSearchConnectorConfig,
    query: &str,
) -> Result<String, String> {
    let mut cmd = Command::new(connector.command.trim());
    cmd.args(&connector.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (k, v) in &connector.env {
        if !k.trim().is_empty() && !v.trim().is_empty() {
            cmd.env(k, v);
        }
    }

    let mut child = cmd
        .spawn()
        .map_err(|err| format!("Falha ao iniciar servidor MCP de busca: {err}"))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Stdin indisponível no servidor MCP.".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Stdout indisponível no servidor MCP.".to_string())?;
    let mut lines = BufReader::new(stdout).lines();

    let run = async {
        send_rpc_line(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "vokia", "version": env!("CARGO_PKG_VERSION") }
                }
            }),
        )
        .await?;
        let _ = read_rpc_response(&mut lines, 1).await?;

        send_rpc_line(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }),
        )
        .await?;

        let tool_name = match connector
            .tool_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(name) => name.to_string(),
            None => {
                send_rpc_line(
                    &mut stdin,
                    &json!({
                        "jsonrpc": "2.0",
                        "id": 2,
                        "method": "tools/list",
                        "params": {}
                    }),
                )
                .await?;
                let list_resp = read_rpc_response(&mut lines, 2).await?;
                pick_search_tool_name(&list_resp).unwrap_or_else(|| "web_search".to_string())
            }
        };

        send_rpc_line(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": tool_name,
                    "arguments": { "query": query }
                }
            }),
        )
        .await?;
        let call_resp = read_rpc_response(&mut lines, 3).await?;
        extract_mcp_text_from_rpc(&call_resp)
    };

    let result = timeout(Duration::from_secs(MCP_CONNECTOR_TIMEOUT_SECS), run).await;
    let _ = child.kill().await;
    match result {
        Ok(inner) => inner,
        Err(_) => Err("Tempo limite excedido ao consultar conector MCP de busca.".to_string()),
    }
}

async fn send_rpc_line(stdin: &mut tokio::process::ChildStdin, value: &Value) -> Result<(), String> {
    let line = format!("{value}\n");
    stdin
        .write_all(line.as_bytes())
        .await
        .map_err(|err| format!("Falha ao enviar JSON-RPC ao MCP: {err}"))?;
    stdin
        .flush()
        .await
        .map_err(|err| format!("Falha ao enviar JSON-RPC ao MCP: {err}"))
}

async fn read_rpc_response(
    lines: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    expected_id: u64,
) -> Result<Value, String> {
    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|err| format!("Falha ao ler resposta do MCP: {err}"))?
    {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        if value.get("id").and_then(Value::as_u64) == Some(expected_id) {
            return Ok(value);
        }
    }
    Err("Servidor MCP encerrou sem responder.".to_string())
}

fn pick_search_tool_name(list_resp: &Value) -> Option<String> {
    let tools = list_resp
        .pointer("/result/tools")
        .and_then(Value::as_array)?;
    let preferred = ["search", "web", "query", "docs", "brave", "exa", "tavily"];
    for needle in preferred {
        if let Some(name) = tools.iter().find_map(|tool| {
            let name = tool.get("name").and_then(Value::as_str)?;
            name.to_ascii_lowercase().contains(needle).then(|| name.to_string())
        }) {
            return Some(name);
        }
    }
    tools
        .first()
        .and_then(|tool| tool.get("name"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn extract_mcp_text_from_rpc(resp: &Value) -> Result<String, String> {
    if let Some(err) = resp.get("error") {
        let msg = err
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Erro no servidor MCP de busca.");
        return Err(msg.to_string());
    }
    let result = resp.get("result").unwrap_or(resp);
    if result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let msg = extract_content_text(result).unwrap_or_else(|| "Erro no conector MCP.".to_string());
        return Err(msg);
    }
    Ok(extract_content_text(result).unwrap_or_else(|| result.to_string()))
}

fn extract_content_text(value: &Value) -> Option<String> {
    let items = value.get("content").and_then(Value::as_array)?;
    let texts: Vec<&str> = items
        .iter()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .collect();
    (!texts.is_empty()).then(|| texts.join("\n"))
}

pub fn contains_captcha_marker(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    CAPTCHA_MARKERS.iter().any(|marker| lower.contains(marker))
}

pub fn parse_search_response(
    query: &str,
    raw: &str,
    backend: &str,
    step_label: &str,
) -> WebSearchResponse {
    if let Some(json_slice) = extract_json_object(raw) {
        if let Ok(parsed) = serde_json::from_str::<Value>(json_slice) {
            let summary_raw = parsed
                .get("summary")
                .and_then(Value::as_str)
                .unwrap_or(raw)
                .trim();
            let mut sources = Vec::new();
            let mut seen_urls = HashSet::new();
            if let Some(arr) = parsed.get("sources").and_then(Value::as_array) {
                for item in arr {
                    let url = item
                        .get("url")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .unwrap_or("");
                    if is_valid_http_url(url) && seen_urls.insert(url.to_string()) {
                        sources.push(WebSearchSource {
                            title: item
                                .get("title")
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .map(|s| truncate_chars(s, 140)),
                            url: url.to_string(),
                            snippet: item
                                .get("snippet")
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .map(|s| truncate_chars(s, 280)),
                        });
                        if sources.len() >= MAX_WEB_SEARCH_SOURCES {
                            break;
                        }
                    }
                }
            }
            if sources.is_empty() {
                sources = extract_urls_from_text(raw);
            }
            return finalize_response(query, summary_raw, sources, backend, step_label);
        }
    }

    let sources = extract_urls_from_text(raw);
    finalize_response(query, raw.trim(), sources, backend, step_label)
}

fn finalize_response(
    query: &str,
    summary_raw: &str,
    sources: Vec<WebSearchSource>,
    backend: &str,
    step_label: &str,
) -> WebSearchResponse {
    let mut truncated = summary_raw.chars().count() > MAX_WEB_SEARCH_SUMMARY_CHARS;
    let mut summary = truncate_chars(summary_raw, MAX_WEB_SEARCH_SUMMARY_CHARS);
    let total_chars: usize = summary.chars().count()
        + sources
            .iter()
            .map(|s| {
                s.url.chars().count()
                    + s.title.as_ref().map_or(0, |t| t.chars().count())
                    + s.snippet.as_ref().map_or(0, |sn| sn.chars().count())
            })
            .sum::<usize>();
    if total_chars > MAX_WEB_SEARCH_RESPONSE_CHARS {
        let excess = total_chars - MAX_WEB_SEARCH_RESPONSE_CHARS;
        let target = summary.chars().count().saturating_sub(excess);
        summary = truncate_chars(&summary, target.max(200));
        truncated = true;
    }

    WebSearchResponse {
        query: query.to_string(),
        summary,
        sources,
        backend: backend.to_string(),
        step_label: step_label.to_string(),
        truncated,
    }
}

fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (end > start).then_some(&text[start..=end])
}

fn is_valid_http_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://")) && url.len() > 10
}

fn extract_urls_from_text(text: &str) -> Vec<WebSearchSource> {
    let mut sources = Vec::new();
    let mut seen = HashSet::new();
    for token in text.split_whitespace() {
        let cleaned = token.trim_matches(|c: char| {
            matches!(c, '(' | ')' | '[' | ']' | '<' | '>' | '"' | '\'' | ',' | '.' | ';')
        });
        if is_valid_http_url(cleaned) && seen.insert(cleaned.to_string()) {
            sources.push(WebSearchSource {
                title: None,
                url: cleaned.to_string(),
                snippet: None,
            });
            if sources.len() >= MAX_WEB_SEARCH_SOURCES {
                break;
            }
        }
    }
    sources
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut out: String = input.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
#[path = "../../tests/web_search_tests.rs"]
mod tests;
