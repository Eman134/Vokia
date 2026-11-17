use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::super::prompts::role;
use super::*;
use crate::agents::{Agent, AgentRegistry};

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

const TOOLS_RULE_TEXT: &str =
    "Use as ferramentas do Vokia para ler e mexer no backlog e para pedir uma decisão ao usuário; nunca invente ids.";
const DECISION_LINE: &str = "#decisao pergunta|Posso seguir?||";

fn with_team(check: impl FnOnce(&Team)) {
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "vokia-mcp-cut-{}-{}",
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

fn visible_of(reply: &str) -> String {
    let mut out = String::new();
    with_team(|team| {
        let from = Agent::from_stored("iara").expect("agente");
        let parsed = parse_reply(reply, &from, team);
        assert!(parsed.delegation.is_none());
        assert!(parsed.updates.is_empty());
        out = parsed.visible;
    });
    out
}

#[test]
fn linha_card_escrita_pelo_agente_vira_texto_comum_visivel_e_nao_e_consumida() {
    let reply =
        "Registrando.\n#card criar Revisar login|Checar o token\n#card mover item-1|Done\nPronto.";

    assert_eq!(visible_of(reply), reply);
}

#[test]
fn regra_de_ferramentas_so_entra_no_papel_quando_ha_mcp() {
    with_team(|team| {
        let iara = Agent::from_stored("iara").expect("agente");
        let profile = team.profile(&iara).expect("perfil");

        let with = role(profile, &team.roster_line(), "contexto", true);
        let without = role(profile, &team.roster_line(), "contexto", false);

        assert!(with.contains(TOOLS_RULE_TEXT), "{with}");
        assert!(!without.contains(TOOLS_RULE_TEXT), "{without}");
    });
}

#[test]
fn papel_nao_ensina_o_marcador_card_e_so_ensina_decisao_por_texto_sem_mcp() {
    with_team(|team| {
        let iara = Agent::from_stored("iara").expect("agente");
        let profile = team.profile(&iara).expect("perfil");

        for with_tools in [true, false] {
            let text = role(profile, &team.roster_line(), "contexto", with_tools);

            assert!(!text.contains("#card"), "with_tools={with_tools}: {text}");
            assert_eq!(
                text.contains("#decisao"),
                !with_tools,
                "with_tools={with_tools}"
            );
        }
    });
}

#[test]
fn linha_decisao_continua_sendo_aceita_e_a_linha_card_junto_fica_como_texto_visivel() {
    let reply = format!("Vou perguntar.\n#card criar Fantasma|x\n{DECISION_LINE}");

    let decision = parse_decision(&reply).expect("a linha #decisao deveria ser reconhecida");

    assert_eq!(decision.title, "Posso seguir?");
    assert_eq!(decision.visible, "Vou perguntar.\n#card criar Fantasma|x");
    assert_eq!(visible_of(&decision.visible), decision.visible);
}

fn registered(title: &str) -> SlotDecision {
    SlotDecision {
        kind: crate::events::DecisionKind::Aprovacao,
        title: title.to_string(),
        detail: String::new(),
        options: Vec::new(),
    }
}

#[test]
fn decisao_da_ferramenta_vale_mais_que_a_linha_de_texto_e_o_texto_da_resposta_segue_visivel() {
    let reply = format!("Preciso aprovar.\n{DECISION_LINE}");

    let picked = pick_decision(Some(registered("Da ferramenta")), &reply)
        .expect("a decisão registrada deveria subir");

    assert_eq!(picked.title, "Da ferramenta");
    assert_eq!(picked.visible, reply);
}

#[test]
fn sem_decisao_registrada_a_linha_de_texto_continua_aceita_e_sem_nenhuma_nada_sobe() {
    let from_text = pick_decision(None, &format!("Vou perguntar.\n{DECISION_LINE}"))
        .expect("a linha #decisao deveria ser reconhecida");

    assert_eq!(from_text.title, "Posso seguir?");
    assert!(pick_decision(None, "Resposta comum.").is_none());
}

fn iara_and_davi() -> (Agent, Agent) {
    (
        Agent::from_stored("iara").expect("agente"),
        Agent::from_stored("davi").expect("agente"),
    )
}

#[test]
fn delegacao_da_ferramenta_vale_mais_que_a_do_texto_e_tarefa_alternada_a_descarta() {
    let (iara, davi) = iara_and_davi();
    let registered = Some((davi.clone(), "Da ferramenta".to_string()));
    let from_text = Some((iara.clone(), "Do texto".to_string()));

    let chosen = chosen_delegation(registered.clone(), from_text.clone(), false);
    let fallback = chosen_delegation(None, from_text.clone(), false);

    assert_eq!(chosen, Some((davi, "Da ferramenta".to_string())));
    assert_eq!(fallback, from_text);
    assert!(chosen_delegation(registered, from_text, true).is_none());
}

#[test]
fn next_turn_passa_a_vez_ao_agente_delegado_e_empilha_quem_pediu() {
    with_team(|team| {
        let (iara, davi) = iara_and_davi();
        let definition = TaskDefinition {
            nova_tarefa: true,
            objetivo: "teste".to_string(),
            criterion: String::new(),
            verify: false,
            participants: vec![iara.clone()],
            alternate: false,
            variables: Vec::new(),
            stop: None,
        };
        let turn = Turn::first(&definition, &iara, "Faça");
        let delegation = (davi.clone(), "Revise".to_string());

        let next = next_turn(&definition, &turn, "feed", Some(&delegation), team)
            .expect("deveria haver próximo turno");

        assert_eq!(next.speaker, davi);
        assert_eq!(next.base, "Revise");
        assert_eq!(next.requesters, [iara]);
    });
}
