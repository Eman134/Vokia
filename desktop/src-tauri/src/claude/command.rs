use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::mcp::McpConfigFile;
use crate::permissions::PermissionLevel;

pub const DEFAULT_MODEL: &str = "haiku";
const TEXT_ONLY_MAX_TURNS: &str = "3";
const PROJECT_MAX_TURNS: &str = "120";
const CLI_TIMEOUT: Duration = Duration::from_secs(120);
const PROJECT_CLI_TIMEOUT: Duration = Duration::from_secs(600);
// Com uma destas no ambiente a CLI cobraria por uso da API em vez de usar a assinatura. O token OAuth da
// assinatura (CLAUDE_CODE_OAUTH_TOKEN) e a rota (ANTHROPIC_BASE_URL) ficam.
const STRIPPED_AUTH_ENV: [&str; 3] = [
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "AWS_BEARER_TOKEN_BEDROCK",
];
// Proxy corporativo não pode interceptar o loopback do servidor MCP do app.
const NO_PROXY_HOSTS: &str = "127.0.0.1,localhost";
const PERMISSION_MODE: &str = "manual";
const NO_BUILTIN_TOOLS: &str = "";
// Com a busca adiada a CLI gasta uma ida à API (ToolSearch) só para carregar o schema das ferramentas do Vokia.
const TOOL_SEARCH_ENV: &str = "ENABLE_TOOL_SEARCH";
const TOOL_SEARCH_OFF: &str = "false";
// Ignora hooks e regras do usuário (um hook que reescreve o comando faz a regra não casar).
const NO_SETTING_SOURCES: &str = "";
// Ferramentas de arquivo com padrão de caminho ficam na pasta do projeto (cwd); a regra sem padrão libera qualquer caminho, inclusive Glob e Grep.
const FILE_TOOLS: [&str; 4] = ["Edit(./**)", "Write(./**)", "Glob(./**)", "Grep(./**)"];
// Regra por prefixo não enxerga opções que gravam (`git diff --output`, `git branch novo`): esses comandos entram só na forma exata.
const BASH_ALLOWLIST: [&str; 26] = [
    "Bash(ls)",
    "Bash(ls *)",
    "Bash(cat *)",
    "Bash(head *)",
    "Bash(tail *)",
    "Bash(pwd)",
    "Bash(git status)",
    "Bash(git status *)",
    "Bash(git log)",
    "Bash(git log --oneline)",
    "Bash(git diff)",
    "Bash(git diff --stat)",
    "Bash(git show)",
    "Bash(git branch)",
    "Bash(git branch --list)",
    "Bash(git rev-parse *)",
    "Bash(npm test *)",
    "Bash(npm run test *)",
    "Bash(npm run build *)",
    "Bash(npm run typecheck *)",
    "Bash(npm run lint *)",
    "Bash(cargo test *)",
    "Bash(cargo build *)",
    "Bash(cargo check *)",
    "Bash(cargo fmt --check)",
    "Bash(cargo clippy *)",
];
const EDIT_TOOLS: &str = "Read,Edit,Write,Glob,Grep";
const READ_TOOLS: &str = "Read,Glob,Grep";
const READ_ALLOWED: [&str; 2] = ["Glob(./**)", "Grep(./**)"];
// Sem `--tools` o Bash continua disponível e só cai em pergunta; no nível de edição ele precisa sair do conjunto.
struct LevelFlags {
    tools: Option<&'static str>,
    allowed: &'static [&'static [&'static str]],
}
const LIBERADO_FLAGS: LevelFlags = LevelFlags {
    tools: None,
    allowed: &[&FILE_TOOLS, &BASH_ALLOWLIST],
};
const EDICAO_FLAGS: LevelFlags = LevelFlags {
    tools: Some(EDIT_TOOLS),
    allowed: &[&FILE_TOOLS],
};
const LEITURA_FLAGS: LevelFlags = LevelFlags {
    tools: Some(READ_TOOLS),
    allowed: &[&READ_ALLOWED],
};
// Sem isso o executável release (subsistema "windows") abre uma janela de console a cada chamada.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Copy)]
pub enum Access<'a> {
    TextOnly,
    Project(ProjectAccess<'a>),
}

#[derive(Clone, Copy)]
pub struct ProjectAccess<'a> {
    pub dir: &'a Path,
    pub attachments: &'a Path,
    pub extra_dirs: &'a [String],
    pub level: PermissionLevel,
    // Sem as ferramentas nativas da CLI o agente só age pelo MCP do Vokia.
    pub builtin_tools: bool,
}

