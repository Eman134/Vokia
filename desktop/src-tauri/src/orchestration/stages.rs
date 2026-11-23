use crate::agents::Agent;
use crate::db::{Layer, OrchestrationSnapshot, Team as OrchTeam};

// Estados da etapa gravada: em andamento, aguardando o usuário (decisão aberta), escalada ao líder, concluída.
pub(crate) const STAGE_RUNNING: &str = "em_andamento";
pub(crate) const STAGE_WAITING: &str = "aguardando";
pub(crate) const STAGE_ESCALATED: &str = "escalada";
pub(crate) const STAGE_DONE: &str = "concluida";

pub(crate) const FLOW_FULL: &str = "completo";
pub(crate) const FLOW_REDUCED: &str = "reduzido";
// Só esta camada pode ser pulada no fluxo reduzido; Liderança e Implementação (e as demais) são obrigatórias.
pub(crate) const SKIPPABLE_LAYER_KIND: &str = "revisar";
const JUDGMENT_MARK: &str = "#fluxo";
const JUDGMENT_SEPARATOR: char = '|';

pub(crate) const JUDGMENT_RULE: &str = "Você lidera este time com o julgamento de fluxo ligado: antes de delegar uma task nova, avalie a complexidade e escreva uma linha exatamente assim: #fluxo completo|<motivo> ou #fluxo reduzido|<motivo>. O reduzido pula só a camada de Revisão; use o completo quando a task precisar de todas as etapas. Não escreva nada depois dessa linha.";

// Julgamento do líder: o modo ("completo" ou "reduzido") e o motivo, que fica só como registro.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Judgment {
    pub(crate) mode: &'static str,
    pub(crate) motive: String,
}

// A última linha `#fluxo <modo>|<motivo>` válida vale; modo desconhecido vira texto comum.
pub(crate) fn parse_judgment(reply: &str) -> Option<Judgment> {
    reply.lines().rev().find_map(judgment_line)
}

fn judgment_line(line: &str) -> Option<Judgment> {
    let rest = line.trim().strip_prefix(JUDGMENT_MARK)?;
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let (mode, motive) = rest
        .trim()
        .split_once(JUDGMENT_SEPARATOR)
        .unwrap_or((rest.trim(), ""));
    let mode = match mode.trim().to_lowercase().as_str() {
        FLOW_FULL => FLOW_FULL,
        FLOW_REDUCED => FLOW_REDUCED,
        _ => return None,
    };
    Some(Judgment {
        mode,
        motive: motive.trim().to_string(),
    })
}

// A linha do julgamento não vai para a tela: o resto da fala segue como fala do líder.
pub(crate) fn without_judgment_lines(reply: &str) -> String {
    reply
        .lines()
        .filter(|line| judgment_line(line).is_none())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// Camadas estritamente entre a de origem e a de destino, na ordem do mapa. Passagem para trás não pula nada.
pub(crate) fn skipped_layers<'a>(layers: &'a [Layer], from: &str, to: &str) -> Vec<&'a Layer> {
    let find = |id: &str| layers.iter().find(|layer| layer.id == id);
    let (Some(from), Some(to)) = (find(from), find(to)) else {
        return Vec::new();
    };
    if to.position <= from.position {
        return Vec::new();
    }
    let mut skipped: Vec<&Layer> = layers
        .iter()
        .filter(|layer| layer.position > from.position && layer.position < to.position)
        .collect();
    skipped.sort_by_key(|layer| layer.position);
    skipped
}

// Motivo da recusa quando a passagem pula uma camada que o modo do julgamento não permite pular.
// Sem modo gravado, a task vale como completa. None quando a passagem pode seguir.
pub(crate) fn skip_motive(
    layers: &[Layer],
    from_layer: Option<&str>,
    to_layer: Option<&str>,
    flow_mode: Option<&str>,
) -> Option<String> {
    let (from, to) = (from_layer?, to_layer?);
    let reduced = flow_mode == Some(FLOW_REDUCED);
    let blocked = skipped_layers(layers, from, to)
        .into_iter()
        .find(|layer| !(reduced && layer.kind == SKIPPABLE_LAYER_KIND))?;
    let mode = if reduced { FLOW_REDUCED } else { FLOW_FULL };
    Some(format!(
        "A task não pode pular a camada {} no fluxo {mode}.",
        blocked.name
    ))
}

// O líder com a opção de julgar o fluxo ligada, quando o agente é líder desse time.
pub(crate) fn judging_team<'a>(
    snapshot: &'a OrchestrationSnapshot,
    agent: &Agent,
) -> Option<&'a OrchTeam> {
    snapshot
        .teams
        .iter()
        .find(|team| team.judge_full_flow && team.leader_agent.as_deref() == Some(agent.as_str()))
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StallAction {
    // Reenvia a task ao nó da etapa, que retoma de onde parou.
    Resend,
    // Esgotadas as tentativas: a task sobe ao líder do time.
    Escalate,
}

pub(crate) fn stall_action(retries: i64, max_resends: i64) -> StallAction {
    if retries < max_resends {
        StallAction::Resend
    } else {
        StallAction::Escalate
    }
}

#[cfg(test)]
#[path = "../../tests/task_stage_tests.rs"]
mod tests;
