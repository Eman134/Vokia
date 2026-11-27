use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::collections::BTreeMap;
#[cfg(windows)]
use std::ffi::OsString;
#[cfg(windows)]
use std::fs::File;
#[cfg(windows)]
use std::io::{Read, Write};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::io::{FromRawHandle, RawHandle};
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt;
use std::pin::Pin;
use std::process::ExitStatus;
#[cfg(windows)]
use std::sync::OnceLock;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const MAX_SHIM_BYTES: u64 = 64 * 1024;
#[cfg(windows)]
const PIPE_BUFFER_SIZE: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDirectCommand {
    pub program: PathBuf,
    pub prefix_args: Vec<String>,
}

/// Resolve um programa para execução direta, evitando shims `.cmd`/`.bat` no Windows.
pub fn resolve_direct_executable(program: &Path) -> ResolvedDirectCommand {
    #[cfg(not(windows))]
    {
        ResolvedDirectCommand {
            program: program.to_path_buf(),
            prefix_args: Vec::new(),
        }
    }

    #[cfg(windows)]
    {
        let resolved = if program.extension().is_none() {
            which::which(program).unwrap_or_else(|_| program.to_path_buf())
        } else {
            program.to_path_buf()
        };

        if !is_batch_extension(&resolved) {
            return ResolvedDirectCommand {
                program: resolved,
                prefix_args: Vec::new(),
            };
        }

        let sibling_exe = resolved.with_extension("exe");
        if sibling_exe.is_file() {
            return ResolvedDirectCommand {
                program: sibling_exe,
                prefix_args: Vec::new(),
            };
        }

        if let Some(content) = read_shim_capped(&resolved) {
            if let Some(shim_dir) = resolved.parent() {
                if let Some(parsed) = parse_cmd_shim(shim_dir, &content) {
                    return parsed;
                }
            }
        }

        ResolvedDirectCommand {
            program: resolved,
            prefix_args: Vec::new(),
        }
    }
}

#[cfg(windows)]
fn is_batch_extension(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"))
}

#[cfg(windows)]
fn read_shim_capped(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > MAX_SHIM_BYTES {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// Analisa o conteúdo de um shim `.cmd`/`.bat` (npm, pnpm, yarn ou wrapper `.exe`)
/// retornando o executável real e argumentos prefixados quando seguro e verificável em disco.
pub fn parse_cmd_shim(shim_dir: &Path, content: &str) -> Option<ResolvedDirectCommand> {
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('@') && line.to_ascii_lowercase().starts_with("@echo") {
            continue;
        }
        let Some(cmd_segment) = extract_invocation_segment(line) else {
            continue;
        };
        let tokens = tokenize_batch_line(cmd_segment)?;
        if tokens.is_empty() {
            continue;
        }

        let last = tokens.last()?;
        if last != "%*" {
            continue;
        }
        let body = &tokens[..tokens.len() - 1];
        if body.is_empty() {
            continue;
        }

        // Rejeita expansões de batch dinâmicas que não podemos avaliar com segurança.
        if body
            .iter()
            .any(|t| t.contains("%1") || t.contains("%~1") || t.contains('!'))
        {
            return None;
        }

        let prog_token = &body[0];
        let rest = &body[1..];

        if is_node_prog_token(prog_token) {
            if let Some(resolved) = resolve_node_shim(shim_dir, prog_token, rest) {
                return Some(resolved);
            }
            continue;
        }

        if let Some(exe_path) = resolve_shim_path(shim_dir, prog_token) {
            if exe_path
                .extension()
                .and_then(OsStr::to_str)
                .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
                && exe_path.is_file()
            {
                let prefix_args = rest
                    .iter()
                    .map(|arg| expand_dp0_string(shim_dir, arg))
                    .collect();
                return Some(ResolvedDirectCommand {
                    program: exe_path,
                    prefix_args,
                });
            }
        }
    }

    None
}

fn extract_invocation_segment(line: &str) -> Option<&str> {
    if !line.contains("%*") {
        return None;
    }
    // Shims do npm usam `endLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & "%_prog%" "..." %*`.
    let segment = line.rsplit('&').next().unwrap_or(line).trim();
    let segment = segment.trim_start_matches('@').trim();
    if segment.is_empty() {
        None
    } else {
        Some(segment)
    }
}

fn tokenize_batch_line(line: &str) -> Option<Vec<String>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut has_token = false;

    for ch in line.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                has_token = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if has_token {
                    tokens.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            c => {
                current.push(c);
                has_token = true;
            }
        }
    }
    if in_quotes {
        return None;
    }
    if has_token {
        tokens.push(current);
    }
    Some(tokens)
}

