use std::env;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::process::ExitCode;
use std::time::Duration;

use serde_json::{json, Map, Value};

const ENV_URL: &str = "VOKIA_URL";
const ENV_TOKEN: &str = "VOKIA_TOKEN";
const IO_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, PartialEq, Eq)]
struct Endpoint {
    port: u16,
    path: String,
}

#[derive(Debug, PartialEq)]
enum Action {
    Help,
    ListTools { json_raw: bool },
    ListCommands { json_raw: bool, domain: Option<String> },
    CallTool { name: String, arguments: Value },
    CallCommand { name: String, arguments: Value },
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let action = match parse_args(&args, || read_stdin_utf8()) {
        Ok(action) => action,
        Err(msg) => {
            eprintln!("Erro: {msg}");
            return ExitCode::from(2);
        }
    };

    if matches!(action, Action::Help) {
        print_help();
        return ExitCode::SUCCESS;
    }

    let url = match env::var(ENV_URL) {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!("Variável de ambiente {ENV_URL} não definida.");
            return ExitCode::from(2);
        }
    };
    let token = match env::var(ENV_TOKEN) {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!("Variável de ambiente {ENV_TOKEN} não definida.");
            return ExitCode::from(2);
        }
    };

    let endpoint = match parse_loopback_url(&url) {
        Ok(ep) => ep,
        Err(msg) => {
            eprintln!("URL inválida ({ENV_URL}): {msg}");
            return ExitCode::from(2);
        }
    };

    let payload = match &action {
        Action::Help => unreachable!(),
        Action::ListTools { .. } => json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {},
        }),
        Action::ListCommands { domain, .. } => json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "commands/list",
            "params": match domain {
                Some(domain) => json!({ "domain": domain }),
                None => json!({}),
            },
        }),
        Action::CallTool { name, arguments } => json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments,
            },
        }),
        Action::CallCommand { name, arguments } => json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "commands/call",
            "params": {
                "name": name,
                "arguments": arguments,
            },
        }),
    };

    let (status, body) = match post_json(&endpoint, &token, &payload.to_string()) {
        Ok(res) => res,
        Err(msg) => {
            eprintln!("Falha de comunicação com Vokia: {msg}");
            return ExitCode::from(1);
        }
    };

    if status != 200 {
        let detail = body.trim();
        if detail.is_empty() {
            eprintln!("Servidor respondeu HTTP {status}.");
        } else {
            eprintln!("Servidor respondeu HTTP {status}: {detail}");
        }
        return ExitCode::from(1);
    }

    let rpc: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("Resposta JSON inválida do servidor: {err}");
            return ExitCode::from(1);
        }
    };

    if let Some(err) = rpc.get("error").filter(|v| !v.is_null()) {
        let msg = err
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| err.to_string());
        eprintln!("{msg}");
        return ExitCode::from(1);
    }

    let Some(result) = rpc.get("result") else {
        eprintln!("Resposta sem campo 'result'.");
        return ExitCode::from(1);
    };

    match action {
        Action::ListTools { json_raw } | Action::ListCommands { json_raw, .. } => {
            if json_raw {
                println!(
                    "{}",
                    serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string())
                );
            } else {
                let key = if matches!(action, Action::ListTools { .. }) {
                    "tools"
                } else {
                    "commands"
                };
                print_named_list(result, key);
            }
            ExitCode::SUCCESS
        }
        Action::CallTool { .. } | Action::CallCommand { .. } => {
            let text = extract_content_text(result);
            let is_error = result.get("isError").and_then(Value::as_bool) == Some(true);
            if is_error {
                if !text.is_empty() {
                    eprintln!("{text}");
                } else {
                    eprintln!("Ferramenta retornou erro.");
                }
                ExitCode::from(1)
            } else {
                if !text.is_empty() {
                    println!("{text}");
                }
                ExitCode::SUCCESS
            }
        }
        Action::Help => unreachable!(),
    }
}

fn read_stdin_utf8() -> Result<String, String> {
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .map_err(|err| format!("falha ao ler stdin: {err}"))?;
    Ok(buf)
}

