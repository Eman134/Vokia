use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{Listener, Manager};

use super::*;
use crate::agents::Agent;
use crate::db::{HandoffDraft, NewProject};
use crate::mcp::support::build_app;

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);
const EVENT_SETTLE: Duration = Duration::from_millis(250);

struct World {
    app: tauri::App,
    dir: PathBuf,
    events: Arc<Mutex<Vec<Value>>>,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn world_in(dir: PathBuf) -> World {
    let app = build_app(&dir);
    let events: Arc<Mutex<Vec<Value>>> = Arc::default();
    let sink = events.clone();
    app.handle().listen(EVENT_NAME, move |event| {
        sink.lock()
            .unwrap()
            .push(serde_json::from_str(event.payload()).unwrap());
    });
    World { app, dir, events }
}

fn world() -> World {
    let dir = std::env::temp_dir().join(format!(
        "vokia-orch-commands-test-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&dir).unwrap();
    world_in(dir)
}

impl World {
    fn db(&self) -> &Db {
        self.app.state::<DbState>().inner().require().unwrap()
    }

    fn project(&self, name: &str) -> i64 {
        self.db()
            .insert_project(&NewProject {
                name,
                symbol: "folder",
                description: "",
                path: &format!("C:/projetos/{name}"),
                origin_kind: "folder",
                origin_repo: None,
            })
            .unwrap()
            .id
    }

    fn events(&self) -> Vec<Value> {
        std::thread::sleep(EVENT_SETTLE);
        self.events.lock().unwrap().clone()
    }

    fn get(&self, project: i64) -> Result<OrchestrationSnapshot, String> {
        get_orchestration(self.app.state(), self.app.state(), project)
    }

    fn save_layer(&self, project: i64, name: &str, kind: &str) -> Result<Layer, String> {
        save_layer(
            self.app.handle().clone(),
            self.app.state(),
            self.app.state(),
            project,
            LayerDraft {
                id: None,
                name: name.to_string(),
                description: String::new(),
                kind: kind.to_string(),
                instructions: String::new(),
            },
        )
    }

    fn save_rule(&self, project: i64, from: &str, to: &str, action: &str) -> Result<Rule, String> {
        save_rule(
            self.app.handle().clone(),
            self.app.state(),
            self.app.state(),
            project,
            RuleDraft {
                id: None,
                from_layer: from.to_string(),
                to_layer: to.to_string(),
                when_event: "tarefa_concluida".to_string(),
                action: action.to_string(),
                kind: "revisar".to_string(),
                exec_mode: "sequencia".to_string(),
                return_limit: 2,
            },
        )
    }
}

fn changed(project: i64) -> Value {
    json!({ "type": "orchestration-changed", "projectId": project })
}

fn layer_ids(snapshot: &OrchestrationSnapshot) -> Vec<&str> {
    snapshot.layers.iter().map(|l| l.id.as_str()).collect()
}

fn placed(snapshot: &OrchestrationSnapshot) -> Vec<(&str, &str)> {
    let mut list: Vec<_> = snapshot
        .placements
        .iter()
        .map(|p| (p.agent_id.as_str(), p.layer_id.as_str()))
        .collect();
    list.sort();
    list
}

fn sorted<'a>(mut list: Vec<(&'a str, &'a str)>) -> Vec<(&'a str, &'a str)> {
    list.sort();
    list
}

fn orphan() -> PlacementDraft {
    PlacementDraft {
        agent_id: "fantasma".to_string(),
        team_id: "time".to_string(),
        layer_id: "implementacao".to_string(),
        instructions: String::new(),
        max_instances: 1,
    }
}

#[test]
fn primeira_leitura_semeia_e_a_segunda_devolve_o_mesmo_sem_emitir_evento() {
    let world = world();
    let project = world.project("alfa");

    let leader = Agent::ember();
    let first = world.get(project).unwrap();
    let second = world.get(project).unwrap();

    assert_eq!(
        layer_ids(&first),
        vec!["lideranca", "implementacao", "revisao"]
    );
    assert_eq!(first.teams.len(), 1);
    assert_eq!(
        placed(&first),
        sorted(vec![
            ("davi", "implementacao"),
            (leader.as_str(), "lideranca"),
            ("nina", "implementacao"),
            ("rui", "implementacao"),
            ("teo", "revisao"),
        ])
    );
    assert!(first.rules.is_empty());
    assert_eq!(first.settings, OrchestrationSettings::default());
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(&second).unwrap()
    );
    assert!(world.events().is_empty());
}

#[test]
fn projeto_com_configuracao_e_sem_camadas_nao_recebe_a_semente_de_volta() {
    let world = world();
    let project = world.project("alfa");
    world
        .db()
        .save_settings(project, &OrchestrationSettings::default())
        .unwrap();

    let snapshot = world.get(project).unwrap();

    assert!(snapshot.layers.is_empty());
    assert!(snapshot.teams.is_empty());
    assert!(snapshot.placements.is_empty());
}

#[test]
fn projeto_sem_camadas_apos_semear_aceita_criar_camada_nova_do_zero() {
    let world = world();
    let project = world.project("alfa");
    world
        .db()
        .save_settings(project, &OrchestrationSettings::default())
        .unwrap();

    let layer = world.save_layer(project, "Pesquisa", "pesquisar").unwrap();

    assert_eq!((layer.id.as_str(), layer.position), ("pesquisa", 0));
    assert_eq!(layer_ids(&world.get(project).unwrap()), vec!["pesquisa"]);
}

#[test]
fn cada_escrita_bem_sucedida_emite_um_unico_evento_com_o_projeto_certo() {
    let world = world();
    let project = world.project("alfa");
    let other = world.project("beta");

    world.save_layer(project, "Pesquisa", "pesquisar").unwrap();
    world.save_layer(other, "Pesquisa", "pesquisar").unwrap();
    world
        .save_rule(project, "lideranca", "pesquisa", "delegar")
        .unwrap();

    assert_eq!(
        world.events(),
        vec![changed(project), changed(other), changed(project)]
    );
}

#[test]
fn escrita_recusada_pelo_banco_devolve_a_frase_em_portugues_e_nao_emite_evento() {
    let world = world();
    let project = world.project("alfa");
    world
        .save_rule(project, "lideranca", "implementacao", "delegar")
        .unwrap();
    let before = world.events().len();

    let duplicated = world.save_rule(project, "lideranca", "implementacao", "delegar");
    let self_loop = world.save_rule(project, "revisao", "revisao", "delegar");
    let bad_kind = world.save_layer(project, "Nova", "magia");
    let missing = world.save_layer(999, "Nova", "livre");

    assert_eq!(
        duplicated.unwrap_err(),
        "Já existe uma seta de lideranca para implementacao."
    );
    assert_eq!(
        self_loop.unwrap_err(),
        "Uma camada não pode enviar trabalho para ela mesma."
    );
    assert_eq!(
        bad_kind.unwrap_err(),
        "O campo kind da orquestração não aceita o valor \"magia\"."
    );
    assert_eq!(missing.unwrap_err(), "O projeto 999 não existe.");
    assert_eq!(world.events().len(), before);
}

#[test]
fn comandos_de_escrita_recusam_faixas_invalidas_com_a_frase_do_banco() {
    let world = world();
    let project = world.project("alfa");
    world.get(project).unwrap();

    let too_many = place_agent(
        world.app.handle().clone(),
        world.app.state(),
        world.app.state(),
        project,
        PlacementDraft {
            max_instances: 5,
            ..orphan()
        },
    );
    let bad_limit = save_orchestration_settings(
        world.app.handle().clone(),
        world.app.state(),
        world.app.state(),
        project,
        OrchestrationSettings {
            max_parallel: 9,
            ..OrchestrationSettings::default()
        },
    );

    assert_eq!(
        too_many.unwrap_err(),
        "O campo max_instances da orquestração deve ficar entre 1 e 4."
    );
    assert_eq!(
        bad_limit.unwrap_err(),
        "O campo max_parallel da orquestração deve ficar entre 1 e 8."
    );
    assert_eq!(
        world.get(project).unwrap().settings,
        OrchestrationSettings::default()
    );
}

#[test]
fn leitura_de_projeto_inexistente_devolve_a_frase_e_nao_grava_semente() {
    let world = world();

    let error = world.get(999).unwrap_err();

    assert_eq!(error, "O projeto 999 não existe.");
    assert!(world.events().is_empty());
}

#[test]
fn posicao_de_agente_fora_do_elenco_some_da_leitura_e_e_apagada_na_escrita_seguinte() {
    let world = world();
    let project = world.project("alfa");
    world.get(project).unwrap();
    world.db().place_agent(project, &orphan()).unwrap();

    let read = world.get(project).unwrap();
    let still_stored = world.db().orchestration(project).unwrap();
    world.save_layer(project, "Pesquisa", "pesquisar").unwrap();
    let after_write = world.db().orchestration(project).unwrap();

    assert!(!placed(&read).iter().any(|(agent, _)| *agent == "fantasma"));
    assert!(placed(&still_stored)
        .iter()
        .any(|(agent, _)| *agent == "fantasma"));
    assert!(!placed(&after_write)
        .iter()
        .any(|(agent, _)| *agent == "fantasma"));
    assert_eq!(after_write.placements.len(), 5);
}

#[test]
fn agente_criado_depois_da_semente_nao_aparece_posicionado_na_leitura() {
    let world = world();
    let project = world.project("alfa");
    world.get(project).unwrap();

    world
        .app
        .state::<AgentRegistry>()
        .create(&crate::agents::AgentDraft {
            name: "Lia".to_string(),
            role: "Dev backend".to_string(),
            avatar: "vesper".to_string(),
            cli: "claude".to_string(),
            model: "sonnet".to_string(),
        })
        .unwrap();

    let snapshot = world.get(project).unwrap();
    assert_eq!(snapshot.placements.len(), 5);
    assert!(!placed(&snapshot).iter().any(|(agent, _)| *agent == "lia"));
}

#[test]
fn list_movements_devolve_os_20_mais_recentes_sem_semear_nem_emitir() {
    let world = world();
    let project = world.project("alfa");
    for number in 1..=25 {
        world
            .db()
            .record_handoff(
                project,
                &HandoffDraft {
                    run_id: None,
                    from_agent: "iara".to_string(),
                    to_agent: "davi".to_string(),
                    from_layer: None,
                    to_layer: None,
                    action: "delegar".to_string(),
                    kind: "implementar".to_string(),
                    item_id: Some(format!("item-{number}")),
                    return_count: 0,
                    message_id: None,
                },
            )
            .unwrap();
    }

    let moves = list_movements(world.app.state(), project).unwrap();

    assert_eq!(moves.len(), 20);
    assert_eq!(moves[0].item_id.as_deref(), Some("item-25"));
    assert_eq!(moves[19].item_id.as_deref(), Some("item-6"));
    assert!(world.db().orchestration(project).unwrap().layers.is_empty());
    assert!(world.events().is_empty());
    assert_eq!(
        list_movements(world.app.state(), 999).unwrap_err(),
        "O projeto 999 não existe."
    );
}

#[test]
fn elenco_que_nao_carrega_faz_o_comando_falhar_sem_semente_sem_poda_e_sem_evento() {
    let dir = std::env::temp_dir().join(format!(
        "vokia-orch-commands-test-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("agents.json"), "{ isto não é json").unwrap();
    let world = world_in(dir);
    let project = world.project("alfa");
    let expected = world
        .app
        .state::<AgentRegistry>()
        .team()
        .err()
        .unwrap()
        .to_string();

    let read = world.get(project);
    let write = world.save_layer(project, "Pesquisa", "pesquisar");

    assert_eq!(read.unwrap_err(), expected);
    assert_eq!(write.unwrap_err(), expected);
    let snapshot = world.db().orchestration(project).unwrap();
    assert!(snapshot.layers.is_empty() && snapshot.teams.is_empty());
    assert!(world.events().is_empty());
}
