use std::path::Path;

use crate::claude::{Access, ClaudeError, ProjectAccess};
use crate::permissions::PermissionLevel;

use super::*;

fn project_access(level: PermissionLevel) -> ProjectAccess<'static> {
    ProjectAccess {
        dir: Path::new("C:/test/project"),
        attachments: Path::new("C:/test/attachments"),
        level,
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
    assert_eq!(ProviderAdapter::from_cli("claude").unwrap(), ProviderAdapter::Claude);
    assert_eq!(ProviderAdapter::from_cli("codex").unwrap(), ProviderAdapter::Codex);
    assert_eq!(ProviderAdapter::from_cli("opencode").unwrap(), ProviderAdapter::OpenCode);
    assert_eq!(ProviderAdapter::from_cli("antigravity").unwrap(), ProviderAdapter::Antigravity);

    let err = ProviderAdapter::from_cli("desconhecido").unwrap_err();
    assert!(
        err.to_string().contains("Provedor de CLI desconhecido"),
        "mensagem inesperada: {err}"
    );

    assert!(ProviderAdapter::Claude.supports_mcp());
    assert!(!ProviderAdapter::Codex.supports_mcp());
    assert!(!ProviderAdapter::OpenCode.supports_mcp());
    assert!(!ProviderAdapter::Antigravity.supports_mcp());

    assert!(ProviderAdapter::Claude.supports_live_session());
    assert!(!ProviderAdapter::Codex.supports_live_session());
    assert!(!ProviderAdapter::OpenCode.supports_live_session());
    assert!(!ProviderAdapter::Antigravity.supports_live_session());
}

#[test]
fn run_cli_process_com_exit_code_diferente_de_zero_retorna_claude_error_exit() {
    tauri::async_runtime::block_on(async {
        let mut cmd = tokio::process::Command::new("cmd.exe");
        cmd.args(["/C", "echo falha no processo 1>&2 & exit 42"]);
        let pid = std::sync::atomic::AtomicU32::new(0);
        let res = run_cli_process(cmd, Access::TextOnly, &pid, None).await;
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

// ==========================================
// 2. CMD-6: Codex Adapter
// ==========================================

#[test]
fn codex_build_command_text_only_e_project_levels_com_user_args_antes_das_flags() {
    let program = Path::new("codex");
    let user_args = vec!["--user-flag".to_string(), "custom_val".to_string()];
    let model = "gpt-6-luna";

    // TextOnly
    let cmd_text = codex::build_command(program, &user_args, model, Access::TextOnly);
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
    );
    let args_edicao = args_of(&cmd_edicao);
    assert_eq!(args_edicao[position(&args_edicao, "-s") + 1], "workspace-write");

    // Project: Liberado
    let cmd_liberado = codex::build_command(
        program,
        &user_args,
        model,
        Access::Project(project_access(PermissionLevel::Liberado)),
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
    assert_eq!(args_text[2], "--format");
    assert_eq!(args_text[3], "json");
    assert_eq!(args_text[4], "-m");
    assert_eq!(args_text[5], "openai/gpt-6-astra");
    assert_eq!(args_text[6], "--");
    assert_eq!(args_text[7], "Backend Lead\n\nanalise o código");

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
    let pos_dashdash = position(&args_leitura, "--");
    assert_eq!(pos_dashdash, args_leitura.len() - 2);
    assert_eq!(args_leitura[pos_dashdash + 1], "Backend Lead\n\nanalise o código");

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
    assert_eq!(args_text[1], "-p");
    assert_eq!(args_text[2], "Tech Lead\n\ncrie o plano");
    assert_eq!(args_text[3], "--output-format");
    assert_eq!(args_text[4], "stream-json");
    assert_eq!(args_text[5], "--model");
    assert_eq!(args_text[6], "gemini-4-hydrogen-medium");
    assert_eq!(args_text[7], "--disable-slash-commands");
    assert_eq!(args_text[8], "--sandbox");
    assert_eq!(args_text[9], "--mode");
    assert_eq!(args_text[10], "plan");

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
