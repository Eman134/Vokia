use std::path::Path;

use crate::claude::{Access, ClaudeError, ProjectAccess};
use crate::permissions::PermissionLevel;

use super::*;

fn project_access(level: PermissionLevel) -> ProjectAccess<'static> {
    ProjectAccess {
        dir: Path::new("C:/test/project"),
        attachments: Path::new("C:/test/attachments"),
        level,
        builtin_tools: true,
    }
}

fn args_of(command: &tokio::process::Command) -> Vec<String> {
    command
        .as_std()
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

fn position(args: &[String], flag: &str) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .unwrap_or_else(|| panic!("{flag} deveria estar nos argumentos: {args:?}"))
}

// ==========================================
// 1. ProviderAdapter
// ==========================================

#[test]
fn provider_adapter_from_cli_reconhece_os_quatro_provedores_e_rejeita_desconhecido() {
    let providers = crate::providers::ProviderRegistry::seeded();
    let claude = ProviderAdapter::from_cli("claude", &providers).unwrap();
    let codex = ProviderAdapter::from_cli("codex", &providers).unwrap();
    let opencode = ProviderAdapter::from_cli("opencode", &providers).unwrap();
    let antigravity = ProviderAdapter::from_cli("antigravity", &providers).unwrap();

    assert!(matches!(claude, ProviderAdapter::Claude(_)));
    assert!(matches!(codex, ProviderAdapter::CodexAppServer(_)));
    assert!(matches!(opencode, ProviderAdapter::OpenCode(_)));
    assert!(matches!(antigravity, ProviderAdapter::Antigravity(_)));

    let mut codex_jsonl_cfg = providers.get("codex").unwrap();
    codex_jsonl_cfg.protocol = crate::providers::ProviderProtocol::CodexJsonl;
    let codex_jsonl = ProviderAdapter::from_config(codex_jsonl_cfg);
    assert!(matches!(codex_jsonl, ProviderAdapter::Codex(_)));
    assert!(!codex_jsonl.supports_live_session());

    let err = ProviderAdapter::from_cli("desconhecido", &providers).unwrap_err();
    assert!(
        err.to_string().contains("Provedor de CLI desconhecido"),
        "mensagem inesperada: {err}"
    );

    assert!(claude.supports_mcp());
    assert!(codex.supports_mcp());
    assert!(!opencode.supports_mcp());
    assert!(!antigravity.supports_mcp());

    assert!(claude.supports_live_session());
    assert!(codex.supports_live_session());
    assert!(!opencode.supports_live_session());
    assert!(!antigravity.supports_live_session());
}

#[test]
fn run_cli_process_com_exit_code_diferente_de_zero_retorna_claude_error_exit() {
    tauri::async_runtime::block_on(async {
        let mut cmd = tokio::process::Command::new("cmd.exe");
        cmd.args(["/C", "echo falha no processo 1>&2 & exit 42"]);
        let pid = std::sync::atomic::AtomicU32::new(0);
        let res = run_cli_process(cmd, Access::TextOnly, &pid, None, None, |_| {}).await;
        match res {
            Err(ClaudeError::Exit { code, message }) => {
                assert_eq!(code, Some(42));
                assert!(message.contains("falha no processo"), "mensagem: {message}");
            }
            Err(other) => panic!("esperava ClaudeError::Exit, obteve {:?}", other),
            Ok(_) => panic!("processo com exit 42 deveria ter retornado erro"),
        }
    });
}

#[test]
fn run_cli_process_ignora_linhas_jsonl_malformadas_sem_abortar() {
    tauri::async_runtime::block_on(async {
        let mut cmd = tokio::process::Command::new("cmd.exe");
        cmd.args([
            "/C",
            "echo {malformed_json & echo {\"type\":\"result\",\"result\":\"ok\"}",
        ]);
        let pid = std::sync::atomic::AtomicU32::new(0);
        let mut lines = Vec::new();
        let res = run_cli_process(cmd, Access::TextOnly, &pid, None, None, |line| {
            lines.push(line.to_string());
        })
        .await;
        assert!(res.is_ok(), "não deveria abortar em JSONL malformado: {res:?}");
        assert_eq!(lines.len(), 2);
    });
}

// ==========================================
// 2. CMD-6: Codex Adapter
// ==========================================

#[test]
fn codex_build_command_text_only_e_project_levels_com_user_args_antes_das_flags() {
    let program = Path::new("codex");
    let user_args = vec!["--user-flag".to_string(), "custom_val".to_string()];
    let model = "gpt-6-luna";

    // TextOnly
    let cmd_text = codex::build_command(program, &user_args, model, Access::TextOnly, None);
    let args_text = args_of(&cmd_text);
    assert_eq!(args_text[0], "--user-flag");
    assert_eq!(args_text[1], "custom_val");
    assert_eq!(args_text[2], "exec");
    assert_eq!(args_text[3], "--json");
    assert_eq!(args_text[4], "--skip-git-repo-check");
    assert_eq!(args_text[5], "-m");
    assert_eq!(args_text[6], "gpt-6-luna");
    assert_eq!(args_text[7], "-s");
    assert_eq!(args_text[8], "read-only");
    assert_eq!(args_text[9], "-");

    // Project: Leitura
    let cmd_leitura = codex::build_command(
        program,
        &user_args,
        model,
        Access::Project(project_access(PermissionLevel::Leitura)),
        None,
    );
    let args_leitura = args_of(&cmd_leitura);
    assert!(position(&args_leitura, "--user-flag") < position(&args_leitura, "exec"));
    assert_eq!(args_leitura[position(&args_leitura, "-s") + 1], "read-only");

    // Project: Edicao
    let cmd_edicao = codex::build_command(
        program,
        &user_args,
        model,
        Access::Project(project_access(PermissionLevel::Edicao)),
        None,
    );
    let args_edicao = args_of(&cmd_edicao);
    assert_eq!(args_edicao[position(&args_edicao, "-s") + 1], "workspace-write");

    // Project: Liberado
    let cmd_liberado = codex::build_command(
        program,
        &user_args,
        model,
        Access::Project(project_access(PermissionLevel::Liberado)),
        None,
    );
    let args_liberado = args_of(&cmd_liberado);
    assert_eq!(args_liberado[position(&args_liberado, "-s") + 1], "danger-full-access");
}

#[test]
fn codex_parse_jsonl_output_processa_eventos_item_completed_turn_failed_e_ignora_desconhecidos() {
    // 1. Mensagens válidas combinadas, ignorando item.updated e eventos desconhecidos
    let jsonl = r#"
{"type": "thread.created", "thread_id": "t-1"}
{"type": "item.updated", "item": {"type": "agent_message", "text": "Texto intermediário streaming (deve ser ignorado)"}}
{"type": "item.completed", "item": {"type": "agent_message", "text": "Primeira linha"}}
{"type": "random.event", "value": 42}
{"type": "item.updated", "item": {"type": "agent_message", "text": "Outro update intermediário (deve ser ignorado)"}}
{"type": "item.completed", "item": {"type": "agent_message", "text": "Segunda linha"}}
"#;
    let reply = codex::parse_jsonl_output(jsonl.as_bytes()).unwrap();
    assert_eq!(reply.text, "Primeira linha\n\nSegunda linha");

    // 2. Erro via turn.failed
    let error_jsonl = r#"
{"type": "turn.failed", "error": {"message": "Limite de tokens atingido no Codex"}}
"#;
    match codex::parse_jsonl_output(error_jsonl.as_bytes()) {
        Err(ClaudeError::Reported(msg)) => assert_eq!(msg, "Limite de tokens atingido no Codex"),
        _ => panic!("esperava Reported"),
    }

    // 3. Erro via error
    match codex::parse_jsonl_output(br#"{"type": "error", "message": "Erro de auth"}"#) {
        Err(ClaudeError::Reported(msg)) => assert_eq!(msg, "Erro de auth"),
        _ => panic!("esperava Reported"),
    }

    // 4. Texto puro não-JSON volta como fallback
    let plain_text = b"Resposta simples em texto puro sem JSON";
    let reply_plain = codex::parse_jsonl_output(plain_text).unwrap();
    assert_eq!(reply_plain.text, "Resposta simples em texto puro sem JSON");
}

// ==========================================
// 3. CMD-7: OpenCode Adapter
// ==========================================

