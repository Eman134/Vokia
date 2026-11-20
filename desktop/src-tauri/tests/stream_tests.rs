use super::*;

const SYSTEM_LINE: &str = r#"{"type":"system","subtype":"init","session_id":"sess-1"}"#;
const RESULT_LINE: &str = r#"{"type":"result","subtype":"success","is_error":false,"result":"ok","session_id":"sess-1"}"#;
const RATE_LIMIT_LINE: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","rateLimitType":"five_hour","unifiedWindows":{"five_hour":{"utilization":0.33,"resetsAt":1793083800,"status":"allowed"},"seven_day":{"utilization":0.52,"resetsAt":1793426400,"status":"allowed"}}},"uuid":"u-1","session_id":"sess-1"}"#;
const RATE_LIMIT_LATER_LINE: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","rateLimitType":"five_hour","unifiedWindows":{"five_hour":{"utilization":0.91,"resetsAt":1793083800,"status":"allowed"},"seven_day":{"utilization":0.55,"resetsAt":1793426400,"status":"allowed"}}},"uuid":"u-2","session_id":"sess-1"}"#;
const RATE_LIMIT_EXPIRED_LINE: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","rateLimitType":"five_hour","unifiedWindows":{"five_hour":{"utilization":0.91,"resetsAt":1700000000,"status":"allowed"},"seven_day":{"utilization":0.55,"resetsAt":1793426400,"status":"allowed"}}},"uuid":"u-3","session_id":"sess-1"}"#;
const RATE_LIMIT_MALFORMED_LINE: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"unifiedWindows":{"five_hour":{"utilization":"alto","resetsAt":1793083800},"seven_day":{"utilization":0.52,"resetsAt":1793426400}}}}"#;
const RATE_LIMIT_TRUNCATED_LINE: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"unifiedWindows":{"five_hour":{"utilization":0.33"#;
const NOT_JSON_LINE: &str = "isto não é json";

fn stream(lines: &[&str]) -> Vec<u8> {
    lines.join("\n").into_bytes()
}

#[test]
fn linha_de_limite_valida_vira_janelas_de_uso_e_reset() {
    let stdout = stream(&[SYSTEM_LINE, RATE_LIMIT_LINE, RESULT_LINE]);

    let (response, limits) = parse_stream(&stdout).unwrap();
    let limits = limits.expect("a linha rate_limit_event deveria virar limites");

    assert_eq!(response.result, "ok");
    assert_eq!(limits.five_hour.utilization, 0.33);
    assert_eq!(limits.five_hour.resets_at, 1793083800);
    assert_eq!(limits.seven_day.utilization, 0.52);
    assert_eq!(limits.seven_day.resets_at, 1793426400);
}

#[test]
fn ultima_linha_de_limite_valida_prevalece() {
    let stdout = stream(&[RATE_LIMIT_LINE, RATE_LIMIT_LATER_LINE, RESULT_LINE]);

    let (_, limits) = parse_stream(&stdout).unwrap();

    assert_eq!(limits.unwrap().five_hour.utilization, 0.91);
}

#[test]
fn linha_de_limite_malformada_mantem_o_ultimo_valor_valido() {
    let stdout = stream(&[RATE_LIMIT_LINE, RATE_LIMIT_MALFORMED_LINE, RESULT_LINE]);

    let (response, limits) = parse_stream(&stdout).unwrap();

    assert_eq!(response.result, "ok");
    assert_eq!(limits.unwrap().five_hour.utilization, 0.33);
}

#[test]
fn linha_de_limite_malformada_sem_valor_anterior_devolve_sem_limites() {
    let stdout = stream(&[RATE_LIMIT_MALFORMED_LINE, RESULT_LINE]);

    let (response, limits) = parse_stream(&stdout).unwrap();

    assert_eq!(response.result, "ok");
    assert!(limits.is_none());
}

#[test]
fn linha_de_limite_truncada_falha_como_json_invalido() {
    let stdout = stream(&[RATE_LIMIT_TRUNCATED_LINE, RESULT_LINE]);

    assert!(matches!(
        parse_stream(&stdout),
        Err(ClaudeError::InvalidJson(_))
    ));
}

#[test]
fn linha_que_nao_e_json_continua_falhando_como_antes() {
    let stdout = stream(&[NOT_JSON_LINE, RESULT_LINE]);

    assert!(matches!(
        parse_stream(&stdout),
        Err(ClaudeError::InvalidJson(_))
    ));
}

#[test]
fn sem_linha_de_limite_devolve_resposta_sem_limites() {
    let stdout = stream(&[SYSTEM_LINE, RESULT_LINE]);

    let (response, limits) = parse_stream(&stdout).unwrap();

    assert_eq!(response.result, "ok");
    assert!(limits.is_none());
}

#[test]
fn reset_ja_passado_chega_sem_alteracao() {
    // A expiração é decidida no front (useUsage.visible); o parser só repassa o dado.
    let stdout = stream(&[RATE_LIMIT_EXPIRED_LINE, RESULT_LINE]);

    let (_, limits) = parse_stream(&stdout).unwrap();

    assert_eq!(limits.unwrap().five_hour.resets_at, 1700000000);
}

#[test]
fn quebras_de_linha_crlf_nao_quebram_a_leitura() {
    let stdout = format!("{RATE_LIMIT_LINE}\r\n{RESULT_LINE}\r\n").into_bytes();

    let (response, limits) = parse_stream(&stdout).unwrap();

    assert_eq!(response.result, "ok");
    assert_eq!(limits.unwrap().seven_day.utilization, 0.52);
}

#[test]
fn stream_sem_linha_result_falha_com_missing_result() {
    let stdout = stream(&[RATE_LIMIT_LINE]);

    assert!(matches!(
        parse_stream(&stdout),
        Err(ClaudeError::MissingResult)
    ));
}

const REAL_RATE_LIMIT_LINE: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1795240800,"rateLimitType":"seven_day","overageStatus":"rejected","overageDisabledReason":"org_level_disabled","isUsingOverage":false,"unifiedWindows":{"five_hour":{"utilization":0,"resetsAt":1795225200,"status":"allowed"},"seven_day":{"utilization":1,"resetsAt":1795240800,"status":"rejected","surpassedThreshold":1}}},"uuid":"fbaa6668-b4e5-42f1-b4d0-8b931036682e","session_id":"00e7c1b9-3a1b-4a7e-80b1-a65d975f3d43"}"#;
const REAL_ASSISTANT_LINE: &str = r#"{"type":"assistant","message":{"diagnostics":null,"id":"12d3bbbb-ba17-4abc-8af7-a8e0e21fd69e","container":null,"model":"<synthetic>","role":"assistant","stop_details":null,"stop_reason":"stop_sequence","stop_sequence":"","type":"message","usage":{"output_tokens_details":null,"input_tokens":0,"output_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0},"content":[{"type":"text","text":"You've hit your weekly limit · resets 3am (America/Sao_Paulo)"}]},"error":"rate_limit","is_api_error_message":true,"api_error_status":429,"api_error":"usage_limit_reached"}"#;
const REAL_RESULT_LINE: &str = r#"{"duration_api_ms":0,"stop_reason":"stop_sequence","session_id":"00e7c1b9-3a1b-4a7e-80b1-a65d975f3d43","total_cost_usd":0,"is_error":true,"num_turns":1,"subtype":"success","api_error_status":429,"api_error_type":"rate_limit_error","api_error_origin":"first_party","api_error":"usage_limit_reached","result":"You've hit your weekly limit · resets 3am (America/Sao_Paulo)","type":"result"}"#;