pub(crate) fn timeout_for(access: Access<'_>) -> Duration {
    match access {
        Access::TextOnly => CLI_TIMEOUT,
        Access::Project(_) => PROJECT_CLI_TIMEOUT,
    }
}

// Encerra o processo e seus filhos (as ferramentas que o claude lança) sem depender do drop do LiveSession.
pub fn kill_process_tree(pid: u32) {
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut command = std::process::Command::new("taskkill");
        command
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = std::process::Command::new("kill");
        command.args(["-KILL", &pid.to_string()]);
        command
    };
    // Falha quando o processo já saiu, que é o resultado desejado.
    let _ = command.output();
}

pub(crate) fn agent_command(
    program: &Path,
    user_args: &[String],
    leading: &[&str],
    role: &str,
    model: &str,
    access: Access<'_>,
    mcp: Option<&McpConfigFile>,
) -> Command {
    let mut command = base_command_for_provider(program, "claude");
    command.kill_on_drop(true);
    command.env(TOOL_SEARCH_ENV, TOOL_SEARCH_OFF);
    // Argumentos do usuário e mensagem de `-p` vêm antes das opções variádicas
    // (`--tools`, `--allowedTools`), que os engoliriam; `--append-system-prompt` fica por último.
    command.args(user_args);
    command.args(leading);
    command.args([
        "--output-format",
        "stream-json",
        "--verbose",
        "--model",
        model,
    ]);
    match access {
        Access::TextOnly => {
            command.args(["--tools", NO_BUILTIN_TOOLS, "--max-turns", TEXT_ONLY_MAX_TURNS]);
        }
        Access::Project(project) => {
            command.current_dir(project.dir);
            command
                .arg("--add-dir")
                .arg(project.dir)
                .arg(project.attachments);
            for extra_dir in project.extra_dirs {
                command.arg("--add-dir").arg(extra_dir);
            }
            command
                .args(["--permission-mode", PERMISSION_MODE])
                .args(["--setting-sources", NO_SETTING_SOURCES]);
            let tools = if project.builtin_tools {
                flags_for(project.level).tools
            } else {
                Some(NO_BUILTIN_TOOLS)
            };
            if let Some(tools) = tools {
                command.args(["--tools", tools]);
            }
            command
                .args(["--allowedTools", &allowed_with_mcp(project.level, mcp)])
                .args(["--max-turns", PROJECT_MAX_TURNS]);
            if let Some(mcp) = mcp {
                command
                    .arg("--mcp-config")
                    .arg(mcp.path())
                    .arg("--strict-mcp-config");
            }
        }
    }
    command.args(["--append-system-prompt", role]);
    command
}

fn flags_for(level: PermissionLevel) -> &'static LevelFlags {
    match level {
        PermissionLevel::Liberado => &LIBERADO_FLAGS,
        PermissionLevel::Edicao => &EDICAO_FLAGS,
        PermissionLevel::Leitura => &LEITURA_FLAGS,
    }
}

fn allowed_tools(level: PermissionLevel) -> String {
    flags_for(level)
        .allowed
        .iter()
        .flat_map(|group| group.iter())
        .copied()
        .collect::<Vec<_>>()
        .join(",")
}

// Os nomes completos saem do registro do servidor; o curinga `mcp__vokia` não é usado.
fn allowed_with_mcp(level: PermissionLevel, mcp: Option<&McpConfigFile>) -> String {
    let mcp_tools = mcp.map(McpConfigFile::allowed_tools).unwrap_or_default();
    std::iter::once(allowed_tools(level))
        .chain(mcp_tools.iter().cloned())
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn base_command_for_provider(cli: &Path, provider_id: &str) -> Command {
    let mut command = base_command(cli);
    crate::provider_env::apply_global_provider_env(provider_id, &mut command);
    command
}

pub(crate) fn base_command(cli: &Path) -> Command {
    let mut command = Command::new(cli);
    command.env("NO_PROXY", NO_PROXY_HOSTS);
    // Com stdin aberto a CLI espera 3 s por dados antes de responder.
    command.stdin(Stdio::null());
    for name in STRIPPED_AUTH_ENV {
        command.env_remove(name);
    }
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(test)]
#[path = "../../tests/command_tests.rs"]
mod tests;