#[test]
fn opencode_build_command_text_only_e_leitura_e_edicao_e_recusa_liberado_conforme_d4() {
    let program = Path::new("opencode");
    let user_args = vec!["--user-arg".to_string()];
    let role = "Backend Lead";
    let model = "openai/gpt-6-astra";
    let message = "analise o código";

    // TextOnly
    let cmd_text = opencode::build_command(program, &user_args, role, model, Access::TextOnly, message).unwrap();
    let args_text = args_of(&cmd_text);
    assert_eq!(args_text[0], "--user-arg");
    assert_eq!(args_text[1], "run");
    assert_eq!(args_text[2], "--pure");
    assert_eq!(args_text[3], "--format");
    assert_eq!(args_text[4], "json");
    assert_eq!(args_text[5], "-m");
    assert_eq!(args_text[6], "openai/gpt-6-astra");
    assert_eq!(args_text.len(), 7);

    // Project: Leitura
    let cmd_leitura = opencode::build_command(
        program,
        &user_args,
        role,
        model,
        Access::Project(project_access(PermissionLevel::Leitura)),
        message,
    ).unwrap();
    let args_leitura = args_of(&cmd_leitura);
    assert!(args_leitura.contains(&"--dir".to_string()));
    assert!(!args_leitura.iter().any(|arg| arg.contains('\n')));

    // Project: Edicao
    let cmd_edicao = opencode::build_command(
        program,
        &user_args,
        role,
        model,
        Access::Project(project_access(PermissionLevel::Edicao)),
        message,
    ).unwrap();
    let args_edicao = args_of(&cmd_edicao);
    assert!(args_edicao.contains(&"--dir".to_string()));

    // Project: Liberado -> DEVE SER RECUSADO CONFORME DECISÃO D4
    let err_liberado = opencode::build_command(
        program,
        &user_args,
        role,
        model,
        Access::Project(project_access(PermissionLevel::Liberado)),
        message,
    ).unwrap_err();
    match err_liberado {
        ClaudeError::Reported(msg) => {
            assert!(
                msg.contains("OpenCode não oferece sandbox"),
                "mensagem inesperada para Liberado no OpenCode: {msg}"
            );
        }
        other => panic!("esperava Reported para Liberado no OpenCode, obteve {:?}", other),
    }
}

#[test]
fn opencode_parse_json_output_extrai_text_reporta_error_e_ignora_desconhecidos() {
    let jsonl = r#"
{"type": "init", "session": "s1"}
{"type": "text", "part": {"text": "Bloco 1 do OpenCode"}}
{"type": "step.start", "step": 1}
{"type": "text", "content": "Bloco 2 do OpenCode"}
"#;
    let reply = opencode::parse_json_output(jsonl.as_bytes()).unwrap();
    assert_eq!(reply.text, "Bloco 1 do OpenCode\n\nBloco 2 do OpenCode");

    let err_json = r#"{"type": "error", "error": {"data": {"message": "Falha crítica no OpenCode"}}}"#;
    match opencode::parse_json_output(err_json.as_bytes()) {
        Err(ClaudeError::Reported(msg)) => assert_eq!(msg, "Falha crítica no OpenCode"),
        _ => panic!("esperava Reported"),
    }

    let plain = b"Resposta simples em texto puro do OpenCode";
    let reply_plain = opencode::parse_json_output(plain).unwrap();
    assert_eq!(reply_plain.text, "Resposta simples em texto puro do OpenCode");
}

// ==========================================
// 4. CMD-8: Antigravity Adapter
// ==========================================

#[test]
fn antigravity_build_command_text_only_leitura_edicao_e_liberado() {
    let program = Path::new("agy");
    let user_args = vec!["--user-agy".to_string()];
    let role = "Tech Lead";
    let model = "gemini-4-hydrogen-medium";
    let message = "crie o plano";

    // TextOnly
    let cmd_text = antigravity::build_command(program, &user_args, role, model, Access::TextOnly, message);
    let args_text = args_of(&cmd_text);
    assert_eq!(args_text[0], "--user-agy");
    assert_eq!(args_text[1], "-p=");
    assert_eq!(args_text[2], "--input-format");
    assert_eq!(args_text[3], "stream-json");
    assert_eq!(args_text[4], "--output-format");
    assert_eq!(args_text[5], "stream-json");
    assert_eq!(args_text[6], "--model");
    assert_eq!(args_text[7], "gemini-4-hydrogen-medium");
    assert_eq!(args_text[8], "--disable-slash-commands");
    assert_eq!(args_text[9], "--sandbox");
    assert_eq!(args_text[10], "--mode");
    assert_eq!(args_text[11], "plan");
    assert!(!args_text.iter().any(|arg| arg.contains('\n')));

    // Project: Leitura
    let cmd_leitura = antigravity::build_command(
        program,
        &user_args,
        role,
        model,
        Access::Project(project_access(PermissionLevel::Leitura)),
        message,
    );
    let args_leitura = args_of(&cmd_leitura);
    assert_eq!(args_leitura[position(&args_leitura, "--mode") + 1], "plan");
    assert!(args_leitura.contains(&"--sandbox".to_string()));

    // Project: Edicao
    let cmd_edicao = antigravity::build_command(
        program,
        &user_args,
        role,
        model,
        Access::Project(project_access(PermissionLevel::Edicao)),
        message,
    );
    let args_edicao = args_of(&cmd_edicao);
    assert_eq!(args_edicao[position(&args_edicao, "--mode") + 1], "accept-edits");
    assert!(args_edicao.contains(&"--sandbox".to_string()));

    // Project: Liberado
    let cmd_liberado = antigravity::build_command(
        program,
        &user_args,
        role,
        model,
        Access::Project(project_access(PermissionLevel::Liberado)),
        message,
    );
    let args_liberado = args_of(&cmd_liberado);
    assert!(args_liberado.contains(&"--dangerously-skip-permissions".to_string()));
    assert!(!args_liberado.contains(&"--sandbox".to_string()));
}

#[test]
fn antigravity_parse_stream_output_processa_result_assistant_e_erros() {
    // 1. Evento result com is_error: false
    let result_json = r#"
{"type": "conversation.start"}
{"type": "result", "is_error": false, "result": "Resultado completo do Antigravity"}
"#;
    let reply = antigravity::parse_stream_output(result_json.as_bytes()).unwrap();
    assert_eq!(reply.text, "Resultado completo do Antigravity");

    // 2. Evento result com is_error: true
    let err_result = r#"{"type": "result", "is_error": true, "result": "Erro interno no Antigravity"}"#;
    match antigravity::parse_stream_output(err_result.as_bytes()) {
        Err(ClaudeError::Reported(msg)) => assert_eq!(msg, "Erro interno no Antigravity"),
        _ => panic!("esperava Reported"),
    }

    // 3. Evento assistant
    let assistant_json = r#"
{"type": "assistant", "message": {"content": [{"type": "text", "text": "Mensagem assistente 1"}]}}
{"type": "assistant", "message": {"content": [{"type": "text", "text": "Mensagem assistente 2"}]}}
"#;
    let reply_asst = antigravity::parse_stream_output(assistant_json.as_bytes()).unwrap();
    assert_eq!(reply_asst.text, "Mensagem assistente 1\n\nMensagem assistente 2");

    // 4. Evento error
    let err_event = r#"{"type": "error", "message": "Falha de rede"}"#;
    match antigravity::parse_stream_output(err_event.as_bytes()) {
        Err(ClaudeError::Reported(msg)) => assert_eq!(msg, "Falha de rede"),
        _ => panic!("esperava Reported"),
    }

    // 5. Fallback para texto puro
    let plain = b"Resposta simples em texto puro do Antigravity";
    let reply_plain = antigravity::parse_stream_output(plain).unwrap();
    assert_eq!(reply_plain.text, "Resposta simples em texto puro do Antigravity");
}