fn is_node_prog_token(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    lower == "%_prog%"
        || lower == "node"
        || lower == "node.exe"
        || lower.ends_with("\\node.exe")
        || lower.ends_with("/node.exe")
}

fn resolve_node_shim(
    shim_dir: &Path,
    prog_token: &str,
    rest: &[String],
) -> Option<ResolvedDirectCommand> {
    if rest.is_empty() {
        return None;
    }

    let script_idx = rest.iter().position(|arg| is_js_script_arg(arg))?;
    let script_path = resolve_shim_path(shim_dir, &rest[script_idx])?;
    if !script_path.is_file() {
        return None;
    }

    let node_exe = resolve_node_executable(shim_dir, prog_token)?;

    let mut prefix_args = Vec::with_capacity(rest.len());
    for (idx, arg) in rest.iter().enumerate() {
        if idx == script_idx {
            prefix_args.push(script_path.to_string_lossy().into_owned());
        } else {
            prefix_args.push(expand_dp0_string(shim_dir, arg));
        }
    }

    Some(ResolvedDirectCommand {
        program: node_exe,
        prefix_args,
    })
}

fn resolve_node_executable(shim_dir: &Path, prog_token: &str) -> Option<PathBuf> {
    if prog_token.to_ascii_lowercase().contains("dp0") {
        if let Some(local_node) = resolve_shim_path(shim_dir, prog_token) {
            if local_node.is_file() {
                return Some(local_node);
            }
        }
    }

    let local_node = shim_dir.join("node.exe");
    if local_node.is_file() {
        return Some(local_node);
    }

    which::which("node").ok()
}

fn is_js_script_arg(arg: &str) -> bool {
    let lower = arg.to_ascii_lowercase();
    lower.ends_with(".js") || lower.ends_with(".mjs") || lower.ends_with(".cjs")
}

fn resolve_shim_path(shim_dir: &Path, raw: &str) -> Option<PathBuf> {
    let expanded = expand_dp0_string(shim_dir, raw);
    if expanded.contains('%') {
        return None;
    }
    Some(PathBuf::from(expanded))
}

fn expand_dp0_string(shim_dir: &Path, input: &str) -> String {
    let mut out = String::new();
    let mut rest = input;
    let base = shim_dir.to_string_lossy();
    let base_trimmed = base.trim_end_matches(['\\', '/']);

    while !rest.is_empty() {
        let lower = rest.to_ascii_lowercase();
        if let Some((prefix_len, sep_after)) = match_dp0_prefix(&lower) {
            out.push_str(base_trimmed);
            let after = &rest[prefix_len..];
            if !sep_after && !after.is_empty() && !after.starts_with(['\\', '/']) {
                out.push(std::path::MAIN_SEPARATOR);
            }
            rest = after;
        } else {
            let mut chars = rest.chars();
            if let Some(ch) = chars.next() {
                out.push(ch);
                rest = chars.as_str();
            }
        }
    }
    out
}

fn match_dp0_prefix(lower: &str) -> Option<(usize, bool)> {
    for pat in ["%~dp0\\", "%~dp0/", "%dp0%\\", "%dp0%/"] {
        if lower.starts_with(pat) {
            return Some((pat.len() - 1, true));
        }
    }
    for pat in ["%~dp0", "%dp0%"] {
        if lower.starts_with(pat) {
            return Some((pat.len(), false));
        }
    }
    None
}

