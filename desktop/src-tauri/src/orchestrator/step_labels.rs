use std::env;

use serde_json::Value;

pub const MAX_ARG_CHARS: usize = 48;

const ELLIPSIS: char = '…';
const REDACTED: &str = "[oculto]";
const MAX_UNWRAP_DEPTH: usize = 8;

const SECRET_PREFIXES: [&str; 17] = [
    "sk-ant-",
    "sk-proj-",
    "sk-",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "xoxb-",
    "xoxp-",
    "xoxa-",
    "AKIA",
    "AIza",
    "glpat-",
    "npm_",
    "pypi-",
];

const SECRET_NAME_HINTS: [&str; 13] = [
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "API_KEY",
    "APIKEY",
    "PRIVATE_KEY",
    "CREDENTIAL",
    "AUTHORIZATION",
    "ACCESS_KEY",
    "AUTH_KEY",
    "SIGNING_KEY",
    "CLIENT_SECRET",
];

const SECRET_FLAGS: [&str; 17] = [
    "--token",
    "--access-token",
    "--password",
    "--passwd",
    "--secret",
    "--api-key",
    "--apikey",
    "--auth",
    "--bearer",
    "--oauth2-bearer",
    "--user",
    "-u",
    "--proxy-user",
    "-U",
    "--client-secret",
    "--private-key",
    "--key",
];