#[test]
fn antigravity_build_command_p_colado_nunca_seguido_de_valor_solto_e_prompt_fora_dos_args() {
    let program = Path::new("agy");
    let role = "PAPEL-SECRETO";
    let message = "linha um\nlinha dois MENSAGEM-SECRETA";
    let acessos = vec![
        Access::TextOnly,
        Access::Project(project_access(PermissionLevel::Leitura)),
        Access::Project(project_access(PermissionLevel::Edicao)),
        Access::Project(project_access(PermissionLevel::Liberado)),
    ];
    for access in acessos {
        let cmd = antigravity::build_command(program, &[], role, "m", access, message);
        let args = args_of(&cmd);
        assert_eq!(args.iter().filter(|a| a.as_str() == "-p=").count(), 1);
        assert!(!args.iter().any(|a| a == "-p"), "-p solto consumiria o proximo arg");
        let i = position(&args, "-p=");
        assert_eq!(args[i + 1], "--input-format");
        assert_eq!(args[i + 2], "stream-json");
        assert!(!args.iter().any(|a| a.contains("PAPEL-SECRETO") || a.contains("MENSAGEM-SECRETA")));
        assert!(!args.iter().any(|a| a.contains('\n')));
    }
}

#[test]
fn antigravity_parse_event_result_sucesso_usa_response_e_usage() {
    let out = r#"{"event":"step_update","step_update":{"step_type":"agent_response","text_delta":"parcial"}}
{"event":"result","result":{"status":"SUCCESS","response":"Resposta final\n","usage":{"input_tokens":10,"output_tokens":4,"cache_read_tokens":2}}}
"#;
    let reply = antigravity::parse_stream_output(out.as_bytes()).unwrap();
    assert_eq!(reply.text, "Resposta final");
    let usage = reply.usage.expect("usage esperado");
    assert_eq!(usage.input_tokens, Some(10));
    assert_eq!(usage.output_tokens, Some(4));
    assert_eq!(usage.cached_input_tokens, Some(2));
}

#[test]
fn antigravity_parse_event_result_status_error_retorna_reported_com_mensagem() {
    let out = r#"{"event":"step_update","step_update":{"step_type":"agent_response","text_delta":"x"}}
{"event":"result","result":{"status":"ERROR","error":"falha do agy"}}
"#;
    match antigravity::parse_stream_output(out.as_bytes()) {
        Err(ClaudeError::Reported(msg)) => assert_eq!(msg, "falha do agy"),
        other => panic!("esperava Reported, veio {:?}", other.map(|r| r.text)),
    }
    let sem_msg = r#"{"event":"result","result":{"status":"error"}}"#;
    match antigravity::parse_stream_output(sem_msg.as_bytes()) {
        Err(ClaudeError::Reported(msg)) => assert_eq!(msg, "erro desconhecido no Antigravity"),
        other => panic!("esperava Reported, veio {:?}", other.map(|r| r.text)),
    }
}

#[test]
fn antigravity_parse_step_update_text_delta_sem_result_concatena_deltas_e_ignora_outros_steps() {
    let out = r#"{"event":"step_update","step_update":{"step_type":"agent_response","text_delta":"Ola, "}}
{"event":"step_update","step_update":{"step_type":"tool_call","text_delta":"IGNORADO"}}
{"event":"step_update","step_update":{"step_type":"agent_response","text_delta":"mundo"}}
"#;
    let reply = antigravity::parse_stream_output(out.as_bytes()).unwrap();
    assert_eq!(reply.text, "Ola, mundo");
}

#[test]
fn antigravity_parse_formato_antigo_por_type_continua_compativel() {
    let antigo = r#"{"type":"result","is_error":false,"result":"texto antigo"}"#;
    assert_eq!(antigravity::parse_stream_output(antigo.as_bytes()).unwrap().text, "texto antigo");
    let antigo_erro = r#"{"type":"result","is_error":true,"result":"erro antigo"}"#;
    match antigravity::parse_stream_output(antigo_erro.as_bytes()) {
        Err(ClaudeError::Reported(msg)) => assert_eq!(msg, "erro antigo"),
        other => panic!("esperava Reported, veio {:?}", other.map(|r| r.text)),
    }
}

#[test]
fn tok4_flags_de_retomada_e_extracao_de_sessao_em_opencode_e_antigravity() {
    let program = Path::new("cli");
    let no_args = Vec::new();
    let opencode_resume = opencode::build_command_with_resume(
        Path::new("opencode"),
        &no_args,
        "role",
        "modelo",
        Access::TextOnly,
        "responder",
        Some("opencode-session-7"),
    )
    .unwrap();
    let opencode_args = args_of(&opencode_resume);
    assert_eq!(opencode_args[0], "run");
    assert_eq!(opencode_args[1], "--pure");
    assert_eq!(opencode_args[position(&opencode_args, "-s") + 1], "opencode-session-7");

    let antigravity_resume = antigravity::build_command_with_resume(
        program,
        &no_args,
        "role",
        "modelo",
        Access::TextOnly,
        "responder",
        Some("agy-conversation-4"),
    );
    let antigravity_args = args_of(&antigravity_resume);
    assert_eq!(
        antigravity_args[position(&antigravity_args, "--conversation") + 1],
        "agy-conversation-4"
    );

    let opencode_reply = opencode::parse_json_output(
        br#"{"type":"text","sessionID":"opencode-session-7","part":{"text":"Resposta"}}"#,
    )
    .unwrap();
    assert_eq!(opencode_reply.text, "Resposta");
    assert_eq!(opencode_reply.session_id.as_deref(), Some("opencode-session-7"));

    let antigravity_reply = antigravity::parse_stream_output(
        br#"{"event":"result","result":{"status":"SUCCESS","response":"Resposta","conversation_id":"agy-conversation-4"}}"#,
    )
    .unwrap();
    assert_eq!(antigravity_reply.text, "Resposta");
    assert_eq!(antigravity_reply.session_id.as_deref(), Some("agy-conversation-4"));
}

// ==========================================
// 5. LLM-1: Seed de providers.json, custom provider, modelo livre vs InvalidModel, model_tier via tiers
// ==========================================

#[test]
fn llm1_seed_de_providers_json_em_diretorio_vazio_cria_quatro_provedores_iniciais() {
    let dir = std::env::temp_dir().join(format!(
        "vokia-llm1-seed-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let providers_file = dir.join("providers.json");
    assert!(!providers_file.exists());

    let registry = crate::providers::ProviderRegistry::load(&dir);
    assert!(providers_file.exists(), "providers.json deve ser criado no seed");

    let list = registry.list().expect("carregar lista de provedores");
    assert_eq!(list.len(), 4, "deve semear exatamente 4 provedores");

    let ids: Vec<&str> = list.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"claude"));
    assert!(ids.contains(&"codex"));
    assert!(ids.contains(&"opencode"));
    assert!(ids.contains(&"antigravity"));

    for provider in &list {
        assert!(!provider.default_command.trim().is_empty());
        assert!(!provider.default_model.trim().is_empty());
        assert!(!provider.tiers.basic.trim().is_empty());
        assert!(!provider.tiers.intermediate.trim().is_empty());
        assert!(!provider.tiers.advanced.trim().is_empty());
    }

    let _ = std::fs::remove_dir_all(&dir);
}


