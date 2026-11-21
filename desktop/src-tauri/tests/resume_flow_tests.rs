// Retomada até a fila do destinatário: com a chave dele ocupada, handle_request enfileira o job em vez de abrir a CLI,
// e o job enfileirado mostra quem recebe, com que card e com que texto.
const RESUME_NOTE_TEXT: &str = "Retomando o pedido interrompido de onde parou.";
const HANDOFFS_TEXT: &str = "Delegações já feitas:";
const COORDINATION_TEXT: &str = "Coordene a continuação";
const MESSAGES_TEXT: &str = "Mensagens já trocadas";
const ORIGINAL_REQUEST_TEXT: &str = "Pedido original: Implementar o login";
const RESUME_WINDOW_MS: i64 = 10_000;

struct QueuedResume {
    text: String,
    item_id: Option<String>,
    resumed_run: Option<i64>,
}

fn resume_project(world: &World) -> i64 {
    world
        .db()
        .insert_project(&NewProject {
            name: "retomada",
            symbol: "folder",
            description: "",
            path: &world.dir.to_string_lossy(),
            origin_kind: "folder",
            origin_repo: None,
        })
        .expect("o projeto deveria ser criado")
        .id
}

fn now_ms() -> i64 {
    i64::try_from(crate::orchestrator::now_millis().unwrap()).unwrap()
}

fn interrupted_run(world: &World, project_id: i64, agent: &str, started_at: i64) -> i64 {
    let db = world.db();
    let id = db
        .start_run(project_id, agent, "Implementar o login", None, Some("item-1"), started_at)
        .unwrap();
    db.interrupt_run(id).unwrap();
    id
}

fn delegation(world: &World, project_id: i64, from: &str, to: &str) {
    world
        .db()
        .record_handoff(
            project_id,
            &crate::db::HandoffDraft {
                run_id: None,
                from_agent: from.to_string(),
                to_agent: to.to_string(),
                from_layer: None,
                to_layer: None,
                action: "delegar".to_string(),
                kind: "implementar".to_string(),
                item_id: Some("item-1".to_string()),
                return_count: 0,
                message_id: None,
                event: None,
                edge_id: None,
            },
        )
        .expect("a delegação deveria ser gravada");
}

fn occupy(world: &World, project_id: i64, agent: &str) {
    let state = world.app.state::<crate::state::AppState>();
    let mut flow = state.flow.lock().unwrap();
    flow.pending.insert(
        crate::session_key::SessionKey {
            project_id,
            agent: Agent::from_stored(agent).unwrap(),
        },
        std::collections::VecDeque::new(),
    );
}