/// Configura um `tokio::process::Command` resolvendo shims e aplicando `CREATE_NO_WINDOW`.
pub fn configure_tokio_command(program: &Path) -> tokio::process::Command {
    let resolved = resolve_direct_executable(program);
    let mut command = tokio::process::Command::new(&resolved.program);
    if !resolved.prefix_args.is_empty() {
        command.args(&resolved.prefix_args);
    }
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// Configura um `std::process::Command` resolvendo shims e aplicando `CREATE_NO_WINDOW`.
pub fn configure_std_command(program: &Path) -> std::process::Command {
    let resolved = resolve_direct_executable(program);
    let mut command = std::process::Command::new(&resolved.program);
    if !resolved.prefix_args.is_empty() {
        command.args(&resolved.prefix_args);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

pub struct SilentChild {
    pub stdin: Option<SilentChildStdin>,
    pub stdout: Option<SilentChildStdout>,
    pub stderr: Option<SilentChildStderr>,
    inner: SilentChildInner,
}

enum SilentChildInner {
    Tokio(tokio::process::Child),
    #[cfg(windows)]
    Win32(Win32Child),
}

pub enum SilentChildStdin {
    Tokio(tokio::process::ChildStdin),
    #[cfg(windows)]
    Pipe(tokio::io::DuplexStream),
}

pub enum SilentChildStdout {
    Tokio(tokio::process::ChildStdout),
    #[cfg(windows)]
    Pipe(tokio::io::DuplexStream),
}

pub enum SilentChildStderr {
    Tokio(tokio::process::ChildStderr),
    #[cfg(windows)]
    Pipe(tokio::io::DuplexStream),
}

impl AsyncWrite for SilentChildStdin {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            Self::Tokio(inner) => Pin::new(inner).poll_write(cx, buf),
            #[cfg(windows)]
            Self::Pipe(inner) => Pin::new(inner).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            Self::Tokio(inner) => Pin::new(inner).poll_flush(cx),
            #[cfg(windows)]
            Self::Pipe(inner) => Pin::new(inner).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            Self::Tokio(inner) => Pin::new(inner).poll_shutdown(cx),
            #[cfg(windows)]
            Self::Pipe(inner) => Pin::new(inner).poll_shutdown(cx),
        }
    }
}

impl AsyncRead for SilentChildStdout {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match &mut *self {
            Self::Tokio(inner) => Pin::new(inner).poll_read(cx, buf),
            #[cfg(windows)]
            Self::Pipe(inner) => Pin::new(inner).poll_read(cx, buf),
        }
    }
}

impl AsyncRead for SilentChildStderr {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match &mut *self {
            Self::Tokio(inner) => Pin::new(inner).poll_read(cx, buf),
            #[cfg(windows)]
            Self::Pipe(inner) => Pin::new(inner).poll_read(cx, buf),
        }
    }
}

impl SilentChild {
    fn from_tokio(mut child: tokio::process::Child) -> Self {
        let stdin = child.stdin.take().map(SilentChildStdin::Tokio);
        let stdout = child.stdout.take().map(SilentChildStdout::Tokio);
        let stderr = child.stderr.take().map(SilentChildStderr::Tokio);
        Self {
            stdin,
            stdout,
            stderr,
            inner: SilentChildInner::Tokio(child),
        }
    }

    pub fn id(&self) -> Option<u32> {
        match &self.inner {
            SilentChildInner::Tokio(child) => child.id(),
            #[cfg(windows)]
            SilentChildInner::Win32(child) => Some(child.pid),
        }
    }

    pub async fn wait(&mut self) -> io::Result<ExitStatus> {
        match &mut self.inner {
            SilentChildInner::Tokio(child) => child.wait().await,
            #[cfg(windows)]
            SilentChildInner::Win32(child) => child.wait().await,
        }
    }

    pub async fn kill(&mut self) -> io::Result<()> {
        match &mut self.inner {
            SilentChildInner::Tokio(child) => child.kill().await,
            #[cfg(windows)]
            SilentChildInner::Win32(child) => child.kill().await,
        }
    }

    pub fn start_kill(&mut self) -> io::Result<()> {
        match &mut self.inner {
            SilentChildInner::Tokio(child) => child.start_kill(),
            #[cfg(windows)]
            SilentChildInner::Win32(child) => child.start_kill(),
        }
    }
}

