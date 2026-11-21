use std::time::Duration;

const EVENT_SETTLE: Duration = Duration::from_millis(200);

#[test]
fn todo_comando_do_dominio_artifacts_responde_a_dispatch_e_recusa_argumentos_vazios() {
    use crate::commands::dispatcher::{dispatch, list_commands};

    let world = world();
    let commands = list_commands();
    let artifact_cmds: Vec<_> = commands
        .iter()
        .filter(|cmd| cmd["domain"] == "artifacts")
        .collect();

    assert_eq!(
        artifact_cmds.len(),
        8,
        "devem existir 8 comandos no domínio artifacts"
    );

    for cmd in &artifact_cmds {
        let name = cmd["name"].as_str().expect("nome do comando");
        let result =
            tauri::async_runtime::block_on(dispatch(world.app.handle(), name, json!({})));

        if name == "list_artifact_kinds" {
            let inner = result.expect("list_artifact_kinds deve ser reconhecido pelo dispatcher");
            assert!(
                inner.is_ok(),
                "list_artifact_kinds não exige argumentos extras: {inner:?}"
            );
        } else {
            let inner = result.unwrap_or_else(|| {
                panic!("comando {name} deveria ser reconhecido pelo dispatcher, mas retornou None")
            });
            match inner {
                Err(err) => {
                    assert!(
                        err.contains("Argumentos inválidos"),
                        "esperava erro de parse de argumentos para {name}, obteve: {err}"
                    );
                }
                Ok(val) => {
                    panic!("esperava erro de argumentos para {name}, mas obteve sucesso: {val:?}");
                }
            }
        }
    }
}

#[test]
fn camel_case_project_id_e_campos_sao_aceitos_e_snake_case_falha() {
    use crate::commands::dispatcher::dispatch;

    let world = world();
    let project = world.project("proj_camel");

    let res_list_ok = tauri::async_runtime::block_on(dispatch(
        world.app.handle(),
        "list_artifacts",
        json!({ "projectId": project }),
    ))
    .expect("list_artifacts reconhecido");
    assert!(res_list_ok.is_ok(), "projectId camelCase deve ser aceito: {res_list_ok:?}");

    let res_list_err = tauri::async_runtime::block_on(dispatch(
        world.app.handle(),
        "list_artifacts",
        json!({ "project_id": project }),
    ))
    .expect("list_artifacts reconhecido");
    match res_list_err {
        Err(err) => {
            assert!(
                err.contains("projectId"),
                "deve acusar campo faltante projectId: {err}"
            );
        }
        Ok(val) => panic!("esperava erro para snake_case, mas sucedeu: {val:?}"),
    }

    let res_create_ok = tauri::async_runtime::block_on(dispatch(
        world.app.handle(),
        "create_artifact",
        json!({
            "projectId": project,
            "kind": "checklist",
            "title": "Checklist CamelCase",
            "summary": "Resumo"
        }),
    ))
    .expect("create_artifact reconhecido");
    assert!(res_create_ok.is_ok(), "create_artifact camelCase deve passar: {res_create_ok:?}");
    let val = res_create_ok.unwrap();
    assert_eq!(val["id"], "checklist-1");

    let res_create_err = tauri::async_runtime::block_on(dispatch(
        world.app.handle(),
        "create_artifact",
        json!({
            "project_id": project,
            "kind": "checklist",
            "title": "Checklist SnakeCase"
        }),
    ))
    .expect("create_artifact reconhecido");
    match res_create_err {
        Err(err) => {
            assert!(
                err.contains("projectId"),
                "deve acusar campo faltante projectId: {err}"
            );
        }
        Ok(val) => panic!("esperava erro para snake_case, mas sucedeu: {val:?}"),
    }
}

#[test]
fn escrita_via_comando_emite_evento_artifacts_changed() {
    use crate::commands::dispatcher::dispatch;

    let world = world();
    let project = world.project("proj_events");

    let events: Arc<Mutex<Vec<Value>>> = Arc::default();
    let sink = events.clone();
    world.app.handle().listen(EVENT_NAME, move |event| {
        if let Ok(val) = serde_json::from_str::<Value>(event.payload()) {
            if val["type"] == json!("artifacts-changed") {
                sink.lock().unwrap().push(val);
            }
        }
    });

    let res_create = tauri::async_runtime::block_on(dispatch(
        world.app.handle(),
        "create_artifact",
        json!({
            "projectId": project,
            "kind": "checklist",
            "title": "Checklist do Evento"
        }),
    ))
    .expect("create_artifact")
    .expect("create_artifact ok");
    assert_eq!(res_create["id"], "checklist-1");

    let res_update = tauri::async_runtime::block_on(dispatch(
        world.app.handle(),
        "update_artifact",
        json!({
            "projectId": project,
            "id": "checklist-1",
            "title": "Checklist Editado"
        }),
    ))
    .expect("update_artifact")
    .expect("update_artifact ok");
    assert_eq!(res_update["title"], "Checklist Editado");

    let res_check = tauri::async_runtime::block_on(dispatch(
        world.app.handle(),
        "set_artifact_check",
        json!({
            "projectId": project,
            "id": "checklist-1",
            "index": 0,
            "done": true
        }),
    ))
    .expect("set_artifact_check")
    .expect("set_artifact_check ok");
    assert_eq!(res_check["id"], "checklist-1");

    let res_del = tauri::async_runtime::block_on(dispatch(
        world.app.handle(),
        "delete_artifact",
        json!({
            "projectId": project,
            "id": "checklist-1"
        }),
    ))
    .expect("delete_artifact")
    .expect("delete_artifact ok");
    assert_eq!(res_del, Value::Null);

    std::thread::sleep(EVENT_SETTLE);

    let received = events.lock().unwrap().clone();
    assert_eq!(
        received.len(),
        4,
        "todas as 4 operações de escrita devem emitir artifacts-changed: {received:?}"
    );
    for evt in &received {
        assert_eq!(evt["type"], "artifacts-changed");
        assert_eq!(evt["projectId"], project);
    }
}