fn queued(world: &World, project_id: i64, agent: &str) -> Vec<QueuedResume> {
    let state = world.app.state::<crate::state::AppState>();
    let flow = state.flow.lock().unwrap();
    let key = crate::session_key::SessionKey {
        project_id,
        agent: Agent::from_stored(agent).unwrap(),
    };
    flow.pending
        .get(&key)
        .map(|jobs| {
            jobs.iter()
                .map(|job| QueuedResume {
                    text: job.text.clone(),
                    item_id: job.context.item_id.clone(),
                    resumed_run: job.context.resumed_run,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn resume(world: &World, id: i64) -> Result<Value, String> {
    tauri::async_runtime::block_on(crate::commands::dispatcher::dispatch(
        world.app.handle(),
        "resume_interrupted_run",
        json!({ "id": id }),
    ))
    .expect("resume_interrupted_run deveria ser reconhecido")
}

fn feed(world: &World, project_id: i64) -> Vec<(String, String, String)> {
    let conn = world.db().conn().unwrap();
    let mut statement = conn
        .prepare("SELECT from_agent, to_agent, text FROM messages WHERE project_id = ?1 ORDER BY id")
        .unwrap();
    statement
        .query_map([project_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn run_state(world: &World, id: i64) -> String {
    world
        .db()
        .conn()
        .unwrap()
        .query_row("SELECT state FROM orch_runs WHERE id = ?1", [id], |row| row.get(0))
        .unwrap()
}

fn run_count(world: &World) -> i64 {
    world
        .db()
        .conn()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM orch_runs", [], |row| row.get(0))
        .unwrap()
}

#[test]
fn retomar_fluxo_com_delegacao_vai_ao_ember_sem_card_no_contexto() {
    let world = world();
    let project_id = resume_project(&world);
    let id = interrupted_run(&world, project_id, "vexa", now_ms() - RESUME_WINDOW_MS);
    delegation(&world, project_id, "vexa", "hefesto");
    occupy(&world, project_id, "ember");
    occupy(&world, project_id, "vexa");
    let feed_before = feed(&world, project_id);
    let runs_before = run_count(&world);

    resume(&world, id).expect("a retomada deveria ser aceita e enfileirada");

    let jobs = queued(&world, project_id, "ember");
    assert_eq!(jobs.len(), 1, "o Ember deveria receber a continuação");
    assert!(queued(&world, project_id, "vexa").is_empty(), "o agente original não recebe a continuação");
    assert_eq!(jobs[0].item_id, None, "o Ember coordena sem prender a sessão do card");
    assert_eq!(jobs[0].resumed_run, Some(id));
    assert!(jobs[0].text.contains(ORIGINAL_REQUEST_TEXT), "{}", jobs[0].text);
    assert!(jobs[0].text.contains(HANDOFFS_TEXT), "{}", jobs[0].text);
    assert!(jobs[0].text.contains("- vexa → hefesto (implementar de item-1)"), "{}", jobs[0].text);
    assert!(jobs[0].text.contains(COORDINATION_TEXT), "{}", jobs[0].text);
    let added: Vec<_> = feed(&world, project_id).into_iter().skip(feed_before.len()).collect();
    assert_eq!(
        added,
        [("ember".to_string(), "user".to_string(), RESUME_NOTE_TEXT.to_string())],
        "a retomada só avisa quem continua; não regrava o pedido do usuário"
    );
    assert_eq!(run_count(&world), runs_before, "a retomada não cria registro novo");
    assert_eq!(run_state(&world, id), "em_andamento");
}

#[test]
fn retomar_sem_delegacao_continua_no_agente_original_com_o_card_original() {
    let world = world();
    let project_id = resume_project(&world);
    let started_at = now_ms() - RESUME_WINDOW_MS;
    let id = interrupted_run(&world, project_id, "vexa", started_at);
    world
        .db()
        .insert_message(
            project_id,
            crate::events::Speaker::Agent(Agent::vexa()),
            crate::events::Speaker::User,
            "Criei a tabela de usuários",
            (started_at + 1) as u64,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    occupy(&world, project_id, "vexa");

    resume(&world, id).expect("a retomada deveria ser aceita e enfileirada");

    let jobs = queued(&world, project_id, "vexa");
    assert_eq!(jobs.len(), 1);
    assert!(queued(&world, project_id, "ember").is_empty());
    assert_eq!(jobs[0].item_id.as_deref(), Some("item-1"));
    assert_eq!(jobs[0].resumed_run, Some(id));
    assert!(jobs[0].text.contains(MESSAGES_TEXT), "{}", jobs[0].text);
    assert!(jobs[0].text.contains("- vexa → user: Criei a tabela de usuários"), "{}", jobs[0].text);
    assert!(!jobs[0].text.contains(HANDOFFS_TEXT), "{}", jobs[0].text);
    assert!(!jobs[0].text.contains(COORDINATION_TEXT), "{}", jobs[0].text);
    let last = feed(&world, project_id).pop().unwrap();
    assert_eq!(last, ("vexa".to_string(), "user".to_string(), RESUME_NOTE_TEXT.to_string()));
}

#[test]
fn ember_original_com_delegacao_continua_no_ember_sem_instrucao_de_coordenacao() {
    let world = world();
    let project_id = resume_project(&world);
    let id = interrupted_run(&world, project_id, "ember", now_ms() - RESUME_WINDOW_MS);
    delegation(&world, project_id, "ember", "vexa");
    occupy(&world, project_id, "ember");

    resume(&world, id).expect("a retomada deveria ser aceita e enfileirada");

    let jobs = queued(&world, project_id, "ember");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].item_id.as_deref(), Some("item-1"), "o próprio Ember mantém o card original");
    assert!(jobs[0].text.contains(HANDOFFS_TEXT), "{}", jobs[0].text);
    assert!(!jobs[0].text.contains(COORDINATION_TEXT), "{}", jobs[0].text);
}

#[test]
fn segundo_clique_em_retomar_e_recusado_e_nao_enfileira_de_novo() {
    let world = world();
    let project_id = resume_project(&world);
    let id = interrupted_run(&world, project_id, "vexa", now_ms() - RESUME_WINDOW_MS);
    occupy(&world, project_id, "vexa");

    resume(&world, id).expect("a primeira retomada deveria ser aceita");
    let second = resume(&world, id).unwrap_err();

    assert!(second.ends_with(&format!("O pedido {id} não está mais interrompido.")), "{second}");
    assert_eq!(queued(&world, project_id, "vexa").len(), 1);
}

#[test]
fn erro_depois_de_reivindicar_devolve_o_pedido_aos_interrompidos() {
    let world = world();
    let project_id = resume_project(&world);
    let id = interrupted_run(&world, project_id, "vexa", now_ms() - RESUME_WINDOW_MS);
    world
        .db()
        .conn()
        .unwrap()
        .execute("UPDATE orch_runs SET agent = 'Agente-X' WHERE id = ?1", [id])
        .unwrap();
    let feed_before = feed(&world, project_id).len();

    resume(&world, id).expect_err("agente desconhecido não pode ser retomado");

    assert_eq!(run_state(&world, id), "interrompido");
    assert_eq!(world.db().interrupted_runs().unwrap().iter().filter(|run| run.id == id).count(), 1);
    assert_eq!(feed(&world, project_id).len(), feed_before, "nada vai ao feed quando a retomada falha antes de começar");
}