/// Inicia um processo de agente/fallback de forma silenciosa.
/// No Windows, aplica `CREATE_NO_WINDOW` e usa o `tokio::process::Command` padrão
/// (evitando `CreateDesktopW` e shims `.cmd`/`.bat` já resolvidos por `configure_tokio_command`).
pub fn spawn_silent(mut command: tokio::process::Command) -> io::Result<SilentChild> {
    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.spawn().map(SilentChild::from_tokio)
}

#[cfg(windows)]
#[allow(dead_code)]
mod win32 {
    use super::*;
    use windows_sys::Win32::Foundation::{
        CloseHandle, HANDLE, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, SetHandleInformation,
        TRUE, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::System::Pipes::CreatePipe;
    use windows_sys::Win32::System::StationsAndDesktops::{CloseDesktop, CreateDesktopW, HDESK};
    use windows_sys::Win32::System::Threading::{
        CREATE_UNICODE_ENVIRONMENT, CreateProcessW, GetExitCodeProcess, INFINITE,
        PROCESS_INFORMATION, STARTF_USESHOWWINDOW, STARTF_USESTDHANDLES, STARTUPINFOW,
        TerminateProcess, WaitForSingleObject,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    // Permissões mínimas para criar janelas ocultas no desktop isolado sem SwitchDesktop.
    const HIDDEN_DESKTOP_ACCESS: u32 = 0x0002 | 0x0001 | 0x0080 | 0x0040 | 0x0008;

    pub(super) struct HiddenDesktop {
        _handle: HDESK,
        name_wide: Vec<u16>,
    }

    // HDESK é seguro para compartilhamento somente-leitura do nome após criado.
    unsafe impl Send for HiddenDesktop {}
    unsafe impl Sync for HiddenDesktop {}

    impl Drop for HiddenDesktop {
        fn drop(&mut self) {
            if !self._handle.is_null() {
                unsafe {
                    CloseDesktop(self._handle);
                }
            }
        }
    }

    pub(super) fn hidden_desktop() -> Option<&'static HiddenDesktop> {
        static DESKTOP: OnceLock<Option<HiddenDesktop>> = OnceLock::new();
        DESKTOP
            .get_or_init(|| {
                let name = format!("vokia_hidden_{}", std::process::id());
                let name_wide = to_wide_null(OsStr::new(&name));
                let handle = unsafe {
                    CreateDesktopW(
                        name_wide.as_ptr(),
                        std::ptr::null(),
                        std::ptr::null(),
                        0,
                        HIDDEN_DESKTOP_ACCESS,
                        std::ptr::null(),
                    )
                };
                if handle.is_null() {
                    None
                } else {
                    Some(HiddenDesktop {
                        _handle: handle,
                        name_wide,
                    })
                }
            })
            .as_ref()
    }

    struct OwnedHandle(HANDLE);

    unsafe impl Send for OwnedHandle {}
    unsafe impl Sync for OwnedHandle {}

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
    }

    pub(super) struct Win32Child {
        pub pid: u32,
        process_handle: Option<OwnedHandle>,
    }

    impl Win32Child {
        pub async fn wait(&mut self) -> io::Result<ExitStatus> {
            let Some(handle) = self.process_handle.take() else {
                return Ok(ExitStatus::from_raw(0));
            };
            tokio::task::spawn_blocking(move || {
                let handle = handle;
                let wait_res = unsafe { WaitForSingleObject(handle.0, INFINITE) };
                if wait_res != WAIT_OBJECT_0 {
                    return Err(io::Error::last_os_error());
                }
                let mut code: u32 = 0;
                let ok = unsafe { GetExitCodeProcess(handle.0, &mut code) };
                if ok == 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(ExitStatus::from_raw(code))
            })
            .await
            .map_err(io::Error::other)?
        }

        pub fn start_kill(&mut self) -> io::Result<()> {
            let Some(handle) = self.process_handle.as_ref() else {
                return Ok(());
            };
            let ok = unsafe { TerminateProcess(handle.0, 1) };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }

        pub async fn kill(&mut self) -> io::Result<()> {
            let _ = self.start_kill();
            let _ = self.wait().await?;
            Ok(())
        }
    }

    pub(super) fn spawn_in_hidden_desktop(
        command: &mut tokio::process::Command,
        desktop: &HiddenDesktop,
    ) -> io::Result<SilentChild> {
        let std_cmd = command.as_std();
        let program_path = Path::new(std_cmd.get_program());
        let resolved = if program_path.is_absolute() {
            program_path.to_path_buf()
        } else {
            which::which(program_path).unwrap_or_else(|_| program_path.to_path_buf())
        };
        let app_wide = to_wide_null(resolved.as_os_str());
        let mut cmdline_wide = build_command_line(&resolved, std_cmd.get_args());

        let env_block = build_environment_block(std_cmd.get_envs());
        let env_ptr = env_block
            .as_ref()
            .map_or(std::ptr::null(), |block| block.as_ptr().cast());

        let cwd_wide = std_cmd.get_current_dir().map(|dir| to_wide_null(dir.as_os_str()));
        let cwd_ptr = cwd_wide
            .as_ref()
            .map_or(std::ptr::null(), |dir| dir.as_ptr());

        let (stdin_read, stdin_write) = create_inheritable_pipe(true)?;
        let (stdout_read, stdout_write) = create_inheritable_pipe(false)?;
        let (stderr_read, stderr_write) = create_inheritable_pipe(false)?;

        let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
        si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        si.lpDesktop = desktop.name_wide.as_ptr() as *mut u16;
        si.dwFlags = STARTF_USESHOWWINDOW | STARTF_USESTDHANDLES;
        si.wShowWindow = SW_HIDE as u16;
        si.hStdInput = stdin_read.0;
        si.hStdOutput = stdout_write.0;
        si.hStdError = stderr_write.0;

        let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        let flags = CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT;

        let created = unsafe {
            CreateProcessW(
                app_wide.as_ptr(),
                cmdline_wide.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                TRUE,
                flags,
                env_ptr,
                cwd_ptr,
                &si,
                &mut pi,
            )
        };

        // Fecha imediatamente as pontas herdadas pelo filho no processo pai para evitar deadlock de EOF.
        drop(stdin_read);
        drop(stdout_write);
        drop(stderr_write);

        if created == 0 {
            return Err(io::Error::last_os_error());
        }

        unsafe {
            CloseHandle(pi.hThread);
        }
        let process_handle = OwnedHandle(pi.hProcess);
        let pid = pi.dwProcessId;

        let stdin_file = unsafe { File::from_raw_handle(into_raw_handle(stdin_write)) };
        let stdout_file = unsafe { File::from_raw_handle(into_raw_handle(stdout_read)) };
        let stderr_file = unsafe { File::from_raw_handle(into_raw_handle(stderr_read)) };

        let stdin_stream = bridge_stdin(stdin_file);
        let stdout_stream = bridge_stdout(stdout_file);
        let stderr_stream = bridge_stdout(stderr_file);

        Ok(SilentChild {
            stdin: Some(SilentChildStdin::Pipe(stdin_stream)),
            stdout: Some(SilentChildStdout::Pipe(stdout_stream)),
            stderr: Some(SilentChildStderr::Pipe(stderr_stream)),
            inner: SilentChildInner::Win32(Win32Child {
                pid,
                process_handle: Some(process_handle),
            }),
        })
    }

    fn into_raw_handle(owned: OwnedHandle) -> RawHandle {
        let raw = owned.0 as RawHandle;
        std::mem::forget(owned);
        raw
    }