fn parse_loopback_url(raw: &str) -> Result<Endpoint, String> {
    let trimmed = raw.trim();
    let rest = trimmed
        .strip_prefix("http://")
        .ok_or_else(|| "apenas URLs http:// em loopback são aceitas.".to_string())?;
    let (authority, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };
    let (host, port_str) = match authority.split_once(':') {
        Some((h, p)) => (h, p),
        None => (authority, "80"),
    };
    if host != "127.0.0.1" && !host.eq_ignore_ascii_case("localhost") {
        return Err(format!(
            "host '{host}' recusado; apenas 127.0.0.1 ou localhost é permitido."
        ));
    }
    let port: u16 = port_str
        .parse()
        .map_err(|_| format!("porta inválida: '{port_str}'."))?;
    if port == 0 {
        return Err("porta 0 não é válida.".to_string());
    }
    Ok(Endpoint {
        port,
        path: path.to_string(),
    })
}

fn post_json(endpoint: &Endpoint, token: &str, body: &str) -> Result<(u16, String), String> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, endpoint.port));
    let mut stream = TcpStream::connect_timeout(&addr, IO_TIMEOUT)
        .map_err(|err| format!("erro ao conectar em {addr}: {err}"))?;
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));

    let request = format!(
        "POST {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        endpoint.path,
        endpoint.port,
        token.trim(),
        body.len(),
        body
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|err| format!("erro ao enviar requisição: {err}"))?;
    let _ = stream.flush();

    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|err| format!("erro ao ler resposta: {err}"))?;

    parse_http_response(&raw)
}

fn parse_http_response(raw: &[u8]) -> Result<(u16, String), String> {
    let (head_bytes, body_bytes) = if let Some(pos) = find_bytes(raw, b"\r\n\r\n") {
        (&raw[..pos], &raw[pos + 4..])
    } else if let Some(pos) = find_bytes(raw, b"\n\n") {
        (&raw[..pos], &raw[pos + 2..])
    } else {
        return Err("resposta HTTP malformada (sem separador de cabeçalho).".to_string());
    };

    let head = String::from_utf8_lossy(head_bytes);
    let mut lines = head.lines();
    let status_line = lines
        .next()
        .ok_or_else(|| "resposta HTTP sem status.".to_string())?;
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| format!("linha de status HTTP inválida: '{status_line}'."))?;

    let is_chunked = lines.any(|line| {
        if let Some((name, val)) = line.split_once(':') {
            name.trim().eq_ignore_ascii_case("transfer-encoding")
                && val.to_ascii_lowercase().contains("chunked")
        } else {
            false
        }
    });

    let decoded = if is_chunked {
        decode_chunked(body_bytes)?
    } else {
        body_bytes.to_vec()
    };

    let body =
        String::from_utf8(decoded).map_err(|err| format!("corpo da resposta não é UTF-8: {err}"))?;
    Ok((status, body))
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn decode_chunked(mut data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let Some(crlf) = find_bytes(data, b"\r\n") else {
            break;
        };
        let size_str = String::from_utf8_lossy(&data[..crlf]);
        let size_hex = size_str.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_hex, 16)
            .map_err(|_| format!("tamanho de chunk inválido: '{size_hex}'."))?;
        data = &data[crlf + 2..];
        if size == 0 {
            break;
        }
        if data.len() < size {
            return Err("chunk HTTP truncado.".to_string());
        }
        out.extend_from_slice(&data[..size]);
        data = &data[size..];
        if data.starts_with(b"\r\n") {
            data = &data[2..];
        }
    }
    Ok(out)
}

fn extract_content_text(result: &Value) -> String {
    let Some(items) = result.get("content").and_then(Value::as_array) else {
        return String::new();
    };
    items
        .iter()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}

fn print_named_list(result: &Value, key: &str) {
    let Some(items) = result.get(key).and_then(Value::as_array) else {
        return;
    };
    for item in items {
        let name = item.get("name").and_then(Value::as_str).unwrap_or("");
        let desc = item.get("description").and_then(Value::as_str).unwrap_or("");
        match item.get("domain").and_then(Value::as_str) {
            Some(domain) => println!("[{domain}] {name} — {desc}"),
            None => println!("{name} — {desc}"),
        }
        if let Some(when) = item.get("when_to_use").and_then(Value::as_str) {
            println!("  Quando usar: {when}");
        }
    }
}