const URL_SECRET_PARAMS: [&str; 9] = [
    "api_key",
    "apikey",
    "api-key",
    "token",
    "access_token",
    "auth",
    "secret",
    "password",
    "passwd",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepAction<'a> {
    Read { path: &'a str },
    Edit { path: &'a str },
    List { path: Option<&'a str> },
    Search { query: Option<&'a str> },
    WebSearch { query: Option<&'a str> },
    WebFetch { url: &'a str },
    Command { command: &'a str },
}

pub fn label_for_action(action: StepAction<'_>) -> Option<String> {
    label_for_action_with_home(action, current_home().as_deref())
}

pub fn label_for_command(command: &str) -> Option<String> {
    label_for_command_with_home(command, current_home().as_deref())
}

pub fn label_for_tool(name: &str, input: &Value) -> Option<String> {
    label_for_tool_with_home(name, input, current_home().as_deref())
}

pub fn label_for_codex_item(item: &Value) -> Option<String> {
    label_for_codex_item_with_home(item, current_home().as_deref())
}

pub fn label_for_opencode_event(value: &Value) -> Option<String> {
    let part = &value["part"];
    let name = first_str(&[
        part.get("tool"),
        part.get("name"),
        value.get("tool"),
        value.get("name"),
    ])?;
    let input = first_value(&[
        part.pointer("/state/input"),
        part.get("input"),
        part.get("arguments"),
        value.get("input"),
        value.get("arguments"),
    ]);
    let owned = parse_maybe_json_string(input);
    label_for_tool(name, owned.as_ref().unwrap_or(&Value::Null))
}

pub fn label_for_antigravity_step(step: &Value) -> Option<String> {
    let name = first_str(&[
        step.get("tool_name"),
        step.get("name"),
        step.get("tool"),
        step.pointer("/tool_call/name"),
        step.pointer("/tool_use/name"),
    ]);
    if let Some(tool_name) = name {
        let input = first_value(&[
            step.get("tool_input"),
            step.get("input"),
            step.get("arguments"),
            step.get("args"),
            step.pointer("/tool_call/input"),
            step.pointer("/tool_call/arguments"),
            step.pointer("/tool_use/input"),
        ]);
        let owned = parse_maybe_json_string(input);
        return label_for_tool(tool_name, owned.as_ref().unwrap_or(&Value::Null));
    }
    if let Some(cmd) = step.get("command").and_then(Value::as_str) {
        return label_for_command(cmd);
    }
    if let Some(query) = step.get("query").and_then(Value::as_str) {
        return label_for_action(StepAction::WebSearch { query: Some(query) });
    }
    None
}

fn label_for_codex_item_with_home(item: &Value, home: Option<&str>) -> Option<String> {
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
    match item_type {
        "command_execution" | "commandExecution" => {
            if let Some(actions) = item
                .get("commandActions")
                .or_else(|| item.get("command_actions"))
                .and_then(Value::as_array)
            {
                for action in actions {
                    if let Some(label) = label_for_codex_command_action(action, home) {
                        return Some(label);
                    }
                }
            }
            let cmd = item
                .get("command")
                .or_else(|| item.get("aggregatedCommand"))
                .or_else(|| item.get("aggregated_command"))
                .and_then(Value::as_str)?;
            label_for_command_with_home(cmd, home)
        }
        "file_change" | "fileChange" | "patch_apply" | "patchApply" => {
            let path = item
                .get("changes")
                .and_then(Value::as_array)
                .and_then(|arr| arr.first())
                .and_then(|first| first.get("path").or_else(|| first.get("file_path")))
                .or_else(|| item.get("path"))
                .or_else(|| item.get("file_path"))
                .and_then(Value::as_str)
                .unwrap_or("");
            label_for_action_with_home(StepAction::Edit { path }, home)
        }
        "web_search" | "webSearch" => {
            let query = item
                .get("query")
                .or_else(|| item.pointer("/action/query"))
                .and_then(Value::as_str);
            label_for_action_with_home(StepAction::WebSearch { query }, home)
        }
        "mcp_tool_call" | "mcpToolCall" | "function_call" | "functionCall" | "tool_use"
        | "toolUse" => {
            let name = first_str(&[
                item.get("tool"),
                item.get("name"),
                item.pointer("/function/name"),
            ])?;
            let raw_input = first_value(&[
                item.get("arguments"),
                item.get("input"),
                item.pointer("/function/arguments"),
            ]);
            let owned = parse_maybe_json_string(raw_input);
            label_for_tool_with_home(name, owned.as_ref().unwrap_or(&Value::Null), home)
        }
        _ => None,
    }
}

fn label_for_codex_command_action(action: &Value, home: Option<&str>) -> Option<String> {
    let kind = action
        .get("type")
        .or_else(|| action.get("kind"))
        .and_then(Value::as_str)?
        .to_ascii_lowercase();
    match kind.as_str() {
        "read" => {
            let path = first_str(&[action.get("path"), action.get("name")]).unwrap_or("");
            label_for_action_with_home(StepAction::Read { path }, home)
        }
        "edit" | "write" => {
            let path = first_str(&[action.get("path"), action.get("name")]).unwrap_or("");
            label_for_action_with_home(StepAction::Edit { path }, home)
        }
        "listfiles" | "list_files" | "list" => {
            let path = first_str(&[action.get("path"), action.get("dir")]);
            label_for_action_with_home(StepAction::List { path }, home)
        }
        "search" | "grep" => {
            let query = first_str(&[action.get("query"), action.get("pattern")]);
            label_for_action_with_home(StepAction::Search { query }, home)
        }
        "websearch" | "web_search" => {
            let query = first_str(&[action.get("query")]);
            label_for_action_with_home(StepAction::WebSearch { query }, home)
        }
        "unknown" | "command" => {
            let cmd = action.get("command").and_then(Value::as_str)?;
            label_for_action_with_home(StepAction::Command { command: cmd }, home)
        }
        _ => None,
    }
}

fn label_for_tool_with_home(name: &str, input: &Value, home: Option<&str>) -> Option<String> {
    let lower = name.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return None;
    }
    if lower == "app_command_call" || lower == "mcp__vokia__app_command_call" {
        if input.get("name").and_then(Value::as_str) == Some("web_search") {
            let args = parse_maybe_json_string(input.get("arguments"));
            let args_ref = args.as_ref().unwrap_or(&Value::Null);
            let query = first_str(&[
                args_ref.get("query"),
                args_ref.get("q"),
                args_ref.get("search_query"),
                args_ref.get("term"),
            ]);
            return label_for_action_with_home(StepAction::WebSearch { query }, home);
        }
    }
    let normalized_name = lower
        .strip_prefix("mcp__vokia__")
        .unwrap_or(lower.as_str());
    match normalized_name {
        "read" | "read_file" | "view_file" | "file_read" => {
            let path = first_str(&[
                input.get("file_path"),
                input.get("path"),
                input.get("target_file"),
                input.get("AbsolutePath"),
                input.get("filename"),
                input.get("file"),
            ])
            .unwrap_or("");
            label_for_action_with_home(StepAction::Read { path }, home)
        }
        "edit"
        | "multiedit"
        | "write"
        | "write_file"
        | "edit_file"
        | "replace_file_content"
        | "multi_replace_file_content"
        | "write_to_file"
        | "notebookedit"
        | "apply_patch"
        | "patch"
        | "str_replace_editor" => {
            let path = first_str(&[
                input.get("file_path"),
                input.get("path"),
                input.get("target_file"),
                input.get("TargetFile"),
                input.get("notebook_path"),
                input.get("filename"),
                input.get("file"),
            ])
            .unwrap_or("");
            label_for_action_with_home(StepAction::Edit { path }, home)
        }
        "glob" | "listfiles" | "list_files" | "list_dir" | "ls" | "find_by_name" | "file_search" => {
            let path = first_str(&[
                input.get("path"),
                input.get("dir"),
                input.get("directory"),
                input.get("DirectoryPath"),
                input.get("SearchDirectory"),
                input.get("pattern"),
            ]);
            label_for_action_with_home(StepAction::List { path }, home)
        }
        "grep" | "grep_search" | "search" | "search_files" | "ripgrep" | "code_search"
        | "find_in_files" => {
            let query = first_str(&[
                input.get("pattern"),
                input.get("query"),
                input.get("Query"),
                input.get("regex"),
                input.get("text"),
            ]);
            label_for_action_with_home(StepAction::Search { query }, home)
        }
        "web_search" | "websearch" | "search_web" | "google_search" | "brave_search" => {
            let query = first_str(&[
                input.get("query"),
                input.get("q"),
                input.get("search_query"),
                input.get("term"),
            ]);
            label_for_action_with_home(StepAction::WebSearch { query }, home)
        }
        "webfetch" | "web_fetch" | "fetch" | "fetch_url" | "url_fetch" | "read_url_content"
        | "http_get" => {
            let url = first_str(&[input.get("url"), input.get("Url"), input.get("uri")])
                .unwrap_or("");
            label_for_action_with_home(StepAction::WebFetch { url }, home)
        }
        "bash" | "shell" | "run_command" | "exec" | "execute" | "terminal" | "command"
        | "powershell" | "pwsh" | "cmd" => {
            let cmd = first_str(&[
                input.get("command"),
                input.get("CommandLine"),
                input.get("cmd"),
                input.get("script"),
            ])?;
            label_for_action_with_home(StepAction::Command { command: cmd }, home)
        }
        "memory_graph_query" => {
            let query = first_str(&[input.get("query"), input.get("q")]);
            let cleaned = query.map(sanitize_arg).filter(|s| !s.is_empty());
            Some(match cleaned {
                Some(term) => format!("Consultando grafo: '{term}'"),
                None => "Consultando grafo de memória".to_string(),
            })
        }
        "memory_graph_save" => {
            let cat = first_str(&[input.get("category")]).unwrap_or("memória");
            Some(format!("Salvando no grafo ({cat})"))
        }
        _ => None,
    }
}

fn label_for_action_with_home(action: StepAction<'_>, home: Option<&str>) -> Option<String> {
    match action {
        StepAction::Read { path } => Some(format_file_label("Lendo", "Lendo arquivo", path)),
        StepAction::Edit { path } => Some(format_file_label("Editando", "Editando arquivo", path)),
        StepAction::List { path } => {
            let cleaned = path.map(|p| sanitize_list_path(p, home)).filter(|s| !s.is_empty());
            Some(match cleaned {
                Some(dir) => format!("Listando {dir}"),
                None => "Listando arquivos".to_string(),
            })
        }
        StepAction::Search { query } => {
            let cleaned = query.map(sanitize_arg).filter(|s| !s.is_empty());
            Some(match cleaned {
                Some(term) => format!("Buscando '{term}'"),
                None => "Buscando texto nos arquivos".to_string(),
            })
        }
        StepAction::WebSearch { query } => {
            let cleaned = query.map(sanitize_arg).filter(|s| !s.is_empty());
            Some(match cleaned {
                Some(term) => format!("Pesquisando na web: '{term}'"),
                None => "Pesquisando na web…".to_string(),
            })
        }
        StepAction::WebFetch { url } => {
            let host = extract_host(&redact_secrets(url)).map(|h| truncate_arg(&h));
            Some(match host {
                Some(h) => format!("Acessando {h}"),
                None => "Acessando URL".to_string(),
            })
        }
        StepAction::Command { command } => label_for_command_with_home(command, home),
    }
}

fn label_for_command_with_home(command: &str, home: Option<&str>) -> Option<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }
    let safe = redact_secrets(trimmed);
    let unwrapped = unwrap_shell_command(&safe, 0);
    let segment = first_meaningful_segment(&unwrapped)?;
    let tokens = tokenize_shell(segment);
    if tokens.is_empty() {
        return None;
    }
    let slice = strip_command_prefixes(&tokens);
    if slice.is_empty() {
        return None;
    }
    classify_program_tokens(slice, home)
}

fn format_file_label(verb: &str, fallback: &str, path: &str) -> String {
    let safe = redact_secrets(path);
    let name = file_basename(&safe);
    if name.is_empty() {
        fallback.to_string()
    } else {
        format!("{verb} {}", truncate_arg(&name))
    }
}

fn sanitize_list_path(path: &str, home: Option<&str>) -> String {
    let safe = redact_secrets(path);
    let shortened = shorten_home(safe.trim(), home);
    truncate_arg(&shortened)
}

fn sanitize_arg(arg: &str) -> String {
    let safe = redact_secrets(arg);
    truncate_arg(&safe)
}

pub fn truncate_arg(text: &str) -> String {
    let normalized: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= MAX_ARG_CHARS {
        return normalized;
    }
    let mut out: String = normalized.chars().take(MAX_ARG_CHARS - 1).collect();
    out.push(ELLIPSIS);
    out
}

pub fn shorten_home(path: &str, home: Option<&str>) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let Some(home_dir) = home.map(str::trim).filter(|h| !h.is_empty()) else {
        return trimmed.to_string();
    };
    let home_norm = home_dir.trim_end_matches(['/', '\\']);
    if home_norm.is_empty() {
        return trimmed.to_string();
    }
    let path_slash = trimmed.replace('\\', "/");
    let home_slash = home_norm.replace('\\', "/");
    let is_windows = home_slash.as_bytes().get(1) == Some(&b':');
    let matches = if is_windows {
        path_slash.len() >= home_slash.len()
            && path_slash[..home_slash.len()].eq_ignore_ascii_case(&home_slash)
    } else {
        path_slash.starts_with(&home_slash)
    };
    if !matches {
        return trimmed.to_string();
    }
    let rest = &path_slash[home_slash.len()..];
    if rest.is_empty() || rest == "/" {
        "~".to_string()
    } else if let Some(tail) = rest.strip_prefix('/') {
        if tail.is_empty() {
            "~".to_string()
        } else {
            format!("~/{tail}")
        }
    } else {
        trimmed.to_string()
    }
}

pub fn redact_secrets(text: &str) -> String {
    let step0 = crate::provider_env::redact_registered_secrets(text);
    let step1 = redact_url_credentials(&step0);
    let step2 = redact_url_query_secrets(&step1);
    redact_tokens_and_assignments(&step2)
}

fn redact_url_credentials(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(scheme_pos) = rest.find("://") {
        let after_scheme = scheme_pos + 3;
        out.push_str(&rest[..after_scheme]);
        let tail = &rest[after_scheme..];
        let auth_end = tail
            .find(['/', '?', '#', ' ', '\t', '\r', '\n', '"', '\''])
            .unwrap_or(tail.len());
        let authority = &tail[..auth_end];
        if let Some(at_pos) = authority.rfind('@') {
            out.push_str(&authority[at_pos + 1..]);
            rest = &tail[auth_end..];
        } else {
            out.push_str(authority);
            rest = &tail[auth_end..];
        }
    }
    out.push_str(rest);
    out
}

fn redact_url_query_secrets(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let ch = text[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
        if ch == '?' || ch == '&' {
            let rest = &text[i..];
            let seg_end = rest
                .find(['&', ' ', '\t', '\r', '\n', '"', '\'', '#'])
                .unwrap_or(rest.len());
            let segment = &rest[..seg_end];
            if let Some((key, val)) = segment.split_once('=') {
                let key_lower = key.to_ascii_lowercase();
                if !val.is_empty() && URL_SECRET_PARAMS.iter().any(|p| key_lower == *p) {
                    out.push_str(key);
                    out.push('=');
                    out.push_str(REDACTED);
                    i += seg_end;
                }
            }
        }
    }
    out
}

fn redact_tokens_and_assignments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut hide_next = false;
    for piece in text.split_inclusive(char::is_whitespace) {
        let word = piece.trim_end();
        let tail = &piece[word.len()..];
        if word.is_empty() {
            out.push_str(piece);
            continue;
        }
        let core = word.trim_matches(['"', '\'', '`', ',', ';']);
        let lower = core.to_ascii_lowercase();
        if hide_next {
            if lower == "bearer" || lower == "basic" || lower == "token" {
                out.push_str(piece);
            } else {
                out.push_str(&replace_core(word, core, REDACTED));
                out.push_str(tail);
                hide_next = false;
            }
            continue;
        }
        if lower == "bearer" {
            out.push_str(piece);
            hide_next = true;
            continue;
        }
        if is_secret_flag(core) {
            out.push_str(piece);
            hide_next = true;
            continue;
        }
        if let Some(masked) = secret_flag_assignment_masked(word, core) {
            out.push_str(&masked);
            out.push_str(tail);
            continue;
        }
        if let Some(masked) = env_assignment_masked(word, core) {
            out.push_str(&masked);
            out.push_str(tail);
            continue;
        }
        if is_secret_header_label(core) {
            out.push_str(piece);
            hide_next = true;
            continue;
        }
        if let Some(masked) = prefixed_secret_masked(word, core) {
            out.push_str(&masked);
            out.push_str(tail);
            continue;
        }
        out.push_str(piece);
    }
    out
}

fn replace_core(word: &str, core: &str, replacement: &str) -> String {
    if core.is_empty() {
        return word.to_string();
    }
    word.replacen(core, replacement, 1)
}

fn is_secret_flag(core: &str) -> bool {
    let lower = core.to_ascii_lowercase();
    SECRET_FLAGS.iter().any(|flag| lower == *flag)
}

fn secret_flag_assignment_masked(word: &str, core: &str) -> Option<String> {
    let (flag, val) = core.split_once('=')?;
    if val.is_empty() || !is_secret_flag(flag) {
        return None;
    }
    let idx = word.rfind(val)?;
    Some(format!("{}{REDACTED}{}", &word[..idx], &word[idx + val.len()..]))
}

fn env_assignment_masked(word: &str, core: &str) -> Option<String> {
    let (raw_name, value) = core.split_once('=')?;
    let name = raw_name.trim_start_matches("$env:").trim_start_matches('$');
    if value.is_empty() || !is_sensitive_name(name) {
        return None;
    }
    let idx = word.rfind(value)?;
    Some(format!("{}{REDACTED}{}", &word[..idx], &word[idx + value.len()..]))
}

fn is_secret_header_label(core: &str) -> bool {
    let Some(prefix) = core.strip_suffix(':') else {
        return false;
    };
    is_sensitive_name(prefix.trim_matches(['"', '\'', '`']))
}

fn prefixed_secret_masked(word: &str, core: &str) -> Option<String> {
    let has_prefix = SECRET_PREFIXES.iter().any(|p| core.starts_with(*p));
    let is_jwt = core.starts_with("eyJ") && core.matches('.').count() == 2 && core.len() >= 20;
    if (has_prefix && core.chars().count() >= 8) || is_jwt {
        Some(replace_core(word, core, REDACTED))
    } else {
        None
    }
}

fn is_sensitive_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    SECRET_NAME_HINTS.iter().any(|hint| upper.contains(*hint))
}

fn current_home() -> Option<String> {
    env::var("HOME")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| env::var("USERPROFILE").ok().filter(|v| !v.trim().is_empty()))
}