    fn create_inheritable_pipe(child_reads: bool) -> io::Result<(OwnedHandle, OwnedHandle)> {
        let sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: std::ptr::null_mut(),
            bInheritHandle: TRUE,
        };
        let mut read_handle: HANDLE = std::ptr::null_mut();
        let mut write_handle: HANDLE = std::ptr::null_mut();
        let ok = unsafe { CreatePipe(&mut read_handle, &mut write_handle, &sa, 0) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let read_owned = OwnedHandle(read_handle);
        let write_owned = OwnedHandle(write_handle);

        let parent_handle = if child_reads {
            write_owned.0
        } else {
            read_owned.0
        };
        let ok_inherit = unsafe { SetHandleInformation(parent_handle, HANDLE_FLAG_INHERIT, 0) };
        if ok_inherit == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok((read_owned, write_owned))
    }

    fn bridge_stdin(mut file: File) -> tokio::io::DuplexStream {
        use tokio::io::AsyncReadExt;
        let (client, mut server) = tokio::io::duplex(PIPE_BUFFER_SIZE);
        let rt = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            let mut buf = [0u8; 8192];
            loop {
                match rt.block_on(server.read(&mut buf)) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if file.write_all(&buf[..n]).is_err() {
                            break;
                        }
                        let _ = file.flush();
                    }
                }
            }
        });
        client
    }

    fn bridge_stdout(mut file: File) -> tokio::io::DuplexStream {
        use tokio::io::AsyncWriteExt;
        let (client, mut server) = tokio::io::duplex(PIPE_BUFFER_SIZE);
        let rt = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            let mut buf = [0u8; 8192];
            loop {
                match file.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if rt.block_on(server.write_all(&buf[..n])).is_err() {
                            break;
                        }
                    }
                }
            }
            let _ = rt.block_on(server.shutdown());
        });
        client
    }

    fn to_wide_null(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    fn build_command_line<'a>(
        program: &Path,
        args: impl Iterator<Item = &'a OsStr>,
    ) -> Vec<u16> {
        let mut line = String::new();
        push_win32_quoted(&mut line, &program.to_string_lossy());
        for arg in args {
            line.push(' ');
            push_win32_quoted(&mut line, &arg.to_string_lossy());
        }
        let mut wide: Vec<u16> = line.encode_utf16().collect();
        wide.push(0);
        wide
    }

    fn push_win32_quoted(out: &mut String, arg: &str) {
        if !arg.is_empty() && !arg.contains([' ', '\t', '"', '\n', '\x0b']) {
            out.push_str(arg);
            return;
        }
        out.push('"');
        let mut backslashes = 0usize;
        for ch in arg.chars() {
            if ch == '\\' {
                backslashes += 1;
            } else if ch == '"' {
                for _ in 0..(backslashes * 2 + 1) {
                    out.push('\\');
                }
                out.push('"');
                backslashes = 0;
            } else {
                for _ in 0..backslashes {
                    out.push('\\');
                }
                backslashes = 0;
                out.push(ch);
            }
        }
        for _ in 0..(backslashes * 2) {
            out.push('\\');
        }
        out.push('"');
    }

    fn build_environment_block<'a>(
        overrides: impl Iterator<Item = (&'a OsStr, Option<&'a OsStr>)>,
    ) -> Option<Vec<u16>> {
        let overrides_vec: Vec<_> = overrides.collect();
        if overrides_vec.is_empty() {
            return None;
        }

        // Variáveis no Windows são case-insensitive; normaliza a chave para preservar env_remove.
        let mut env_map: BTreeMap<String, (OsString, OsString)> = BTreeMap::new();
        for (k, v) in std::env::vars_os() {
            let key_upper = k.to_string_lossy().to_ascii_uppercase();
            env_map.insert(key_upper, (k, v));
        }

        for (k, v_opt) in overrides_vec {
            let key_upper = k.to_string_lossy().to_ascii_uppercase();
            match v_opt {
                Some(v) => {
                    env_map.insert(key_upper, (k.to_os_string(), v.to_os_string()));
                }
                None => {
                    env_map.remove(&key_upper);
                }
            }
        }

        let mut block = Vec::new();
        for (_, (k, v)) in env_map {
            block.extend(k.encode_wide());
            block.push(b'=' as u16);
            block.extend(v.encode_wide());
            block.push(0);
        }
        if block.is_empty() {
            block.push(0);
        }
        block.push(0);
        Some(block)
    }
}

#[cfg(windows)]
use win32::Win32Child;