fn parse_args<F>(args: &[String], read_stdin: F) -> Result<Action, String>
where
    F: FnOnce() -> Result<String, String>,
{
    let Some((cmd, rest)) = args.split_first() else {
        return Ok(Action::Help);
    };
    match cmd.as_str() {
        "-h" | "--help" | "help" => Ok(Action::Help),
        "tools" => {
            let mut json_raw = false;
            for arg in rest {
                match arg.as_str() {
                    "--json" => json_raw = true,
                    other => return Err(format!("flag desconhecida em 'tools': {other}")),
                }
            }
            Ok(Action::ListTools { json_raw })
        }
        "commands" => {
            let mut json_raw = false;
            let mut domain = None;
            let mut i = 0;
            while i < rest.len() {
                match rest[i].as_str() {
                    "--json" => json_raw = true,
                    "--domain" => {
                        i += 1;
                        domain = Some(next_val(rest, i, "--domain")?);
                    }
                    other => return Err(format!("flag desconhecida em 'commands': {other}")),
                }
                i += 1;
            }
            Ok(Action::ListCommands { json_raw, domain })
        }
        "call" => parse_call(rest, read_stdin),
        "cmd" => parse_cmd(rest, read_stdin),
        "list-backlog" => {
            let col = optional_flag(rest, "--column")?;
            let mut map = Map::new();
            if let Some(c) = col {
                map.insert("column".to_string(), json!(c));
            }
            Ok(Action::CallTool {
                name: "list_backlog".to_string(),
                arguments: Value::Object(map),
            })
        }
        "read-card" => {
            let flags = parse_flags(rest, &["--id"])?;
            let id = require_one(&flags, "--id")?;
            Ok(Action::CallTool {
                name: "read_card".to_string(),
                arguments: json!({ "id": id }),
            })
        }
        "create-card" => {
            let flags = parse_flags(rest, &["--title", "--description", "--team"])?;
            let title = require_one(&flags, "--title")?;
            let mut map = Map::new();
            map.insert("title".to_string(), json!(title));
            if let Some(d) = get_one(&flags, "--description") {
                map.insert("description".to_string(), json!(d));
            }
            if let Some(t) = get_one(&flags, "--team") {
                map.insert("team".to_string(), json!(t));
            }
            Ok(Action::CallTool {
                name: "create_card".to_string(),
                arguments: Value::Object(map),
            })
        }
        "move-card" => {
            let flags = parse_flags(rest, &["--id", "--column"])?;
            let id = require_one(&flags, "--id")?;
            let column = require_one(&flags, "--column")?;
            Ok(Action::CallTool {
                name: "move_card".to_string(),
                arguments: json!({ "id": id, "column": column }),
            })
        }
        "update-card" => {
            let flags = parse_flags(rest, &["--id", "--title", "--description"])?;
            let id = require_one(&flags, "--id")?;
            let mut map = Map::new();
            map.insert("id".to_string(), json!(id));
            if let Some(t) = get_one(&flags, "--title") {
                map.insert("title".to_string(), json!(t));
            }
            if let Some(d) = get_one(&flags, "--description") {
                map.insert("description".to_string(), json!(d));
            }
            Ok(Action::CallTool {
                name: "update_card".to_string(),
                arguments: Value::Object(map),
            })
        }
        "add-label" => item_label_cmd("backlog_add_label", rest),
        "remove-label" => item_label_cmd("backlog_remove_label", rest),
        "set-responsible" => {
            let flags = parse_flags(rest, &["--item-id", "--agent"])?;
            let item_id = require_one(&flags, "--item-id")?;
            let agent = require_one(&flags, "--agent")?;
            Ok(Action::CallTool {
                name: "backlog_set_responsible".to_string(),
                arguments: json!({ "item_id": item_id, "agent": agent }),
            })
        }
        "archive-card" => item_only_cmd("backlog_archive_item", rest),
        "restore-card" => item_only_cmd("backlog_restore_item", rest),
        "delete-card" => item_only_cmd("backlog_delete_item", rest),
        "request-decision" => parse_request_decision(rest),
        "propose-request" => {
            let flags = parse_flags(rest, &["--title", "--detail"])?;
            let title = require_one(&flags, "--title")?;
            let mut map = Map::new();
            map.insert("title".to_string(), json!(title));
            if let Some(d) = get_one(&flags, "--detail") {
                map.insert("detail".to_string(), json!(d));
            }
            Ok(Action::CallTool {
                name: "propose_request".to_string(),
                arguments: Value::Object(map),
            })
        }
        "delegate" => parse_delegate(rest),
        "get-variables" => no_args_cmd("get_variables", rest),
        "set-variable" => parse_set_variable(rest),
        "get-orchestration" => no_args_cmd("get_orchestration", rest),
        "list-agents" => no_args_cmd("list_agents", rest),
        "save-item-plan" => parse_save_item_plan(rest, read_stdin),
        "set-criterion" => {
            let flags = parse_flags(rest, &["--artifact-id", "--criterion-id", "--done"])?;
            let artifact_id = require_one(&flags, "--artifact-id")?;
            let criterion_id = require_one(&flags, "--criterion-id")?;
            let done_raw = require_one(&flags, "--done")?;
            let done = match done_raw.as_str() {
                "true" => true,
                "false" => false,
                other => return Err(format!("--done deve ser true ou false, recebeu '{other}'")),
            };
            Ok(Action::CallTool {
                name: "artifact_set_criterion".to_string(),
                arguments: json!({
                    "artifact_id": artifact_id,
                    "criterion_id": criterion_id,
                    "done": done,
                }),
            })
        }
        "artifact" => parse_artifact(rest, read_stdin),
        "list-movements" => no_args_cmd("list_movements", rest),
        "post-channel-message" => {
            let flags = parse_flags(rest, &["--channel", "--text"])?;
            let channel = require_one(&flags, "--channel")?;
            let text = require_one(&flags, "--text")?;
            Ok(Action::CallTool {
                name: "post_channel_message".to_string(),
                arguments: json!({ "channel": channel, "text": text }),
            })
        }
        other => Err(format!(
            "subcomando desconhecido: '{other}'. Use --help para ver os comandos."
        )),
    }
}