fn unwrap_shell_command(command: &str, depth: usize) -> String {
    if depth >= MAX_UNWRAP_DEPTH {
        return command.to_string();
    }
    let tokens = tokenize_shell(command);
    if tokens.is_empty() {
        return command.to_string();
    }
    let stripped = strip_command_prefixes(&tokens);
    let Some(first) = stripped.first() else {
        return command.to_string();
    };
    let prog = normalize_program(first);
    let args = &stripped[1..];
    let inner = match prog.as_str() {
        "bash" | "sh" | "zsh" | "dash" | "fish" | "ash" => extract_posix_shell_arg(args),
        "cmd" => extract_cmd_arg(args),
        "pwsh" | "powershell" => extract_pwsh_arg(args),
        _ => None,
    };
    match inner {
        Some(next) if !next.trim().is_empty() => unwrap_shell_command(&next, depth + 1),
        _ => command.to_string(),
    }
}

fn extract_posix_shell_arg(args: &[String]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "--" {
            return None;
        }
        if arg == "-o" || arg == "+o" {
            i += 2;
            continue;
        }
        if arg.starts_with('-') && !arg.starts_with("--") && arg.len() > 1 {
            if arg[1..].contains('c') {
                return args.get(i + 1).cloned();
            }
            i += 1;
            continue;
        }
        return None;
    }
    None
}

