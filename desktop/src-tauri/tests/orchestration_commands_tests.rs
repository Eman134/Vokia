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
        "vokia-orch-commands-test-{}-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::SeqCst),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |time| time.as_nanos())
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

    fn get(&self) -> Result<OrchestrationSnapshot, String> {
        get_orchestration(self.app.state(), self.app.state())
    }

    fn save_layer(&self, name: &str, kind: &str) -> Result<Layer, String> {
        save_layer(
            self.app.handle().clone(),
            self.app.state(),
            self.app.state(),
            LayerDraft {
                id: None,
                name: name.to_string(),
                description: String::new(),
                kind: kind.to_string(),
                instructions: String::new(),
            },
        )
    }
}

fn changed() -> Value {
    json!({ "type": "orchestration-changed" })
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
        pos_x: None,
        pos_y: None,
        criteria: String::new(),
        max_worktrees: None,
    }
}

#[test]
fn primeira_leitura_semeia_e_a_segunda_devolve_o_mesmo_sem_emitir_evento() {
    let world = world();

    let leader = Agent::axiom();
    let first = world.get().unwrap();
    let second = world.get().unwrap();

    assert_eq!(
        layer_ids(&first),
        vec!["lideranca", "implementacao", "revisao"]
    );
    assert_eq!(first.teams.len(), 1);
    assert_eq!(
        placed(&first),
        sorted(vec![
            ("vexa", "implementacao"),
            (leader.as_str(), "lideranca"),
            ("kairo", "implementacao"),
            ("oracle", "implementacao"),
            ("sentry", "revisao"),
        ])
    );
    assert!(first.edges.is_empty());
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
    world
        .db()
        .save_settings(&OrchestrationSettings::default())
        .unwrap();

    let snapshot = world.get().unwrap();

    assert!(snapshot.layers.is_empty());
    assert!(snapshot.teams.is_empty());
    assert!(snapshot.placements.is_empty());
}

#[test]
fn projeto_sem_camadas_apos_semear_aceita_criar_camada_nova_do_zero() {
    let world = world();
    world
        .db()
        .save_settings(&OrchestrationSettings::default())
        .unwrap();

    let layer = world.save_layer("Pesquisa", "pesquisar").unwrap();

    assert_eq!((layer.id.as_str(), layer.position), ("pesquisa", 0));
    assert_eq!(layer_ids(&world.get().unwrap()), vec!["pesquisa"]);
}

#[test]
fn cada_escrita_bem_sucedida_emite_um_unico_evento_sem_projeto() {
    let world = world();

    world.save_layer("Pesquisa", "pesquisar").unwrap();
    world.save_layer("Extra", "livre").unwrap();
    world.save_layer("Terceira", "livre").unwrap();

    assert_eq!(world.events(), vec![changed(), changed(), changed()]);
}

#[test]
fn escrita_recusada_pelo_banco_devolve_a_frase_em_portugues_e_nao_emite_evento() {
    let world = world();
    world.save_layer("Pesquisa", "pesquisar").unwrap();
    let before = world.events().len();

    let bad_kind = world.save_layer("Nova", "magia");

    assert_eq!(
        bad_kind.unwrap_err(),
        "O campo kind da orquestração não aceita o valor \"magia\"."
    );
    assert_eq!(world.events().len(), before);
}

#[test]
fn comandos_de_escrita_recusam_faixas_invalidas_com_a_frase_do_banco() {
    let world = world();
    world.get().unwrap();

    let too_many = place_agent(
        world.app.handle().clone(),
        world.app.state(),
        world.app.state(),
        PlacementDraft {
            max_instances: 5,
            ..orphan()
        },
    );
    let bad_limit = save_orchestration_settings(
        world.app.handle().clone(),
        world.app.state(),
        world.app.state(),
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
        world.get().unwrap().settings,
        OrchestrationSettings::default()
    );
}

#[test]
fn leitura_sem_nenhum_projeto_cadastrado_semeia_e_devolve_a_orquestracao_global() {
    let world = world();

    let snapshot = world.get().unwrap();

    assert_eq!(snapshot.placements.len(), 5);
    assert!(world.events().is_empty());
}

#[test]
fn posicao_de_agente_fora_do_elenco_some_da_leitura_e_e_apagada_na_escrita_seguinte() {
    let world = world();
    world.get().unwrap();
    world.db().place_agent(&orphan()).unwrap();

    let read = world.get().unwrap();
    let still_stored = world.db().orchestration().unwrap();
    world.save_layer("Pesquisa", "pesquisar").unwrap();
    let after_write = world.db().orchestration().unwrap();

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
    world.get().unwrap();

    world
        .app
        .state::<AgentRegistry>()
        .create(
            &crate::agents::AgentDraft {
                name: "Lia".to_string(),
                role: "Backend Developer".to_string(),
                avatar: "onix".to_string(),
                cli: "claude".to_string(),
                model: "sonnet".to_string(),
                model_tier: None,
                reasoning_effort: None,
                command: "claude".to_string(),
                follow_default_llm: false,
                fallback: Vec::new(),
            },
            &crate::providers::ProviderRegistry::seeded(),
        )
        .unwrap();

    let snapshot = world.get().unwrap();
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
                    from_agent: "axiom".to_string(),
                    to_agent: "vexa".to_string(),
                    from_layer: None,
                    to_layer: None,
                    action: "delegar".to_string(),
                    kind: "implementar".to_string(),
                    item_id: Some(format!("item-{number}")),
                    return_count: 0,
                    message_id: None,
                    event: None,
                    edge_id: None,
                },
            )
            .unwrap();
    }

    let moves = read_movements(world.db(), project).unwrap();

    assert_eq!(moves.len(), 20);
    assert_eq!(moves[0].item_id.as_deref(), Some("item-25"));
    assert_eq!(moves[19].item_id.as_deref(), Some("item-6"));
    assert!(world.db().orchestration().unwrap().layers.is_empty());
    assert!(world.events().is_empty());
    assert_eq!(
        read_movements(world.db(), 999).unwrap_err(),
        "O projeto 999 não existe."
    );
}

#[test]
fn elenco_que_nao_carrega_faz_o_comando_falhar_sem_semente_sem_poda_e_sem_evento() {
    let dir = std::env::temp_dir().join(format!(
        "vokia-orch-commands-test-{}-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::SeqCst),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |time| time.as_nanos())
    ));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("agents.json"), "{ isto não é json").unwrap();
    let world = world_in(dir);
    let expected = world
        .app
        .state::<AgentRegistry>()
        .team()
        .err()
        .unwrap()
        .to_string();

    let read = world.get();
    let write = world.save_layer("Pesquisa", "pesquisar");

    assert_eq!(read.unwrap_err(), expected);
    assert_eq!(write.unwrap_err(), expected);
    let snapshot = world.db().orchestration().unwrap();
    assert!(snapshot.layers.is_empty() && snapshot.teams.is_empty());
    assert!(world.events().is_empty());
}