fn no_args_cmd(tool: &str, rest: &[String]) -> Result<Action, String> {
    if !rest.is_empty() {
        return Err(format!("'{tool}' não aceita argumentos extras."));
    }
    Ok(Action::CallTool {
        name: tool.to_string(),
        arguments: json!({}),
    })
}

fn item_only_cmd(tool: &str, rest: &[String]) -> Result<Action, String> {
    let flags = parse_flags(rest, &["--item-id"])?;
    let item_id = require_one(&flags, "--item-id")?;
    Ok(Action::CallTool {
        name: tool.to_string(),
        arguments: json!({ "item_id": item_id }),
    })
}

fn item_label_cmd(tool: &str, rest: &[String]) -> Result<Action, String> {
    let flags = parse_flags(rest, &["--item-id", "--label"])?;
    let item_id = require_one(&flags, "--item-id")?;
    let label = require_one(&flags, "--label")?;
    Ok(Action::CallTool {
        name: tool.to_string(),
        arguments: json!({ "item_id": item_id, "label": label }),
    })
}

fn parse_call<F>(rest: &[String], read_stdin: F) -> Result<Action, String>
where
    F: FnOnce() -> Result<String, String>,
{
    let Some((tool, tail)) = rest.split_first() else {
        return Err("uso: vokia-cli call <tool_name> [--json <json> | --stdin]".to_string());
    };
    let mut raw_json: Option<String> = None;
    let mut from_stdin = false;
    let mut i = 0;
    while i < tail.len() {
        match tail[i].as_str() {
            "--json" => {
                i += 1;
                let val = tail
                    .get(i)
                    .ok_or_else(|| "falta valor após --json.".to_string())?;
                raw_json = Some(val.clone());
            }
            "--stdin" => from_stdin = true,
            other => return Err(format!("argumento desconhecido em 'call': {other}")),
        }
        i += 1;
    }
    if raw_json.is_some() && from_stdin {
        return Err("use apenas --json ou --stdin, não ambos.".to_string());
    }
    let payload_text = if from_stdin {
        read_stdin()?
    } else if let Some(text) = raw_json {
        text
    } else {
        "{}".to_string()
    };
    let parsed: Value = serde_json::from_str(payload_text.trim())
        .map_err(|err| format!("JSON inválido: {err}"))?;
    if !parsed.is_object() {
        return Err("argumentos de ferramenta devem ser um objeto JSON.".to_string());
    }
    Ok(Action::CallTool {
        name: tool.clone(),
        arguments: parsed,
    })
}