fn extract_cmd_arg(args: &[String]) -> Option<String> {
    for (idx, arg) in args.iter().enumerate() {
        if arg.eq_ignore_ascii_case("/c") || arg.eq_ignore_ascii_case("/k") {
            let rest = &args[idx + 1..];
            if rest.is_empty() {
                return None;
            }
            return Some(if rest.len() == 1 {
                rest[0].clone()
            } else {
                rest.join(" ")
            });
        }
    }
    None
}

fn extract_pwsh_arg(args: &[String]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        let lower = args[i].to_ascii_lowercase();
        match lower.as_str() {
            "-c" | "-command" => {
                let rest = &args[i + 1..];
                if rest.is_empty() {
                    return None;
                }
                return Some(if rest.len() == 1 {
                    rest[0].clone()
                } else {
                    rest.join(" ")
                });
            }
            "-executionpolicy" | "-ep" | "-windowstyle" | "-w" | "-workingdirectory"
            | "-wd" | "-inputformat" | "-outputformat" | "-configurationname" => {
                i += 2;
            }
            s if s.starts_with('-') => {
                i += 1;
            }
            _ => return None,
        }
    }
    None
}

fn first_meaningful_segment(command: &str) -> Option<&str> {
    let segments = split_chain_segments(command);
    for seg in segments {
        let trimmed = seg.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Take the first command in a pipeline (`cmd1 | cmd2`)
        let first_pipe = split_first_pipe(trimmed).trim();
        if first_pipe.is_empty() || is_setup_command(first_pipe) {
            continue;
        }
        return Some(first_pipe);
    }
    None
}

fn split_chain_segments(command: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let bytes = command.as_bytes();
    let mut start = 0;
    let mut i = 0;
    let mut in_single = false;
    let mut in_double = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_single {
            if b == b'\'' {
                in_single = false;
            }
            i += 1;
            continue;
        }
        if in_double {
            if b == b'\\' && i + 1 < bytes.len() && (bytes[i + 1] == b'"' || bytes[i + 1] == b'\\') {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_double = false;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' => {
                in_single = true;
                i += 1;
            }
            b'"' => {
                in_double = true;
                i += 1;
            }
            b';' | b'\n' => {
                parts.push(&command[start..i]);
                i += 1;
                start = i;
            }
            b'&' if i + 1 < bytes.len() && bytes[i + 1] == b'&' => {
                parts.push(&command[start..i]);
                i += 2;
                start = i;
            }
            b'|' if i + 1 < bytes.len() && bytes[i + 1] == b'|' => {
                parts.push(&command[start..i]);
                i += 2;
                start = i;
            }
            _ => i += 1,
        }
    }
    parts.push(&command[start..]);
    parts
}

fn split_first_pipe(segment: &str) -> &str {
    let bytes = segment.as_bytes();
    let mut i = 0;
    let mut in_single = false;
    let mut in_double = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_single {
            if b == b'\'' {
                in_single = false;
            }
            i += 1;
            continue;
        }
        if in_double {
            if b == b'\\' && i + 1 < bytes.len() && (bytes[i + 1] == b'"' || bytes[i + 1] == b'\\') {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_double = false;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' => in_single = true,
            b'"' => in_double = true,
            b'|' if bytes.get(i + 1) != Some(&b'|') => return &segment[..i],
            _ => {}
        }
        i += 1;
    }
    segment
}

fn is_setup_command(segment: &str) -> bool {
    let tokens = tokenize_shell(segment);
    let Some(first) = tokens.first() else {
        return true;
    };
    let lower = normalize_program(first);
    if matches!(
        lower.as_str(),
        "cd" | "set-location" | "sl" | "pushd" | "popd" | "export" | "unset" | "true" | ":"
    ) {
        return true;
    }
    if lower == "set" && tokens.get(1).is_some_and(|t| t.contains('=') || t.starts_with('-')) {
        return true;
    }
    if first.starts_with("$env:") || first.starts_with("$ENV:") {
        return true;
    }
    if tokens.iter().all(|t| is_env_assignment(t)) {
        return true;
    }
    false
}

fn tokenize_shell(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_single {
            if ch == '\'' {
                in_single = false;
            } else {
                current.push(ch);
            }
            continue;
        }
        if in_double {
            if ch == '\\' {
                if let Some(&next) = chars.peek() {
                    if next == '"' || next == '\\' {
                        current.push(chars.next().unwrap());
                        continue;
                    }
                }
                current.push(ch);
            } else if ch == '"' {
                in_double = false;
            } else {
                current.push(ch);
            }
            continue;
        }
        match ch {
            '\'' => {
                in_single = true;
                in_token = true;
            }
            '"' => {
                in_double = true;
                in_token = true;
            }
            '\\' => {
                if let Some(&next) = chars.peek() {
                    if next.is_whitespace() || next == '"' || next == '\'' {
                        current.push(chars.next().unwrap());
                        in_token = true;
                        continue;
                    }
                }
                current.push(ch);
                in_token = true;
            }
            c if c.is_whitespace() => {
                if in_token {
                    tokens.push(std::mem::take(&mut current));
                    in_token = false;
                }
            }
            _ => {
                current.push(ch);
                in_token = true;
            }
        }
    }
    if in_token {
        tokens.push(current);
    }
    tokens
}