#[test]
fn llm1_adicao_de_provedor_customizado_sem_recompilar_e_persistencia() {
    let dir = std::env::temp_dir().join(format!(
        "vokia-llm1-custom-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let registry = crate::providers::ProviderRegistry::load(&dir);
    assert_eq!(registry.list().unwrap().len(), 4);

    let custom = crate::providers::ProviderConfig {
        id: "meu-llm-custom".to_string(),
        label: "Meu LLM Customizado".to_string(),
        enabled: true,
        disabled_reason: None,
        protocol: crate::providers::ProviderProtocol::GenericCli,
        default_command: "meu-llm".to_string(),
        default_model: "modelo-base".to_string(),
        models: vec!["modelo-base".to_string(), "modelo-avancado".to_string()],
        free_models: vec![],
        tiers: crate::providers::ModelTiers {
            basic: "modelo-base".to_string(),
            intermediate: "modelo-base".to_string(),
            advanced: "modelo-avancado".to_string(),
        },
        supports_mcp: false,
        tool_transport: None,
        supports_sandbox: false,
        reserved_flags: vec!["--danger".to_string()],
        rate_limit_patterns: vec!["quota_error".to_string()],
        model_unavailable_patterns: vec!["no_access".to_string()],
        discovery: None,
        generic: Some(crate::providers::GenericCliSpec {
            prompt_args: vec!["-p".to_string(), "{prompt}".to_string()],
            prompt_via_stdin: false,
            model_flag: Some("--model".to_string()),
            dir_flag: None,
            json_output_args: vec![],
            text_json_paths: vec![],
            error_json_paths: vec![],
            model_json_path: None,
            input_tokens_json_path: None,
            output_tokens_json_path: None,
            cost_usd_json_path: None,
            sandbox_args: std::collections::BTreeMap::new(),
        }),
        model_grouping: crate::providers::ModelGroupingMode::Flat,
        effort: None,
        auth_probe: None,
        install_hint: None,
        connect_hint: None,
        badge_code: None,
        display_order: None,
        builtin_seed: false,
    };

    let saved = registry.save_provider(custom.clone()).expect("salvar custom provider");
    assert_eq!(saved.id, "meu-llm-custom");

    // Verifica que agora tem 5 provedores
    let list = registry.list().unwrap();
    assert_eq!(list.len(), 5);
    assert!(list.iter().any(|p| p.id == "meu-llm-custom"));

    // Recarrega em nova instância e confirma persistência em providers.json
    let reloaded = crate::providers::ProviderRegistry::load(&dir);
    let loaded_custom = reloaded.get("meu-llm-custom").expect("recuperar custom persistido");
    assert_eq!(loaded_custom.label, "Meu LLM Customizado");
    assert_eq!(loaded_custom.default_command, "meu-llm");

    let _ = std::fs::remove_dir_all(&dir);
}


#[test]
fn llm1_modelo_livre_valido_aceito_vs_invalid_model_em_vazio_ou_com_quebra() {
    let dir = std::env::temp_dir().join(format!(
        "vokia-llm1-model-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let providers = crate::providers::ProviderRegistry::load(&dir);
    let agents = crate::agents::AgentRegistry::load(&dir);

    // 1. Modelo livre arbitrário ("modelo-futuro-2026") é aceito mesmo fora dos presets conhecidos
    let draft_valido = crate::agents::AgentDraft {
        name: "LivreAgent".to_string(),
        role: "Testador de modelo livre".to_string(),
        avatar: "artesao".to_string(),
        cli: "claude".to_string(),
        model: "modelo-futuro-2026".to_string(),
        model_tier: None,
        reasoning_effort: None,
        follow_default_llm: false,
        fallback: vec![],
        command: "claude".to_string(),
    };
    let created = agents.create(&draft_valido, &providers).expect("modelo livre válido deve ser criado");
    assert_eq!(created.model, "modelo-futuro-2026");

    // 2. Modelo com quebra de linha gera InvalidModel
    let draft_quebra = crate::agents::AgentDraft {
        name: "QuebraAgent".to_string(),
        role: "Testador de modelo quebrado".to_string(),
        avatar: "pensador".to_string(),
        cli: "claude".to_string(),
        model: "modelo\ninvalido".to_string(),
        model_tier: None,
        reasoning_effort: None,
        follow_default_llm: false,
        fallback: vec![],
        command: "claude".to_string(),
    };
    match agents.create(&draft_quebra, &providers) {
        Err(crate::agents::AgentError::InvalidModel(m)) => assert!(m.contains('\n')),
        other => panic!("esperava InvalidModel para quebra de linha, obteve: {other:?}"),
    }

    // 3. Modelo vazio sem tier gera InvalidModel
    let draft_vazio = crate::agents::AgentDraft {
        name: "VazioAgent".to_string(),
        role: "Testador de modelo vazio".to_string(),
        avatar: "vigia".to_string(),
        cli: "claude".to_string(),
        model: "".to_string(),
        model_tier: None,
        reasoning_effort: None,
        follow_default_llm: false,
        fallback: vec![],
        command: "claude".to_string(),
    };
    match agents.create(&draft_vazio, &providers) {
        Err(crate::agents::AgentError::InvalidModel(_)) => {}
        other => panic!("esperava InvalidModel para modelo vazio sem tier, obteve: {other:?}"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}


#[test]
fn llm1_model_tier_basic_intermediate_advanced_resolvido_via_tiers() {
    let providers = crate::providers::ProviderRegistry::seeded();
    let claude = providers.get("claude").unwrap();
    let codex = providers.get("codex").unwrap();

    // Claude
    assert_eq!(
        crate::orchestrator::fallback::resolve_model_for_provider(
            &claude,
            Some(crate::providers::ModelTier::Basic),
            ""
        ),
        "haiku"
    );
    assert_eq!(
        crate::orchestrator::fallback::resolve_model_for_provider(
            &claude,
            Some(crate::providers::ModelTier::Intermediate),
            ""
        ),
        "sonnet"
    );
    assert_eq!(
        crate::orchestrator::fallback::resolve_model_for_provider(
            &claude,
            Some(crate::providers::ModelTier::Advanced),
            ""
        ),
        "opus"
    );

    // Codex
    assert_eq!(
        crate::orchestrator::fallback::resolve_model_for_provider(
            &codex,
            Some(crate::providers::ModelTier::Basic),
            ""
        ),
        "gpt-5.6-luna"
    );
    assert_eq!(
        crate::orchestrator::fallback::resolve_model_for_provider(
            &codex,
            Some(crate::providers::ModelTier::Intermediate),
            ""
        ),
        "gpt-5.6-terra"
    );
    assert_eq!(
        crate::orchestrator::fallback::resolve_model_for_provider(
            &codex,
            Some(crate::providers::ModelTier::Advanced),
            ""
        ),
        "gpt-6-luna"
    );

    // Modelo explícito sem tier resolve o modelo específico informado
    assert_eq!(
        crate::orchestrator::fallback::resolve_model_for_provider(
            &claude,
            None,
            "custom-override"
        ),
        "custom-override"
    );

    // Sem tier e sem modelo explícito resolve o default_model do provedor
    assert_eq!(
        crate::orchestrator::fallback::resolve_model_for_provider(
            &claude,
            None,
            ""
        ),
        "haiku"
    );
}

// ==========================================
// 6. LLM-2: Generic build_command sem shell, recusa Liberado sem sandbox, parse_output, classify_provider_error
// ==========================================

#[test]
fn llm2_generic_build_command_sem_shell_e_recusa_liberado_quando_sandbox_false() {
    let mut provider = crate::providers::initial_providers()[0].clone();
    provider.id = "custom-cli".to_string();
    provider.label = "Custom CLI".to_string();
    provider.supports_sandbox = false;
    provider.generic = Some(crate::providers::GenericCliSpec {
        prompt_args: vec!["-p".to_string(), "{prompt}".to_string()],
        prompt_via_stdin: false,
        model_flag: Some("--model".to_string()),
        dir_flag: Some("--cwd".to_string()),
        json_output_args: vec!["--json".to_string()],
        text_json_paths: vec!["text".to_string()],
        error_json_paths: vec![],
        model_json_path: None,
        input_tokens_json_path: None,
        output_tokens_json_path: None,
        cost_usd_json_path: None,
        sandbox_args: std::collections::BTreeMap::new(),
    });

    let program = Path::new("minha-cli-bin");
    let user_args = vec!["--extra-user-flag".to_string()];
    let role = "Analista";
    let model = "modelo-x";
    let message = "Processar dados";

    // 1. TextOnly: sem shell (programa é exatamente 'minha-cli-bin')
    let (cmd, stdin) = generic::build_command(
        program,
        &user_args,
        role,
        model,
        Access::TextOnly,
        message,
        &provider,
    )
    .expect("build_command para TextOnly deve funcionar");
    assert!(stdin.is_none());
    let args = args_of(&cmd);
    assert_eq!(cmd.as_std().get_program().to_string_lossy(), "minha-cli-bin");
    assert!(args.contains(&"--extra-user-flag".to_string()));
    assert!(args.contains(&"--json".to_string()));
    assert_eq!(args[position(&args, "--model") + 1], "modelo-x");
    assert_eq!(args[position(&args, "-p") + 1], "Analista\n\nProcessar dados");

    // 2. Project com Leitura: sucesso
    let (cmd_leitura, _) = generic::build_command(
        program,
        &user_args,
        role,
        model,
        Access::Project(project_access(PermissionLevel::Leitura)),
        message,
        &provider,
    )
    .expect("build_command para Leitura deve funcionar");
    let args_leitura = args_of(&cmd_leitura);
    assert!(args_leitura.contains(&"--cwd".to_string()));

    // 3. Project com Liberado mas supports_sandbox == false: recusa explícita
    let err_liberado = generic::build_command(
        program,
        &user_args,
        role,
        model,
        Access::Project(project_access(PermissionLevel::Liberado)),
        message,
        &provider,
    )
    .expect_err("Liberado sem supports_sandbox deve falhar");

    match err_liberado {
        ClaudeError::Reported(msg) => {
            assert!(
                msg.contains("não oferece sandbox equivalente para o nível Liberado"),
                "mensagem inesperada: {msg}"
            );
        }
        other => panic!("esperava Reported, obteve: {other:?}"),
    }
}

#[test]
fn llm2_generic_parse_output_texto_puro_e_jsonl_field_com_extracao() {
    let mut provider = crate::providers::initial_providers()[0].clone();

    // 1. Sem json paths configurados -> retorna texto bruto
    provider.generic = None;
    let reply_raw = generic::parse_output(b"  Resposta em texto puro direto   \n", &provider, "m").unwrap();
    assert_eq!(reply_raw.text, "Resposta em texto puro direto");

    // 2. Com text_json_paths e error_json_paths configurados
    let mut spec = crate::providers::GenericCliSpec::default();
    spec.text_json_paths = vec!["message.content".to_string(), "text".to_string()];
    spec.error_json_paths = vec!["error.message".to_string()];
    provider.generic = Some(spec);

    let jsonl_ok = b"{\"type\": \"start\"}\n{\"message\": {\"content\": \"Parte 1 do texto\"}}\n{\"text\": \"Parte 2 do texto\"}\n";
    let reply_json = generic::parse_output(jsonl_ok, &provider, "m").unwrap();
    assert_eq!(reply_json.text, "Parte 1 do texto\n\nParte 2 do texto");

    // 3. Linha com erro em error_json_paths -> classify_provider_error / Reported
    let jsonl_err = "{\"type\": \"error\", \"error\": {\"message\": \"Falha de autorizacao interna\"}}\n".as_bytes();
    let Err(ClaudeError::Reported(msg)) = generic::parse_output(jsonl_err, &provider, "m") else {
        panic!("esperava Reported");
    };
    assert_eq!(msg, "Falha de autorizacao interna");
}

#[test]
fn llm2_classify_provider_error_retorna_limit_exhausted_vs_model_unavailable() {
    let limit_patterns = vec!["rate_limit".to_string(), "usage_limit_reached".to_string(), "quota".to_string()];
    let unavail_patterns = vec!["model_not_found".to_string(), "does not have access to".to_string(), "not available".to_string()];

    // 1. ModelUnavailable (ex: conta sem acesso ao modelo neon)
    let msg_unavail = "User account does not have access to model gemini-4-neon-medium.";
    let err_unavail = crate::claude::classify_provider_error(
        "antigravity",
        "gemini-4-neon-medium",
        msg_unavail,
        &limit_patterns,
        &unavail_patterns,
    );
    match err_unavail {
        Some(ClaudeError::ModelUnavailable { provider, model, detail }) => {
            assert_eq!(provider, "antigravity");
            assert_eq!(model, "gemini-4-neon-medium");
            assert_eq!(detail, msg_unavail);
        }
        other => panic!("esperava ModelUnavailable, obteve: {other:?}"),
    }

    // 2. LimitExhausted (rate limit padrão)
    let msg_limit = "Error: usage_limit_reached for current period.";
    let err_limit = crate::claude::classify_provider_error(
        "claude",
        "haiku",
        msg_limit,
        &limit_patterns,
        &unavail_patterns,
    );
    assert!(matches!(err_limit, Some(ClaudeError::LimitExhausted)));

    // 3. Erro qualquer não classificado como limite nem indisponível
    let msg_other = "SyntaxError: Unexpected token in request.";
    let err_other = crate::claude::classify_provider_error(
        "claude",
        "haiku",
        msg_other,
        &limit_patterns,
        &unavail_patterns,
    );
    assert!(err_other.is_none());
}

// ==========================================
// 7. LLM-3: parse_models_lines, parse_codex_models_cache, inspect_cli, delete_provider("claude")
// ==========================================

#[test]
fn llm3_parse_models_lines_filtra_cabecalhos_e_extrai_modelos() {
    let sample = r#"
Available models for this CLI:
------------------------------
# OpenAI models:
openai/gpt-6-astra   (recommended, high speed)
openai/gpt-5.6-luna
=== Antigravity models ===
gemini-4-hydrogen-medium
gemini-4-neon-medium:
"#;
    let models = crate::providers::parse_models_lines(sample);
    assert_eq!(
        models,
        vec![
            "openai/gpt-6-astra".to_string(),
            "openai/gpt-5.6-luna".to_string(),
            "gemini-4-hydrogen-medium".to_string(),
        ]
    );
}

#[test]
fn llm3_parse_codex_models_cache_e_parse_json_models() {
    // 1. parse_codex_models_cache com campo models
    let cache_json = r#"{
        "models": [
            {"id": "gpt-6-luna", "name": "GPT-6 Luna"},
            {"slug": "gpt-5.6-terra"}
        ]
    }"#;
    let codex_models = crate::providers::parse_codex_models_cache(cache_json).unwrap();
    assert_eq!(codex_models, vec!["gpt-6-luna", "gpt-5.6-terra"]);

    // 2. parse_json_models com json_path aninhado
    let nested_json = r#"{
        "data": {
            "available": [
                {"id": "custom-1"},
                {"slug": "custom-2"}
            ]
        }
    }"#;
    let json_models = crate::providers::parse_json_models(nested_json, Some("data.available")).unwrap();
    assert_eq!(json_models, vec!["custom-1", "custom-2"]);
}

#[tokio::test]
async fn llm3_inspect_cli_sem_shell_cli_ausente_e_truncamento_8kb() {
    // 1. CLI inexistente retorna found: false sem erro nem panic
    let result = crate::commands::providers::inspect_cli("cli_totalmente_inexistente_xyz_12345".to_string())
        .await
        .expect("inspect_cli de comando inexistente deve retornar Ok com found: false");
    assert!(!result.found);
    assert!(result.program_path.is_none());
    assert!(result.help_text.is_empty());

    // 2. build_setup_prompt inclui ajuda truncada e schema JSON esperado sem shell
    let prompt = crate::commands::providers::build_setup_prompt(
        "MinhaCli".to_string(),
        "minha-cli".to_string(),
        "Texto de ajuda da CLI".to_string(),
    );
    assert!(prompt.contains("MinhaCli"));
    assert!(prompt.contains("minha-cli"));
    assert!(prompt.contains("generic-cli"));
    assert!(prompt.contains("Nunca use shell"));
}

#[test]
fn llm3_delete_provider_claude_recusado() {
    let dir = std::env::temp_dir().join(format!(
        "vokia-llm3-delete-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let app = crate::mcp::support::build_app(&dir);
    use tauri::Manager;

    let err = crate::commands::providers::delete_provider(
        app.state::<crate::providers::ProviderRegistry>(),
        app.state::<crate::agents::AgentRegistry>(),
        "claude".to_string(),
    )
    .expect_err("remoção do provedor claude deve ser recusada");

    assert!(
        err.contains("O provedor padrão 'claude' não pode ser removido"),
        "mensagem inesperada: {err}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}


// ==========================================
// 8. LLM-4: resolve_candidates prioridades, filtros Ember MCP, sandbox Liberado, CooldownTracker, expiração, update_ember_llm
// ==========================================

#[test]
fn llm4_resolve_candidates_prioridade_override_follow_default_agent_e_global_fallback() {
    let providers = crate::providers::ProviderRegistry::seeded();
    let state = crate::state::AppState::new(Ok(std::path::PathBuf::from("claude")));

    let agent = crate::agents::Agent::from_stored("axiom").unwrap();
    let profile = crate::agents::AgentProfile {
        id: agent.clone(),
        name: "Axiom".to_string(),
        role: "Líder".to_string(),
        avatar: "pensador".to_string(),
        cli: "claude".to_string(),
        model: "haiku".to_string(),
        model_tier: None,
        reasoning_effort: None,
        follow_default_llm: true,
        fallback: vec![crate::orchestrator::fallback::LlmTarget {
            cli: "codex".to_string(),
            model_tier: None,
            model: Some("gpt-6-luna".to_string()),
            reasoning_effort: None,
        }],
        command: "claude".to_string(),
        builtin: false,
        extra_instructions: String::new(),
    };

    let global_active = crate::orchestrator::fallback::LlmTarget {
        cli: "antigravity".to_string(),
        model_tier: None,
        model: Some("gemini-4-hydrogen-medium".to_string()),
        reasoning_effort: None,
    };
    let global_fallback = vec![crate::orchestrator::fallback::LlmTarget {
        cli: "opencode".to_string(),
        model_tier: None,
        model: Some("openai/gpt-6-astra".to_string()),
        reasoning_effort: None,
    }];

    // 1. Sem override temporário:
    // Ordem esperada: follow_default (antigravity) -> profile (claude) -> agent fallback (codex) -> global fallback (opencode)
    let candidates = crate::orchestrator::fallback::resolve_candidates(
        &agent,
        &profile,
        &providers,
        &state,
        Some(&global_active),
        &global_fallback,
        None,
    );
    assert_eq!(candidates.len(), 4);
    assert_eq!(candidates[0].cli, "antigravity");
    assert_eq!(candidates[1].cli, "claude");
    assert_eq!(candidates[2].cli, "codex");
    assert_eq!(candidates[3].cli, "opencode");

    // 2. Com override temporário:
    // Ele assume a primeira posição absoluta!
    let now = crate::orchestrator::fallback::now_millis_u64();
    let temp_override = crate::orchestrator::fallback::TemporaryOverride {
        agent: agent.clone(),
        cli: "codex".to_string(),
        model: "gpt-5.6-luna".to_string(),
        model_tier: None,
        reasoning_effort: None,
        command: "codex".to_string(),
        original_cli: "claude".to_string(),
        original_model: "haiku".to_string(),
        original_tier: None,
        original_reasoning_effort: None,
        reason: crate::orchestrator::fallback::OverrideReason::UsageLimit,
        applied_at: now,
        expires_at: Some(now + 60_000),
    };
    state.temporary_overrides.lock().unwrap().insert(agent.clone(), temp_override);

    let candidates_with_ov = crate::orchestrator::fallback::resolve_candidates(
        &agent,
        &profile,
        &providers,
        &state,
        Some(&global_active),
        &global_fallback,
        None,
    );
    assert_eq!(candidates_with_ov[0].cli, "codex");
    assert_eq!(candidates_with_ov[0].model, "gpt-5.6-luna");
}

#[test]
fn mdl1_modelo_especifico_em_active_llm_propaga_para_ember_e_lider_e_faz_fallback_em_cooldown() {
    let providers = crate::providers::ProviderRegistry::seeded();
    let state = crate::state::AppState::new(Ok(std::path::PathBuf::from("claude")));

    // Usuário escolheu um modelo específico no seletor da ActionBar (model_tier: None, model: Some(...))
    let global_active = crate::orchestrator::fallback::LlmTarget {
        cli: "opencode".to_string(),
        model_tier: None,
        model: Some("openai/gpt-5.4-mini".to_string()),
        reasoning_effort: None,
    };
    let global_fallback = vec![crate::orchestrator::fallback::LlmTarget {
        cli: "antigravity".to_string(),
        model_tier: None,
        model: Some("gemini-4-argon-medium".to_string()),
        reasoning_effort: None,
    }];

    // 1. Ember (follow_default_llm: true) recebe o modelo específico do active_llm como 1º candidato
    let ember = crate::agents::Agent::ember();
    let ember_profile = crate::agents::AgentProfile {
        id: ember.clone(),
        name: "Ember".to_string(),
        role: "Orquestrador geral".to_string(),
        avatar: "ember".to_string(),
        cli: "claude".to_string(),
        model: "haiku".to_string(),
        model_tier: None,
        reasoning_effort: None,
        follow_default_llm: true,
        fallback: Vec::new(),
        command: "claude".to_string(),
        builtin: true,
        extra_instructions: String::new(),
    };
    let ember_candidates = crate::orchestrator::fallback::resolve_candidates(
        &ember,
        &ember_profile,
        &providers,
        &state,
        Some(&global_active),
        &global_fallback,
        None,
    );
    assert_eq!(ember_candidates[0].cli, "opencode");
    assert_eq!(ember_candidates[0].model, "openai/gpt-5.4-mini");
    assert_eq!(
        ember_candidates[0].provider.effective_tool_transport(),
        crate::providers::ToolTransport::VokiaCli
    );
    assert_eq!(ember_candidates[1].cli, "claude");
    assert_eq!(ember_candidates[1].model, "haiku");
    assert_eq!(ember_candidates[2].cli, "antigravity");
    assert_eq!(ember_candidates[2].model, "gemini-4-argon-medium");

    // 2. Quando o modelo escolhido atinge limite/indisponibilidade e entra em cooldown,
    // a cadeia cai automaticamente para o próximo candidato sem tentar o modelo bloqueado.
    crate::orchestrator::fallback::mark_cooldown(&state, "opencode", "openai/gpt-5.4-mini");
    let after_limit = crate::orchestrator::fallback::resolve_candidates(
        &ember,
        &ember_profile,
        &providers,
        &state,
        Some(&global_active),
        &global_fallback,
        None,
    );
    assert_eq!(after_limit[0].cli, "claude");
    assert_eq!(after_limit[0].model, "haiku");
    assert_eq!(after_limit[1].cli, "antigravity");
    assert_eq!(after_limit[1].model, "gemini-4-argon-medium");
}

#[test]
fn llm4_resolve_candidates_filtra_ember_mcp_sandbox_liberado_e_cooldown_tracker() {
    let providers = crate::providers::ProviderRegistry::seeded();
    let state = crate::state::AppState::new(Ok(std::path::PathBuf::from("claude")));

    // 1. Filtro MCP do Ember:
    // Ember só pode usar provedores com supports_mcp == true (ex: claude)
    let ember = crate::agents::Agent::ember();
    let ember_profile = crate::agents::AgentProfile {
        id: ember.clone(),
        name: "Ember".to_string(),
        role: "Assistente".to_string(),
        avatar: "artesao".to_string(),
        cli: "claude".to_string(),
        model: "haiku".to_string(),
        model_tier: None,
        reasoning_effort: None,
        follow_default_llm: false,
        fallback: vec![
            crate::orchestrator::fallback::LlmTarget {
                cli: "opencode".to_string(), // supports_mcp: false
                model_tier: None,
                model: Some("openai/gpt-6-astra".to_string()),
                reasoning_effort: None,
            },
            crate::orchestrator::fallback::LlmTarget {
                cli: "claude".to_string(), // supports_mcp: true
                model_tier: None,
                model: Some("sonnet".to_string()),
                reasoning_effort: None,
            },
        ],
        command: "claude".to_string(),
        builtin: true,
        extra_instructions: String::new(),
    };

    let ember_candidates = crate::orchestrator::fallback::resolve_candidates(
        &ember,
        &ember_profile,
        &providers,
        &state,
        None,
        &[],
        None,
    );
    // OpenCode tem VokiaCli (has_tools == true), então é aceito para Ember conforme P1!
    assert!(ember_candidates.iter().all(|c| c.provider.has_tools()));
    assert_eq!(ember_candidates.len(), 3); // haiku, opencode e sonnet

    // 2. Filtro Sandbox no nível Liberado:
    // OpenCode tem supports_sandbox: false, então deve ser filtrado no Liberado
    let axiom = crate::agents::Agent::from_stored("axiom").unwrap();
    let axiom_profile = crate::agents::AgentProfile {
        id: axiom.clone(),
        name: "Axiom".to_string(),
        role: "Líder".to_string(),
        avatar: "pensador".to_string(),
        cli: "opencode".to_string(),
        model: "openai/gpt-6-astra".to_string(),
        model_tier: None,
        reasoning_effort: None,
        follow_default_llm: false,
        fallback: vec![crate::orchestrator::fallback::LlmTarget {
            cli: "claude".to_string(),
            model_tier: None,
            model: Some("haiku".to_string()),
            reasoning_effort: None,
        }],
        command: "opencode".to_string(),
        builtin: false,
        extra_instructions: String::new(),
    };

    let liberado_candidates = crate::orchestrator::fallback::resolve_candidates(
        &axiom,
        &axiom_profile,
        &providers,
        &state,
        None,
        &[],
        Some(PermissionLevel::Liberado),
    );
    // opencode descartado pois supports_sandbox == false; resta claude
    assert_eq!(liberado_candidates.len(), 1);
    assert_eq!(liberado_candidates[0].cli, "claude");

    // 3. CooldownTracker:
    // Se marcamos cooldown para (claude, haiku), ele é ignorado
    crate::orchestrator::fallback::mark_cooldown(&state, "claude", "haiku");
    let after_cooldown = crate::orchestrator::fallback::resolve_candidates(
        &axiom,
        &axiom_profile,
        &providers,
        &state,
        None,
        &[],
        Some(PermissionLevel::Liberado),
    );
    assert!(after_cooldown.is_empty(), "candidato em cooldown deve ser filtrado");
}

#[test]
fn llm4_expiracao_automatica_vs_clear_temporary_override() {
    let providers = crate::providers::ProviderRegistry::seeded();
    let state = crate::state::AppState::new(Ok(std::path::PathBuf::from("claude")));
    let agent = crate::agents::Agent::from_stored("axiom").unwrap();
    let profile = crate::agents::AgentProfile {
        id: agent.clone(),
        name: "Axiom".to_string(),
        role: "Líder".to_string(),
        avatar: "pensador".to_string(),
        cli: "claude".to_string(),
        model: "haiku".to_string(),
        model_tier: None,
        reasoning_effort: None,
        follow_default_llm: false,
        fallback: vec![],
        command: "claude".to_string(),
        builtin: false,
        extra_instructions: String::new(),
    };

    // 1. Override expirado: expires_at no passado
    let now = crate::orchestrator::fallback::now_millis_u64();
    let expired_ov = crate::orchestrator::fallback::TemporaryOverride {
        agent: agent.clone(),
        cli: "codex".to_string(),
        model: "gpt-6-luna".to_string(),
        model_tier: None,
        reasoning_effort: None,
        command: "codex".to_string(),
        original_cli: "claude".to_string(),
        original_model: "haiku".to_string(),
        original_tier: None,
        original_reasoning_effort: None,
        reason: crate::orchestrator::fallback::OverrideReason::UsageLimit,
        applied_at: now - 10_000,
        expires_at: Some(now - 1_000), // Expirado há 1s
    };
    state.temporary_overrides.lock().unwrap().insert(agent.clone(), expired_ov);

    let candidates = crate::orchestrator::fallback::resolve_candidates(
        &agent,
        &profile,
        &providers,
        &state,
        None,
        &[],
        None,
    );
    // Automaticamente remove o expirado e retorna claude
    assert_eq!(candidates[0].cli, "claude");
    assert!(
        state.temporary_overrides.lock().unwrap().get(&agent).is_none(),
        "override expirado deve ter sido expurgado da memória"
    );

    // 2. Override ativo: não expira e é retornado
    let active_ov = crate::orchestrator::fallback::TemporaryOverride {
        agent: agent.clone(),
        cli: "codex".to_string(),
        model: "gpt-6-luna".to_string(),
        model_tier: None,
        reasoning_effort: None,
        command: "codex".to_string(),
        original_cli: "claude".to_string(),
        original_model: "haiku".to_string(),
        original_tier: None,
        original_reasoning_effort: None,
        reason: crate::orchestrator::fallback::OverrideReason::ManualTemp,
        applied_at: now,
        expires_at: Some(now + 100_000),
    };
    state.temporary_overrides.lock().unwrap().insert(agent.clone(), active_ov);

    let candidates_active = crate::orchestrator::fallback::resolve_candidates(
        &agent,
        &profile,
        &providers,
        &state,
        None,
        &[],
        None,
    );
    assert_eq!(candidates_active[0].cli, "codex");

    // Limpeza explícita (clear_temporary_override)
    state.temporary_overrides.lock().unwrap().remove(&agent);
    let candidates_cleared = crate::orchestrator::fallback::resolve_candidates(
        &agent,
        &profile,
        &providers,
        &state,
        None,
        &[],
        None,
    );
    assert_eq!(candidates_cleared[0].cli, "claude");
}

#[test]
fn llm4_update_ember_llm_aceita_mcp_e_recusa_sem_mcp() {
    let dir = std::env::temp_dir().join(format!(
        "vokia-llm4-ember-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let providers = crate::providers::ProviderRegistry::load(&dir);
    let agents = crate::agents::AgentRegistry::load(&dir);

    // 1. Provedor com MCP (claude): aceito com sucesso
    let updated = agents
        .update_ember_llm(
            Some("claude".to_string()),
            None,
            Some("opus".to_string()),
            Some("high".to_string()),
            vec![],
            &providers,
        )
        .expect("update_ember_llm com Claude deve ter sucesso");
    assert_eq!(updated.model, "opus");
    assert_eq!(updated.reasoning_effort.as_deref(), Some("high"));

    // 2. Provedor sem ferramentas (ToolTransport::None): recusa explícita
    let no_tools = crate::providers::ProviderConfig {
        id: "sem-tools".to_string(),
        label: "Sem Tools".to_string(),
        enabled: true,
        disabled_reason: None,
        protocol: crate::providers::ProviderProtocol::GenericCli,
        default_command: "sem-tools".to_string(),
        default_model: "base".to_string(),
        models: vec!["base".to_string()],
        free_models: vec![],
        tiers: crate::providers::ModelTiers {
            basic: "base".to_string(),
            intermediate: "base".to_string(),
            advanced: "base".to_string(),
        },
        supports_mcp: false,
        tool_transport: Some(crate::providers::ToolTransport::None),
        supports_sandbox: false,
        reserved_flags: vec![],
        rate_limit_patterns: vec![],
        model_unavailable_patterns: vec![],
        discovery: None,
        generic: None,
        model_grouping: crate::providers::ModelGroupingMode::Flat,
        effort: None,
        auth_probe: None,
        install_hint: None,
        connect_hint: None,
        badge_code: None,
        display_order: None,
        builtin_seed: false,
    };
    providers.save_provider(no_tools).expect("salvar provedor sem tools");
    let err = agents
        .update_ember_llm(
            Some("sem-tools".to_string()),
            None,
            Some("base".to_string()),
            None,
            vec![],
            &providers,
        )
        .expect_err("update_ember_llm com ToolTransport::None deve falhar");

    match err {
        crate::agents::AgentError::SaveFailed(msg) => {
            assert!(
                msg.contains("O agente Ember exige um provedor com suporte a ferramentas"),
                "mensagem inesperada: {msg}"
            );
        }
        other => panic!("esperava SaveFailed com recusa de ferramentas, obteve: {other:?}"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn req085_declarative_effort_mapping_and_suffix_resolution_across_providers() {
    let providers = crate::providers::ProviderRegistry::seeded();

    // 1. Flag mode (Claude): sonnet/opus aceitam max, haiku restringe a low/medium/high
    let claude = providers.get("claude").unwrap();
    assert_eq!(
        claude.supported_efforts_for_model("opus"),
        vec!["low", "medium", "high", "max"]
    );
    assert_eq!(
        claude.supported_efforts_for_model("haiku"),
        vec!["low", "medium", "high"]
    );
    assert_eq!(
        claude.resolve_effort_args("opus", Some("high")),
        vec!["--effort", "high"]
    );
    assert!(claude.resolve_effort_args("haiku", Some("max")).is_empty());

    // 2. Flag mode com value_map (Codex): max -> xhigh sem hardcoding no adapter
    let codex = providers.get("codex").unwrap();
    assert_eq!(
        codex.resolve_effort_args("gpt-6-luna", Some("max")),
        vec!["-c", "model_reasoning_effort=\"xhigh\""]
    );
    assert_eq!(
        codex.resolve_effort_args("gpt-6-luna", Some("medium")),
        vec!["-c", "model_reasoning_effort=\"medium\""]
    );

    // 3. ModelSuffix mode (Antigravity/Gemini): substitui sufixo de forma idempotente
    let gemini = providers.get("antigravity").unwrap();
    assert_eq!(
        gemini.resolve_model_with_effort("gemini-4-hydrogen-medium", Some("high")),
        "gemini-4-hydrogen-high"
    );
    assert_eq!(
        gemini.resolve_model_with_effort("gemini-4-hydrogen", Some("low")),
        "gemini-4-hydrogen-low"
    );
    assert!(gemini.resolve_effort_args("gemini-4-hydrogen-high", Some("high")).is_empty());

    // 4. OpenCode: variantes descobertas com '/' colapsam e resolvem sufixo apenas quando presentes
    let mut opencode = providers.get("opencode").unwrap();
    opencode.models = vec![
        "openai/gpt-6-astra/low".to_string(),
        "openai/gpt-6-astra/high".to_string(),
        "opencode/big-pickle:free".to_string(),
    ];
    assert_eq!(
        opencode.supported_efforts_for_model("openai/gpt-6-astra/low"),
        vec!["low", "high"]
    );
    assert_eq!(
        opencode.resolve_model_with_effort("openai/gpt-6-astra/low", Some("high")),
        "openai/gpt-6-astra/high"
    );
    assert!(opencode.supported_efforts_for_model("opencode/big-pickle:free").is_empty());
    assert_eq!(
        opencode.resolve_model_with_effort("opencode/big-pickle:free", Some("high")),
        "opencode/big-pickle:free"
    );
}

#[test]
fn req085_reasoning_effort_persists_in_agent_profile_and_llm_target() {
    let dir = std::env::temp_dir().join(format!(
        "vokia-req085-effort-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let providers = crate::providers::ProviderRegistry::load(&dir);
    let agents = crate::agents::AgentRegistry::load(&dir);

    let draft = crate::agents::AgentDraft {
        name: "Planner".to_string(),
        role: "Tech Lead".to_string(),
        avatar: "onix".to_string(),
        cli: "codex".to_string(),
        model: "gpt-6-luna".to_string(),
        model_tier: None,
        reasoning_effort: Some("MAX".to_string()),
        follow_default_llm: false,
        fallback: vec![crate::orchestrator::fallback::LlmTarget {
            cli: "antigravity".to_string(),
            model_tier: None,
            model: Some("gemini-4-hydrogen-medium".to_string()),
            reasoning_effort: Some("high".to_string()),
        }],
        command: "codex".to_string(),
    };
    let created = agents.create(&draft, &providers).expect("criar agente com effort");
    assert_eq!(created.reasoning_effort.as_deref(), Some("max"));

    // Recarrega do disco e verifica persistência JSON em camelCase
    let reloaded = crate::agents::AgentRegistry::load(&dir);
    let team = reloaded.team().unwrap();
    let stored = team.profile(&created.id).expect("agente recarregado");
    assert_eq!(stored.reasoning_effort.as_deref(), Some("max"));
    assert_eq!(
        stored.fallback[0].reasoning_effort.as_deref(),
        Some("high")
    );

    // LlmTarget serializa reasoningEffort em camelCase e desserializa JSON legado sem o campo
    let serialized = serde_json::to_string(&stored.fallback[0]).unwrap();
    assert!(serialized.contains("\"reasoningEffort\":\"high\""));
    let legacy: crate::orchestrator::fallback::LlmTarget =
        serde_json::from_str(r#"{"cli":"codex","model":"gpt-6-luna"}"#).unwrap();
    assert_eq!(legacy.reasoning_effort, None);

    // resolve_candidates propaga reasoning_effort e resolve sufixo no fallback do Gemini
    let state = crate::state::AppState::new(Ok(std::path::PathBuf::from("codex")));
    let candidates = crate::orchestrator::fallback::resolve_candidates(
        &stored.id,
        stored,
        &providers,
        &state,
        None,
        &[],
        None,
    );
    assert_eq!(candidates[0].cli, "codex");
    assert_eq!(candidates[0].reasoning_effort.as_deref(), Some("max"));
    assert_eq!(candidates[1].cli, "antigravity");
    assert_eq!(candidates[1].model, "gemini-4-hydrogen-high");
    assert_eq!(candidates[1].reasoning_effort.as_deref(), Some("high"));

    // Esforço inválido com espaço/caractere proibido é rejeitado
    let bad_draft = crate::agents::AgentDraft {
        name: "BadEffort".to_string(),
        avatar: "ambar".to_string(),
        reasoning_effort: Some("high; rm -rf".to_string()),
        ..draft
    };
    assert!(matches!(
        reloaded.create(&bad_draft, &providers),
        Err(crate::agents::AgentError::InvalidReasoningEffort(_))
    ));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn req085_evaluate_provider_status_abstract_probe() {
    let missing = crate::providers::ProviderConfig {
        id: "inexistente-cli".to_string(),
        label: "Inexistente".to_string(),
        enabled: true,
        disabled_reason: None,
        protocol: crate::providers::ProviderProtocol::GenericCli,
        default_command: "vokia-binary-definitely-missing-987654".to_string(),
        default_model: "m1".to_string(),
        models: vec!["m1".to_string()],
        free_models: vec![],
        tiers: crate::providers::ModelTiers {
            basic: "m1".to_string(),
            intermediate: "m1".to_string(),
            advanced: "m1".to_string(),
        },
        supports_mcp: false,
        tool_transport: Some(crate::providers::ToolTransport::VokiaCli),
        supports_sandbox: false,
        reserved_flags: vec![],
        rate_limit_patterns: vec![],
        model_unavailable_patterns: vec![],
        discovery: None,
        generic: None,
        model_grouping: crate::providers::ModelGroupingMode::Flat,
        effort: None,
        auth_probe: None,
        install_hint: Some("install custom".to_string()),
        connect_hint: None,
        badge_code: Some("IN".to_string()),
        display_order: None,
        builtin_seed: false,
    };
    let (installed, status, _) = crate::commands::providers::evaluate_provider_status(&missing);
    assert!(!installed);
    assert_eq!(
        status,
        crate::commands::providers::ProviderConnectionStatus::Missing
    );
}

#[test]
fn billing_or_auth_error_disables_provider_and_excludes_from_candidates() {
    let dir = std::env::temp_dir().join(format!(
        "vokia-provider-disable-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    // 1. Classifica erro de assinatura vencida / pagamento pendente sem tratá-lo como falha de CLI
    let billing_msg = "Claude Code can't use your Claude subscription, usually because a payment is past due · Pay or renew at https://claude.ai/settings/billing";
    let classified = crate::claude::classify_provider_error(
        "claude",
        "sonnet",
        billing_msg,
        &[],
        &[],
    )
    .expect("deve classificar erro de billing/assinatura");
    assert!(classified.is_auth_or_billing());
    let flow_err = crate::orchestrator::error::FlowError::Claude(classified);
    assert!(flow_err.is_provider_disabled());
    assert!(!flow_err.is_cli_failure());

    // 2. Desabilitar o provedor o exclui de resolve_candidates para qualquer agente
    let providers = crate::providers::ProviderRegistry::load(&dir);
    let agents = crate::agents::AgentRegistry::load(&dir);
    let team = agents.team().unwrap();
    let app_state = crate::state::AppState::new(Ok(std::path::PathBuf::from("claude")));
    let agent = crate::agents::Agent::ember();
    let profile = team.profile(&agent).unwrap().clone();

    let before = crate::orchestrator::fallback::resolve_candidates(
        &agent,
        &profile,
        &providers,
        &app_state,
        None,
        &[crate::orchestrator::fallback::LlmTarget {
            cli: "codex".to_string(),
            model_tier: Some(crate::providers::ModelTier::Intermediate),
            model: None,
            reasoning_effort: None,
        }],
        None,
    );
    assert!(before.iter().any(|c| c.cli == profile.cli));

    let disabled = providers
        .set_enabled(&profile.cli, false, Some("Assinatura vencida ou pagamento pendente."))
        .expect("desabilitar provedor");
    assert!(!disabled.enabled);
    assert!(disabled.disabled_reason.is_some());

    let after = crate::orchestrator::fallback::resolve_candidates(
        &agent,
        &profile,
        &providers,
        &app_state,
        None,
        &[crate::orchestrator::fallback::LlmTarget {
            cli: "codex".to_string(),
            model_tier: Some(crate::providers::ModelTier::Intermediate),
            model: None,
            reasoning_effort: None,
        }],
        None,
    );
    assert!(after.iter().all(|c| c.cli != profile.cli));

    // 3. Reativar limpa o motivo e volta a permitir uso
    let reenabled = providers
        .set_enabled(&profile.cli, true, None)
        .expect("reativar provedor");
    assert!(reenabled.enabled);
    assert!(reenabled.disabled_reason.is_none());

    let _ = std::fs::remove_dir_all(&dir);
}


