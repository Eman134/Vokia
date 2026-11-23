use serde_json::Value;

// Com o modo bypass ligado, estas ações continuam pedindo aprovação: apagar fora do projeto, push forçado,
// reset e clean destrutivos, e qualquer caminho fora da pasta do projeto ou da pasta de worktrees dele.
const COMMAND_FIELD: &str = "command";
const PATH_FIELD: &str = "file_path";

const FORCE_FLAG: &str = "-f";
const FORCE_PREFIX: &str = "--force";
const HARD_FLAG: &str = "--hard";

// Sem pasta do projeto conhecida, nada é considerado seguro: o pedido segue para o usuário.
pub(crate) fn is_destructive(input: &Value, project_dir: Option<&str>, worktrees_dir: Option<&str>) -> bool {
    let Some(project) = project_dir.and_then(normalized_root) else {
        return true;
    };
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
    if force_or_hard_reset(&tokens) {
        return true;
    }
    tokens
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
    tokens.iter().enumerate().any(|(index, token)| {
        if token != "git" {
            return false;
        }
        let Some(verb) = tokens.get(index + 1) else {
            return false;
        };
        let rest = &tokens[index + 2..];
        match verb.as_str() {
            "push" => rest.iter().any(|flag| is_force_flag(flag)),
            "reset" => rest.iter().any(|flag| flag == HARD_FLAG),
            "clean" => rest.iter().any(|flag| is_short_flag_with_f(flag)),
            _ => false,
        }
    })
}

fn is_short_flag_with_f(flag: &str) -> bool {
    flag.starts_with('-') && !flag.starts_with("--") && flag.contains('f')
}

fn is_force_flag(flag: &str) -> bool {
    flag == FORCE_FLAG
        || flag.starts_with(FORCE_PREFIX)
        || (flag.starts_with('-') && !flag.starts_with("--") && flag.contains('f'))
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
