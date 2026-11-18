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
        vec![
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "AWS_BEARER_TOKEN_BEDROCK"
        ]
    );
}

#[test]
fn comando_base_mantem_o_token_da_assinatura_e_a_rota() {
    let command = base_command(Path::new("claude"));

    let touched: Vec<&OsStr> = command.as_std().get_envs().map(|(name, _)| name).collect();

    assert!(!touched.contains(&OsStr::new("CLAUDE_CODE_OAUTH_TOKEN")));
    assert!(!touched.contains(&OsStr::new("ANTHROPIC_BASE_URL")));
}

const MCP_CONFIG_FLAG: &str = "--mcp-config";
const STRICT_FLAG: &str = "--strict-mcp-config";
const APPEND_FLAG: &str = "--append-system-prompt";
const ALLOWED_FLAG: &str = "--allowedTools";
const LEVELS: [PermissionLevel; 3] = [
    PermissionLevel::Liberado,
    PermissionLevel::Edicao,
    PermissionLevel::Leitura,
];

struct McpFixture {
    // O app mantém o banco e o AppHandle que registram as ferramentas do servidor.
    _app: tauri::App,
    dir: std::path::PathBuf,
    session: crate::mcp::McpSession,
}

fn mcp_fixture(folder: &str) -> McpFixture {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "vokia-command-mcp-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    let config_dir = dir.join(folder);
    let app = crate::mcp::support::build_app(&dir);
    let session = tauri::async_runtime::block_on(async {
        let server = crate::mcp::McpServer::start().await.expect("servidor MCP");
        server.attach(app.handle().clone());
        server
            .open_session(
                &config_dir,
                crate::mcp::Binding {
                    project_id: 1,
                    agent: crate::agents::Agent::from_stored("iara").expect("agente"),
                    agent_name: "Iara".to_string(),
                    slot: crate::mcp::ExchangeSlot::default(),
                },
            )
            .expect("sessão MCP")
    });
    McpFixture {
        _app: app,
        dir,
        session,
    }
}

impl Drop for McpFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn args_of(access: Access<'_>, mcp: Option<&McpConfigFile>) -> Vec<String> {
    agent_command(
        Path::new("claude"),
        &["-p", "oi"],
        "papel",
        "haiku",
        access,
        mcp,
    )
    .as_std()
    .get_args()
    .map(|arg| arg.to_string_lossy().into_owned())
    .collect()
}

fn project_access(level: PermissionLevel) -> ProjectAccess<'static> {
    ProjectAccess {
        dir: Path::new("."),
        level,
    }
}

fn position(args: &[String], flag: &str) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .unwrap_or_else(|| panic!("{flag} deveria estar nos argumentos: {args:?}"))
}

#[test]
fn com_mcp_o_arquivo_vem_antes_do_append_e_o_strict_logo_depois_nos_tres_niveis() {
    let fixture = mcp_fixture("mcp");
    let config = fixture.session.config();

    for level in LEVELS {
        let args = args_of(Access::Project(project_access(level)), Some(config));

        let at = position(&args, MCP_CONFIG_FLAG);
        assert_eq!(args[at + 1], config.path().to_string_lossy(), "{level:?}");
        assert_eq!(args[at + 2], STRICT_FLAG, "{level:?}");
        assert!(at + 2 < position(&args, APPEND_FLAG), "{level:?}");
        assert_eq!(args[args.len() - 2], APPEND_FLAG, "{level:?}");
    }
}

#[test]
fn caminho_do_arquivo_com_espaco_segue_como_um_unico_argumento() {
    let fixture = mcp_fixture("pasta com espaço");
    let config = fixture.session.config();

    let args = args_of(
        Access::Project(project_access(PermissionLevel::Edicao)),
        Some(config),
    );

    let at = position(&args, MCP_CONFIG_FLAG);
    assert!(args[at + 1].contains("pasta com espaço"));
    assert_eq!(args[at + 1], config.path().to_string_lossy());
}

#[test]
fn sem_mcp_nenhum_argumento_de_mcp_aparece_nos_tres_niveis() {
    for level in LEVELS {
        let args = args_of(Access::Project(project_access(level)), None);

        assert!(
            !args
                .iter()
                .any(|arg| arg == MCP_CONFIG_FLAG || arg == STRICT_FLAG),
            "{level:?}"
        );
        assert!(!args.iter().any(|arg| arg.contains("mcp__")), "{level:?}");
    }
}

