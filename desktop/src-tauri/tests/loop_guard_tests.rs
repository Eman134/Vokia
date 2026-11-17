use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::agents::AgentRegistry;

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

fn agent(id: &str) -> Agent {
    Agent::from_stored(id).expect("id de agente válido no teste")
}

fn with_team(check: impl FnOnce(&Team)) {
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "vokia-loop-guard-test-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&dir).expect("diretório temporário do time");
    let team = AgentRegistry::load(&dir)
        .team()
        .expect("o time padrão deveria carregar");
    check(&team);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn par_repetido_sobe_a_cada_repasse() {
    let mut counter = PassCounter::default();
    let (iara, davi) = (Agent::ember(), agent("davi"));

    assert_eq!(counter.register(&iara, &davi), 1);
    assert_eq!(counter.register(&iara, &davi), 2);
    assert_eq!(counter.register(&iara, &davi), 3);
}

#[test]
fn pares_diferentes_nao_somam_e_ida_e_volta_sao_pares_distintos() {
    let mut counter = PassCounter::default();
    let (iara, davi, nina) = (Agent::ember(), agent("davi"), agent("nina"));

    assert_eq!(counter.register(&iara, &davi), 1);
    assert_eq!(counter.register(&iara, &nina), 1);
    assert_eq!(counter.register(&davi, &iara), 1);
}

#[test]
fn reset_zera_todos_os_pares_e_nao_so_o_que_avancou() {
    let mut counter = PassCounter::default();
    let (iara, davi, nina) = (Agent::ember(), agent("davi"), agent("nina"));
    counter.register(&iara, &davi);
    counter.register(&iara, &davi);
    counter.register(&davi, &nina);

    counter.reset();

    assert_eq!(counter.register(&iara, &davi), 1);
    assert_eq!(counter.register(&davi, &nina), 1);
}

#[test]
fn by_speaker_soma_so_os_repasses_do_falante() {
    let mut counter = PassCounter::default();
    let (iara, davi, nina) = (Agent::ember(), agent("davi"), agent("nina"));
    counter.register(&iara, &davi);
    counter.register(&iara, &davi);
    counter.register(&iara, &nina);
    counter.register(&davi, &iara);

    assert_eq!(counter.by_speaker(&iara), 3);
    assert_eq!(counter.by_speaker(&davi), 1);
    assert_eq!(counter.by_speaker(&nina), 0);
}

#[test]
fn mensagem_do_limite_lista_pela_ordem_do_primeiro_repasse_com_nomes_do_time() {
    with_team(|team| {
        let mut counter = PassCounter::default();
        let (iara, davi) = (Agent::ember(), agent("davi"));
        counter.register(&iara, &davi);
        counter.register(&davi, &iara);
        counter.register(&iara, &davi);
        counter.register(&davi, &iara);
        counter.register(&iara, &davi);

        let (leader, peer) = (team.name_of(&iara), team.name_of(&davi));
        assert_eq!(
            pass_limit_message(&counter, &iara, &davi, team),
            format!("A tarefa parou: {leader} repassou para {peer} 3 vezes sem o estado avançar. Repasses desde o último avanço: {leader} → {peer} 3, {peer} → {leader} 2.")
        );
    });
}

#[test]
fn cadeia_com_um_repasse_por_agente_nunca_chega_ao_limite() {
    let mut counter = PassCounter::default();
    let chain = [
        Agent::ember(),
        agent("davi"),
        agent("nina"),
        agent("teo"),
        agent("rui"),
    ];

    for pair in chain.windows(2) {
        assert_eq!(
            counter.register(&pair[0], &pair[1]),
            1,
            "par {:?}",
            pair[0].as_str()
        );
    }
}

#[test]
fn coordenadora_com_ajudantes_diferentes_nunca_chega_ao_limite() {
    let mut counter = PassCounter::default();
    let iara = Agent::ember();

    for helper in ["davi", "nina", "teo", "rui"] {
        assert_eq!(counter.register(&iara, &agent(helper)), 1);
    }
}

#[test]
fn register_devolve_o_limite_exatamente_na_terceira_repeticao() {
    let mut counter = PassCounter::default();
    let (iara, davi) = (Agent::ember(), agent("davi"));

    let counts: Vec<u32> = (0..PASS_REPEAT_LIMIT)
        .map(|_| counter.register(&iara, &davi))
        .collect();

    assert_eq!(counts.last().copied(), Some(PASS_REPEAT_LIMIT));
    assert_eq!(counts.len(), 3);
}
