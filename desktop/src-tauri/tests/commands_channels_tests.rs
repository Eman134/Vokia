use super::*;

const KNOWN_AGENTS: [&str; 4] = ["axiom", "vexa", "kairo", "sentry"];
const INVALID_NAME: &str =
    "Nome de canal inválido: use de 2 a 32 letras minúsculas, números ou hífen.";
const TOO_FEW: &str = "Um canal precisa de pelo menos 2 agentes.";

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|id| id.to_string()).collect()
}

fn normalize(name: &str, members: &[&str]) -> Result<(String, Vec<String>), String> {
    normalize_channel(name, &ids(members), |id| KNOWN_AGENTS.contains(&id))
}

#[test]
fn nome_com_espaco_nas_pontas_hashtag_e_maiuscula_vira_minusculo_sem_marcas() {
    let (name, _) = normalize("  #Front-End2 ", &["axiom", "vexa"]).unwrap();

    assert_eq!(name, "front-end2");
}

#[test]
fn nomes_nos_limites_de_tamanho_sao_aceitos() {
    let longest = "a".repeat(32);

    assert_eq!(normalize("ab", &["axiom", "vexa"]).unwrap().0, "ab");
    assert_eq!(normalize(&longest, &["axiom", "vexa"]).unwrap().0, longest);
}

#[test]
fn nomes_fora_das_regras_falham_com_a_mensagem_de_nome_invalido() {
    let too_long = "a".repeat(33);
    let invalid = [
        "",
        "   ",
        "#",
        "a",
        too_long.as_str(),
        "ação",
        "front end",
        "front_end",
        "-front",
        "##front",
        "fro#nt",
    ];

    for name in invalid {
        assert_eq!(
            normalize(name, &["axiom", "vexa"]),
            Err(INVALID_NAME.to_string()),
            "nome {name:?}"
        );
    }
}

#[test]
fn geral_com_ou_sem_hashtag_e_maiuscula_e_reservado() {
    for name in ["geral", "#geral", " GERAL "] {
        assert_eq!(
            normalize(name, &["axiom", "vexa"]),
            Err("O canal #geral já existe.".to_string()),
            "nome {name:?}"
        );
    }
}

#[test]
fn menos_de_dois_agentes_distintos_falha() {
    for members in [&["axiom", "axiom"][..], &["vexa"], &[]] {
        assert_eq!(
            normalize("time", members),
            Err(TOO_FEW.to_string()),
            "membros {members:?}"
        );
    }
}

#[test]
fn membros_repetidos_sao_removidos_mantendo_a_ordem_da_primeira_aparicao() {
    let (_, members) = normalize("time", &["sentry", "axiom", "sentry", "vexa", "axiom"]).unwrap();

    assert_eq!(members, ids(&["sentry", "axiom", "vexa"]));
}

#[test]
fn id_fora_do_registro_falha_citando_o_id() {
    assert_eq!(
        normalize("time", &["axiom", "fantasma"]),
        Err("Agente desconhecido: fantasma.".to_string())
    );
    assert_eq!(
        normalize("time", &["fantasma"]),
        Err("Agente desconhecido: fantasma.".to_string())
    );
}

#[test]
fn nome_invalido_vence_o_membro_desconhecido() {
    assert_eq!(
        normalize("Ação", &["fantasma", "axiom"]),
        Err(INVALID_NAME.to_string())
    );
}