fn parse_cmd<F>(rest: &[String], read_stdin: F) -> Result<Action, String>
where
    F: FnOnce() -> Result<String, String>,
{
    const USAGE: &str =
        "uso: vokia-cli cmd <command_name> [--json <json> | --stdin | --<flag> <valor>...]";
    let Some((name, tail)) = rest.split_first() else {
        return Err(USAGE.to_string());
    };
    let mut raw_json: Option<String> = None;
    let mut from_stdin = false;
    let mut flags = Map::new();
    let mut i = 0;
    while i < tail.len() {
        let arg = tail[i].as_str();
        match arg {
            "--json" => {
                i += 1;
                let val = tail
                    .get(i)
                    .ok_or_else(|| "falta valor após --json.".to_string())?;
                raw_json = Some(val.clone());
            }
            "--stdin" => from_stdin = true,
            _ => {
                let key = arg
                    .strip_prefix("--")
                    .filter(|key| !key.is_empty())
                    .ok_or_else(|| format!("argumento desconhecido em 'cmd': {arg}"))?;
                i += 1;
                let val = tail
                    .get(i)
                    .ok_or_else(|| format!("falta valor após {arg}."))?;
                flags.insert(kebab_to_camel(key), parse_scalar_flag_value(val));
            }
        }
        i += 1;
    }
    let has_json = raw_json.is_some() || from_stdin;
    if raw_json.is_some() && from_stdin {
        return Err("use apenas --json ou --stdin, não ambos.".to_string());
    }
    if has_json && !flags.is_empty() {
        return Err("não misture --json/--stdin com flags dinâmicas.".to_string());
    }
    let arguments = if has_json {
        let payload_text = match raw_json {
            Some(text) => text,
            None => read_stdin()?,
        };
        let parsed: Value = serde_json::from_str(payload_text.trim())
            .map_err(|err| format!("JSON inválido: {err}"))?;
        if !parsed.is_object() {
            return Err("argumentos de comando devem ser um objeto JSON.".to_string());
        }
        parsed
    } else {
        Value::Object(flags)
    };
    Ok(Action::CallCommand {
        name: name.clone(),
        arguments,
    })
}

fn kebab_to_camel(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    let mut upper_next = false;
    for ch in key.chars() {
        if ch == '-' {
            upper_next = true;
        } else if upper_next {
            out.extend(ch.to_uppercase());
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn parse_scalar_flag_value(raw: &str) -> Value {
    match raw {
        "true" => json!(true),
        "false" => json!(false),
        _ => raw
            .parse::<i64>()
            .map_or_else(|_| json!(raw), |num| json!(num)),
    }
}

fn parse_request_decision(rest: &[String]) -> Result<Action, String> {
    let mut kind: Option<String> = None;
    let mut title: Option<String> = None;
    let mut detail: Option<String> = None;
    let mut options: Vec<String> = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--kind" => {
                i += 1;
                kind = Some(next_val(rest, i, "--kind")?);
            }
            "--title" => {
                i += 1;
                title = Some(next_val(rest, i, "--title")?);
            }
            "--detail" => {
                i += 1;
                detail = Some(next_val(rest, i, "--detail")?);
            }
            "--option" => {
                i += 1;
                options.push(next_val(rest, i, "--option")?);
            }
            other => return Err(format!("argumento desconhecido em 'request-decision': {other}")),
        }
        i += 1;
    }
    let kind = kind.ok_or_else(|| "flag obrigatória ausente: --kind".to_string())?;
    let title = title.ok_or_else(|| "flag obrigatória ausente: --title".to_string())?;
    let mut map = Map::new();
    map.insert("kind".to_string(), json!(kind));
    map.insert("title".to_string(), json!(title));
    if let Some(d) = detail {
        map.insert("detail".to_string(), json!(d));
    }
    if !options.is_empty() {
        map.insert("options".to_string(), json!(options));
    }
    Ok(Action::CallTool {
        name: "request_decision".to_string(),
        arguments: Value::Object(map),
    })
}

