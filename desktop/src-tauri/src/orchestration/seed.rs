use crate::agents::{Agent, Team};
use crate::db::{
    Db, DbError, LayerDraft, OrchestrationSeed, OrchestrationSettings, PlacementDraft, TeamDraft,
};

const TEAM_ID: &str = "time";
const TEAM_NAME: &str = "Time";
const SENTRY_ID: &str = "sentry";
const LEADER_KIND: &str = "planejar";
const IMPLEMENTATION_KIND: &str = "implementar";
const REVIEW_KIND: &str = "revisar";
const LEADER_LAYER_ID: &str = "lideranca";
const IMPLEMENTATION_LAYER_ID: &str = "implementacao";
const REVIEW_LAYER_ID: &str = "revisao";
const LEADER_MAX_INSTANCES: i64 = 2;
const IMPLEMENTATION_MAX_INSTANCES: i64 = 3;
const REVIEW_MAX_INSTANCES: i64 = 2;

pub(crate) fn build(team: &Team) -> OrchestrationSeed {
    let placements = team
        .ids()
        .filter(|agent| *agent != &Agent::ember())
        .map(|agent| {
            let layer = layer_of(agent);
            PlacementDraft {
                agent_id: agent.as_str().to_string(),
                team_id: TEAM_ID.to_string(),
                layer_id: layer.to_string(),
                instructions: String::new(),
                max_instances: default_instances_for(layer),
                pos_x: None,
                pos_y: None,
                criteria: String::new(),
                max_worktrees: None,
            }
        })
        .collect();
    OrchestrationSeed {
        settings: OrchestrationSettings::default(),
        layers: vec![
            layer("Liderança", "planeja e delega", LEADER_KIND),
            layer("Implementação", "escreve o código", IMPLEMENTATION_KIND),
            layer("Revisão", "testa e aprova", REVIEW_KIND),
        ],
        team: TeamDraft {
            id: None,
            name: TEAM_NAME.to_string(),
            instructions: None,
        },
        placements,
    }
}

// Roda a cada início: times sem líder gravado recebem o primeiro agente planejar do elenco.
pub(crate) fn fill_missing_leaders(db: &Db, team: &Team) -> Result<(), DbError> {
    let roster: Vec<String> = team.ids().map(|id| id.as_str().to_string()).collect();
    db.fill_missing_team_leaders(&roster)
}

fn default_instances_for(layer_id: &str) -> i64 {
    match layer_id {
        LEADER_LAYER_ID => LEADER_MAX_INSTANCES,
        REVIEW_LAYER_ID => REVIEW_MAX_INSTANCES,
        _ => IMPLEMENTATION_MAX_INSTANCES,
    }
}

fn layer_of(agent: &Agent) -> &'static str {
    if *agent == Agent::axiom() {
        LEADER_LAYER_ID
    } else if agent.as_str() == SENTRY_ID {
        REVIEW_LAYER_ID
    } else {
        IMPLEMENTATION_LAYER_ID
    }
}

fn layer(name: &str, description: &str, kind: &str) -> LayerDraft {
    LayerDraft {
        id: None,
        name: name.to_string(),
        description: description.to_string(),
        kind: kind.to_string(),
        instructions: String::new(),
    }
}

#[cfg(test)]
#[path = "../../tests/orchestration_seed_tests.rs"]
mod tests;
