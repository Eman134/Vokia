use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::ChildStdin;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::timeout;

use crate::claude::{base_command, kill_process_tree};
use crate::provider_env::{apply_provider_env, redact_registered_secrets, ProviderEnvRegistry};
use crate::providers::{AuthFlowKind, ProviderAuthDiagnosticRules, ProviderConfig};

const VERSION_TIMEOUT_SECS: u64 = 3;
const LOGOUT_TIMEOUT_SECS: u64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderDiagnosticState {
    NotInstalled,
    InstalledSignedOut,
    Connected,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutableSource {
    Path,
    KnownDir,
    Explicit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedExecutable {
    pub program: PathBuf,
    pub base_args: Vec<String>,
    pub source: ExecutableSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticClassification {
    pub category: String, // "unauthorized_401" | "forbidden_403" | "incompatible" | "other"
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDiagnosticResult {
    pub installed: bool,
    pub state: ProviderDiagnosticState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_source: Option<ExecutableSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_category: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LoginSessionPhase {
    Starting,
    AwaitingBrowser,
    AwaitingDeviceCode,
    AwaitingPasteCode,
    Completed,
    Cancelled,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderLoginState {
    pub provider_id: String,
    pub flow_id: String,
    pub kind: AuthFlowKind,
    pub phase: LoginSessionPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_category: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderLogoutResult {
    pub ok: bool,
    pub provider_id: String,
    pub shares_terminal_login: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    pub message: String,
}

struct ActiveLoginSession {
    state: ProviderLoginState,
    pid: Option<u32>,
    stdin: Option<Arc<AsyncMutex<ChildStdin>>>,
}

#[derive(Default)]
pub struct ProviderLoginManager {
    sessions: Arc<Mutex<HashMap<String, ActiveLoginSession>>>,
}

impl ProviderLoginManager {
    pub fn get_state(&self, provider_id: &str) -> Option<ProviderLoginState> {
        let guard = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        guard.get(provider_id).map(|s| s.state.clone())
    }

    pub async fn start_login(
        &self,
        config: &ProviderConfig,
        flow_id: Option<&str>,
        api_key: Option<&str>,
        env_registry: &ProviderEnvRegistry,
    ) -> Result<ProviderLoginState, String> {
        let flow = select_auth_flow(config, flow_id)?;

        // Cancela qualquer login anterior do mesmo provedor antes de iniciar outro.
        let _ = self.cancel_login(&config.id);

        match flow.kind {
            AuthFlowKind::FreeNoLogin => {
                let state = ProviderLoginState {
                    provider_id: config.id.clone(),
                    flow_id: flow.id.clone(),
                    kind: flow.kind,
                    phase: LoginSessionPhase::Completed,
                    url: None,
                    user_code: None,
                    message: Some("Modo gratuito disponível sem login.".to_string()),
                    error_category: None,
                };
                self.set_session_state(&config.id, state.clone(), None, None);
                Ok(state)
            }
            AuthFlowKind::ApiKey => {
                let env_key = flow
                    .env_var
                    .as_deref()
                    .filter(|s| !s.trim().is_empty())
                    .ok_or_else(|| {
                        format!(
                            "Fluxo '{}' do provedor '{}' não define variável de chave.",
                            flow.id, config.label
                        )
                    })?;
                let key_value = api_key
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| format!("Informe o valor para {env_key}."))?;
                env_registry.set_var(&config.id, env_key, key_value, true)?;
                let state = ProviderLoginState {
                    provider_id: config.id.clone(),
                    flow_id: flow.id.clone(),
                    kind: flow.kind,
                    phase: LoginSessionPhase::Completed,
                    url: None,
                    user_code: None,
                    message: Some(format!("{env_key} salva no cofre seguro do sistema.")),
                    error_category: None,
                };
                self.set_session_state(&config.id, state.clone(), None, None);
                Ok(state)
            }
            AuthFlowKind::Browser | AuthFlowKind::DeviceCode | AuthFlowKind::PasteCode => {
                let resolved = resolve_provider_executable(&config.default_command)?;
                let mut args = resolved.base_args.clone();
                args.extend(flow.command_args.iter().cloned());

                let mut cmd = base_command(&resolved.program);
                let env_map = env_registry.resolve_env_for_provider(&config.id);
                apply_provider_env(&mut cmd, &env_map);
                cmd.args(&args)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                let mut child = cmd
                    .spawn()
                    .map_err(|err| format!("Falha ao iniciar login de {}: {err}", config.label))?;
                let pid = child.id();
                let stdin = child.stdin.take().map(|s| Arc::new(AsyncMutex::new(s)));
                let stdout = child.stdout.take();
                let stderr = child.stderr.take();

                let initial_phase = match flow.kind {
                    AuthFlowKind::DeviceCode => LoginSessionPhase::AwaitingDeviceCode,
                    AuthFlowKind::PasteCode => LoginSessionPhase::AwaitingPasteCode,
                    _ => LoginSessionPhase::AwaitingBrowser,
                };
                let initial_state = ProviderLoginState {
                    provider_id: config.id.clone(),
                    flow_id: flow.id.clone(),
                    kind: flow.kind,
                    phase: initial_phase,
                    url: None,
                    user_code: None,
                    message: Some(match flow.kind {
                        AuthFlowKind::DeviceCode => {
                            "Aguardando link e código de dispositivo da CLI...".to_string()
                        }
                        AuthFlowKind::PasteCode => {
                            "Conclua a autorização no link e cole o código abaixo.".to_string()
                        }
                        _ => "Aguardando conclusão do login no navegador...".to_string(),
                    }),
                    error_category: None,
                };
                self.set_session_state(&config.id, initial_state.clone(), pid, stdin);

                let sessions_ref = Arc::clone(&self.sessions);
                let provider_id = config.id.clone();
                let diag_rules = config.auth_diagnostics.clone();

                tokio::spawn(async move {
                    let collected_output = Arc::new(AsyncMutex::new(String::new()));
                    let out_sessions = Arc::clone(&sessions_ref);
                    let out_pid = provider_id.clone();
                    let out_buf = Arc::clone(&collected_output);
                    let stdout_task = stdout.map(|stream| {
                        tokio::spawn(async move {
                            let mut lines = BufReader::new(stream).lines();
                            while let Ok(Some(line)) = lines.next_line().await {
                                let clean = redact_registered_secrets(&line);
                                {
                                    let mut buf = out_buf.lock().await;
                                    if buf.len() < 8192 {
                                        buf.push_str(&clean);
                                        buf.push('\n');
                                    }
                                }
                                update_login_output(&out_sessions, &out_pid, &clean);
                            }
                        })
                    });

                    let err_sessions = Arc::clone(&sessions_ref);
                    let err_pid = provider_id.clone();
                    let err_buf = Arc::clone(&collected_output);
                    let stderr_task = stderr.map(|stream| {
                        tokio::spawn(async move {
                            let mut lines = BufReader::new(stream).lines();
                            while let Ok(Some(line)) = lines.next_line().await {
                                let clean = redact_registered_secrets(&line);
                                {
                                    let mut buf = err_buf.lock().await;
                                    if buf.len() < 8192 {
                                        buf.push_str(&clean);
                                        buf.push('\n');
                                    }
                                }
                                update_login_output(&err_sessions, &err_pid, &clean);
                            }
                        })
                    });

                    let status = child.wait().await;
                    if let Some(t) = stdout_task {
                        let _ = t.await;
                    }
                    if let Some(t) = stderr_task {
                        let _ = t.await;
                    }
                    let combined = collected_output.lock().await.clone();

                    let mut guard = sessions_ref.lock().unwrap_or_else(PoisonError::into_inner);
                    if let Some(session) = guard.get_mut(&provider_id) {
                        session.pid = None;
                        session.stdin = None;
                        if session.state.phase == LoginSessionPhase::Cancelled {
                            return;
                        }
                        match status {
                            Ok(exit) if exit.success() => {
                                session.state.phase = LoginSessionPhase::Completed;
                                session.state.message =
                                    Some("Autenticação concluída com sucesso.".to_string());
                                session.state.error_category = None;
                            }
                            Ok(exit) => {
                                let classified = classify_auth_diagnostic_error(
                                    &combined,
                                    diag_rules.as_ref(),
                                );
                                session.state.phase = LoginSessionPhase::Error;
                                session.state.error_category = Some(classified.category);
                                session.state.message = Some(if classified.message.trim().is_empty() {
                                    format!("Processo de login encerrou com código {:?}.", exit.code())
                                } else {
                                    classified.message
                                });
                            }
                            Err(err) => {
                                session.state.phase = LoginSessionPhase::Error;
                                session.state.error_category = Some("other".to_string());
                                session.state.message =
                                    Some(format!("Erro ao aguardar processo de login: {err}"));
                            }
                        }
                    }
                });

                Ok(initial_state)
            }
        }
    }

    pub async fn submit_code(&self, provider_id: &str, code: &str) -> Result<ProviderLoginState, String> {
        let trimmed = code.trim();
        if trimmed.is_empty() {
            return Err("Informe o código de autenticação.".to_string());
        }
        let (stdin_handle, state) = {
            let guard = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
            let session = guard
                .get(provider_id)
                .ok_or_else(|| format!("Nenhum login ativo para '{provider_id}'."))?;
            (session.stdin.clone(), session.state.clone())
        };
        let stdin_arc = stdin_handle.ok_or_else(|| {
            "O processo de login não está aceitando entrada via stdin.".to_string()
        })?;
        // Envia código apenas pelo stdin da CLI, nunca para logs.
        let mut stdin = stdin_arc.lock().await;
        stdin
            .write_all(format!("{trimmed}\n").as_bytes())
            .await
            .map_err(|err| format!("Falha ao enviar código para a CLI: {err}"))?;
        let _ = stdin.flush().await;
        Ok(state)
    }

    pub fn cancel_login(&self, provider_id: &str) -> Result<ProviderLoginState, String> {
        let mut guard = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(session) = guard.get_mut(provider_id) else {
            return Ok(ProviderLoginState {
                provider_id: provider_id.to_string(),
                flow_id: String::new(),
                kind: AuthFlowKind::Browser,
                phase: LoginSessionPhase::Cancelled,
                url: None,
                user_code: None,
                message: Some("Nenhum login em andamento.".to_string()),
                error_category: None,
            });
        };
        if let Some(pid) = session.pid.take() {
            kill_process_tree(pid);
        }
        session.stdin = None;
        session.state.phase = LoginSessionPhase::Cancelled;
        session.state.message = Some("Login cancelado.".to_string());
        Ok(session.state.clone())
    }

    fn set_session_state(
        &self,
        provider_id: &str,
        state: ProviderLoginState,
        pid: Option<u32>,
        stdin: Option<Arc<AsyncMutex<ChildStdin>>>,
    ) {
        let mut guard = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        guard.insert(
            provider_id.to_string(),
            ActiveLoginSession { state, pid, stdin },
        );
    }
}

fn select_auth_flow<'a>(
    config: &'a ProviderConfig,
    flow_id: Option<&str>,
) -> Result<&'a crate::providers::ProviderAuthFlowSpec, String> {
    if config.auth_flows.is_empty() {
        return Err(format!(
            "O provedor '{}' não possui fluxos de autenticação declarados.",
            config.label
        ));
    }
    if let Some(wanted) = flow_id.map(str::trim).filter(|s| !s.is_empty()) {
        config
            .auth_flows
            .iter()
            .find(|flow| flow.id == wanted)
            .ok_or_else(|| {
                format!(
                    "Fluxo de autenticação '{wanted}' não encontrado para '{}'.",
                    config.label
                )
            })
    } else {
        Ok(&config.auth_flows[0])
    }
}

fn update_login_output(
    sessions: &Arc<Mutex<HashMap<String, ActiveLoginSession>>>,
    provider_id: &str,
    line: &str,
) {
    let url = extract_first_url(line);
    let user_code = extract_device_code(line);
    if url.is_none() && user_code.is_none() {
        return;
    }
    let mut guard = sessions.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(session) = guard.get_mut(provider_id) {
        if session.state.url.is_none() {
            if let Some(u) = url {
                session.state.url = Some(u);
            }
        }
        if session.state.user_code.is_none() {
            if let Some(c) = user_code {
                session.state.user_code = Some(c);
            }
        }
    }
}

pub fn extract_first_url(text: &str) -> Option<String> {
    for token in text.split_whitespace() {
        let trimmed = token.trim_matches(|ch: char| matches!(ch, '"' | '\'' | '(' | ')' | ',' | '.'));
        if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
            return Some(trimmed.to_string());
        }
    }
    None
}

pub fn extract_device_code(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let has_code_hint = lower.contains("code") || lower.contains("código");
    for token in text.split_whitespace() {
        let clean = token.trim_matches(|ch: char| {
            matches!(ch, '"' | '\'' | '(' | ')' | ',' | '.' | ':' | ';' | '[' | ']')
        });
        // Padrão clássico de device code: XXXX-YYYY (4+ chars alfanuméricos maiúsculos com hífen)
        if let Some((left, right)) = clean.split_once('-') {
            if (3..=6).contains(&left.len())
                && (3..=6).contains(&right.len())
                && left.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
                && right.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            {
                return Some(clean.to_string());
            }
        }
        if has_code_hint
            && (6..=12).contains(&clean.len())
            && clean.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            && clean.chars().any(|c| c.is_ascii_digit())
        {
            return Some(clean.to_string());
        }
    }
    None
}

pub fn resolve_provider_executable(command: &str) -> Result<ResolvedExecutable, String> {
    let trimmed = command.trim();
    let direct_path = Path::new(trimmed);
    if direct_path.is_file() {
        return Ok(ResolvedExecutable {
            program: direct_path.to_path_buf(),
            base_args: Vec::new(),
            source: ExecutableSource::Explicit,
        });
    }
    let tokens = shlex::split(trimmed)
        .ok_or_else(|| "Comando inválido: aspas não fechadas.".to_string())?;
    let Some((program, rest)) = tokens.split_first() else {
        return Err("Comando do provedor não pode ficar vazio.".to_string());
    };
    let base_args = rest.to_vec();

    let prog_path = Path::new(program);
    if prog_path.is_absolute() || program.contains('/') || program.contains('\\') {
        if prog_path.is_file() {
            return Ok(ResolvedExecutable {
                program: prog_path.to_path_buf(),
                base_args,
                source: ExecutableSource::Explicit,
            });
        }
        return Err(format!("Executável '{program}' não encontrado no caminho informado."));
    }

    if let Ok(found) = which::which(program) {
        return Ok(ResolvedExecutable {
            program: found,
            base_args,
            source: ExecutableSource::Path,
        });
    }

    for dir in known_cli_dirs() {
        for candidate in executable_candidates(&dir, program) {
            if candidate.is_file() {
                return Ok(ResolvedExecutable {
                    program: candidate,
                    base_args,
                    source: ExecutableSource::KnownDir,
                });
            }
        }
    }

    Err(format!(
        "Executável '{program}' não encontrado no PATH nem em pastas conhecidas."
    ))
}

fn known_cli_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    if let Some(h) = &home {
        dirs.push(h.join(".local").join("bin"));
        dirs.push(h.join(".npm-global").join("bin"));
        dirs.push(h.join(".bun").join("bin"));
    }
    #[cfg(windows)]
    {
        if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
            dirs.push(appdata.join("npm"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            dirs.push(local.join("Programs"));
        }
    }
    #[cfg(not(windows))]
    {
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
    }
    dirs
}

fn executable_candidates(dir: &Path, program: &str) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        vec![
            dir.join(format!("{program}.exe")),
            dir.join(format!("{program}.cmd")),
            dir.join(format!("{program}.bat")),
            dir.join(program),
        ]
    }
    #[cfg(not(windows))]
    {
        vec![dir.join(program)]
    }
}

pub fn classify_auth_diagnostic_error(
    raw_output: &str,
    rules: Option<&ProviderAuthDiagnosticRules>,
) -> DiagnosticClassification {
    let clean = redact_registered_secrets(raw_output);
    let lower = clean.to_ascii_lowercase();
    let default_rules = ProviderAuthDiagnosticRules::default();
    let r = rules.unwrap_or(&default_rules);

    let incompatible_hit = r
        .incompatible_patterns
        .iter()
        .any(|p| lower.contains(&p.to_ascii_lowercase()))
        || lower.contains("settings.json")
        || lower.contains("schema version")
        || lower.contains("newer version");
    if incompatible_hit {
        return DiagnosticClassification {
            category: "incompatible".to_string(),
            message: format!(
                "CLI ou banco local incompatível: {}",
                clean.lines().next().unwrap_or("verifique a versão instalada.")
            ),
        };
    }

    let forbidden_hit = r
        .forbidden_403_patterns
        .iter()
        .any(|p| lower.contains(&p.to_ascii_lowercase()))
        || lower.contains("403")
        || lower.contains("forbidden");
    if forbidden_hit {
        let hint = r.forbidden_403_hint.clone().unwrap_or_else(|| {
            "Requisição recusada (403): verifique plano ativo, permissões da conta ou rede/VPN."
                .to_string()
        });
        return DiagnosticClassification {
            category: "forbidden_403".to_string(),
            message: hint,
        };
    }

    let unauthorized_hit = r
        .unauthorized_401_patterns
        .iter()
        .any(|p| lower.contains(&p.to_ascii_lowercase()))
        || lower.contains("401")
        || lower.contains("unauthorized");
    if unauthorized_hit {
        let hint = r.unauthorized_401_hint.clone().unwrap_or_else(|| {
            "Credencial recusada (401). Verifique se a chave ou sessão ainda é válida.".to_string()
        });
        return DiagnosticClassification {
            category: "unauthorized_401".to_string(),
            message: hint,
        };
    }

    DiagnosticClassification {
        category: "other".to_string(),
        message: clean.trim().to_string(),
    }
}

pub fn diagnose_provider_sync(
    config: &ProviderConfig,
    env_registry: Option<&ProviderEnvRegistry>,
) -> ProviderDiagnosticResult {
    let resolved = match resolve_provider_executable(&config.default_command) {
        Ok(res) => res,
        Err(_) => {
            return ProviderDiagnosticResult {
                installed: false,
                state: ProviderDiagnosticState::NotInstalled,
                version: None,
                executable_path: None,
                executable_source: None,
                detail: Some(
                    config
                        .install_hint
                        .clone()
                        .unwrap_or_else(|| "CLI não encontrada no PATH nem em pastas conhecidas.".to_string()),
                ),
                error_category: None,
            };
        }
    };

    let version = probe_version_sync(&resolved.program, &resolved.base_args, config);
    let executable_path = Some(resolved.program.display().to_string());
    let executable_source = Some(resolved.source);

    if let Err((category, err_msg)) = version {
        return ProviderDiagnosticResult {
            installed: true,
            state: ProviderDiagnosticState::Error,
            version: None,
            executable_path,
            executable_source,
            detail: Some(err_msg),
            error_category: Some(category),
        };
    }
    let version_str = version.ok().flatten();

    let Some(probe) = &config.auth_probe else {
        return ProviderDiagnosticResult {
            installed: true,
            state: ProviderDiagnosticState::Connected,
            version: version_str,
            executable_path,
            executable_source,
            detail: None,
            error_category: None,
        };
    };

    let has_process_env = probe.env_vars.iter().any(|name| {
        std::env::var(name)
            .ok()
            .is_some_and(|value| !value.trim().is_empty())
    });
    let has_stored_env = env_registry
        .is_some_and(|reg| reg.has_saved_var(&config.id, &probe.env_vars));
    let has_cred_file = probe.credential_files.iter().any(|rel| {
        resolve_user_path(rel)
            .ok()
            .and_then(|path| std::fs::metadata(path).ok())
            .is_some_and(|meta| meta.is_file() && meta.len() > 0)
    });

    if has_process_env || has_stored_env || has_cred_file {
        return ProviderDiagnosticResult {
            installed: true,
            state: ProviderDiagnosticState::Connected,
            version: version_str,
            executable_path,
            executable_source,
            detail: None,
            error_category: None,
        };
    }

    if probe.free_without_login {
        return ProviderDiagnosticResult {
            installed: true,
            state: ProviderDiagnosticState::Connected,
            version: version_str,
            executable_path,
            executable_source,
            detail: Some("Modo gratuito disponível sem login.".to_string()),
            error_category: None,
        };
    }

    ProviderDiagnosticResult {
        installed: true,
        state: ProviderDiagnosticState::InstalledSignedOut,
        version: version_str,
        executable_path,
        executable_source,
        detail: Some("CLI instalada, mas sem autenticação detectada.".to_string()),
        error_category: None,
    }
}

fn probe_version_sync(
    program: &Path,
    base_args: &[String],
    config: &ProviderConfig,
) -> Result<Option<String>, (String, String)> {
    let version_args = config
        .auth_diagnostics
        .as_ref()
        .map(|d| d.version_args.clone())
        .filter(|args| !args.is_empty())
        .unwrap_or_else(|| vec!["--version".to_string()]);

    let mut cmd = std::process::Command::new(program);
    cmd.args(base_args)
        .args(&version_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }

    let Ok(output) = cmd.output() else {
        return Ok(None);
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    if !output.status.success() {
        let classified = classify_auth_diagnostic_error(&combined, config.auth_diagnostics.as_ref());
        if classified.category != "other" {
            return Err((classified.category, classified.message));
        }
        return Ok(None);
    }
    let first_line = stdout
        .lines()
        .chain(stderr.lines())
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string);
    Ok(first_line)
}

pub async fn logout_provider(
    config: &ProviderConfig,
    env_registry: &ProviderEnvRegistry,
) -> Result<ProviderLogoutResult, String> {
    let spec = config.logout.as_ref();
    let shares_terminal_login = spec.is_some_and(|s| s.shares_terminal_login);
    let warning = spec.and_then(|s| s.warning.clone());

    if let Some(s) = spec {
        for var_name in &s.clear_env_vars {
            let _ = env_registry.delete_var(&config.id, var_name);
        }
        if !s.command_args.is_empty() {
            if let Ok(resolved) = resolve_provider_executable(&config.default_command) {
                let mut args = resolved.base_args;
                args.extend(s.command_args.iter().cloned());
                let mut cmd = base_command(&resolved.program);
                cmd.args(&args)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                if let Ok(mut child) = cmd.spawn() {
                    let pid = child.id();
                    if timeout(Duration::from_secs(LOGOUT_TIMEOUT_SECS), child.wait())
                        .await
                        .is_err()
                    {
                        if let Some(p) = pid {
                            kill_process_tree(p);
                        }
                    }
                }
            }
        }
    }

    let _ = VERSION_TIMEOUT_SECS; // mantido para diagnóstico futuro assíncrono
    Ok(ProviderLogoutResult {
        ok: true,
        provider_id: config.id.clone(),
        shares_terminal_login,
        warning,
        message: format!("Sessão de '{}' desconectada.", config.label),
    })
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