fn strip_command_prefixes(tokens: &[String]) -> &[String] {
    let mut i = 0;
    while i < tokens.len() {
        let tok = tokens[i].as_str();
        if is_env_assignment(tok) {
            i += 1;
            continue;
        }
        let lower = normalize_program(tok);
        match lower.as_str() {
            "sudo" => {
                i += 1;
                while i < tokens.len() && tokens[i].starts_with('-') {
                    let flag = tokens[i].as_str();
                    i += if matches!(flag, "-u" | "-g" | "-C" | "-p" | "-r" | "-t" | "-T") {
                        2
                    } else {
                        1
                    };
                }
            }
            "env" => {
                i += 1;
                while i < tokens.len() {
                    let t = tokens[i].as_str();
                    if t == "--" {
                        i += 1;
                        break;
                    }
                    if matches!(t, "-u" | "-C" | "-S" | "-P") {
                        i += 2;
                        continue;
                    }
                    if t.starts_with('-') || is_env_assignment(t) {
                        i += 1;
                        continue;
                    }
                    break;
                }
            }
            "command" | "nohup" | "time" => {
                i += 1;
                while i < tokens.len() && tokens[i].starts_with('-') {
                    i += 1;
                }
            }
            "nice" => {
                i += 1;
                if tokens.get(i).map(String::as_str) == Some("-n") {
                    i += 2;
                }
            }
            "timeout" => {
                i += 1;
                while i < tokens.len() && tokens[i].starts_with('-') {
                    let flag = tokens[i].as_str();
                    i += if matches!(flag, "-k" | "-s" | "--signal" | "--kill-after") {
                        2
                    } else {
                        1
                    };
                }
                if i < tokens.len() {
                    i += 1; // duration argument
                }
            }
            _ => break,
        }
    }
    &tokens[i.min(tokens.len())..]
}

fn is_env_assignment(token: &str) -> bool {
    let Some((name, _)) = token.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
}

fn normalize_program(raw: &str) -> String {
    let base = file_basename(raw);
    let lower = base.to_ascii_lowercase();
    for ext in [".exe", ".cmd", ".bat", ".ps1"] {
        if let Some(stripped) = lower.strip_suffix(ext) {
            return stripped.to_string();
        }
    }
    lower
}

fn file_basename(path: &str) -> String {
    let trimmed = path.trim().trim_end_matches(['/', '\\']);
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return String::new();
    }
    trimmed
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_string()
}

fn classify_program_tokens(tokens: &[String], home: Option<&str>) -> Option<String> {
    let raw_prog = tokens.first()?;
    let prog = normalize_program(raw_prog);
    if prog.is_empty() {
        return None;
    }
    let args = &tokens[1..];
    match prog.as_str() {
        "git" => Some(classify_git(args)),
        "rg" | "grep" | "egrep" | "fgrep" | "ag" | "ack" | "git-grep" | "select-string"
        | "sls" | "findstr" => Some("Buscando texto nos arquivos".to_string()),
        "cat" | "head" | "tail" | "bat" | "less" | "more" | "type" | "get-content" | "gc" => {
            let file = extract_read_file(&prog, args);
            Some(match file {
                Some(path) => format_file_label("Lendo", "Lendo arquivo", &path),
                None => "Lendo arquivo".to_string(),
            })
        }
        "sed" | "awk" | "gawk" | "jq" => {
            let file = extract_script_tool_file(&prog, args);
            Some(match file {
                Some(path) => format_file_label("Lendo", "Lendo arquivo", &path),
                None => "Lendo arquivo".to_string(),
            })
        }
        "ls" | "dir" | "find" | "fd" | "tree" | "get-childitem" | "gci" => {
            let dir = extract_list_path(&prog, args);
            Some(match dir {
                Some(path) => {
                    let display = sanitize_list_path(&path, home);
                    if display.is_empty() {
                        "Listando arquivos".to_string()
                    } else {
                        format!("Listando {display}")
                    }
                }
                None => "Listando arquivos".to_string(),
            })
        }
        "curl" | "wget" | "invoke-webrequest" | "iwr" | "invoke-restmethod" | "irm" => {
            if let Some(host) = extract_url_host_from_args(&prog, args) {
                Some(format!("Acessando {}", truncate_arg(&host)))
            } else {
                Some(format!("Rodando {}", truncate_arg(&prog)))
            }
        }
        "cargo" => Some(classify_cargo(args)),
        "npm" | "pnpm" | "yarn" | "bun" => Some(classify_node_pm(&prog, args)),
        "npx" | "pnpx" | "bunx" => Some(classify_npx(&prog, args, home)),
        "vitest" | "pytest" | "playwright" | "jest" | "mocha" | "ava" | "rspec" | "phpunit"
        | "ctest" => Some(format!("Rodando testes ({})", truncate_arg(&prog))),
        "go" => Some(classify_go(args)),
        "dotnet" => Some(classify_dotnet(args)),
        "python" | "python3" | "py" => Some(classify_python(&prog, args)),
        "tsc" | "rustc" | "make" | "cmake" | "ninja" | "gradle" | "gradlew" | "mvn" | "mvnw"
        | "esbuild" | "webpack" => Some(format!("Compilando ({})", truncate_arg(&prog))),
        "vite" => {
            if args.first().is_some_and(|a| a.eq_ignore_ascii_case("build")) {
                Some("Compilando (vite)".to_string())
            } else {
                Some("Rodando vite".to_string())
            }
        }
        _ => Some(format!("Rodando {}", truncate_arg(&prog))),
    }
}

fn classify_git(args: &[String]) -> String {
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if matches!(
            arg,
            "-C" | "-c" | "--git-dir" | "--work-tree" | "--namespace" | "--super-prefix"
        ) {
            i += 2;
            continue;
        }
        if arg.starts_with('-') {
            i += 1;
            continue;
        }
        let sub = arg.to_ascii_lowercase();
        if matches!(
            sub.as_str(),
            "status" | "diff" | "log" | "show" | "branch" | "blame"
        ) {
            return "Verificando git".to_string();
        }
        return "Rodando git".to_string();
    }
    "Rodando git".to_string()
}

fn classify_cargo(args: &[String]) -> String {
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg.starts_with('+') {
            i += 1;
            continue;
        }
        if matches!(arg, "--manifest-path" | "--color" | "-Z" | "--config") {
            i += 2;
            continue;
        }
        if arg.starts_with('-') {
            i += 1;
            continue;
        }
        return match arg.to_ascii_lowercase().as_str() {
            "test" | "nextest" => "Rodando testes (cargo)".to_string(),
            "build" | "check" | "clippy" | "rustc" | "doc" => "Compilando (cargo)".to_string(),
            _ => "Rodando cargo".to_string(),
        };
    }
    "Rodando cargo".to_string()
}

