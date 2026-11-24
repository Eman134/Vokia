use serde_json::Value;

use super::broker::{COMMAND_FIELD, PATH_FIELD};

// No modo auto estes pedidos continuam indo ao usuário: apagar fora do projeto, push forçado, reset e clean
// destrutivos, rede, instalação de pacotes e qualquer caminho fora da pasta do projeto ou da pasta de worktrees dele.
const FORCE_FLAG: &str = "-f";
const FORCE_PREFIX: &str = "--force";
const HARD_FLAG: &str = "--hard";
const GIT: &str = "git";
const NETWORK_TOOLS: [&str; 2] = ["WebFetch", "WebSearch"];
const NETWORK_COMMANDS: [&str; 12] = [
    "curl",
    "wget",
    "ssh",
    "scp",
    "sftp",
    "ftp",
    "nc",
    "ncat",
    "telnet",
    "invoke-webrequest",
    "invoke-restmethod",
    "iwr",
];
const NETWORK_GIT_VERBS: [&str; 4] = ["push", "pull", "fetch", "clone"];
const PACKAGE_MANAGERS: [&str; 14] = [
    "npm", "pnpm", "yarn", "bun", "pip", "pip3", "cargo", "gem", "brew", "apt", "apt-get", "winget", "choco", "go",
];
const PACKAGE_VERBS: [&str; 7] = ["install", "i", "add", "get", "remove", "uninstall", "upgrade"];

// Sem pasta do projeto conhecida, nada é considerado seguro: o pedido segue para o usuário.
pub(super) fn is_demanding(
    tool: &str,
    input: &Value,
    project_dir: Option<&str>,
    worktrees_dir: Option<&str>,
) -> bool {
    let Some(project) = project_dir.and_then(normalized_root) else {
        return true;
    };
    if NETWORK_TOOLS.contains(&tool) {
        return true;
    }
    let roots: Vec<String> = std::iter::once(project)
        .chain(worktrees_dir.and_then(normalized_root))
        .collect();
    if let Some(path) = input.get(PATH_FIELD).and_then(Value::as_str) {
        return !inside_any(&normalize(path), &roots);
    }
    let Some(command) = input.get(COMMAND_FIELD).and_then(Value::as_str) else {
        return false;
    };
    let tokens: Vec<String> = command
        .split_whitespace()
        .map(|token| token.trim_matches(|c| c == '"' || c == '\'').to_lowercase())
        .collect();
    // Apagar é destrutivo quando o alvo sai do projeto; por isso basta checar todo caminho que aparece no comando.
    force_or_hard_reset(&tokens)
        || reaches_network(&tokens)
        || installs_packages(&tokens)
        || tokens
            .iter()
            .any(|token| is_path_like(token) && !inside_any(&normalize(token), &roots))
}

fn normalized_root(path: &str) -> Option<String> {
    Some(normalize(path)).filter(|root| !root.is_empty())
}

fn inside_any(path: &str, roots: &[String]) -> bool {
    roots.iter().any(|root| inside(path, root))
}

// `git push` com force, `git reset --hard` e `git clean` com -f.
fn force_or_hard_reset(tokens: &[String]) -> bool {
    git_calls(tokens).any(|(verb, rest)| match verb {
        "push" => rest.iter().any(|flag| is_force_flag(flag)),
        "reset" => rest.iter().any(|flag| flag == HARD_FLAG),
        "clean" => rest.iter().any(|flag| is_short_flag_with_f(flag)),
        _ => false,
    })
}

fn reaches_network(tokens: &[String]) -> bool {
    tokens.iter().any(|token| NETWORK_COMMANDS.contains(&token.as_str()))
        || git_calls(tokens).any(|(verb, _)| NETWORK_GIT_VERBS.contains(&verb))
}

fn installs_packages(tokens: &[String]) -> bool {
    tokens.windows(2).any(|pair| {
        PACKAGE_MANAGERS.contains(&pair[0].as_str()) && PACKAGE_VERBS.contains(&pair[1].as_str())
    })
}

// Cada `git <verbo> <resto...>` que aparece no comando.
fn git_calls(tokens: &[String]) -> impl Iterator<Item = (&str, &[String])> {
    tokens.iter().enumerate().filter_map(|(index, token)| {
        if token != GIT {
            return None;
        }
        let verb = tokens.get(index + 1)?;
        Some((verb.as_str(), &tokens[index + 2..]))
    })
}

fn is_short_flag_with_f(flag: &str) -> bool {
    flag.starts_with('-') && !flag.starts_with("--") && flag.contains('f')
}

fn is_force_flag(flag: &str) -> bool {
    flag == FORCE_FLAG || flag.starts_with(FORCE_PREFIX) || is_short_flag_with_f(flag)
}

fn is_path_like(token: &str) -> bool {
    let bytes = token.as_bytes();
    let drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    token.starts_with('/')
        || token.starts_with('\\')
        || token.starts_with('~')
        || drive
        || token.split(['/', '\\']).any(|segment| segment == "..")
}

fn normalize(path: &str) -> String {
    path.trim().replace('\\', "/").to_lowercase().trim_end_matches('/').to_string()
}

fn inside(path: &str, root: &str) -> bool {
    if path.split('/').any(|segment| segment == "..") {
        return false;
    }
    path == root || path.starts_with(&format!("{root}/"))
}
