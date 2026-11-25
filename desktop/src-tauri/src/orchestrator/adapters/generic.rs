use std::path::Path;
use std::sync::atomic::AtomicU32;

use serde_json::Value as Json;
use tokio::process::Command;

use crate::claude::{
    base_command, classify_provider_error, Access, ClaudeError, Reply, TurnActivity, TurnUsage,
};
use crate::mcp::McpSession;
use crate::permissions::PermissionLevel;
use crate::providers::ProviderConfig;

pub(crate) fn build_command(
    program: &Path,
    user_args: &[String],
    role: &str,
    model: &str,
    access: Access<'_>,
    message: &str,
    provider: &ProviderConfig,
) -> Result<(Command, Option<String>), ClaudeError> {
    if let Access::Project(project) = access {
        if project.level == PermissionLevel::Liberado && !provider.supports_sandbox {
            return Err(ClaudeError::Reported(format!(
                "{} não oferece sandbox equivalente para o nível Liberado.",
                provider.label
            )));
        }
    }

    let spec = provider.generic.as_ref().cloned().unwrap_or_default();
    let mut command = base_command(program);
    command.kill_on_drop(true);
    command.args(user_args);

    let prompt = format!("{role}\n\n{message}");
    let (dir_str, attachments_str, sandbox_key) = match access {
        Access::TextOnly => ("", "", "text-only"),
        Access::Project(project) => {
            command.current_dir(project.dir);
            (
                project.dir.to_str().unwrap_or(""),
                project.attachments.to_str().unwrap_or(""),
                project.level.as_str(),
            )
        }
    };

    let substitute = |template: &str| -> String {
        template
            .replace("{model}", model)
            .replace("{prompt}", &prompt)
            .replace("{dir}", dir_str)
            .replace("{attachments}", attachments_str)
    };

    for arg in &spec.json_output_args {
        command.arg(substitute(arg));
    }
    if let Some(flag) = &spec.model_flag {
        command.arg(flag).arg(model);
    }
    if let Access::Project(_) = access {
        if let Some(flag) = &spec.dir_flag {
            command.arg(flag).arg(dir_str);
        }
    }
    if let Some(args) = spec.sandbox_args.get(sandbox_key) {
        for arg in args {
            command.arg(substitute(arg));
        }
    }

    let stdin_input = if spec.prompt_via_stdin {
        Some(prompt)
    } else if spec.prompt_args.is_empty() {
        command.arg(&prompt);
        None
    } else {
        for arg in &spec.prompt_args {
            command.arg(substitute(arg));
        }
        None
    };

    Ok((command, stdin_input))
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn run_turn(
    program: &Path,
    user_args: &[String],
    role: &str,
    model: &str,
    access: Access<'_>,
    message: &str,
    pid: &AtomicU32,
    provider: &ProviderConfig,
    vokia_cli: Option<(&McpSession, &Path)>,
) -> Result<Reply, ClaudeError> {
    let (mut command, stdin_input) =
        build_command(program, user_args, role, model, access, message, provider)?;
    if let Some((session, cli_path)) = vokia_cli {
        super::inject_vokia_cli_env(&mut command, session, cli_path);
    }
    let stdout = super::run_cli_process(command, access, pid, stdin_input, Some((provider, model)), |_| {})
        .await?;
    parse_output(&stdout, provider, model)
}

pub(crate) fn parse_output(
    stdout: &[u8],
    provider: &ProviderConfig,
    model: &str,
) -> Result<Reply, ClaudeError> {
    let raw = String::from_utf8_lossy(stdout);
    let Some(spec) = provider.generic.as_ref() else {
        return Ok(Reply {
            text: raw.trim().to_string(),
            limits: None,
            activity: TurnActivity::default(),
            usage: None,
            session_id: None,
        });
    };

    if spec.text_json_paths.is_empty()
        && spec.error_json_paths.is_empty()
        && spec.model_json_path.is_none()
        && spec.input_tokens_json_path.is_none()
        && spec.output_tokens_json_path.is_none()
        && spec.cost_usd_json_path.is_none()
    {
        return Ok(Reply {
            text: raw.trim().to_string(),
            limits: None,
            activity: TurnActivity::default(),
            usage: None,
            session_id: None,
        });
    }

    let mut texts: Vec<String> = Vec::new();
    let mut saw_json = false;
    let mut found_model: Option<String> = None;
    let mut input_tokens: Option<u64> = None;
    let mut output_tokens: Option<u64> = None;
    let mut cost_usd: Option<f64> = None;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Json>(trimmed) else {
            continue;
        };
        saw_json = true;

        for path in &spec.error_json_paths {
            if let Some(error_text) = extract_json_path(&value, path) {
                if !error_text.trim().is_empty() {
                    return Err(classify_provider_error(
                        &provider.id,
                        model,
                        &error_text,
                        &provider.rate_limit_patterns,
                        &provider.model_unavailable_patterns,
                    )
                    .unwrap_or(ClaudeError::Reported(error_text)));
                }
            }
        }

        for path in &spec.text_json_paths {
            if let Some(text) = extract_json_path(&value, path) {
                if !text.trim().is_empty() {
                    texts.push(text);
                }
            }
        }

        if let Some(path) = &spec.model_json_path {
            if let Some(m) = extract_json_path(&value, path) {
                let trimmed_m = m.trim();
                if !trimmed_m.is_empty() {
                    found_model = Some(trimmed_m.to_string());
                }
            }
        }
        if let Some(path) = &spec.input_tokens_json_path {
            if let Some(n) = extract_json_u64(&value, path) {
                input_tokens = Some(n);
            }
        }
        if let Some(path) = &spec.output_tokens_json_path {
            if let Some(n) = extract_json_u64(&value, path) {
                output_tokens = Some(n);
            }
        }
        if let Some(path) = &spec.cost_usd_json_path {
            if let Some(c) = extract_json_f64(&value, path) {
                cost_usd = Some(c);
            }
        }
    }

    let text = if !texts.is_empty() {
        texts.join("\n\n")
    } else if !saw_json {
        raw.trim().to_string()
    } else {
        String::new()
    };

    let usage = if found_model.is_some()
        || input_tokens.is_some()
        || output_tokens.is_some()
        || cost_usd.is_some()
    {
        Some(TurnUsage {
            provider: Some(provider.id.clone()),
            model: found_model.or_else(|| (!model.trim().is_empty()).then(|| model.to_string())),
            input_tokens,
            output_tokens,
            cached_input_tokens: None,
            cache_write_tokens: None,
            thought_tokens: None,
            cost_usd,
            is_fallback: None,
        })
    } else {
        None
    };

    Ok(Reply {
        text,
        limits: None,
        activity: TurnActivity::default(),
        usage,
        session_id: None,
    })
}

fn extract_json_value<'a>(value: &'a Json, path: &str) -> Option<&'a Json> {
    let mut current = value;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

fn extract_json_u64(value: &Json, path: &str) -> Option<u64> {
    let val = extract_json_value(value, path)?;
    val.as_u64()
        .or_else(|| val.as_str().and_then(|s| s.trim().parse().ok()))
}

fn extract_json_f64(value: &Json, path: &str) -> Option<f64> {
    let val = extract_json_value(value, path)?;
    val.as_f64()
        .or_else(|| val.as_str().and_then(|s| s.trim().parse().ok()))
}

fn extract_json_path(value: &Json, path: &str) -> Option<String> {
    let current = extract_json_value(value, path)?;
    current.as_str().map(str::to_string)
}