#[test]
fn tool_mcp_artifact_escritas_emitem_evento_artifacts_changed() {
    let world = world();
    let project = world.project("proj_mcp_events");
    let axiom = world.agent("axiom", "Axiom", project);

    let events: Arc<Mutex<Vec<Value>>> = Arc::default();
    let sink = events.clone();
    world.app.handle().listen(EVENT_NAME, move |event| {
        if let Ok(val) = serde_json::from_str::<Value>(event.payload()) {
            if val["type"] == json!("artifacts-changed") {
                sink.lock().unwrap().push(val);
            }
        }
    });

    let create_outcome = world.call(
        &axiom,
        "artifact",
        json!({
            "op": "create",
            "kind": "checklist",
            "title": "Checklist MCP Evento"
        }),
    );
    assert!(!create_outcome.is_error, "erro: {}", create_outcome.text);

    let update_outcome = world.call(
        &axiom,
        "artifact",
        json!({
            "op": "update",
            "id": "checklist-1",
            "title": "Checklist MCP Atualizado"
        }),
    );
    assert!(!update_outcome.is_error, "erro: {}", update_outcome.text);

    let check_outcome = world.call(
        &axiom,
        "artifact",
        json!({
            "op": "check",
            "id": "checklist-1",
            "index": 0,
            "done": true
        }),
    );
    assert!(!check_outcome.is_error, "erro: {}", check_outcome.text);

    std::thread::sleep(EVENT_SETTLE);

    let received = events.lock().unwrap().clone();
    assert_eq!(
        received.len(),
        3,
        "create, update e check via tool MCP artifact devem emitir artifacts-changed: {received:?}"
    );
    for evt in &received {
        assert_eq!(evt["type"], "artifacts-changed");
        assert_eq!(evt["projectId"], project);
    }
}

#[test]
fn tool_mcp_artifact_com_op_create_usa_binding_project_id_e_grava_autor_binding_agent() {
    let world = world();
    let project_a = world.project("proj_a");
    let project_b = world.project("proj_b");

    let axiom_a = world.agent("axiom", "Axiom", project_a);
    let vexa_b = world.agent("vexa", "Vexa", project_b);

    let outcome_a = world.call(
        &axiom_a,
        "artifact",
        json!({
            "op": "create",
            "kind": "checklist",
            "title": "Checklist do Axiom"
        }),
    );
    assert!(!outcome_a.is_error, "erro: {}", outcome_a.text);
    let doc_a: Value = serde_json::from_str(&outcome_a.text).unwrap();
    assert_eq!(doc_a["id"], "checklist-1");
    assert_eq!(doc_a["author"], "axiom");

    let outcome_b = world.call(
        &vexa_b,
        "artifact",
        json!({
            "op": "create",
            "kind": "checklist",
            "title": "Checklist da Vexa"
        }),
    );
    assert!(!outcome_b.is_error, "erro: {}", outcome_b.text);
    let doc_b: Value = serde_json::from_str(&outcome_b.text).unwrap();
    assert_eq!(doc_b["id"], "checklist-1");
    assert_eq!(doc_b["author"], "vexa");

    let doc_db_a = match world.db().get_artifact(project_a, "checklist-1").unwrap() {
        crate::db::ArtifactDetail::Document(doc) => doc,
        other => panic!("esperava Document, obteve {other:?}"),
    };
    assert_eq!(doc_db_a.title, "Checklist do Axiom");
    assert_eq!(doc_db_a.author.as_deref(), Some("axiom"));

    let doc_db_b = match world.db().get_artifact(project_b, "checklist-1").unwrap() {
        crate::db::ArtifactDetail::Document(doc) => doc,
        other => panic!("esperava Document, obteve {other:?}"),
    };
    assert_eq!(doc_db_b.title, "Checklist da Vexa");
    assert_eq!(doc_db_b.author.as_deref(), Some("vexa"));

    let read_a = world.call(
        &axiom_a,
        "artifact",
        json!({
            "op": "read",
            "id": "checklist-1"
        }),
    );
    assert!(!read_a.is_error);
    let read_val_a: Value = serde_json::from_str(&read_a.text).unwrap();
    assert_eq!(read_val_a["title"], "Checklist do Axiom");
    assert_eq!(read_val_a["author"], "axiom");
}

