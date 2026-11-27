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
    mcp_fixture_for(folder, crate::mcp::ToolScope::Coordinator)
}

fn mcp_fixture_for(folder: &str, scope: crate::mcp::ToolScope) -> McpFixture {
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
                    agent: crate::agents::Agent::from_stored("axiom").expect("agente"),
                    agent_name: "Axiom".to_string(),
                    slot: crate::mcp::ExchangeSlot::default(),
                    scope,
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
        &[],
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
        attachments: Path::new("."),
        extra_dirs: &[],
        level,
        builtin_tools: true,
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
    let config = fixture.session.config().expect("config");

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
    let config = fixture.session.config().expect("config");

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

    let args = args_of(Access::TextOnly, Some(fixture.session.config().expect("config")));

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
            Some(fixture.session.config().expect("config")),
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
        &[],
        &["-p", "oi"],
        "papel",
        "haiku",
        Access::Project(project_access(PermissionLevel::Leitura)),
        Some(fixture.session.config().expect("config")),
    );

    assert_eq!(no_proxy(&base).as_deref(), Some("127.0.0.1,localhost"));
    assert_eq!(no_proxy(&agent).as_deref(), Some("127.0.0.1,localhost"));
}

const VOKIA_TOOLS: [&str; 27] = [
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
    "mcp__vokia__get_orchestration",
    "mcp__vokia__list_agents",
    "mcp__vokia__save_item_plan",
    "mcp__vokia__artifact_set_criterion",
    "mcp__vokia__artifact",
    "mcp__vokia__list_movements",
    "mcp__vokia__post_channel_message",
    "mcp__vokia__memory_graph_query",
    "mcp__vokia__memory_graph_save",
    "mcp__vokia__app_commands_list",
    "mcp__vokia__app_command_call",
];

