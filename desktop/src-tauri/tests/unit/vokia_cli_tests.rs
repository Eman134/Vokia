use super::*;

fn s(items: &[&str]) -> Vec<String> {
    items.iter().map(|v| (*v).to_string()).collect()
}

#[test]
fn valida_url_apenas_loopback() {
    assert_eq!(
        parse_loopback_url("http://127.0.0.1:43210/mcp").unwrap(),
        Endpoint {
            port: 43210,
            path: "/mcp".to_string()
        }
    );
    assert!(parse_loopback_url("http://localhost:8080/mcp").is_ok());
    assert!(parse_loopback_url("http://example.com/mcp").is_err());
    assert!(parse_loopback_url("https://127.0.0.1:8080/mcp").is_err());
}

#[test]
fn parse_set_variable_int_e_request_decision_multi_option() {
    let var = parse_args(
        &s(&["set-variable", "--name", "count", "--op", "set", "--value", "42", "--int", "--share"]),
        || Ok(String::new()),
    )
    .unwrap();
    assert_eq!(
        var,
        Action::CallTool {
            name: "set_variable".to_string(),
            arguments: json!({ "name": "count", "op": "set", "value": 42, "share": true }),
        }
    );

    let dec = parse_args(
        &s(&[
            "request-decision",
            "--kind",
            "choice",
            "--title",
            "Qual?",
            "--option",
            "A",
            "--option",
            "B",
        ]),
        || Ok(String::new()),
    )
    .unwrap();
    assert_eq!(
        dec,
        Action::CallTool {
            name: "request_decision".to_string(),
            arguments: json!({ "kind": "choice", "title": "Qual?", "options": ["A", "B"] }),
        }
    );
}

