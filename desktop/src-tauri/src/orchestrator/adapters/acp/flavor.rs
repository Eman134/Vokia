use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value as Json};
use tokio::process::Command;

use crate::claude::{base_command, Access, ClaudeError};
use crate::mcp::RelayEndpoint;
use crate::providers::ProviderConfig;

pub const OPENCODE_DISABLED_TOOLS: [&str; 3] = [
    "question",
    "opencode_list_mcp_resources",
    "opencode_read_mcp_resource",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcpSteeringMode {
    CustomMethod(&'static str),
    SecondPrompt,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredModel {
    pub id: String,
    pub is_free: bool,
}

pub struct AcpSpawnSpec {
    pub command: Command,
    pub temp_files: Vec<PathBuf>,
}

pub trait AcpFlavor: Send + Sync {
    #[cfg_attr(not(test), allow(dead_code))]
    fn flavor_id(&self) -> &'static str;

    fn build_spawn_spec(
        &self,
        program: &Path,
        user_args: &[String],
        access: Access<'_>,
        relay: Option<&RelayEndpoint>,
        temp_dir: &Path,
    ) -> Result<AcpSpawnSpec, ClaudeError>;

    fn sort_models(&self, models: Vec<DiscoveredModel>) -> Vec<String> {
        let mut out = Vec::with_capacity(models.len());
        for m in models {
            if !out.contains(&m.id) {
                out.push(m.id);
            }
        }
        out
    }

    fn relist_delay(&self) -> Option<Duration> {
        None
    }

    fn steering_mode(&self) -> AcpSteeringMode {
        AcpSteeringMode::SecondPrompt
    }

    fn wakes_after_prompt_ends(&self) -> bool {
        false
    }

    fn checks_background_via_session_list(&self) -> bool {
        false
    }

    fn supports_context_rotation(&self) -> bool {
        false
    }

    fn is_lost_session_error(&self, code: i64, message: &str) -> bool {
        if matches!(code, -32602 | -32002) {
            return true;
        }
        let lower = message.to_ascii_lowercase();
        lower.contains("session not found")
            || lower.contains("unknown session")
            || lower.contains("invalid session")
    }
}

pub fn resolve_flavor(provider: Option<&ProviderConfig>, program: &Path) -> Box<dyn AcpFlavor> {
    let id = provider
        .map(|p| p.id.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let exe_stem = program
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if id == "opencode" || exe_stem == "opencode" {
        return Box::new(OpenCodeFlavor);
    }
    if matches!(id.as_str(), "antigravity" | "gemini" | "gemini-cli")
        || matches!(exe_stem.as_str(), "agy" | "antigravity" | "gemini")
    {
        let is_gemini_cli = id == "gemini-cli" || exe_stem == "gemini";
        return Box::new(GeminiFlavor { is_gemini_cli });
    }
    Box::new(GenericAcpFlavor)
}

pub struct OpenCodeFlavor;

impl AcpFlavor for OpenCodeFlavor {
    fn flavor_id(&self) -> &'static str {
        "opencode"
    }

    fn build_spawn_spec(
        &self,
        program: &Path,
        user_args: &[String],
        access: Access<'_>,
        relay: Option<&RelayEndpoint>,
        temp_dir: &Path,
    ) -> Result<AcpSpawnSpec, ClaudeError> {
        let mut command = base_acp_command(program, user_args, access);
        if !has_subcommand(user_args, "acp") {
            command.arg("acp");
        }

        let mut temp_files = Vec::new();
        let mut tools_map = serde_json::Map::new();
        for tool in OPENCODE_DISABLED_TOOLS {
            tools_map.insert(tool.to_string(), Json::Bool(false));
        }

        let mut cfg = serde_json::Map::new();
        cfg.insert("tools".to_string(), Json::Object(tools_map));

        if let Some(endpoint) = relay {
            let mut mcp_map = serde_json::Map::new();
            for (name, url) in &endpoint.server_urls {
                mcp_map.insert(
                    name.clone(),
                    json!({
                        "type": "remote",
                        "url": url,
                        "headers": {
                            "Authorization": format!("Bearer {}", endpoint.token)
                        },
                        "enabled": true
                    }),
                );
            }
            cfg.insert("mcp".to_string(), Json::Object(mcp_map));
        }

        std::fs::create_dir_all(temp_dir).map_err(|err| {
            ClaudeError::Mcp(format!("falha ao criar diretório temporário do OpenCode: {err}"))
        })?;
        let unique = format!(
            "opencode-acp-{}-{}.json",
            std::process::id(),
            crate::mcp::random_hex(6).unwrap_or_else(|_| "0".to_string())
        );
        let config_path = temp_dir.join(unique);
        let config_json = serde_json::to_string_pretty(&Json::Object(cfg))
            .map_err(|err| ClaudeError::Mcp(err.to_string()))?;
        std::fs::write(&config_path, config_json).map_err(|err| {
            ClaudeError::Mcp(format!("falha ao gravar OPENCODE_CONFIG temporário: {err}"))
        })?;
        command.env("OPENCODE_CONFIG", &config_path);
        temp_files.push(config_path);

        Ok(AcpSpawnSpec {
            command,
            temp_files,
        })
    }

    fn sort_models(&self, models: Vec<DiscoveredModel>) -> Vec<String> {
        let mut free = Vec::new();
        let mut paid = Vec::new();
        for model in models {
            let is_free = model.is_free || is_opencode_free_model_id(&model.id);
            if is_free {
                if !free.contains(&model.id) {
                    free.push(model.id);
                }
            } else if !paid.contains(&model.id) {
                paid.push(model.id);
            }
        }
        free.extend(paid);
        free
    }

    fn relist_delay(&self) -> Option<Duration> {
        Some(Duration::from_secs(5))
    }

    fn steering_mode(&self) -> AcpSteeringMode {
        AcpSteeringMode::CustomMethod("_session/steering")
    }

    fn checks_background_via_session_list(&self) -> bool {
        true
    }
}

pub struct GeminiFlavor {
    pub is_gemini_cli: bool,
}

impl AcpFlavor for GeminiFlavor {
    fn flavor_id(&self) -> &'static str {
        if self.is_gemini_cli {
            "gemini-cli"
        } else {
            "gemini"
        }
    }

    fn build_spawn_spec(
        &self,
        program: &Path,
        user_args: &[String],
        access: Access<'_>,
        _relay: Option<&RelayEndpoint>,
        temp_dir: &Path,
    ) -> Result<AcpSpawnSpec, ClaudeError> {
        let mut command = base_acp_command(program, user_args, access);
        if self.is_gemini_cli {
            if !has_flag(user_args, "--experimental-acp") && !has_subcommand(user_args, "acp") {
                command.arg("--experimental-acp");
            }
        } else {
            if !has_subcommand(user_args, "acp") {
                command.arg("acp");
            }
            if !has_flag(user_args, "--gemini-dir") {
                let abs_gemini_dir = resolve_absolute_gemini_dir(access, temp_dir);
                command.arg("--gemini-dir").arg(abs_gemini_dir);
            }
            if !has_flag(user_args, "--app-data-dir") {
                command.arg("--app-data-dir").arg(".vokia-agy-data");
            }
        }
        Ok(AcpSpawnSpec {
            command,
            temp_files: Vec::new(),
        })
    }

    fn wakes_after_prompt_ends(&self) -> bool {
        true
    }

    fn supports_context_rotation(&self) -> bool {
        true
    }
}

pub struct GenericAcpFlavor;

impl AcpFlavor for GenericAcpFlavor {
    fn flavor_id(&self) -> &'static str {
        "generic-acp"
    }

    fn build_spawn_spec(
        &self,
        program: &Path,
        user_args: &[String],
        access: Access<'_>,
        _relay: Option<&RelayEndpoint>,
        _temp_dir: &Path,
    ) -> Result<AcpSpawnSpec, ClaudeError> {
        let mut command = base_acp_command(program, user_args, access);
        if !has_subcommand(user_args, "acp") {
            command.arg("acp");
        }
        Ok(AcpSpawnSpec {
            command,
            temp_files: Vec::new(),
        })
    }
}

fn base_acp_command(program: &Path, user_args: &[String], access: Access<'_>) -> Command {
    let mut command = base_command(program);
    command.kill_on_drop(true);
    command.args(user_args);
    if let Access::Project(project) = access {
        command.current_dir(project.dir);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        command.process_group(0);
    }
    command
}

fn has_subcommand(args: &[String], sub: &str) -> bool {
    args.iter().any(|arg| arg == sub)
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter()
        .any(|arg| arg == flag || arg.starts_with(&format!("{flag}=")))
}

fn resolve_absolute_gemini_dir(access: Access<'_>, temp_dir: &Path) -> PathBuf {
    if let Access::Project(project) = access {
        let joined = project.dir.join(".gemini");
        if joined.is_absolute() {
            return joined;
        }
    }
    if temp_dir.is_absolute() {
        return temp_dir.join(".gemini");
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".gemini")
}

pub fn is_opencode_free_model_id(id: &str) -> bool {
    let lower = id.trim().to_ascii_lowercase();
    lower.ends_with(":free")
        || lower.starts_with("opencode/")
        || lower.contains("big-pickle")
        || lower.contains("grok-code")
}

pub fn build_acp_mcp_servers(relay: Option<&RelayEndpoint>) -> Vec<Json> {
    let Some(endpoint) = relay else {
        return Vec::new();
    };
    let mut servers = Vec::new();
    let ordered: BTreeMap<&String, &String> = endpoint.server_urls.iter().collect();
    for (name, url) in ordered {
        servers.push(json!({
            "name": name,
            "type": "http",
            "url": url,
            "headers": [
                { "name": "Authorization", "value": format!("Bearer {}", endpoint.token) }
            ]
        }));
    }
    servers
}
