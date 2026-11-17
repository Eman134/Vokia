use std::ffi::OsStr;

use super::*;

fn removed_vars(command: &Command) -> Vec<String> {
    command
        .as_std()
        .get_envs()
        .filter(|(_, value)| value.is_none())
        .map(|(name, _)| name.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn comando_base_remove_as_credenciais_de_api_do_ambiente() {
    let command = base_command(Path::new("claude"));

    let mut removed = removed_vars(&command);
    removed.sort();

    assert_eq!(
        removed,
        vec!["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "AWS_BEARER_TOKEN_BEDROCK"]
    );
}

#[test]
fn comando_base_mantem_o_token_da_assinatura_e_a_rota() {
    let command = base_command(Path::new("claude"));

    let touched: Vec<&OsStr> = command.as_std().get_envs().map(|(name, _)| name).collect();

    assert!(!touched.contains(&OsStr::new("CLAUDE_CODE_OAUTH_TOKEN")));
    assert!(!touched.contains(&OsStr::new("ANTHROPIC_BASE_URL")));
}
