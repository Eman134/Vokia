use tauri::State;

use crate::agents::{Agent, AgentRegistry};
use crate::db::{Channel, DbState};

const GENERAL_CHANNEL: &str = "geral";
const NAME_MIN_CHARS: usize = 2;
const NAME_MAX_CHARS: usize = 32;
const MIN_MEMBERS: usize = 2;
const INVALID_NAME_MESSAGE: &str =
    "Nome de canal inválido: use de 2 a 32 letras minúsculas, números ou hífen.";
const GENERAL_TAKEN_MESSAGE: &str = "O canal #geral já existe.";
const TOO_FEW_MEMBERS_MESSAGE: &str = "Um canal precisa de pelo menos 2 agentes.";

#[tauri::command]
pub fn list_channels(state: State<'_, DbState>, project_id: i64) -> Result<Vec<Channel>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.list_channels(project_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_channel(
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    project_id: i64,
    name: String,
    members: Vec<String>,
) -> Result<Channel, String> {
    let team = registry.team().map_err(|error| error.to_string())?;
    let (name, members) = normalize_channel(&name, &members, |id| {
        Agent::from_stored(id).is_some_and(|agent| team.profile(&agent).is_some())
    })?;
    let db = state.require().map_err(|error| error.to_string())?;
    db.create_channel(project_id, &name, &members)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_channel(state: State<'_, DbState>, project_id: i64, id: i64) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.delete_channel(project_id, id)
        .map_err(|error| error.to_string())
}

pub(crate) fn normalize_channel(
    name: &str,
    members: &[String],
    is_agent: impl Fn(&str) -> bool,
) -> Result<(String, Vec<String>), String> {
    let name = normalize_name(name)?;
    let mut distinct: Vec<String> = Vec::with_capacity(members.len());
    for id in members {
        if !is_agent(id) {
            return Err(format!("Agente desconhecido: {id}."));
        }
        if !distinct.contains(id) {
            distinct.push(id.clone());
        }
    }
    if distinct.len() < MIN_MEMBERS {
        return Err(TOO_FEW_MEMBERS_MESSAGE.to_string());
    }
    Ok((name, distinct))
}

fn normalize_name(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    let name = trimmed.strip_prefix('#').unwrap_or(trimmed).to_lowercase();
    let length = name.chars().count();
    let valid_chars = name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !(NAME_MIN_CHARS..=NAME_MAX_CHARS).contains(&length) || !valid_chars || name.starts_with('-')
    {
        return Err(INVALID_NAME_MESSAGE.to_string());
    }
    if name == GENERAL_CHANNEL {
        return Err(GENERAL_TAKEN_MESSAGE.to_string());
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KNOWN_AGENTS: [&str; 4] = ["iara", "davi", "nina", "teo"];
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
        let (name, _) = normalize("  #Front-End2 ", &["iara", "davi"]).unwrap();

        assert_eq!(name, "front-end2");
    }

    #[test]
    fn nomes_nos_limites_de_tamanho_sao_aceitos() {
        let longest = "a".repeat(32);

        assert_eq!(normalize("ab", &["iara", "davi"]).unwrap().0, "ab");
        assert_eq!(normalize(&longest, &["iara", "davi"]).unwrap().0, longest);
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
                normalize(name, &["iara", "davi"]),
                Err(INVALID_NAME.to_string()),
                "nome {name:?}"
            );
        }
    }

    #[test]
    fn geral_com_ou_sem_hashtag_e_maiuscula_e_reservado() {
        for name in ["geral", "#geral", " GERAL "] {
            assert_eq!(
                normalize(name, &["iara", "davi"]),
                Err("O canal #geral já existe.".to_string()),
                "nome {name:?}"
            );
        }
    }

    #[test]
    fn menos_de_dois_agentes_distintos_falha() {
        for members in [&["iara", "iara"][..], &["davi"], &[]] {
            assert_eq!(
                normalize("time", members),
                Err(TOO_FEW.to_string()),
                "membros {members:?}"
            );
        }
    }

    #[test]
    fn membros_repetidos_sao_removidos_mantendo_a_ordem_da_primeira_aparicao() {
        let (_, members) = normalize("time", &["teo", "iara", "teo", "davi", "iara"]).unwrap();

        assert_eq!(members, ids(&["teo", "iara", "davi"]));
    }

    #[test]
    fn id_fora_do_registro_falha_citando_o_id() {
        assert_eq!(
            normalize("time", &["iara", "fantasma"]),
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
            normalize("Ação", &["fantasma", "iara"]),
            Err(INVALID_NAME.to_string())
        );
    }
}
