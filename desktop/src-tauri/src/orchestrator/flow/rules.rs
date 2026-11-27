//! Regras puras de decisão e construção de fluxo (sem `AppHandle` nem banco).

use crate::agents::Agent;
use crate::events::DecisionKind;
use crate::mcp::SlotDecision;
use crate::orchestration::policy;
use crate::task::{TaskDefinition, TaskKind, Variable};

use super::super::validation::VALIDATION_RETURN;

pub(super) fn returns_without_motive(answer: &str) -> bool {
    answer
        .strip_prefix(VALIDATION_RETURN)
        .is_some_and(|rest| rest.trim_start_matches(':').trim().is_empty())
}

// Estouro de limite do mapa vira decisão do usuário, com o motivo da política.
pub(super) fn limit_decision(motive: String) -> SlotDecision {
    SlotDecision {
        kind: DecisionKind::Pergunta,
        title: policy::LIMIT_TITLE.to_string(),
        detail: motive,
        options: Vec::new(),
    }
}

pub(super) fn parallel_definition(
    agent: &Agent,
    objective: String,
    variables: Vec<Variable>,
) -> TaskDefinition {
    TaskDefinition {
        kind: TaskKind::Demanda,
        nova_tarefa: false,
        objetivo: objective,
        criterion: String::new(),
        verify: false,
        participants: vec![agent.clone()],
        alternate: false,
        variables,
        stop: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devolucao_sem_motivo_exige_texto_nao_vazio() {
        assert!(returns_without_motive(VALIDATION_RETURN));
        assert!(returns_without_motive(&format!("{VALIDATION_RETURN}:   ")));
        assert!(!returns_without_motive(&format!("{VALIDATION_RETURN}: faltou teste")));
        assert!(!returns_without_motive("Concluir"));
    }

    #[test]
    fn limit_decision_usa_titulo_da_politica_e_motivo() {
        let decision = limit_decision("teto de 2 passagens".to_string());
        assert!(matches!(decision.kind, DecisionKind::Pergunta));
        assert_eq!(decision.title, policy::LIMIT_TITLE);
        assert_eq!(decision.detail, "teto de 2 passagens");
        assert!(decision.options.is_empty());
    }

    #[test]
    fn parallel_definition_isola_participante_sem_alternancia() {
        let agent = Agent::vexa();
        let def = parallel_definition(&agent, "Revisar PR".to_string(), Vec::new());
        assert_eq!(def.participants, vec![agent]);
        assert_eq!(def.objetivo, "Revisar PR");
        assert!(!def.alternate);
        assert!(!def.verify);
    }
}