#[test]
fn cada_ferramenta_do_vokia_aparece_em_allowed_tools_nos_tres_niveis() {
    let fixture = mcp_fixture("mcp");

    for level in LEVELS {
        let args = args_of(
            Access::Project(project_access(level)),
            Some(fixture.session.config().expect("config")),
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
    assert_eq!(fixture.session.config().expect("config").allowed_tools(), VOKIA_TOOLS);
}

#[test]
fn as_ferramentas_do_mcp_entram_depois_das_do_nivel_sem_remover_nenhuma() {
    let fixture = mcp_fixture("mcp");

    for level in LEVELS {
        let without = args_of(Access::Project(project_access(level)), None);
        let with = args_of(
            Access::Project(project_access(level)),
            Some(fixture.session.config().expect("config")),
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

#[test]
fn argumentos_do_usuario_entram_antes_de_tools_allowed_tools_e_append_system_prompt() {
    let fixture = mcp_fixture("mcp");
    let user_args = vec!["--user-custom-flag".to_string(), "custom_val".to_string()];
    let command = agent_command(
        Path::new("claude"),
        &user_args,
        &["-p", "oi"],
        "Tech Lead",
        "sonnet",
        Access::Project(project_access(PermissionLevel::Edicao)),
        Some(fixture.session.config().expect("config")),
    );
    let args: Vec<String> = command
        .as_std()
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();

    let pos_user = position(&args, "--user-custom-flag");
    let pos_tools = position(&args, "--tools");
    let pos_allowed = position(&args, ALLOWED_FLAG);
    let pos_append = position(&args, APPEND_FLAG);

    assert!(pos_user < pos_tools, "user_args ({pos_user}) deve vir antes de --tools ({pos_tools})");
    assert!(pos_user < pos_allowed, "user_args ({pos_user}) deve vir antes de --allowedTools ({pos_allowed})");
    assert!(pos_user < pos_append, "user_args ({pos_user}) deve vir antes de --append-system-prompt ({pos_append})");
}

#[test]
fn command_domain_cobre_todos_os_commands_e_retorna_none_para_desconhecido() {
    use crate::commands::dispatcher::{command_domain, list_commands};

    let commands = list_commands();
    assert!(!commands.is_empty());
    for cmd in &commands {
        let name = cmd["name"].as_str().expect("name");
        let domain = cmd["domain"].as_str().expect("domain");
        assert_eq!(
            command_domain(name),
            Some(domain),
            "comando {name}"
        );
    }

    assert_eq!(command_domain("role_create"), Some("roles"));
    assert_eq!(command_domain("create_agent"), Some("agents"));
    assert_eq!(command_domain("get_settings"), Some("settings"));
    assert_eq!(command_domain("delete_channel"), Some("channels"));
    assert_eq!(command_domain("comando_inexistente_xyz"), None);
    assert_eq!(command_domain(""), None);
}

const COORDINATION_TOOLS: [&str; 8] = [
    "mcp__vokia__backlog_archive_item",
    "mcp__vokia__backlog_restore_item",
    "mcp__vokia__backlog_delete_item",
    "mcp__vokia__get_orchestration",
    "mcp__vokia__list_agents",
    "mcp__vokia__list_movements",
    "mcp__vokia__app_commands_list",
    "mcp__vokia__app_command_call",
];

#[test]
fn worker_recebe_em_allowed_tools_so_as_ferramentas_de_trabalho() {
    let fixture = mcp_fixture_for("mcp", crate::mcp::ToolScope::Worker);
    let allowed = fixture.session.config().expect("config").allowed_tools();

    let expected: Vec<&str> = VOKIA_TOOLS
        .into_iter()
        .filter(|name| !COORDINATION_TOOLS.contains(name))
        .collect();
    assert_eq!(allowed, expected);
}

#[test]
fn sem_ferramentas_nativas_o_agente_fica_so_com_o_mcp_nos_tres_niveis() {
    let fixture = mcp_fixture("mcp");

    for level in LEVELS {
        let access = ProjectAccess {
            builtin_tools: false,
            ..project_access(level)
        };
        let args = args_of(Access::Project(access), Some(fixture.session.config().expect("config")));

        let at = position(&args, "--tools");
        assert_eq!(args[at + 1], "", "{level:?}");
        assert_eq!(args.iter().filter(|arg| *arg == "--tools").count(), 1, "{level:?}");
        let allowed = &args[position(&args, ALLOWED_FLAG) + 1];
        assert!(allowed.contains("mcp__vokia__delegate"), "{level:?}: {allowed}");
    }
}

#[test]
fn agente_desliga_a_busca_adiada_de_ferramentas_da_cli() {
    let tool_search = |command: &Command| {
        command
            .as_std()
            .get_envs()
            .find(|(name, _)| *name == OsStr::new("ENABLE_TOOL_SEARCH"))
            .and_then(|(_, value)| value.map(|value| value.to_string_lossy().into_owned()))
    };

    for access in [Access::TextOnly, Access::Project(project_access(PermissionLevel::Edicao))] {
        let command = agent_command(Path::new("claude"), &[], &["-p", "oi"], "papel", "haiku", access, None);
        assert_eq!(tool_search(&command).as_deref(), Some("false"));
    }
}

#[test]
fn comando_base_preserva_no_proxy() {
    let command = base_command(Path::new("claude"));
    let no_proxy = command
        .as_std()
        .get_envs()
        .find(|(name, _)| *name == OsStr::new("NO_PROXY"))
        .and_then(|(_, value)| value.map(|v| v.to_string_lossy().into_owned()));
    assert_eq!(no_proxy.as_deref(), Some("127.0.0.1,localhost"));
}

#[test]
fn parse_cmd_shim_resolve_shim_npm_com_dp0_e_espacos_no_caminho() {
    let temp = std::env::temp_dir().join(format!(
        "vokia-shim-npm-{}-dir com espaco",
        std::process::id()
    ));
    let script_dir = temp.join("node_modules").join("pkg").join("bin");
    std::fs::create_dir_all(&script_dir).expect("criar dir");
    let node_exe = temp.join("node.exe");
    let script_js = script_dir.join("cli.js");
    std::fs::write(&node_exe, b"").expect("criar node.exe fake");
    std::fs::write(&script_js, b"console.log('ok');").expect("criar cli.js");

    let shim = r#"@ECHO off
GOTO start
:find_dp0
SET dp0=%~dp0
EXIT /b
:start
SETLOCAL
CALL :find_dp0
endLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & "%_prog%"  "%dp0%\node_modules\pkg\bin\cli.js" %*
"#;

    let resolved = crate::silent_process::parse_cmd_shim(&temp, shim)
        .expect("shim npm deve resolver para node.exe + script");
    assert_eq!(resolved.program, node_exe);
    assert_eq!(
        resolved.prefix_args,
        vec![script_js.to_string_lossy().into_owned()]
    );

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn parse_cmd_shim_resolve_wrapper_para_exe_nativo() {
    let temp = std::env::temp_dir().join(format!("vokia-shim-exe-{}", std::process::id()));
    let bin_dir = temp.join("bin");
    std::fs::create_dir_all(&bin_dir).expect("criar dir");
    let target_exe = bin_dir.join("tool.exe");
    std::fs::write(&target_exe, b"").expect("criar tool.exe fake");

    let shim = "@echo off\n\"%~dp0\\bin\\tool.exe\" --stdio %*\n";
    let resolved = crate::silent_process::parse_cmd_shim(&temp, shim)
        .expect("wrapper .exe deve resolver diretamente");
    assert_eq!(resolved.program, target_exe);
    assert_eq!(resolved.prefix_args, vec!["--stdio".to_string()]);

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn parse_cmd_shim_faz_fallback_seguro_para_batch_complexo() {
    let temp = std::env::temp_dir().join(format!("vokia-shim-complex-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp);

    let shim = "@echo off\nfor %%i in (%*) do echo %%i\npowershell -Command %1 %*\n";
    assert!(crate::silent_process::parse_cmd_shim(&temp, shim).is_none());

    let _ = std::fs::remove_dir_all(&temp);
}

#[cfg(windows)]
#[test]
fn resolve_direct_executable_prefere_exe_irmao_ao_lado_de_cmd() {
    let temp = std::env::temp_dir().join(format!("vokia-sibling-exe-{}", std::process::id()));
    std::fs::create_dir_all(&temp).expect("criar dir");
    let cmd_path = temp.join("meu-cli.cmd");
    let exe_path = temp.join("meu-cli.exe");
    std::fs::write(&cmd_path, b"@echo off\r\necho hi %*\r\n").expect("criar .cmd");
    std::fs::write(&exe_path, b"").expect("criar .exe");

    let resolved = crate::silent_process::resolve_direct_executable(&cmd_path);
    assert_eq!(resolved.program, exe_path);
    assert!(resolved.prefix_args.is_empty());

    let _ = std::fs::remove_dir_all(&temp);
}

#[cfg(windows)]
#[test]
fn spawn_silent_executa_no_windows_e_captura_stdout_stderr_e_status() {
    use tokio::io::AsyncReadExt;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");

    rt.block_on(async {
        let mut cmd = crate::silent_process::configure_tokio_command(Path::new("cmd.exe"));
        cmd.args(["/c", "echo linha_out & echo linha_err 1>&2"]);
        let mut child = crate::silent_process::spawn_silent(cmd).expect("spawn_silent");
        drop(child.stdin.take());

        let mut out = Vec::new();
        let mut err = Vec::new();
        if let Some(mut stdout) = child.stdout.take() {
            stdout.read_to_end(&mut out).await.expect("ler stdout");
        }
        if let Some(mut stderr) = child.stderr.take() {
            stderr.read_to_end(&mut err).await.expect("ler stderr");
        }
        let status = child.wait().await.expect("wait");
        assert!(status.success());
        assert!(String::from_utf8_lossy(&out).contains("linha_out"));
        assert!(String::from_utf8_lossy(&err).contains("linha_err"));
    });
}
