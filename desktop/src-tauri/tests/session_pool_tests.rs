use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::agents::replica_id;

const PROJECT: i64 = 1;
const WAIT: Duration = Duration::from_secs(2);
const BLOCKED_PROBE: Duration = Duration::from_millis(150);

fn base_agent() -> Agent {
    Agent::from_stored("vexa").expect("agente")
}

#[test]
fn duas_instancias_do_mesmo_agente_seguram_a_sessao_ao_mesmo_tempo() {
    tauri::async_runtime::block_on(async {
        let pool = SessionPool::default();
        let mut base_exchange = ExchangeSessions::new(&pool, PROJECT);
        let mut replica_exchange = ExchangeSessions::new(&pool, PROJECT);

        let base = base_exchange.hold(&base_agent()).await;
        let replica = tokio::time::timeout(WAIT, replica_exchange.hold(&replica_id(&base_agent(), 2)))
            .await
            .expect("a réplica não pode esperar a base");

        assert!(!Arc::ptr_eq(&base.slot, &replica.slot));
    });
}

#[test]
fn a_mesma_instancia_em_duas_trocas_espera_a_primeira_soltar() {
    tauri::async_runtime::block_on(async {
        let pool = SessionPool::default();
        let mut first = ExchangeSessions::new(&pool, PROJECT);
        let mut second = ExchangeSessions::new(&pool, PROJECT);

        let held = first.hold(&base_agent()).await;
        let blocked = tokio::time::timeout(BLOCKED_PROBE, second.hold(&base_agent())).await;
        assert!(blocked.is_err(), "a mesma instância deveria esperar");

        drop(held);
        tokio::time::timeout(WAIT, second.hold(&base_agent()))
            .await
            .expect("solta a primeira, a segunda assume");
    });
}
