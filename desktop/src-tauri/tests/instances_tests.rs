use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::agents::AgentRegistry;

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

fn vexa() -> Agent {
    Agent::from_stored("vexa").expect("agente")
}

fn busy_among(busy: &[Agent]) -> impl Fn(&Agent) -> bool + '_ {
    move |agent| busy.contains(agent)
}

#[test]
fn base_ociosa_e_escolhida_antes_de_qualquer_replica() {
    assert_eq!(pick(&vexa(), 3, busy_among(&[])), Some(vexa()));
}

#[test]
fn base_ocupada_cria_a_proxima_instancia_na_ordem() {
    let base = vexa();
    let one_busy = [base.clone()];
    let two_busy = [base.clone(), replica_id(&base, 2)];

    assert_eq!(pick(&base, 3, busy_among(&one_busy)), Some(replica_id(&base, 2)));
    assert_eq!(pick(&base, 3, busy_among(&two_busy)), Some(replica_id(&base, 3)));
}

#[test]
fn instancia_intermediaria_ociosa_e_reaproveitada_antes_da_proxima() {
    let base = vexa();
    let busy = [base.clone(), replica_id(&base, 3)];

    assert_eq!(pick(&base, 4, busy_among(&busy)), Some(replica_id(&base, 2)));
}

#[test]
fn todas_as_instancias_ate_max_instances_ocupadas_nao_cria_outra() {
    let base = vexa();
    let busy = [base.clone(), replica_id(&base, 2), replica_id(&base, 3)];

    assert_eq!(pick(&base, 3, busy_among(&busy)), None);
}

#[test]
fn max_instances_um_nunca_cria_replica() {
    let base = vexa();
    let busy = [base.clone()];

    assert_eq!(pick(&base, 1, busy_among(&busy)), None);
}

#[test]
fn replica_mostra_o_nome_do_perfil_base_seguido_do_numero() {
    let dir = std::env::temp_dir().join(format!(
        "vokia-instances-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("diretório temporário");
    let team = AgentRegistry::load(&dir).team().expect("time");
    let vexa = vexa();

    assert_eq!(display_name(&team, &vexa), "Vexa");
    assert_eq!(display_name(&team, &replica_id(&vexa, 2)), "Vexa 2");
    assert_eq!(display_name(&team, &replica_id(&vexa, 3)), "Vexa 3");
    let _ = std::fs::remove_dir_all(&dir);
}
