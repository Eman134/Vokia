use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::*;
use crate::agents::{AgentDraft, AgentError, AgentRegistry};
use crate::db::NewProject;
use crate::db::{Db, DbError, OrchestrationSettings};

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        let dir = std::env::temp_dir().join(format!(
            "vokia-orch-seed-test-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn full_team(dir: &TempDir) -> Team {
    AgentRegistry::load(&dir.0)
        .team()
        .expect("o time padrão deveria carregar")
}

fn team_without(dir: &TempDir, removed: &str) -> Team {
    AgentRegistry::load(&dir.0).team().unwrap();
    let file = dir.0.join("agents.json");
    let mut parsed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    parsed["agents"]
        .as_array_mut()
        .unwrap()
        .retain(|agent| agent["id"] != removed);
    fs::write(&file, parsed.to_string()).unwrap();
    AgentRegistry::load(&dir.0)
        .team()
        .expect("o time editado deveria carregar")
}

fn draft(name: &str) -> AgentDraft {
    AgentDraft {
        name: name.to_string(),
        role: "Dev backend".to_string(),
        avatar: "vesper".to_string(),
        cli: "claude".to_string(),
        model: "sonnet".to_string(),
    }
}

fn project(db: &Db, name: &str) -> i64 {
    db.insert_project(&NewProject {
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

fn placement_layers(db: &Db, project: i64) -> Vec<(String, String)> {
    db.orchestration(project)
        .unwrap()
        .placements
        .into_iter()
        .map(|placement| (placement.agent_id, placement.layer_id))
        .collect()
}

fn pair(agent: &str, layer: &str) -> (String, String) {
    (agent.to_string(), layer.to_string())
}

fn table_rows(db: &Db, table: &str) -> i64 {
    db.conn()
        .unwrap()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

#[test]
fn semente_do_elenco_padrao_cria_tres_camadas_um_time_e_uma_posicao_por_agente() {
    let dir = TempDir::new();

    let seed = build(&full_team(&dir));

    let layers: Vec<(&str, &str, &str, &str)> = seed
        .layers
        .iter()
        .map(|l| {
            (
                l.name.as_str(),
                l.description.as_str(),
                l.kind.as_str(),
                l.instructions.as_str(),
            )
        })
        .collect();
    assert_eq!(
        layers,
        vec![
            ("Liderança", "planeja e delega", "planejar", ""),
            ("Implementação", "escreve o código", "implementar", ""),
            ("Revisão", "testa e aprova", "revisar", ""),
        ]
    );
    assert_eq!((seed.team.id, seed.team.name.as_str()), (None, "Time"));
    assert_eq!(seed.settings, OrchestrationSettings::default());
    let placed: Vec<(&str, &str, &str, i64)> = seed
        .placements
        .iter()
        .map(|p| {
            (
                p.agent_id.as_str(),
                p.team_id.as_str(),
                p.layer_id.as_str(),
                p.max_instances,
            )
        })
        .collect();
    assert_eq!(
        placed,
        vec![
            ("iara", "time", "lideranca", 1),
            ("davi", "time", "implementacao", 1),
            ("nina", "time", "implementacao", 1),
            ("teo", "time", "revisao", 1),
            ("rui", "time", "implementacao", 1),
        ]
    );
}

#[test]
fn semente_sem_teo_no_elenco_nao_posiciona_teo_e_deixa_revisao_sem_agente() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let project = project(&db, "alfa");

    db.seed_orchestration(project, &build(&team_without(&dir, "teo")))
        .unwrap();

    assert_eq!(
        placement_layers(&db, project),
        vec![
            pair("davi", "implementacao"),
            pair("iara", "lideranca"),
            pair("nina", "implementacao"),
            pair("rui", "implementacao"),
        ]
    );
    assert_eq!(db.orchestration(project).unwrap().layers.len(), 3);
}

#[test]
fn semente_gravada_duas_vezes_no_mesmo_projeto_nao_duplica_nada() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let project = project(&db, "alfa");
    let seed = build(&full_team(&dir));

    let first = db.seed_orchestration(project, &seed).unwrap();
    let second = db.seed_orchestration(project, &seed).unwrap();

    let snapshot = db.orchestration(project).unwrap();
    assert_eq!((first, second), (true, false));
    assert_eq!(
        (
            snapshot.layers.len(),
            snapshot.teams.len(),
            snapshot.placements.len(),
            snapshot.rules.len()
        ),
        (3, 1, 5, 0)
    );
}

#[test]
fn semente_nao_roda_em_projeto_que_ja_tem_linha_de_configuracao() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let project = project(&db, "alfa");
    let custom = OrchestrationSettings {
        enforcement: "on".to_string(),
        ..OrchestrationSettings::default()
    };
    db.save_settings(project, &custom).unwrap();

    let wrote = db
        .seed_orchestration(project, &build(&full_team(&dir)))
        .unwrap();

    let snapshot = db.orchestration(project).unwrap();
    assert!(!wrote);
    assert_eq!(snapshot.settings, custom);
    assert!(
        snapshot.layers.is_empty() && snapshot.teams.is_empty() && snapshot.placements.is_empty()
    );
}

#[test]
fn duas_threads_semeando_o_mesmo_projeto_gravam_uma_unica_vez() {
    let dir = TempDir::new();
    let db = Arc::new(Db::open(&dir.0).unwrap());
    let project = project(&db, "alfa");
    let seed = Arc::new(build(&full_team(&dir)));

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let (db, seed) = (db.clone(), seed.clone());
            std::thread::spawn(move || db.seed_orchestration(project, &seed).unwrap())
        })
        .collect();
    let mut wrote: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    wrote.sort();

    assert_eq!(wrote, vec![false, true]);
    assert_eq!(db.orchestration(project).unwrap().layers.len(), 3);
    assert_eq!(table_rows(&db, "orch_placements"), 5);
}

