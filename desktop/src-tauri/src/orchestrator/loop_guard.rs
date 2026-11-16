use std::collections::HashMap;

use crate::agents::{Agent, Team};

pub(super) const LOOP_REPEAT_LIMIT: u32 = 3;
pub(super) const LOOP_MESSAGE: &str = "A conversa entre os agentes parou: a mesma mensagem se repetiu em turnos seguidos sem avançar a tarefa.";
pub(super) const PASS_REPEAT_LIMIT: u32 = 3;
pub(super) const REJECTION_LIMIT: u32 = 3;
pub(super) const SEM_AVANCO_LIMIT: u32 = 3;

#[derive(Default)]
pub(super) struct LoopGuard {
    last: HashMap<Agent, (String, u32)>,
}

impl LoopGuard {
    pub(super) fn register(&mut self, agent: Agent, message: &str) -> u32 {
        let normalized = normalize_message(message);
        let (last, repeats) = self
            .last
            .entry(agent)
            .or_insert_with(|| (normalized.clone(), 0));
        if *last == normalized {
            *repeats += 1;
        } else {
            *last = normalized;
            *repeats = 1;
        }
        *repeats
    }
}

fn normalize_message(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

// Por par (quem repassa, para quem): uma coordenadora que delega a ajudantes diferentes não é loop.
// Vive só dentro de run_turns, então nasce e morre com a tarefa.
#[derive(Default)]
pub(super) struct PassCounter {
    pairs: Vec<(Agent, Agent, u32)>,
}

impl PassCounter {
    pub(super) fn register(&mut self, from: &Agent, to: &Agent) -> u32 {
        if let Some(pair) = self
            .pairs
            .iter_mut()
            .find(|(pair_from, pair_to, _)| pair_from == from && pair_to == to)
        {
            pair.2 += 1;
            return pair.2;
        }
        self.pairs.push((from.clone(), to.clone(), 1));
        1
    }

    pub(super) fn by_speaker(&self, from: &Agent) -> u32 {
        self.pairs
            .iter()
            .filter(|(pair_from, _, _)| pair_from == from)
            .map(|(_, _, count)| count)
            .sum()
    }

    pub(super) fn reset(&mut self) {
        self.pairs.clear();
    }

    pub(super) fn history(&self, team: &Team) -> String {
        self.pairs
            .iter()
            .map(|(from, to, count)| {
                format!("{} → {} {count}", team.name_of(from), team.name_of(to))
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub(super) fn pass_limit_message(
    counter: &PassCounter,
    from: &Agent,
    to: &Agent,
    team: &Team,
) -> String {
    format!(
        "A tarefa parou: {} repassou para {} {PASS_REPEAT_LIMIT} vezes sem o estado avançar. Repasses desde o último avanço: {}.",
        team.name_of(from),
        team.name_of(to),
        counter.history(team),
    )
}

pub(super) fn rejection_message() -> String {
    format!("A tarefa parou: o estado não avançou em {REJECTION_LIMIT} turnos seguidos.")
}

pub(super) fn idle_limit_message(step: usize, from: &Agent, to: &Agent, team: &Team) -> String {
    format!(
        "A tarefa parou: sem avanço desde o passo {step}, par {} → {}.",
        team.name_of(from),
        team.name_of(to),
    )
}

#[cfg(test)]
mod tests {
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
        let (iara, davi) = (agent("iara"), agent("davi"));

        assert_eq!(counter.register(&iara, &davi), 1);
        assert_eq!(counter.register(&iara, &davi), 2);
        assert_eq!(counter.register(&iara, &davi), 3);
    }

    #[test]
    fn pares_diferentes_nao_somam_e_ida_e_volta_sao_pares_distintos() {
        let mut counter = PassCounter::default();
        let (iara, davi, nina) = (agent("iara"), agent("davi"), agent("nina"));

        assert_eq!(counter.register(&iara, &davi), 1);
        assert_eq!(counter.register(&iara, &nina), 1);
        assert_eq!(counter.register(&davi, &iara), 1);
    }

    #[test]
    fn reset_zera_todos_os_pares_e_nao_so_o_que_avancou() {
        let mut counter = PassCounter::default();
        let (iara, davi, nina) = (agent("iara"), agent("davi"), agent("nina"));
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
        let (iara, davi, nina) = (agent("iara"), agent("davi"), agent("nina"));
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
            let (iara, davi) = (agent("iara"), agent("davi"));
            counter.register(&iara, &davi);
            counter.register(&davi, &iara);
            counter.register(&iara, &davi);
            counter.register(&davi, &iara);
            counter.register(&iara, &davi);

            assert_eq!(
                pass_limit_message(&counter, &iara, &davi, team),
                "A tarefa parou: Iara repassou para Davi 3 vezes sem o estado avançar. Repasses desde o último avanço: Iara → Davi 3, Davi → Iara 2."
            );
        });
    }

    #[test]
    fn cadeia_com_um_repasse_por_agente_nunca_chega_ao_limite() {
        let mut counter = PassCounter::default();
        let chain = ["iara", "davi", "nina", "teo", "rui"].map(agent);

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
        let iara = agent("iara");

        for helper in ["davi", "nina", "teo", "rui"] {
            assert_eq!(counter.register(&iara, &agent(helper)), 1);
        }
    }

    #[test]
    fn register_devolve_o_limite_exatamente_na_terceira_repeticao() {
        let mut counter = PassCounter::default();
        let (iara, davi) = (agent("iara"), agent("davi"));

        let counts: Vec<u32> = (0..PASS_REPEAT_LIMIT)
            .map(|_| counter.register(&iara, &davi))
            .collect();

        assert_eq!(counts.last().copied(), Some(PASS_REPEAT_LIMIT));
        assert_eq!(counts.len(), 3);
    }
}