#[test]
fn parse_call_stdin_com_utf8_e_save_item_plan() {
    let call = parse_args(&s(&["call", "create_card", "--stdin"]), || {
        Ok(r#"{"title":"Ação com \"aspas\" e café"}"#.to_string())
    })
    .unwrap();
    assert_eq!(
        call,
        Action::CallTool {
            name: "create_card".to_string(),
            arguments: json!({ "title": "Ação com \"aspas\" e café" }),
        }
    );

    let plan = parse_args(
        &s(&["save-item-plan", "--item-id", "card-1", "--json", r#"{"summary":"ok"}"#]),
        || Ok(String::new()),
    )
    .unwrap();
    assert_eq!(
        plan,
        Action::CallTool {
            name: "save_item_plan".to_string(),
            arguments: json!({ "item_id": "card-1", "summary": "ok" }),
        }
    );
}

#[test]
fn decodifica_http_chunked() {
    let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";
    let (status, body) = parse_http_response(raw).unwrap();
    assert_eq!(status, 200);
    assert_eq!(body, "hello world");
}

#[test]
fn parse_commands_com_domain_e_json() {
    let com_domain = parse_args(
        &s(&["commands", "--domain", "roles", "--json"]),
        || Ok(String::new()),
    )
    .unwrap();
    assert_eq!(
        com_domain,
        Action::ListCommands {
            json_raw: true,
            domain: Some("roles".to_string()),
        }
    );

    let ordem_invertida = parse_args(
        &s(&["commands", "--json", "--domain", "channels"]),
        || Ok(String::new()),
    )
    .unwrap();
    assert_eq!(
        ordem_invertida,
        Action::ListCommands {
            json_raw: true,
            domain: Some("channels".to_string()),
        }
    );

    let sem_domain = parse_args(&s(&["commands", "--json"]), || Ok(String::new())).unwrap();
    assert_eq!(
        sem_domain,
        Action::ListCommands {
            json_raw: true,
            domain: None,
        }
    );

    let texto_puro = parse_args(&s(&["commands"]), || Ok(String::new())).unwrap();
    assert_eq!(
        texto_puro,
        Action::ListCommands {
            json_raw: false,
            domain: None,
        }
    );
}

#[test]
fn parse_commands_com_domain_sem_valor_retorna_erro() {
    let err = parse_args(&s(&["commands", "--domain"]), || Ok(String::new())).unwrap_err();
    assert!(err.contains("--domain"), "erro esperado: {err}");

    let err_flag = parse_args(&s(&["commands", "--flag-invalida"]), || Ok(String::new())).unwrap_err();
    assert!(err_flag.contains("--flag-invalida"), "erro esperado: {err_flag}");
}

#[test]
fn parse_cmd_com_json_e_flags() {
    let cmd_json = parse_args(
        &s(&["cmd", "role_create", "--json", r#"{"name":"Tester"}"#]),
        || Ok(String::new()),
    )
    .unwrap();
    assert_eq!(
        cmd_json,
        Action::CallCommand {
            name: "role_create".to_string(),
            arguments: json!({ "name": "Tester" }),
        }
    );

    let cmd_flags = parse_args(
        &s(&["cmd", "role_delete", "--id", "42"]),
        || Ok(String::new()),
    )
    .unwrap();
    assert_eq!(
        cmd_flags,
        Action::CallCommand {
            name: "role_delete".to_string(),
            arguments: json!({ "id": 42 }),
        }
    );
}

#[test]
fn parse_artifact_flags_json_e_stdin() {
    let list_cmd = parse_args(&s(&["artifact", "--op", "list"]), || Ok(String::new())).unwrap();
    assert_eq!(
        list_cmd,
        Action::CallTool {
            name: "artifact".to_string(),
            arguments: json!({ "op": "list" }),
        }
    );

    let create_cmd = parse_args(
        &s(&[
            "artifact",
            "--op",
            "create",
            "--kind",
            "checklist",
            "--title",
            "Meu Checklist",
            "--tags",
            "mvp,api",
        ]),
        || Ok(String::new()),
    )
    .unwrap();
    assert_eq!(
        create_cmd,
        Action::CallTool {
            name: "artifact".to_string(),
            arguments: json!({
                "op": "create",
                "kind": "checklist",
                "title": "Meu Checklist",
                "tags": ["mvp", "api"],
            }),
        }
    );

    let update_cmd = parse_args(
        &s(&["artifact", "--op", "update", "--id", "checklist-1", "--title", "Novo Titulo"]),
        || Ok(String::new()),
    )
    .unwrap();
    assert_eq!(
        update_cmd,
        Action::CallTool {
            name: "artifact".to_string(),
            arguments: json!({
                "op": "update",
                "id": "checklist-1",
                "title": "Novo Titulo",
            }),
        }
    );

    let check_cmd = parse_args(
        &s(&["artifact", "--op", "check", "--id", "checklist-1", "--index", "0", "--done", "true"]),
        || Ok(String::new()),
    )
    .unwrap();
    assert_eq!(
        check_cmd,
        Action::CallTool {
            name: "artifact".to_string(),
            arguments: json!({
                "op": "check",
                "id": "checklist-1",
                "index": 0,
                "done": true,
            }),
        }
    );

    let json_cmd = parse_args(
        &s(&["artifact", "--op", "create", "--json", r#"{"kind":"decisao","title":"Nova Arquitetura"}"#]),
        || Ok(String::new()),
    )
    .unwrap();
    assert_eq!(
        json_cmd,
        Action::CallTool {
            name: "artifact".to_string(),
            arguments: json!({
                "op": "create",
                "kind": "decisao",
                "title": "Nova Arquitetura",
            }),
        }
    );

    let stdin_cmd = parse_args(
        &s(&["artifact", "--op", "update", "--id", "checklist-1", "--stdin"]),
        || Ok("# Novo Markdown do artefato\n- [ ] Item 1".to_string()),
    )
    .unwrap();
    assert_eq!(
        stdin_cmd,
        Action::CallTool {
            name: "artifact".to_string(),
            arguments: json!({
                "op": "update",
                "id": "checklist-1",
                "body": "# Novo Markdown do artefato\n- [ ] Item 1",
            }),
        }
    );

    let crlf_cmd = parse_args(
        &s(&["artifact", "--op", "update", "--id", "checklist-1", "--stdin"]),
        || Ok("# T\r\n- [ ] a\r\n".to_string()),
    )
    .unwrap();
    assert_eq!(
        crlf_cmd,
        Action::CallTool {
            name: "artifact".to_string(),
            arguments: json!({
                "op": "update",
                "id": "checklist-1",
                "body": "# T\n- [ ] a\n",
            }),
        }
    );

    let err_sem_id = parse_args(
        &s(&["artifact", "--op", "update", "--stdin"]),
        || Ok("# T".to_string()),
    )
    .unwrap_err();
    assert_eq!(err_sem_id, "flag obrigatória ausente: --id");

    let err_op = parse_args(&s(&["artifact", "--op", "desconhecida"]), || Ok(String::new())).unwrap_err();
    assert!(err_op.contains("--op deve ser"), "erro: {err_op}");

    let err_sem_op = parse_args(&s(&["artifact"]), || Ok(String::new())).unwrap_err();
    assert!(err_sem_op.contains("--op"), "erro: {err_sem_op}");

    let err_ambos = parse_args(
        &s(&["artifact", "--op", "create", "--json", "{}", "--stdin"]),
        || Ok("conteudo".to_string()),
    )
    .unwrap_err();
    assert!(err_ambos.contains("não ambos"), "erro: {err_ambos}");
}

#[test]
fn parse_cmd_artifacts_kebab_case_to_camel_case() {
    let cmd = parse_args(
        &s(&["cmd", "list_artifacts", "--project-id", "12"]),
        || Ok(String::new()),
    )
    .unwrap();
    assert_eq!(
        cmd,
        Action::CallCommand {
            name: "list_artifacts".to_string(),
            arguments: json!({ "projectId": 12 }),
        }
    );

    let cmd_create = parse_args(
        &s(&[
            "cmd",
            "create_artifact",
            "--json",
            r#"{"projectId":1,"kind":"checklist","title":"CLI Teste"}"#,
        ]),
        || Ok(String::new()),
    )
    .unwrap();
    assert_eq!(
        cmd_create,
        Action::CallCommand {
            name: "create_artifact".to_string(),
            arguments: json!({
                "projectId": 1,
                "kind": "checklist",
                "title": "CLI Teste",
            }),
        }
    );
}

#[test]
fn delegate_envia_ping_seconds_como_inteiro_e_omite_quando_ausente() {
    let with_ping = parse_args(
        &s(&["delegate", "--agent", "vexa", "--request", "Revise", "--ping-seconds", "45"]),
        || Ok(String::new()),
    )
    .unwrap();
    assert_eq!(
        with_ping,
        Action::CallTool {
            name: "delegate".to_string(),
            arguments: json!({ "agent": "vexa", "request": "Revise", "ping_seconds": 45 }),
        }
    );

    let without = parse_args(&s(&["delegate", "--agent", "vexa", "--request", "Revise"]), || Ok(String::new())).unwrap();
    assert_eq!(
        without,
        Action::CallTool {
            name: "delegate".to_string(),
            arguments: json!({ "agent": "vexa", "request": "Revise" }),
        }
    );
}

#[test]
fn delegate_recusa_ping_seconds_que_nao_e_inteiro_positivo() {
    for invalid in ["abc", "-5", "2.5", ""] {
        let error = parse_args(
            &s(&["delegate", "--agent", "vexa", "--request", "Revise", "--ping-seconds", invalid]),
            || Ok(String::new()),
        )
        .unwrap_err();
        assert_eq!(error, "--ping-seconds precisa ser um número inteiro de segundos", "{invalid:?}");
    }
}