fn parse_delegate(rest: &[String]) -> Result<Action, String> {
    let flags = parse_flags(rest, &["--agent", "--request", "--kind", "--event", "--item-id", "--ping-seconds"])?;
    let agent = require_one(&flags, "--agent")?;
    let request = require_one(&flags, "--request")?;
    let mut map = Map::new();
    map.insert("agent".to_string(), json!(agent));
    map.insert("request".to_string(), json!(request));
    if let Some(k) = get_one(&flags, "--kind") {
        map.insert("kind".to_string(), json!(k));
    }
    if let Some(e) = get_one(&flags, "--event") {
        map.insert("event".to_string(), json!(e));
    }
    if let Some(id) = get_one(&flags, "--item-id") {
        map.insert("item_id".to_string(), json!(id));
    }
    if let Some(seconds) = get_one(&flags, "--ping-seconds") {
        let seconds: u64 = seconds
            .parse()
            .map_err(|_| "--ping-seconds precisa ser um número inteiro de segundos".to_string())?;
        map.insert("ping_seconds".to_string(), json!(seconds));
    }
    Ok(Action::CallTool {
        name: "delegate".to_string(),
        arguments: Value::Object(map),
    })
}

fn parse_set_variable(rest: &[String]) -> Result<Action, String> {
    let mut name: Option<String> = None;
    let mut op: Option<String> = None;
    let mut value: Option<String> = None;
    let mut as_int = false;
    let mut share = false;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--name" => {
                i += 1;
                name = Some(next_val(rest, i, "--name")?);
            }
            "--op" => {
                i += 1;
                op = Some(next_val(rest, i, "--op")?);
            }
            "--value" => {
                i += 1;
                value = Some(next_val(rest, i, "--value")?);
            }
            "--int" => as_int = true,
            "--share" => share = true,
            other => return Err(format!("argumento desconhecido em 'set-variable': {other}")),
        }
        i += 1;
    }
    let name = name.ok_or_else(|| "flag obrigatória ausente: --name".to_string())?;
    let op = op.ok_or_else(|| "flag obrigatória ausente: --op".to_string())?;
    let raw_val = value.ok_or_else(|| "flag obrigatória ausente: --value".to_string())?;
    let json_val = if as_int {
        let num: i64 = raw_val
            .trim()
            .parse()
            .map_err(|_| format!("valor inteiro inválido para --int: '{raw_val}'"))?;
        json!(num)
    } else {
        json!(raw_val)
    };
    let mut map = Map::new();
    map.insert("name".to_string(), json!(name));
    map.insert("op".to_string(), json!(op));
    map.insert("value".to_string(), json_val);
    if share {
        map.insert("share".to_string(), json!(true));
    }
    Ok(Action::CallTool {
        name: "set_variable".to_string(),
        arguments: Value::Object(map),
    })
}

fn parse_save_item_plan<F>(rest: &[String], read_stdin: F) -> Result<Action, String>
where
    F: FnOnce() -> Result<String, String>,
{
    let mut item_id: Option<String> = None;
    let mut raw_json: Option<String> = None;
    let mut from_stdin = false;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--item-id" => {
                i += 1;
                item_id = Some(next_val(rest, i, "--item-id")?);
            }
            "--json" => {
                i += 1;
                raw_json = Some(next_val(rest, i, "--json")?);
            }
            "--stdin" => from_stdin = true,
            other => return Err(format!("argumento desconhecido em 'save-item-plan': {other}")),
        }
        i += 1;
    }
    let item_id = item_id.ok_or_else(|| "flag obrigatória ausente: --item-id".to_string())?;
    if raw_json.is_some() && from_stdin {
        return Err("use apenas --json ou --stdin, não ambos.".to_string());
    }
    let payload_text = if from_stdin {
        read_stdin()?
    } else if let Some(text) = raw_json {
        text
    } else {
        return Err("informe --json <json> ou --stdin.".to_string());
    };
    let mut parsed: Value = serde_json::from_str(payload_text.trim())
        .map_err(|err| format!("JSON inválido: {err}"))?;
    let Some(obj) = parsed.as_object_mut() else {
        return Err("o plano deve ser um objeto JSON.".to_string());
    };
    obj.insert("item_id".to_string(), json!(item_id));
    Ok(Action::CallTool {
        name: "save_item_plan".to_string(),
        arguments: parsed,
    })
}