#[test]
fn saida_real_da_cli_claude_com_limite_semanal_esgotado_vira_usage_limit_com_resets_at_e_is_limit() {
    let stdout = stream(&[SYSTEM_LINE, REAL_RATE_LIMIT_LINE, REAL_ASSISTANT_LINE, REAL_RESULT_LINE]);
    let (response, limits) = parse_stream(&stdout).expect("parse_stream deve processar a saida real");
    let Err(err) = into_reply(response, limits) else {
        panic!("deveria retornar erro de limite de uso");
    };

    assert!(err.is_limit(), "err.is_limit() deve ser true");
    assert_eq!(err.to_string(), "Limite esgotado");

    let usage_limit = err.usage_limit().expect("deve extrair UsageLimit estruturado");
    assert_eq!(usage_limit.window, LimitName::SevenDay);
    assert_eq!(usage_limit.resets_at, 1795240800);
}

#[test]
fn live_session_com_cli_real_claude_retorna_usage_limit() {
    let Ok(claude_path) = which::which("claude") else {
        return;
    };
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime tokio");
    rt.block_on(async {
        let mut session = crate::claude::LiveSession::open(
            &claude_path,
            &[],
            "",
            "haiku",
            crate::claude::Access::TextOnly,
            None,
            None,
        )
        .expect("LiveSession deve abrir com a CLI real");

        let res = session
            .send(
                "Responda OK",
                |_| async { crate::claude::PermissionVerdict::Deny(None) },
                |_| {},
                |_| {},
            )
            .await;

        let Err(err) = res else {
            panic!("deveria falhar por limite de uso");
        };

        assert!(err.is_limit(), "is_limit() deve ser true na saida real");
        assert_eq!(err.to_string(), "Limite esgotado");
        let limit = err.usage_limit().expect("deve conter UsageLimit estruturado");
        assert_eq!(limit.window, LimitName::SevenDay);
        assert_eq!(limit.resets_at, 1795240800);
    });
}
