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
