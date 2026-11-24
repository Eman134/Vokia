use serde_json::Value;

use super::broker::COMMAND_FIELD;

// Planejamento só lê: qualquer pedido que altere o projeto é negado com um motivo que leva o agente a propor o plano.
pub(super) const PLANNING_DENIAL: &str =
    "Modo planejamento: nada pode ser alterado agora. Apresente o plano ao usuário e aguarde a aprovação antes de executar.";

const FILE_WRITE_TOOLS: [&str; 4] = ["Edit", "Write", "MultiEdit", "NotebookEdit"];
const SHELL_TOOLS: [&str; 2] = ["Bash", "PowerShell"];
const SEGMENT_SEPARATORS: [&str; 4] = ["&&", "||", ";", "|"];
const UNSAFE_SHELL_MARKERS: [&str; 3] = [">", "`", "$("];
const GIT: &str = "git";
const READ_ONLY_COMMANDS: [&str; 22] = [
    "ls",
    "dir",
    "cat",
    "type",
    "head",
    "tail",
    "wc",
    "rg",
    "grep",
    "pwd",
    "echo",
    "stat",
    "tree",
    "sort",
    "uniq",
    "diff",
    "which",
    "get-childitem",
    "gci",
    "get-content",
    "select-string",
    "get-location",
];
const READ_ONLY_GIT_VERBS: [&str; 8] = ["status", "log", "diff", "show", "ls-files", "rev-parse", "blame", "grep"];

#[derive(Debug, PartialEq, Eq)]
pub(super) enum PlanningOutcome {
    Allow,
    Deny,
    // Ferramenta que o planejamento não classifica (leitura de arquivo, busca...): segue o fluxo normal.
    Ask,
}

pub(super) fn planning_outcome(tool: &str, input: &Value) -> PlanningOutcome {
    if FILE_WRITE_TOOLS.contains(&tool) {
        return PlanningOutcome::Deny;
    }
    if !SHELL_TOOLS.contains(&tool) {
        return PlanningOutcome::Ask;
    }
    let read_only = input
        .get(COMMAND_FIELD)
        .and_then(Value::as_str)
        .is_some_and(is_read_only_command);
    if read_only {
        PlanningOutcome::Allow
    } else {
        PlanningOutcome::Deny
    }
}

// Todo trecho encadenado precisa ser leitura; redirecionamento e substituição de comando podem escrever.
fn is_read_only_command(command: &str) -> bool {
    if UNSAFE_SHELL_MARKERS.iter().any(|marker| command.contains(marker)) {
        return false;
    }
    let spaced = SEGMENT_SEPARATORS
        .iter()
        .fold(command.to_lowercase(), |text, separator| text.replace(separator, &format!(" {separator} ")));
    let mut segments = spaced
        .split_whitespace()
        .collect::<Vec<_>>()
        .split(|token| SEGMENT_SEPARATORS.contains(token))
        .map(<[&str]>::to_vec)
        .collect::<Vec<_>>();
    segments.retain(|segment| !segment.is_empty());
    !segments.is_empty() && segments.iter().all(|segment| is_read_only_segment(segment))
}

fn is_read_only_segment(segment: &[&str]) -> bool {
    match segment {
        [GIT, verb, ..] => READ_ONLY_GIT_VERBS.contains(verb),
        [root, ..] => READ_ONLY_COMMANDS.contains(root),
        [] => false,
    }
}