#[test]
fn em_text_only_o_mcp_e_ignorado_mesmo_quando_fornecido() {
    let fixture = mcp_fixture("mcp");

    let args = args_of(Access::TextOnly, Some(fixture.session.config()));

    assert!(!args
        .iter()
        .any(|arg| arg == MCP_CONFIG_FLAG || arg == STRICT_FLAG));
    assert!(!args.iter().any(|arg| arg == ALLOWED_FLAG));
    assert!(!args.iter().any(|arg| arg.contains("mcp__")));
}

#[test]
fn o_curinga_mcp_vokia_sozinho_nunca_entra_em_allowed_tools() {
    let fixture = mcp_fixture("mcp");

    for level in LEVELS {
        let args = args_of(
            Access::Project(project_access(level)),
            Some(fixture.session.config()),
        );

        let allowed = &args[position(&args, ALLOWED_FLAG) + 1];
        assert!(
            !allowed.split(',').any(|tool| tool == "mcp__vokia"),
            "{level:?}: {allowed}"
        );
    }
}

#[test]
fn comando_base_e_de_agente_definem_no_proxy_para_o_loopback() {
    let fixture = mcp_fixture("mcp");
    let no_proxy = |command: &Command| {
        command
            .as_std()
            .get_envs()
            .find(|(name, _)| *name == OsStr::new("NO_PROXY"))
            .and_then(|(_, value)| value.map(|value| value.to_string_lossy().into_owned()))
    };

    let base = base_command(Path::new("claude"));
    let agent = agent_command(
        Path::new("claude"),
        &["-p", "oi"],
        "papel",
        "haiku",
        Access::Project(project_access(PermissionLevel::Leitura)),
        Some(fixture.session.config()),
    );

    assert_eq!(no_proxy(&base).as_deref(), Some("127.0.0.1,localhost"));
    assert_eq!(no_proxy(&agent).as_deref(), Some("127.0.0.1,localhost"));
}

const VOKIA_TOOLS: [&str; 16] = [
    "mcp__vokia__list_backlog",
    "mcp__vokia__read_card",
    "mcp__vokia__create_card",
    "mcp__vokia__move_card",
    "mcp__vokia__update_card",
    "mcp__vokia__backlog_add_label",
    "mcp__vokia__backlog_remove_label",
    "mcp__vokia__backlog_set_responsible",
    "mcp__vokia__backlog_archive_item",
    "mcp__vokia__backlog_restore_item",
    "mcp__vokia__backlog_delete_item",
    "mcp__vokia__request_decision",
    "mcp__vokia__propose_request",
    "mcp__vokia__delegate",
    "mcp__vokia__get_variables",
    "mcp__vokia__set_variable",
];

#[test]
fn cada_ferramenta_do_vokia_aparece_em_allowed_tools_nos_tres_niveis() {
    let fixture = mcp_fixture("mcp");

    for level in LEVELS {
        let args = args_of(
            Access::Project(project_access(level)),
            Some(fixture.session.config()),
        );

        let allowed = &args[position(&args, ALLOWED_FLAG) + 1];
        let tools: Vec<&str> = allowed.split(',').collect();
        for name in VOKIA_TOOLS {
            assert!(
                tools.contains(&name),
                "{level:?}: faltou {name} em {allowed}"
            );
        }
    }
    assert_eq!(fixture.session.config().allowed_tools(), VOKIA_TOOLS);
}

#[test]
fn as_ferramentas_do_mcp_entram_depois_das_do_nivel_sem_remover_nenhuma() {
    let fixture = mcp_fixture("mcp");

    for level in LEVELS {
        let without = args_of(Access::Project(project_access(level)), None);
        let with = args_of(
            Access::Project(project_access(level)),
            Some(fixture.session.config()),
        );

        let base = without[position(&without, ALLOWED_FLAG) + 1].clone();
        let expected = format!("{base},{}", VOKIA_TOOLS.join(","));
        assert_eq!(
            with[position(&with, ALLOWED_FLAG) + 1],
            expected,
            "{level:?}"
        );
    }
}