const ARTIFACT_OPS: [&str; 5] = ["list", "read", "create", "update", "check"];
const ARTIFACT_TEXT_FLAGS: [&str; 5] = ["--id", "--kind", "--title", "--summary", "--body"];
const ARTIFACT_EDIT_FIELDS: [&str; 4] = ["title", "summary", "body", "tags"];

// O corpo Markdown longo entra por --stdin; as demais flags e --json montam o resto dos argumentos da tool.
fn parse_artifact<F>(rest: &[String], read_stdin: F) -> Result<Action, String>
where
    F: FnOnce() -> Result<String, String>,
{
    let mut op: Option<String> = None;
    let mut raw_json: Option<String> = None;
    let mut from_stdin = false;
    let mut flags = Map::new();
    let mut i = 0;
    while i < rest.len() {
        let flag = rest[i].as_str();
        match flag {
            "--op" => {
                i += 1;
                op = Some(next_val(rest, i, flag)?);
            }
            "--json" => {
                i += 1;
                raw_json = Some(next_val(rest, i, flag)?);
            }
            "--stdin" => from_stdin = true,
            "--tags" => {
                i += 1;
                let tags: Vec<Value> = next_val(rest, i, flag)?
                    .split(',')
                    .map(str::trim)
                    .filter(|tag| !tag.is_empty())
                    .map(|tag| json!(tag))
                    .collect();
                flags.insert("tags".to_string(), Value::Array(tags));
            }
            "--index" => {
                i += 1;
                let index: u64 = next_val(rest, i, flag)?
                    .parse()
                    .map_err(|_| "--index deve ser um inteiro não negativo.".to_string())?;
                flags.insert("index".to_string(), json!(index));
            }
            "--done" => {
                i += 1;
                let done = match next_val(rest, i, flag)?.as_str() {
                    "true" => true,
                    "false" => false,
                    other => return Err(format!("--done deve ser true ou false, recebeu '{other}'")),
                };
                flags.insert("done".to_string(), json!(done));
            }
            "--expected-updated-at" => {
                i += 1;
                let expected: i64 = next_val(rest, i, flag)?
                    .parse()
                    .map_err(|_| "--expected-updated-at deve ser um inteiro.".to_string())?;
                flags.insert("expected_updated_at".to_string(), json!(expected));
            }
            _ if ARTIFACT_TEXT_FLAGS.contains(&flag) => {
                i += 1;
                flags.insert(flag[2..].to_string(), json!(next_val(rest, i, flag)?));
            }
            other => return Err(format!("argumento desconhecido em 'artifact': {other}")),
        }
        i += 1;
    }
    let op = op.ok_or_else(|| "flag obrigatória ausente: --op".to_string())?;
    if !ARTIFACT_OPS.contains(&op.as_str()) {
        return Err(format!(
            "--op deve ser {}, recebeu '{op}'",
            ARTIFACT_OPS.join(", ")
        ));
    }
    if raw_json.is_some() && from_stdin {
        return Err("use apenas --json ou --stdin, não ambos.".to_string());
    }
    if from_stdin && flags.contains_key("body") {
        return Err("use apenas --body ou --stdin, não ambos.".to_string());
    }
    let mut arguments = match raw_json {
        Some(text) => match serde_json::from_str::<Value>(text.trim())
            .map_err(|err| format!("JSON inválido: {err}"))?
        {
            Value::Object(map) => map,
            _ => return Err("os argumentos do artefato devem ser um objeto JSON.".to_string()),
        },
        None => Map::new(),
    };
    if from_stdin {
        // CRLF do terminal do Windows não pode entrar no corpo Markdown.
        arguments.insert("body".to_string(), json!(read_stdin()?.replace("\r\n", "\n")));
    }
    arguments.extend(flags);
    arguments.insert("op".to_string(), json!(op));
    require_artifact_fields(&op, &arguments)?;
    Ok(Action::CallTool {
        name: "artifact".to_string(),
        arguments: Value::Object(arguments),
    })
}