#[test]
fn semente_em_projeto_inexistente_falha_e_nao_grava_nada() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let seed = build(&full_team(&dir));

    let error = db.seed_orchestration(999, &seed).unwrap_err();

    assert!(matches!(error, DbError::ProjectNotFound(999)));
    assert_eq!(error.to_string(), "O projeto 999 não existe.");
    for table in [
        "orch_settings",
        "orch_layers",
        "orch_teams",
        "orch_placements",
    ] {
        assert_eq!(table_rows(&db, table), 0, "{table}");
    }
}

#[test]
fn semente_que_falha_no_meio_desfaz_tudo_inclusive_a_linha_de_configuracao() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let project = project(&db, "alfa");
    let mut seed = build(&full_team(&dir));
    seed.placements[0].layer_id = "inexistente".to_string();

    let error = db.seed_orchestration(project, &seed).unwrap_err();

    assert_eq!(
        error.to_string(),
        "A camada inexistente não existe neste projeto."
    );
    for table in [
        "orch_settings",
        "orch_layers",
        "orch_teams",
        "orch_placements",
    ] {
        assert_eq!(table_rows(&db, table), 0, "{table}");
    }
    assert!(db
        .seed_orchestration(project, &build(&full_team(&dir)))
        .unwrap());
}

#[test]
fn agente_novo_criado_depois_da_semente_fica_sem_posicao_ate_alguem_posicionar() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let project = project(&db, "alfa");
    let registry = AgentRegistry::load(&dir.0);
    db.seed_orchestration(project, &build(&registry.team().unwrap()))
        .unwrap();

    registry.create(&draft("Lia")).unwrap();

    let ids: Vec<String> = placement_layers(&db, project)
        .into_iter()
        .map(|(agent, _)| agent)
        .collect();
    assert!(registry
        .team()
        .unwrap()
        .ids()
        .any(|id| id.as_str() == "lia"));
    assert!(!ids.contains(&"lia".to_string()));
    assert_eq!(ids.len(), 5);
}

#[test]
fn registro_recusa_nome_com_espaco_ou_hifen_que_colidiria_com_id_de_replica() {
    let dir = TempDir::new();
    let registry = AgentRegistry::load(&dir.0);

    for name in ["Davi 2", "Davi-2", "Lia-3"] {
        let error = registry.create(&draft(name)).unwrap_err();
        assert!(matches!(error, AgentError::InvalidName), "{name}");
    }
    assert_eq!(registry.team().unwrap().ids().count(), 5);
}