fn classify_node_pm(prog: &str, args: &[String]) -> String {
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if matches!(arg, "--prefix" | "-C" | "--cwd" | "--dir" | "--filter" | "-F") {
            i += 2;
            continue;
        }
        if arg.starts_with('-') {
            i += 1;
            continue;
        }
        let mut sub = arg.to_ascii_lowercase();
        if matches!(sub.as_str(), "run" | "run-script") {
            let mut j = i + 1;
            while j < args.len() && args[j].starts_with('-') {
                j += 1;
            }
            if let Some(script) = args.get(j) {
                sub = script.to_ascii_lowercase();
            }
        }
        if is_test_script(&sub) {
            return format!("Rodando testes ({prog})");
        }
        if is_build_script(&sub) {
            return format!("Compilando ({prog})");
        }
        return format!("Rodando {prog}");
    }
    format!("Rodando {prog}")
}

fn classify_npx(prog: &str, args: &[String], home: Option<&str>) -> String {
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if matches!(arg, "-p" | "--package" | "-c" | "--call") {
            i += 2;
            continue;
        }
        if arg.starts_with('-') {
            i += 1;
            continue;
        }
        return classify_program_tokens(&args[i..], home)
            .unwrap_or_else(|| format!("Rodando {prog}"));
    }
    format!("Rodando {prog}")
}

fn classify_go(args: &[String]) -> String {
    match args.first().map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("test") => "Rodando testes (go)".to_string(),
        Some("build" | "vet") => "Compilando (go)".to_string(),
        _ => "Rodando go".to_string(),
    }
}

fn classify_dotnet(args: &[String]) -> String {
    match args.first().map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("test") => "Rodando testes (dotnet)".to_string(),
        Some("build") => "Compilando (dotnet)".to_string(),
        _ => "Rodando dotnet".to_string(),
    }
}

fn classify_python(prog: &str, args: &[String]) -> String {
    for idx in 0..args.len() {
        if args[idx] == "-m" {
            if let Some(module) = args.get(idx + 1) {
                let lower = module.to_ascii_lowercase();
                if matches!(lower.as_str(), "pytest" | "unittest") {
                    return format!("Rodando testes ({lower})");
                }
            }
        }
    }
    format!("Rodando {prog}")
}

fn is_test_script(sub: &str) -> bool {
    matches!(
        sub,
        "test" | "t" | "vitest" | "jest" | "playwright" | "e2e" | "spec"
    ) || sub.starts_with("test:")
        || sub.starts_with("test-")
}

fn is_build_script(sub: &str) -> bool {
    matches!(sub, "build" | "typecheck" | "tsc" | "compile")
        || sub.starts_with("build:")
        || sub.starts_with("build-")
}

fn extract_read_file(prog: &str, args: &[String]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        let lower = arg.to_ascii_lowercase();
        if matches!(prog, "get-content" | "gc") {
            if matches!(lower.as_str(), "-path" | "-literalpath") {
                return args.get(i + 1).cloned();
            }
            if matches!(
                lower.as_str(),
                "-totalcount" | "-head" | "-first" | "-tail" | "-last" | "-encoding" | "-delimiter"
            ) {
                i += 2;
                continue;
            }
        }
        if matches!(prog, "head" | "tail") && matches!(arg, "-n" | "-c" | "--lines" | "--bytes") {
            i += 2;
            continue;
        }
        if arg == "--" {
            return args.get(i + 1).cloned();
        }
        if arg.starts_with('-') {
            i += 1;
            continue;
        }
        return Some(arg.to_string());
    }
    None
}

fn extract_script_tool_file(prog: &str, args: &[String]) -> Option<String> {
    let mut i = 0;
    let mut has_explicit_script = false;
    let mut positional: Vec<&str> = Vec::new();
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "--" {
            positional.extend(args[i + 1..].iter().map(String::as_str));
            break;
        }
        if matches!(arg, "-e" | "-f" | "--expression" | "--file" | "--from-file") {
            has_explicit_script = true;
            i += 2;
            continue;
        }
        if prog == "awk" || prog == "gawk" {
            if matches!(arg, "-F" | "-v") {
                i += 2;
                continue;
            }
        }
        if prog == "jq" && matches!(arg, "--arg" | "--argjson" | "--slurpfile" | "--rawfile") {
            i += 3;
            continue;
        }
        if arg.starts_with('-') {
            i += 1;
            continue;
        }
        positional.push(arg);
        i += 1;
    }
    if has_explicit_script {
        positional.first().map(|s| (*s).to_string())
    } else {
        positional.get(1).map(|s| (*s).to_string())
    }
}

fn extract_list_path(prog: &str, args: &[String]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        let lower = arg.to_ascii_lowercase();
        if matches!(prog, "get-childitem" | "gci") {
            if matches!(lower.as_str(), "-path" | "-literalpath") {
                return args.get(i + 1).cloned();
            }
            if matches!(
                lower.as_str(),
                "-filter" | "-include" | "-exclude" | "-depth" | "-attributes"
            ) {
                i += 2;
                continue;
            }
        }
        if matches!(prog, "find" | "fd" | "tree")
            && matches!(
                arg,
                "-maxdepth" | "-mindepth" | "-name" | "-iname" | "-type" | "-L" | "-I" | "-P"
                    | "-d" | "-e" | "-t"
            )
        {
            i += 2;
            continue;
        }
        if arg == "--" {
            return args.get(i + 1).cloned();
        }
        if arg.starts_with('-') || (prog == "dir" && arg.starts_with('/')) {
            i += 1;
            continue;
        }
        return Some(arg.to_string());
    }
    None
}

fn extract_url_host_from_args(prog: &str, args: &[String]) -> Option<String> {
    let mut i = 0;
    let mut fallback_candidate: Option<&str> = None;
    while i < args.len() {
        let arg = args[i].as_str();
        let lower = arg.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "-uri" | "--url"
        ) {
            if let Some(url) = args.get(i + 1) {
                if let Some(host) = extract_host(url) {
                    return Some(host);
                }
            }
            i += 2;
            continue;
        }
        if takes_value_for_http(prog, &lower) {
            i += 2;
            continue;
        }
        if arg.starts_with('-') {
            i += 1;
            continue;
        }
        if lower.starts_with("http://") || lower.starts_with("https://") {
            if let Some(host) = extract_host(arg) {
                return Some(host);
            }
        } else if fallback_candidate.is_none() && (arg.contains('.') || arg.starts_with("localhost")) {
            fallback_candidate = Some(arg);
        }
        i += 1;
    }
    fallback_candidate.and_then(extract_host)
}

