use crate::db::OrchestrationSnapshot;

const POLICY_ALWAYS: &str = "sempre";
const POLICY_PARALLEL: &str = "paralelo";
const IMPLEMENT_KIND: &str = "implementar";
const PARALLEL_MIN_IMPLEMENTERS: i64 = 2;

// Pedido sem card (conversa) roda sempre na pasta do projeto; com card, a política decide.
pub(crate) fn should_use(policy: &str, snapshot: &OrchestrationSnapshot, item_id: Option<&str>) -> bool {
    if item_id.is_none() {
        return false;
    }
    match policy {
        POLICY_ALWAYS => true,
        POLICY_PARALLEL => implementer_slots(snapshot) >= PARALLEL_MIN_IMPLEMENTERS,
        _ => false,
    }
}

// Soma de max_instances das posições em camadas de implementação.
fn implementer_slots(snapshot: &OrchestrationSnapshot) -> i64 {
    snapshot
        .placements
        .iter()
        .filter_map(|placement| {
            let layer = snapshot
                .layers
                .iter()
                .find(|layer| layer.id == placement.layer_id)?;
            (layer.kind == IMPLEMENT_KIND).then_some(placement.max_instances)
        })
        .sum()
}

#[cfg(test)]
#[path = "../../tests/worktree_policy_tests.rs"]
mod tests;