#[test]
fn recusas_de_erro_db_error_sao_tratadas_com_refusal_claro_e_nao_system_failure() {
    let world = world();
    let project = world.project("proj_refusals");
    let axiom = world.agent("axiom", "Axiom", project);

    let outcome_op = world.call(
        &axiom,
        "artifact",
        json!({
            "op": "desconhecida"
        }),
    );
    assert!(outcome_op.is_error);
    assert!(
        outcome_op.text.contains("Operação de artefato desconhecida"),
        "mensagem: {}",
        outcome_op.text
    );
    assert!(
        !outcome_op.text.contains("Falha interna do sistema"),
        "não pode ser SYSTEM_FAILURE"
    );

    let outcome_missing_id = world.call(
        &axiom,
        "artifact",
        json!({
            "op": "read"
        }),
    );
    assert!(outcome_missing_id.is_error);
    assert!(
        outcome_missing_id.text.contains("exige o campo 'id'"),
        "mensagem: {}",
        outcome_missing_id.text
    );
    assert!(
        !outcome_missing_id.text.contains("Falha interna do sistema"),
        "não pode ser SYSTEM_FAILURE"
    );

    let outcome_kind = world.call(
        &axiom,
        "artifact",
        json!({
            "op": "create",
            "kind": "tipo_inexistente",
            "title": "Título Válido"
        }),
    );
    assert!(outcome_kind.is_error);
    assert!(
        outcome_kind.text.contains("Tipo de artefato desconhecido"),
        "mensagem: {}",
        outcome_kind.text
    );
    assert!(
        !outcome_kind.text.contains("Falha interna do sistema"),
        "não pode ser SYSTEM_FAILURE"
    );

    let outcome_title = world.call(
        &axiom,
        "artifact",
        json!({
            "op": "create",
            "kind": "checklist",
            "title": "   "
        }),
    );
    assert!(outcome_title.is_error);
    assert!(
        outcome_title.text.contains("O título do artefato não pode ficar vazio"),
        "mensagem: {}",
        outcome_title.text
    );
    assert!(
        !outcome_title.text.contains("Falha interna do sistema"),
        "não pode ser SYSTEM_FAILURE"
    );

    let outcome_not_found = world.call(
        &axiom,
        "artifact",
        json!({
            "op": "read",
            "id": "checklist-999"
        }),
    );
    assert!(outcome_not_found.is_error);
    assert!(
        outcome_not_found.text.contains("O artefato checklist-999 não existe"),
        "mensagem: {}",
        outcome_not_found.text
    );
    assert!(
        !outcome_not_found.text.contains("Falha interna do sistema"),
        "não pode ser SYSTEM_FAILURE"
    );

    let create_ok = world.call(
        &axiom,
        "artifact",
        json!({
            "op": "create",
            "kind": "checklist",
            "title": "Checklist Base"
        }),
    );
    assert!(!create_ok.is_error);

    let outcome_conflict = world.call(
        &axiom,
        "artifact",
        json!({
            "op": "update",
            "id": "checklist-1",
            "title": "Novo Titulo",
            "expected_updated_at": 1
        }),
    );
    assert!(outcome_conflict.is_error);
    assert!(
        outcome_conflict.text.contains("foi alterado depois da sua leitura"),
        "mensagem: {}",
        outcome_conflict.text
    );
    assert!(
        !outcome_conflict.text.contains("Falha interna do sistema"),
        "não pode ser SYSTEM_FAILURE"
    );

    let outcome_index = world.call(
        &axiom,
        "artifact",
        json!({
            "op": "check",
            "id": "checklist-1",
            "index": 999,
            "done": true
        }),
    );
    assert!(outcome_index.is_error);
    assert!(
        outcome_index.text.contains("não tem o item de checklist de índice 999"),
        "mensagem: {}",
        outcome_index.text
    );
    assert!(
        !outcome_index.text.contains("Falha interna do sistema"),
        "não pode ser SYSTEM_FAILURE"
    );

    let card_id = world.create(&axiom, "Card do Plano");
    let save_plan_outcome = world.call(
        &axiom,
        "save_item_plan",
        linked_plan_arguments(&card_id),
    );
    assert!(!save_plan_outcome.is_error, "erro: {}", save_plan_outcome.text);

    let plan_id = format!("plano-{card_id}");
    let outcome_readonly = world.call(
        &axiom,
        "artifact",
        json!({
            "op": "update",
            "id": plan_id,
            "title": "Tentativa de editar plano"
        }),
    );
    assert!(outcome_readonly.is_error);
    assert!(
        outcome_readonly.text.contains("não pode ser alterado aqui"),
        "mensagem: {}",
        outcome_readonly.text
    );
    assert!(
        !outcome_readonly.text.contains("Falha interna do sistema"),
        "não pode ser SYSTEM_FAILURE"
    );
}