fn takes_value_for_http(prog: &str, flag_lower: &str) -> bool {
    if matches!(
        prog,
        "invoke-webrequest" | "iwr" | "invoke-restmethod" | "irm"
    ) {
        return matches!(
            flag_lower,
            "-method"
                | "-headers"
                | "-body"
                | "-outfile"
                | "-contenttype"
                | "-useragent"
                | "-timeoutsec"
                | "-token"
                | "-authentication"
                | "-credential"
        );
    }
    matches!(
        flag_lower,
        "-h" | "--header"
            | "-d"
            | "--data"
            | "--data-raw"
            | "--data-binary"
            | "--data-urlencode"
            | "-x"
            | "--request"
            | "-o"
            | "--output"
            | "-u"
            | "--user"
            | "--proxy-user"
            | "-a"
            | "--user-agent"
            | "-e"
            | "--referer"
            | "-b"
            | "--cookie"
            | "-c"
            | "--cookie-jar"
            | "-f"
            | "--form"
            | "-m"
            | "--max-time"
            | "--connect-timeout"
            | "-w"
            | "--write-out"
            | "--resolve"
            | "--cacert"
            | "--cert"
            | "--key"
            | "--oauth2-bearer"
            | "-q"
            | "--output-document"
            | "--directory-prefix"
            | "-p"
            | "--post-data"
    )
}

pub fn extract_host(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_matches(['"', '\'', '<', '>']);
    if trimmed.is_empty() {
        return None;
    }
    let without_scheme = match trimmed.find("://") {
        Some(idx) => &trimmed[idx + 3..],
        None => trimmed,
    };
    let authority = without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .trim();
    if authority.is_empty() {
        return None;
    }
    let host_port = match authority.rfind('@') {
        Some(idx) => &authority[idx + 1..],
        None => authority,
    };
    let host = if let Some(rest) = host_port.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    let host = host.trim();
    if host.is_empty() || host.contains(char::is_whitespace) || host.contains(REDACTED) {
        return None;
    }
    Some(host.to_string())
}

fn first_str<'a>(candidates: &[Option<&'a Value>]) -> Option<&'a str> {
    candidates
        .iter()
        .filter_map(|opt| opt.and_then(Value::as_str))
        .map(str::trim)
        .find(|s| !s.is_empty())
}

fn first_value<'a>(candidates: &[Option<&'a Value>]) -> Option<&'a Value> {
    candidates
        .iter()
        .flatten()
        .copied()
        .find(|v| !v.is_null())
}