fn require_artifact_fields(op: &str, arguments: &Map<String, Value>) -> Result<(), String> {
    let required: &[&str] = match op {
        "read" => &["id"],
        "create" => &["kind", "title"],
        "update" => &["id"],
        "check" => &["id", "index", "done"],
        _ => &[],
    };
    if let Some(missing) = required.iter().find(|field| !arguments.contains_key(**field)) {
        return Err(format!("flag obrigatória ausente: --{}", missing.replace('_', "-")));
    }
    let has_edit = ARTIFACT_EDIT_FIELDS
        .iter()
        .any(|field| arguments.contains_key(*field));
    if op == "update" && !has_edit {
        return Err("informe ao menos um campo: --title, --summary, --body, --tags ou --stdin.".to_string());
    }
    Ok(())
}

fn optional_flag(rest: &[String], flag: &str) -> Result<Option<String>, String> {
    let flags = parse_flags(rest, &[flag])?;
    Ok(get_one(&flags, flag))
}

fn parse_flags(rest: &[String], allowed: &[&str]) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        let key = &rest[i];
        if !allowed.contains(&key.as_str()) {
            return Err(format!("argumento desconhecido: {key}"));
        }
        i += 1;
        let val = next_val(rest, i, key)?;
        out.push((key.clone(), val));
        i += 1;
    }
    Ok(out)
}

fn next_val(rest: &[String], idx: usize, flag: &str) -> Result<String, String> {
    rest.get(idx)
        .cloned()
        .ok_or_else(|| format!("falta valor após {flag}"))
}

fn get_one(flags: &[(String, String)], key: &str) -> Option<String> {
    flags
        .iter()
        .rev()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
}

fn require_one(flags: &[(String, String)], key: &str) -> Result<String, String> {
    get_one(flags, key).ok_or_else(|| format!("flag obrigatória ausente: {key}"))
}

fn print_help() {
    println!(
        "\
vokia-cli — cliente local das ferramentas do Vokia (requer {ENV_URL} e {ENV_TOKEN})

Uso:
  vokia-cli tools [--json]
  vokia-cli call <tool_name> [--json <json_obj> | --stdin]
  vokia-cli commands [--domain <dom>] [--json]
  vokia-cli cmd <command_name> [--json <json_obj> | --stdin | --<flag> <valor>...]
  vokia-cli list-backlog [--column <col>]
  vokia-cli read-card --id <id>
  vokia-cli create-card --title <t> [--description <d>] [--team <team>]
  vokia-cli move-card --id <id> --column <col>
  vokia-cli update-card --id <id> [--title <t>] [--description <d>]
  vokia-cli add-label --item-id <id> --label <l>
  vokia-cli remove-label --item-id <id> --label <l>
  vokia-cli set-responsible --item-id <id> --agent <a>
  vokia-cli archive-card --item-id <id>
  vokia-cli restore-card --item-id <id>
  vokia-cli delete-card --item-id <id>
  vokia-cli request-decision --kind <k> --title <t> [--detail <d>] [--option <o>]...
  vokia-cli propose-request --title <t> [--detail <d>]
  vokia-cli delegate --agent <a> --request <r> [--kind <k>] [--event <e>] [--item-id <id>] [--ping-seconds <s>]
  vokia-cli get-variables
  vokia-cli set-variable --name <n> --op <set|append> --value <v> [--int] [--share]
  vokia-cli get-orchestration
  vokia-cli list-agents
  vokia-cli save-item-plan --item-id <id> (--json <json> | --stdin)
  vokia-cli set-criterion --artifact-id <a> --criterion-id <c> --done <true|false>
  vokia-cli list-movements
  vokia-cli post-channel-message --channel <c> --text <t>
  vokia-cli artifact --op <list|read|create|update|check> [--id <id>] [--kind <k>] [--title <t>]
                     [--summary <s>] [--tags <a,b>] [--index <n>] [--done <true|false>]
                     [--expected-updated-at <ms>] [--body <md> | --stdin | --json <json_obj>]

Exemplos de artefato:
  echo \"# Corpo\" | vokia-cli artifact --op update --id especificacao-1 --stdin
  vokia-cli cmd create_artifact --json '{{\"projectId\":1,\"kind\":\"especificacao\",\"title\":\"Login\"}}'"
    );
}

#[cfg(test)]
#[path = "../../tests/unit/vokia_cli_tests.rs"]
mod tests;
