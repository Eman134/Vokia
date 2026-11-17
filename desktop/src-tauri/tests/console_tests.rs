use serde_json::json;

use super::*;

fn parse(lines: &[serde_json::Value]) -> Vec<(ConsoleKind, String)> {
    let mut parser = ConsoleParser::default();
    lines
        .iter()
        .flat_map(|line| parser.lines(line.to_string().as_bytes()))
        .map(|line| (line.kind, line.text))
        .collect()
}

fn assistant(blocks: serde_json::Value) -> serde_json::Value {
    json!({ "type": "assistant", "message": { "content": blocks } })
}

fn user(blocks: serde_json::Value) -> serde_json::Value {
    json!({ "type": "user", "message": { "content": blocks } })
}

fn tool_use(id: &str, name: &str, input: serde_json::Value) -> serde_json::Value {
    json!({ "type": "tool_use", "id": id, "name": name, "input": input })
}

fn result(id: &str, content: &str, is_error: bool) -> serde_json::Value {
    json!({ "type": "tool_result", "tool_use_id": id, "is_error": is_error, "content": content })
}

#[test]
fn conteudo_do_write_vem_logo_depois_da_linha_tool() {
    let lines = parse(&[assistant(json!([tool_use("t1", "Write", json!({ "file_path": "notas.txt", "content": "linha1\nlinha2" }))]))]);

    assert_eq!(
        lines,
        vec![
            (ConsoleKind::Tool, "Write notas.txt".to_string()),
            (ConsoleKind::Output, "linha1\nlinha2".to_string()),
        ]
    );
}

#[test]
fn edit_multilinha_mostra_so_o_texto_novo_sem_o_antigo() {
    let lines = parse(&[assistant(json!([tool_use("t1", "Edit", json!({ "file_path": "notas.txt", "old_string": "a\nb", "new_string": "c\nd" }))]))]);

    assert_eq!(
        lines,
        vec![
            (ConsoleKind::Tool, "Edit notas.txt".to_string()),
            (ConsoleKind::Output, "c\nd".to_string()),
        ]
    );
}

#[test]
fn segredos_no_conteudo_do_write_viram_oculto_e_o_texto_em_volta_continua() {
    let content = "API_KEY=ghp_abcdefghijklmnop1234 fim\nAuthorization: Bearer abc123def456ghi789 x\nsk-ant-api03-XXXXXXXXXXXXXXXX solto";
    let lines = parse(&[assistant(json!([tool_use("t1", "Write", json!({ "file_path": "config.txt", "content": content }))]))]);

    assert_eq!(
        lines[1],
        (
            ConsoleKind::Output,
            "API_KEY=[oculto] fim\nAuthorization: Bearer [oculto] x\n[oculto] solto".to_string()
        )
    );
}

#[test]
fn segredo_no_texto_novo_do_edit_vira_oculto() {
    let lines = parse(&[assistant(json!([tool_use("t1", "Edit", json!({ "file_path": "config.txt", "old_string": "x", "new_string": "API_KEY=ghp_abcdefghijklmnop1234" }))]))]);

    assert_eq!(lines[1], (ConsoleKind::Output, "API_KEY=[oculto]".to_string()));
}

#[test]
fn resultado_real_de_write_e_edit_aparece_e_erro_vira_error() {
    let ok = parse(&[user(json!([result("t1", "gravado com sucesso", false)]))]);
    let failed = parse(&[user(json!([result("t1", "arquivo bloqueado", true)]))]);

    assert_eq!(ok, vec![(ConsoleKind::Output, "gravado com sucesso".to_string())]);
    assert_eq!(failed, vec![(ConsoleKind::Error, "arquivo bloqueado".to_string())]);
}

#[test]
fn conteudo_de_4001_caracteres_corta_e_de_4000_sai_inteiro() {
    let limit = CONSOLE_TEXT_MAX_CHARS;
    let exact = "a".repeat(limit);
    let over = "a".repeat(limit + 1);
    let exact_lines = parse(&[assistant(json!([tool_use("t1", "Write", json!({ "file_path": "a.txt", "content": exact }))]))]);
    let over_lines = parse(&[assistant(json!([tool_use("t1", "Write", json!({ "file_path": "a.txt", "content": over }))]))]);

    assert_eq!(exact_lines[1].1.chars().count(), limit);
    assert!(!exact_lines[1].1.ends_with(CUT_SUFFIX));
    assert_eq!(over_lines[1].1.chars().count(), limit);
    assert!(over_lines[1].1.ends_with(CUT_SUFFIX));
}

#[test]
fn dois_tool_use_no_mesmo_bloco_mantem_cada_conteudo_com_sua_ferramenta() {
    let blocks = assistant(json!([
        tool_use("t1", "Write", json!({ "file_path": "a.txt", "content": "CONTEUDO_W" })),
        tool_use("t2", "Edit", json!({ "file_path": "b.txt", "old_string": "ANT", "new_string": "NOV" })),
    ]));
    let results = user(json!([result("t2", "resultado-edit", false), result("t1", "resultado-write", false)]));

    let lines = parse(&[blocks, results]);

    assert_eq!(
        lines,
        vec![
            (ConsoleKind::Tool, "Write a.txt".to_string()),
            (ConsoleKind::Output, "CONTEUDO_W".to_string()),
            (ConsoleKind::Tool, "Edit b.txt".to_string()),
            (ConsoleKind::Output, "NOV".to_string()),
            (ConsoleKind::Output, "resultado-edit".to_string()),
            (ConsoleKind::Output, "resultado-write".to_string()),
        ]
    );
}

#[test]
fn bloco_de_pensamento_no_mesmo_bloco_do_write_nao_gera_linha() {
    let lines = parse(&[assistant(json!([
        { "type": "thinking", "thinking": "PENSAMENTO_E2E" },
        tool_use("t1", "Write", json!({ "file_path": "a.txt", "content": "conteudo" })),
    ]))]);

    assert_eq!(
        lines,
        vec![
            (ConsoleKind::Tool, "Write a.txt".to_string()),
            (ConsoleKind::Output, "conteudo".to_string()),
        ]
    );
}

#[test]
fn write_sem_content_e_edit_sem_new_string_geram_so_a_linha_tool() {
    let lines = parse(&[assistant(json!([
        tool_use("t1", "Write", json!({ "file_path": "a.txt" })),
        tool_use("t2", "Edit", json!({ "file_path": "b.txt", "old_string": "ANTIGO" })),
    ]))]);

    assert_eq!(
        lines,
        vec![
            (ConsoleKind::Tool, "Write a.txt".to_string()),
            (ConsoleKind::Tool, "Edit b.txt".to_string()),
        ]
    );
}

#[test]
fn bash_read_e_grep_continuam_com_resumo_e_resultado_real() {
    let lines = parse(&[
        assistant(json!([
            tool_use("t1", "Bash", json!({ "command": "git status" })),
            tool_use("t2", "Read", json!({ "file_path": "a.txt" })),
            tool_use("t3", "Grep", json!({ "pattern": "TODO" })),
        ])),
        user(json!([result("t1", "On branch main", false)])),
    ]);

    assert_eq!(
        lines,
        vec![
            (ConsoleKind::Tool, "Bash git status".to_string()),
            (ConsoleKind::Tool, "Read a.txt".to_string()),
            (ConsoleKind::Tool, "Grep TODO".to_string()),
            (ConsoleKind::Output, "On branch main".to_string()),
        ]
    );
}
