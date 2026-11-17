use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::super::prompts::role;
use super::*;
use crate::agents::{Agent, AgentRegistry};

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

const TOOLS_RULE_TEXT: &str =
    "Use as ferramentas do Vokia para ler e mexer no backlog, pedir uma decisão ao usuário e ler ou gravar as variáveis da tarefa; nunca invente ids.";
const OLD_MARKERS: [&str; 6] = ["#card", "#decisao", "@<Nome>", "$nome", "@time", "+= item"];

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

fn iara_and_davi() -> (Agent, Agent) {
    (
        Agent::from_stored("iara").expect("agente"),
        Agent::from_stored("davi").expect("agente"),
    )
}

fn empty_definition(participant: Agent) -> TaskDefinition {
    TaskDefinition {
        nova_tarefa: true,
        objetivo: "teste".to_string(),
        criterion: String::new(),
        verify: false,
        participants: vec![participant],
        alternate: false,
        variables: Vec::new(),
        stop: None,
    }
}

#[test]
fn linha_de_marcador_antigo_na_resposta_do_agente_vira_texto_comum_inteiro() {
    let reply = "Registrando.\n#card criar Revisar|x\n#decisao pergunta|Posso?||\n$contagem = 3\n$lista += item\n@Davi: revise isso\nPronto.";

    let feed = feed_text(reply, &[]);

    assert_eq!(feed.as_deref(), Some(reply));
}

#[test]
fn marcador_antigo_sozinho_tambem_aparece_e_nao_vira_variavel_aceita() {
    let reply = "$contagem = 3";

    assert_eq!(feed_text(reply, &[]).as_deref(), Some(reply));
}

#[test]
fn resposta_vazia_com_uma_variavel_aceita_pela_ferramenta_mostra_o_valor() {
    let accepted = [Change::Set {
        name: "contagem".to_string(),
        value: crate::task::VarValue::Number(3),
        share: false,
    }];

    assert_eq!(feed_text("", &accepted).as_deref(), Some("3"));
}

#[test]
fn regra_de_ferramentas_so_entra_no_papel_quando_ha_mcp() {
    with_team(|team| {
        let (iara, _) = iara_and_davi();
        let profile = team.profile(&iara).expect("perfil");

        let with = role(profile, &team.roster_line(), "contexto", true);
        let without = role(profile, &team.roster_line(), "contexto", false);

        assert!(with.contains(TOOLS_RULE_TEXT), "{with}");
        assert!(with.contains("delegate"), "{with}");
        assert!(!without.contains(TOOLS_RULE_TEXT), "{without}");
        assert!(!without.contains("delegate"), "{without}");
    });
}

#[test]
fn papel_nunca_ensina_a_sintaxe_de_texto_do_agente_com_ou_sem_mcp() {
    with_team(|team| {
        let (iara, _) = iara_and_davi();
        let profile = team.profile(&iara).expect("perfil");

        for with_tools in [true, false] {
            let text = role(profile, &team.roster_line(), "contexto", with_tools);

            for marker in OLD_MARKERS {
                assert!(
                    !text.contains(marker),
                    "with_tools={with_tools} {marker}: {text}"
                );
            }
        }
    });
}

#[test]
fn prompt_do_passo_ensina_as_ferramentas_com_mcp_e_nenhuma_sintaxe_sem_mcp() {
    let (iara, _) = iara_and_davi();
    let definition = empty_definition(iara.clone());
    let cache = Cache::default();

    let with = StepPrompts::build(&cache, &definition, &iara, "Faça", true);
    let without = StepPrompts::build(&cache, &definition, &iara, "Faça", false);

    for prompt in [&with.opening, &with.follow_up, &with.minimal] {
        assert!(prompt.contains("set_variable"), "{prompt}");
        assert!(prompt.contains("get_variables"), "{prompt}");
    }
    for prompt in [&without.opening, &without.follow_up, &without.minimal] {
        assert!(!prompt.contains("set_variable"), "{prompt}");
        assert!(prompt.contains("Execute a instrução agora"), "{prompt}");
    }
    for prompt in [
        &with.opening,
        &with.follow_up,
        &with.minimal,
        &without.opening,
        &without.follow_up,
        &without.minimal,
    ] {
        for marker in OLD_MARKERS {
            assert!(!prompt.contains(marker), "{marker}: {prompt}");
        }
    }
}

#[test]
fn next_turn_passa_a_vez_ao_agente_delegado_e_empilha_quem_pediu() {
    with_team(|team| {
        let (iara, davi) = iara_and_davi();
        let definition = empty_definition(iara.clone());
        let turn = Turn::first(&definition, &iara, "Faça");
        let delegation = (davi.clone(), "Revise".to_string());

        let next = next_turn(&definition, &turn, "feed", Some(&delegation), team)
            .expect("deveria haver próximo turno");

        assert_eq!(next.speaker, davi);
        assert_eq!(next.base, "Revise");
        assert_eq!(next.requesters, [iara]);
    });
}

#[test]
fn mencao_do_usuario_na_barra_chega_intacta_ao_primeiro_turno() {
    let (iara, _) = iara_and_davi();
    let definition = empty_definition(iara.clone());

    let turn = Turn::first(&definition, &iara, "@Davi revise o login");

    assert_eq!(turn.base, "Pedido do usuário: @Davi revise o login");
    assert_eq!(turn.received, "@Davi revise o login");
}
