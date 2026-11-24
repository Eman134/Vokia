use std::collections::BTreeMap;
use std::fmt::{self, Display};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};

use crate::agents::{write_atomic, CliPreset};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolTransport {
    #[default]
    None,
    McpNative,
    VokiaCli,
}

const PROVIDERS_FILE: &str = "providers.json";
const PROVIDERS_FILE_TMP: &str = "providers.json.tmp";
const FILE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelTier {
    Basic,
    Intermediate,
    Advanced,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTiers {
    pub basic: String,
    pub intermediate: String,
    pub advanced: String,
}

impl ModelTiers {
    pub fn get(&self, tier: ModelTier) -> &str {
        match tier {
            ModelTier::Basic => &self.basic,
            ModelTier::Intermediate => &self.intermediate,
            ModelTier::Advanced => &self.advanced,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderProtocol {
    ClaudeLive,
    CodexJsonl,
    OpenCodeJson,
    AntigravityStream,
    GenericCli,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDiscoveryConfig {
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub json_path: Option<String>,
    #[serde(default)]
    pub cache_file: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericCliSpec {
    #[serde(default)]
    pub prompt_args: Vec<String>,
    #[serde(default)]
    pub prompt_via_stdin: bool,
    #[serde(default)]
    pub model_flag: Option<String>,
    #[serde(default)]
    pub dir_flag: Option<String>,
    #[serde(default)]
    pub json_output_args: Vec<String>,
    #[serde(default)]
    pub text_json_paths: Vec<String>,
    #[serde(default)]
    pub error_json_paths: Vec<String>,
    #[serde(default)]
    pub model_json_path: Option<String>,
    #[serde(default)]
    pub input_tokens_json_path: Option<String>,
    #[serde(default)]
    pub output_tokens_json_path: Option<String>,
    #[serde(default)]
    pub cost_usd_json_path: Option<String>,
    #[serde(default)]
    pub sandbox_args: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub id: String,
    pub label: String,
    pub protocol: ProviderProtocol,
    pub default_command: String,
    pub default_model: String,
    #[serde(default)]
    pub models: Vec<String>,
    pub tiers: ModelTiers,
    #[serde(default)]
    pub supports_mcp: bool,
    #[serde(default)]
    pub tool_transport: Option<ToolTransport>,
    #[serde(default)]
    pub supports_sandbox: bool,
    #[serde(default)]
    pub reserved_flags: Vec<String>,
    #[serde(default)]
    pub rate_limit_patterns: Vec<String>,
    #[serde(default)]
    pub model_unavailable_patterns: Vec<String>,
    #[serde(default)]
    pub discovery: Option<ModelDiscoveryConfig>,
    #[serde(default)]
    pub generic: Option<GenericCliSpec>,
    #[serde(default)]
    pub builtin_seed: bool,
}

impl ProviderConfig {
    pub fn effective_tool_transport(&self) -> ToolTransport {
        if let Some(transport) = self.tool_transport {
            return transport;
        }
        if self.supports_mcp {
            return ToolTransport::McpNative;
        }
        match self.protocol {
            ProviderProtocol::ClaudeLive | ProviderProtocol::CodexJsonl => {
                ToolTransport::McpNative
            }
            ProviderProtocol::OpenCodeJson
            | ProviderProtocol::AntigravityStream
            | ProviderProtocol::GenericCli => ToolTransport::VokiaCli,
        }
    }

    pub fn has_tools(&self) -> bool {
        !matches!(self.effective_tool_transport(), ToolTransport::None)
    }

    pub fn into_preset(self) -> CliPreset {
        let tool_transport = self.effective_tool_transport();
        CliPreset {
            id: self.id,
            label: self.label,
            models: self.models,
            default_model: self.default_model,
            default_command: self.default_command,
            tiers: self.tiers,
            supports_mcp: self.supports_mcp,
            tool_transport,
            supports_sandbox: self.supports_sandbox,
            builtin_seed: self.builtin_seed,
        }
    }
}

#[derive(Debug)]
pub enum ProviderError {
    LoadFailed(String),
    SaveFailed(String),
    UnknownProvider(String),
}

impl Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderError::LoadFailed(message) | ProviderError::SaveFailed(message) => {
                f.write_str(message)
            }
            ProviderError::UnknownProvider(id) => write!(f, "Provedor desconhecido: {id}."),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProvidersFile {
    version: u32,
    providers: Vec<ProviderConfig>,
}

pub struct ProviderRegistry {
    file: Option<PathBuf>,
    state: Mutex<Result<Vec<ProviderConfig>, String>>,
}

impl ProviderRegistry {
    pub fn load(dir: &Path) -> ProviderRegistry {
        let file = dir.join(PROVIDERS_FILE);
        let state = read_or_seed(dir, &file).map_err(|error| error.to_string());
        ProviderRegistry {
            file: Some(file),
            state: Mutex::new(state),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))] // usado só nos testes
    pub fn seeded() -> ProviderRegistry {
        ProviderRegistry {
            file: None,
            state: Mutex::new(Ok(initial_providers())),
        }
    }

    pub fn list(&self) -> Result<Vec<ProviderConfig>, ProviderError> {
        match &*self.lock() {
            Ok(providers) => Ok(providers.clone()),
            Err(message) => Err(ProviderError::LoadFailed(message.clone())),
        }
    }

    pub fn get(&self, id: &str) -> Result<ProviderConfig, ProviderError> {
        self.list()?
            .into_iter()
            .find(|provider| provider.id == id)
            .ok_or_else(|| ProviderError::UnknownProvider(id.to_string()))
    }

    pub fn presets(&self) -> Result<Vec<CliPreset>, ProviderError> {
        Ok(self
            .list()?
            .into_iter()
            .map(ProviderConfig::into_preset)
            .collect())
    }

    pub fn save_provider(&self, config: ProviderConfig) -> Result<ProviderConfig, ProviderError> {
        let mut state = self.lock();
        let providers = match &mut *state {
            Ok(providers) => providers,
            Err(message) => return Err(ProviderError::LoadFailed(message.clone())),
        };
        let mut next = providers.clone();
        if let Some(existing) = next.iter_mut().find(|item| item.id == config.id) {
            *existing = config.clone();
        } else {
            next.push(config.clone());
        }
        if let Some(file) = &self.file {
            write_file(file, &next).map_err(ProviderError::SaveFailed)?;
        }
        *providers = next;
        Ok(config)
    }

    pub fn remove_provider(&self, id: &str) -> Result<(), ProviderError> {
        let mut state = self.lock();
        let providers = match &mut *state {
            Ok(providers) => providers,
            Err(message) => return Err(ProviderError::LoadFailed(message.clone())),
        };
        let index = providers
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| ProviderError::UnknownProvider(id.to_string()))?;
        let mut next = providers.clone();
        next.remove(index);
        if let Some(file) = &self.file {
            write_file(file, &next).map_err(ProviderError::SaveFailed)?;
        }
        *providers = next;
        Ok(())
    }

    pub fn update_cached_models(&self, id: &str, models: Vec<String>) -> Result<(), ProviderError> {
        let mut state = self.lock();
        let providers = match &mut *state {
            Ok(providers) => providers,
            Err(message) => return Err(ProviderError::LoadFailed(message.clone())),
        };
        let mut next = providers.clone();
        let target = next
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| ProviderError::UnknownProvider(id.to_string()))?;
        target.models = models;
        if let Some(file) = &self.file {
            write_file(file, &next).map_err(ProviderError::SaveFailed)?;
        }
        *providers = next;
        Ok(())
    }

    fn lock(&self) -> MutexGuard<'_, Result<Vec<ProviderConfig>, String>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub(crate) fn initial_presets() -> Vec<CliPreset> {
    initial_providers()
        .into_iter()
        .map(ProviderConfig::into_preset)
        .collect()
}

fn default_rate_limit_patterns() -> Vec<String> {
    vec_str(&[
        "usage_limit_reached",
        "hit your limit",
        "out of extra usage",
        "rate_limit_error",
        "limite esgotado",
        "429",
        "quota",
        "resource_exhausted",
        "rate limit",
        "too many requests",
    ])
}

pub(crate) fn initial_providers() -> Vec<ProviderConfig> {
    let default_rate_limits = default_rate_limit_patterns();
    let default_model_unavailable = vec_str(&[
        "model_not_found",
        "does not have access to",
        "not available for your account",
        "model is not supported",
    ]);

    vec![
        ProviderConfig {
            id: "claude".to_string(),
            label: "Claude Code".to_string(),
            protocol: ProviderProtocol::ClaudeLive,
            default_command: "claude".to_string(),
            default_model: "haiku".to_string(),
            models: vec_str(&["haiku", "sonnet", "opus"]),
            tiers: ModelTiers {
                basic: "haiku".to_string(),
                intermediate: "sonnet".to_string(),
                advanced: "opus".to_string(),
            },
            supports_mcp: true,
            tool_transport: Some(ToolTransport::McpNative),
            supports_sandbox: true,
            reserved_flags: vec_str(&[
                "-p",
                "--output-format",
                "--input-format",
                "--model",
                "--tools",
                "--allowedTools",
                "--mcp-config",
                "--permission-mode",
                "--append-system-prompt",
                "--resume",
            ]),
            rate_limit_patterns: default_rate_limits.clone(),
            model_unavailable_patterns: default_model_unavailable.clone(),
            discovery: None,
            generic: None,
            builtin_seed: true,
        },
        ProviderConfig {
            id: "codex".to_string(),
            label: "Codex".to_string(),
            protocol: ProviderProtocol::CodexJsonl,
            default_command: "codex".to_string(),
            default_model: "gpt-6-luna".to_string(),
            models: vec_str(&["gpt-6-luna", "gpt-5.6-terra", "gpt-5.6-luna"]),
            tiers: ModelTiers {
                basic: "gpt-5.6-luna".to_string(),
                intermediate: "gpt-5.6-terra".to_string(),
                advanced: "gpt-6-luna".to_string(),
            },
            supports_mcp: true,
            tool_transport: Some(ToolTransport::McpNative),
            supports_sandbox: true,
            reserved_flags: vec_str(&[
                "--json",
                "-m",
                "--model",
                "-s",
                "--sandbox",
                "-C",
                "--cd",
                "--add-dir",
                "--dangerously-bypass-approvals-and-sandbox",
                "-c",
                "--config",
                "--approve-for-me",
            ]),
            rate_limit_patterns: default_rate_limits.clone(),
            model_unavailable_patterns: default_model_unavailable.clone(),
            discovery: Some(ModelDiscoveryConfig {
                args: Vec::new(),
                json_path: None,
                cache_file: Some(".codex/models_cache.json".to_string()),
            }),
            generic: None,
            builtin_seed: true,
        },
        ProviderConfig {
            id: "opencode".to_string(),
            label: "OpenCode".to_string(),
            protocol: ProviderProtocol::OpenCodeJson,
            default_command: "opencode".to_string(),
            default_model: "openai/gpt-6-astra".to_string(),
            models: vec_str(&[
                "openai/gpt-6-astra",
                "openai/gpt-5.6-luna",
                "openai/gpt-5.4-mini",
            ]),
            tiers: ModelTiers {
                basic: "openai/gpt-5.4-mini".to_string(),
                intermediate: "openai/gpt-5.6-luna".to_string(),
                advanced: "openai/gpt-6-astra".to_string(),
            },
            supports_mcp: false,
            tool_transport: Some(ToolTransport::VokiaCli),
            supports_sandbox: false,
            reserved_flags: vec_str(&[
                "--pure",
                "--format",
                "-m",
                "--model",
                "--dir",
                "--auto",
                "-s",
                "--session",
                "-c",
                "--continue",
            ]),
            rate_limit_patterns: default_rate_limits.clone(),
            model_unavailable_patterns: default_model_unavailable.clone(),
            discovery: Some(ModelDiscoveryConfig {
                args: vec_str(&["models"]),
                json_path: None,
                cache_file: None,
            }),
            generic: None,
            builtin_seed: true,
        },
        ProviderConfig {
            id: "antigravity".to_string(),
            label: "Antigravity".to_string(),
            protocol: ProviderProtocol::AntigravityStream,
            default_command: "agy".to_string(),
            default_model: "gemini-4-hydrogen-medium".to_string(),
            models: vec_str(&[
                "gemini-4-hydrogen-medium",
                "gemini-4-neon-medium",
                "gemini-4-argon-medium",
                "gemini-3.8-flash-medium",
            ]),
            tiers: ModelTiers {
                basic: "gemini-3.8-flash-medium".to_string(),
                intermediate: "gemini-4-argon-medium".to_string(),
                advanced: "gemini-4-hydrogen-medium".to_string(),
            },
            supports_mcp: false,
            tool_transport: Some(ToolTransport::VokiaCli),
            supports_sandbox: true,
            reserved_flags: vec_str(&[
                "-p",
                "--print",
                "--prompt",
                "--output-format",
                "--input-format",
                "--model",
                "--sandbox",
                "--mode",
                "--dangerously-skip-permissions",
                "--add-dir",
                "--conversation",
                "-c",
                "--continue",
            ]),
            rate_limit_patterns: default_rate_limits,
            model_unavailable_patterns: default_model_unavailable,
            discovery: Some(ModelDiscoveryConfig {
                args: vec_str(&["models"]),
                json_path: None,
                cache_file: None,
            }),
            generic: None,
            builtin_seed: true,
        },
    ]
}

fn vec_str(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

fn load_failed(file: &Path, detail: impl Display) -> ProviderError {
    ProviderError::LoadFailed(format!(
        "Não foi possível carregar os provedores de {}: {detail}.",
        file.display()
    ))
}

fn read_or_seed(dir: &Path, file: &Path) -> Result<Vec<ProviderConfig>, ProviderError> {
    if !file.exists() {
        let seeds = initial_providers();
        fs::create_dir_all(dir).map_err(|error| load_failed(file, error))?;
        write_file(file, &seeds).map_err(|detail| load_failed(file, detail))?;
        return Ok(seeds);
    }
    let text = fs::read_to_string(file).map_err(|error| load_failed(file, error))?;
    let parsed: ProvidersFile =
        serde_json::from_str(&text).map_err(|error| load_failed(file, error))?;
    if parsed.version != FILE_VERSION {
        return Err(load_failed(
            file,
            format!("versão {} desconhecida", parsed.version),
        ));
    }
    let mut providers = parsed.providers;
    let defaults = default_rate_limit_patterns();
    let mut updated = false;
    for provider in &mut providers {
        if !provider.builtin_seed {
            continue;
        }
        for pattern in &defaults {
            if !provider.rate_limit_patterns.iter().any(|p| p == pattern) {
                provider.rate_limit_patterns.push(pattern.clone());
                updated = true;
            }
        }
    }
    if updated {
        write_file(file, &providers).map_err(|detail| load_failed(file, detail))?;
    }
    Ok(providers)
}

fn write_file(file: &Path, providers: &[ProviderConfig]) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&ProvidersFile {
        version: FILE_VERSION,
        providers: providers.to_vec(),
    })
    .map_err(|error| error.to_string())?;
    write_atomic(file, PROVIDERS_FILE_TMP, &text)
}

pub fn parse_models_lines(output: &str) -> Vec<String> {
    let mut models = Vec::new();
    for raw in output.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with("---")
            || trimmed.starts_with("===")
            || trimmed.ends_with(':')
        {
            continue;
        }
        if let Some(token) = trimmed.split_whitespace().next() {
            if !token.is_empty() && !models.iter().any(|existing| existing == token) {
                models.push(token.to_string());
            }
        }
    }
    models
}

pub fn parse_codex_models_cache(json_text: &str) -> Result<Vec<String>, String> {
    let value: serde_json::Value =
        serde_json::from_str(json_text).map_err(|error| format!("JSON inválido: {error}"))?;
    let items = if let Some(array) = value.as_array() {
        array
    } else if let Some(array) = value.get("models").and_then(|v| v.as_array()) {
        array
    } else if let Some(array) = value.get("data").and_then(|v| v.as_array()) {
        array
    } else {
        return Err("Formato de models_cache.json sem array de modelos.".to_string());
    };
    Ok(extract_model_ids(items))
}

pub fn parse_json_models(json_text: &str, json_path: Option<&str>) -> Result<Vec<String>, String> {
    let value: serde_json::Value =
        serde_json::from_str(json_text).map_err(|error| format!("JSON inválido: {error}"))?;
    let mut current = &value;
    if let Some(path) = json_path.filter(|p| !p.trim().is_empty()) {
        for segment in path.split('.') {
            current = current
                .get(segment)
                .ok_or_else(|| format!("Caminho JSON '{path}' não encontrado."))?;
        }
    }
    let items = current
        .as_array()
        .ok_or_else(|| "Resposta JSON não contém um array de modelos.".to_string())?;
    Ok(extract_model_ids(items))
}

fn extract_model_ids(items: &[serde_json::Value]) -> Vec<String> {
    let mut models = Vec::new();
    for item in items {
        let candidate = item
            .as_str()
            .or_else(|| item.get("slug").and_then(|v| v.as_str()))
            .or_else(|| item.get("id").and_then(|v| v.as_str()))
            .or_else(|| item.get("name").and_then(|v| v.as_str()));
        if let Some(name) = candidate.map(str::trim).filter(|s| !s.is_empty()) {
            if !models.iter().any(|existing| existing == name) {
                models.push(name.to_string());
            }
        }
    }
    models
}
