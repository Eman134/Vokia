use crate::agents::{replica_id, replica_of, Agent, Team};

// Nome exibido de réplica: "Davi 2". Agente comum usa o nome do perfil.
pub(crate) fn display_name(team: &Team, agent: &Agent) -> String {
    let Some((base, n)) = replica_of(agent.as_str()) else {
        return team.name_of(agent).to_string();
    };
    match Agent::from_stored(base) {
        Some(base) => format!("{} {n}", team.name_of(&base)),
        None => agent.to_string(),
    }
}

// Instância ociosa do agente base, na ordem base, base-2, base-3…; None quando todas estão ocupadas.
pub(crate) fn pick(
    base: &Agent,
    max_instances: i64,
    busy: impl Fn(&Agent) -> bool,
) -> Option<Agent> {
    std::iter::once(base.clone())
        .chain((2..=max_instances).map(|n| replica_id(base, n as u32)))
        .find(|agent| !busy(agent))
}

#[cfg(test)]
#[path = "../../tests/instances_tests.rs"]
mod tests;