fn parse_maybe_json_string(value: Option<&Value>) -> Option<Value> {
    let v = value?;
    if let Some(raw) = v.as_str() {
        serde_json::from_str::<Value>(raw).ok()
    } else {
        Some(v.clone())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn bash_wrapper_com_cd_e_rg_vira_buscando_texto_nos_arquivos() {
        let label = label_for_command(r#"bash -lc "cd desktop && rg rate_limit src/""#);
        assert_eq!(label.as_deref(), Some("Buscando texto nos arquivos"));
    }

    #[test]
    fn sh_wrapper_com_env_e_git_status_vira_verificando_git() {
        let label = label_for_command(r#"sh -c 'export FOO=1 && cd app && git status'"#);
        assert_eq!(label.as_deref(), Some("Verificando git"));
    }

    #[test]
    fn pwsh_e_powershell_wrappers_com_cmdlets_windows() {
        let read = label_for_command(
            r#"pwsh -NoProfile -Command "Set-Location desktop; Get-Content C:\Users\Kayke\Vokia\src\providers.rs""#,
        );
        assert_eq!(read.as_deref(), Some("Lendo providers.rs"));

        let search = label_for_command(
            r#"powershell -Command "Select-String -Pattern 'rate_limit' -Path desktop/src""#,
        );
        assert_eq!(search.as_deref(), Some("Buscando texto nos arquivos"));

        let list = label_for_command_with_home(
            r#"pwsh -c "Get-ChildItem -Path C:\Users\Kayke\Vokia\src""#,
            Some(r"C:\Users\Kayke"),
        );
        assert_eq!(list.as_deref(), Some("Listando ~/Vokia/src"));
    }

    #[test]
    fn cmd_wrapper_com_cd_e_cargo_test() {
        let label = label_for_command(r#"cmd /c "cd desktop && cargo test --quiet""#);
        assert_eq!(label.as_deref(), Some("Rodando testes (cargo)"));
    }

    #[test]
    fn caminho_absoluto_windows_na_leitura_mostra_apenas_nome_do_arquivo() {
        let label = label_for_tool(
            "Read",
            &json!({ "file_path": r"C:\Users\Kayke\Documents\Projetos\Vokia\desktop\src-tauri\src\providers.rs" }),
        );
        assert_eq!(label.as_deref(), Some("Lendo providers.rs"));
    }

    #[test]
    fn encurta_diretorio_home_para_til_em_windows_e_posix() {
        assert_eq!(
            shorten_home(r"C:\Users\Kayke\Documents\Vokia", Some(r"C:\Users\Kayke")),
            "~/Documents/Vokia"
        );
        assert_eq!(shorten_home(r"C:\Users\Kayke", Some(r"C:\Users\Kayke")), "~");
        assert_eq!(
            shorten_home("/home/alice/projetos/vokia", Some("/home/alice")),
            "~/projetos/vokia"
        );
        let list = label_for_command_with_home(
            "ls -la /home/alice/projetos/vokia/src",
            Some("/home/alice"),
        );
        assert_eq!(list.as_deref(), Some("Listando ~/projetos/vokia/src"));
    }

    #[test]
    fn curl_com_bearer_e_credenciais_na_url_nao_vaza_segredos() {
        let label = label_for_command(
            r#"curl -H "Authorization: Bearer sk-secret-123" https://api.github.com/repos/foo/bar"#,
        );
        assert_eq!(label.as_deref(), Some("Acessando api.github.com"));

        let with_creds = label_for_command(
            "curl https://alice:super_secret_pw@api.example.com:8443/v1/items?api_key=sk-live-99999999",
        );
        assert_eq!(with_creds.as_deref(), Some("Acessando api.example.com"));
        assert!(!with_creds.as_deref().unwrap().contains("super_secret_pw"));
        assert!(!with_creds.as_deref().unwrap().contains("sk-live"));
    }

    #[test]
    fn redacao_mascara_tokens_env_flags_e_url_antes_do_rotulo() {
        let redacted = redact_secrets(
            r#"GITHUB_TOKEN=ghp_1234567890abcdef curl --token my-secret -H "Authorization: Bearer sk-secret-123" https://user:pass@api.github.com/x?token=abc"#,
        );
        assert!(!redacted.contains("ghp_1234567890abcdef"), "{redacted}");
        assert!(!redacted.contains("my-secret"), "{redacted}");
        assert!(!redacted.contains("sk-secret-123"), "{redacted}");
        assert!(!redacted.contains("user:pass"), "{redacted}");
        assert!(!redacted.contains("token=abc"), "{redacted}");
        assert!(redacted.contains("api.github.com"), "{redacted}");

        let search = label_for_tool("Grep", &json!({ "pattern": "Bearer sk-ant-secret123456" }));
        assert_eq!(search.as_deref(), Some("Buscando 'Bearer [oculto]'"));
    }

    #[test]
    fn git_leitura_vs_git_escrita() {
        for sub in ["status", "diff", "log", "show", "branch", "blame"] {
            assert_eq!(
                label_for_command(&format!("git -C desktop {sub}")).as_deref(),
                Some("Verificando git"),
                "subcomando git {sub}"
            );
        }
        assert_eq!(
            label_for_command(r#"git commit -m "feat: add step labels""#).as_deref(),
            Some("Rodando git")
        );
        assert_eq!(
            label_for_command("git push origin main").as_deref(),
            Some("Rodando git")
        );
    }

    #[test]
    fn sudo_env_e_variaveis_sao_ignorados_antes_do_programa() {
        let label = label_for_command(
            "sudo -E env RUST_LOG=debug API_KEY=sk-1234567890abcdef cargo test --quiet",
        );
        assert_eq!(label.as_deref(), Some("Rodando testes (cargo)"));

        let build = label_for_command("CI=true npm run typecheck");
        assert_eq!(build.as_deref(), Some("Compilando (npm)"));

        let vitest = label_for_command("npx vitest run");
        assert_eq!(vitest.as_deref(), Some("Rodando testes (vitest)"));

        let pytest = label_for_command("python3 -m pytest tests/");
        assert_eq!(pytest.as_deref(), Some("Rodando testes (pytest)"));

        let pw = label_for_command("playwright test");
        assert_eq!(pw.as_deref(), Some("Rodando testes (playwright)"));
    }

    #[test]
    fn utilitarios_posix_de_leitura_e_listagem() {
        assert_eq!(
            label_for_command("cat src/main.rs | head -n 20").as_deref(),
            Some("Lendo main.rs")
        );
        assert_eq!(
            label_for_command("sed -n '1,20p' desktop/src/App.vue").as_deref(),
            Some("Lendo App.vue")
        );
        assert_eq!(
            label_for_command("jq -r '.version' desktop/package.json").as_deref(),
            Some("Lendo package.json")
        );
        assert_eq!(
            label_for_command("awk '{print $1}' Cargo.toml").as_deref(),
            Some("Lendo Cargo.toml")
        );
        assert_eq!(
            label_for_command("find src/features -name '*.vue'").as_deref(),
            Some("Listando src/features")
        );
        assert_eq!(
            label_for_command("docker ps").as_deref(),
            Some("Rodando docker")
        );
    }

    #[test]
    fn acoes_estruturadas_do_claude_codex_opencode_e_antigravity() {
        assert_eq!(
            label_for_tool("Edit", &json!({ "file_path": "desktop/src/App.vue" })).as_deref(),
            Some("Editando App.vue")
        );
        assert_eq!(
            label_for_tool("Write", &json!({ "file_path": "src/step_labels.rs" })).as_deref(),
            Some("Editando step_labels.rs")
        );
        assert_eq!(
            label_for_tool("Glob", &json!({ "path": "src/orchestrator", "pattern": "*.rs" })).as_deref(),
            Some("Listando src/orchestrator")
        );
        assert_eq!(
            label_for_tool("Grep", &json!({ "pattern": "rate_limit" })).as_deref(),
            Some("Buscando 'rate_limit'")
        );
        assert_eq!(
            label_for_tool("WebSearch", &json!({ "query": "rust tokio process" })).as_deref(),
            Some("Pesquisando na web: 'rust tokio process'")
        );
        assert_eq!(
            label_for_tool("WebFetch", &json!({ "url": "https://docs.rs/serde/latest" })).as_deref(),
            Some("Acessando docs.rs")
        );

        let codex_read = json!({
            "type": "command_execution",
            "command": "cat src/providers.rs",
            "commandActions": [{ "type": "read", "path": "desktop/src-tauri/src/providers.rs" }]
        });
        assert_eq!(label_for_codex_item(&codex_read).as_deref(), Some("Lendo providers.rs"));

        let codex_list = json!({
            "type": "command_execution",
            "commandActions": [{ "type": "listFiles", "path": "src/features" }]
        });
        assert_eq!(label_for_codex_item(&codex_list).as_deref(), Some("Listando src/features"));

        let codex_search = json!({
            "type": "command_execution",
            "commandActions": [{ "type": "search", "query": "TurnActivity" }]
        });
        assert_eq!(label_for_codex_item(&codex_search).as_deref(), Some("Buscando 'TurnActivity'"));

        let codex_fn = json!({
            "type": "function_call",
            "name": "web_search",
            "arguments": "{\"query\":\"codex app server\"}"
        });
        assert_eq!(
            label_for_codex_item(&codex_fn).as_deref(),
            Some("Pesquisando na web: 'codex app server'")
        );

        let opencode_ev = json!({
            "type": "tool_use",
            "part": {
                "type": "tool_use",
                "tool": "read_file",
                "state": { "input": { "path": "src/events.rs" } }
            }
        });
        assert_eq!(label_for_opencode_event(&opencode_ev).as_deref(), Some("Lendo events.rs"));

        let agy_step = json!({
            "step_type": "tool_use",
            "tool_name": "run_command",
            "tool_input": { "command": "cargo check --bins" }
        });
        assert_eq!(
            label_for_antigravity_step(&agy_step).as_deref(),
            Some("Compilando (cargo)")
        );
    }

    #[test]
    fn truncamento_de_48_caracteres_com_reticencias() {
        let long_term = "a".repeat(70);
        let label = label_for_tool("Grep", &json!({ "pattern": long_term })).unwrap();
        let expected_arg = format!("{}{ELLIPSIS}", "a".repeat(MAX_ARG_CHARS - 1));
        assert_eq!(expected_arg.chars().count(), MAX_ARG_CHARS);
        assert_eq!(label, format!("Buscando '{expected_arg}'"));

        let exact = "b".repeat(MAX_ARG_CHARS);
        assert_eq!(truncate_arg(&exact), exact);
    }

    #[test]
    fn entrada_vazia_ou_ferramenta_desconhecida_retorna_none() {
        assert_eq!(label_for_command("   "), None);
        assert_eq!(label_for_command("cd desktop && export FOO=1"), None);
        assert_eq!(label_for_tool("mcp__vokia__list_backlog", &json!({})), None);
    }
}
