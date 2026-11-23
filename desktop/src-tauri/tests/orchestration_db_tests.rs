use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::db::backlog::NewCard;
use crate::db::projects::NewProject;

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

fn temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "vokia-orchestration-test-{}-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::SeqCst),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |time| time.as_nanos())
    ))
}

struct TempDb {
    dir: PathBuf,
    db: Db,
}

impl TempDb {
    fn open() -> TempDb {
        let dir = temp_dir();
        let db = Db::open(&dir).expect("o banco de teste deveria abrir");
        TempDb { dir, db }
    }

    fn project(&self, name: &str) -> i64 {
        self.db
            .insert_project(&NewProject {
                name,
                symbol: "folder",
                description: "",
                path: &format!("C:/projetos/{name}"),
                origin_kind: "folder",
                origin_repo: None,
            })
            .expect("o projeto deveria ser criado")
            .id
    }

    fn layer(&self, name: &str) -> Layer {
        self.db
            .save_layer(&layer_draft(name))
            .expect("a camada deveria ser salva")
    }

    fn team(&self, name: &str) -> Team {
        self.db
            .save_team(&TeamDraft {
                id: None,
                name: name.to_string(),
                instructions: None,
            })
            .expect("o time deveria ser salvo")
    }

    fn place(&self, agent: &str, team: &str, layer: &str) -> Placement {
        self.db
            .place_agent(&placement_draft(agent, team, layer))
            .expect("o agente deveria ser posicionado")
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn layer_draft(name: &str) -> LayerDraft {
    LayerDraft {
        id: None,
        name: name.to_string(),
        description: String::new(),
        kind: "implementar".to_string(),
        instructions: String::new(),
    }
}

fn placement_draft(agent: &str, team: &str, layer: &str) -> PlacementDraft {
    PlacementDraft {
        agent_id: agent.to_string(),
        team_id: team.to_string(),
        layer_id: layer.to_string(),
        instructions: String::new(),
        max_instances: 1,
        pos_x: None,
        pos_y: None,
        criteria: String::new(),
        max_worktrees: None,
    }
}

fn handoff_draft(item: &str, action: &str) -> HandoffDraft {
    HandoffDraft {
        run_id: Some(7),
        from_agent: "axiom".to_string(),
        to_agent: "vexa".to_string(),
        from_layer: Some("lideranca".to_string()),
        to_layer: None,
        action: action.to_string(),
        kind: "implementar".to_string(),
        item_id: Some(item.to_string()),
        return_count: 0,
        message_id: None,
        event: None,
        edge_id: None,
    }
}

fn layer_order(db: &Db) -> Vec<String> {
    db.orchestration()
        .unwrap()
        .layers
        .into_iter()
        .map(|layer| layer.id)
        .collect()
}

fn message_of<T>(result: Result<T, DbError>) -> String {
    match result {
        Ok(_) => panic!("esperava erro, mas a operação passou"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn projeto_novo_le_o_snapshot_com_os_valores_iniciais_e_sem_nada_cadastrado() {
    let t = TempDb::open();

    let snapshot = t.db.orchestration().unwrap();

    assert_eq!(
        snapshot.settings,
        OrchestrationSettings {
            enforcement: "off".to_string(),
            leader_agent: crate::agents::Agent::axiom().as_str().to_string(),
            step_mode: "lotes".to_string(),
            human_gate: "lote".to_string(),
            max_parallel: 4,
            pair_turn_limit: 8,
            task_turn_limit: 60,
            worktree_policy: "paralelo".to_string(),
        }
    );
    assert!(snapshot.layers.is_empty());
    assert!(snapshot.teams.is_empty());
    assert!(snapshot.placements.is_empty());
    assert!(snapshot.edges.is_empty());
}

#[test]
fn projeto_inexistente_e_recusado_nos_movimentos_que_sao_por_projeto() {
    let t = TempDb::open();
    let missing = 999;
    let expected = "O projeto 999 não existe.";

    assert_eq!(message_of(t.db.list_movements(missing, 5)), expected);
    assert_eq!(
        message_of(t.db.record_handoff(missing, &handoff_draft("item-1", "delegar"))),
        expected
    );
}

#[test]
fn nomes_que_dao_o_mesmo_slug_nao_criam_duas_camadas() {
    let t = TempDb::open();

    let first = t.layer("Revisão");
    let error = message_of(t.db.save_layer(&layer_draft("revisao")));

    assert_eq!(first.id, "revisao");
    assert_eq!(
        error,
        "Já existe uma camada com o identificador revisao neste projeto."
    );
    assert_eq!(layer_order(&t.db), vec!["revisao"]);
}

#[test]
fn nomes_que_dao_o_mesmo_slug_nao_criam_dois_times() {
    let t = TempDb::open();

    t.team("Qualidade Técnica");
    let error = message_of(t.db.save_team(&TeamDraft {
        id: None,
        name: "qualidade  tecnica".to_string(),
        instructions: None,
    }));

    assert_eq!(
        error,
        "Já existe um time com o identificador qualidade-tecnica neste projeto."
    );
}

#[test]
fn camada_chamada_voce_e_recusada_porque_voce_e_o_no_virtual() {
    let t = TempDb::open();

    let error = message_of(t.db.save_layer(&layer_draft("Você")));

    assert_eq!(
        error,
        "\"Você\" é o nó Você e não pode ser o nome de uma camada."
    );
}

#[test]
fn camada_nova_vai_para_o_fim_e_editar_por_id_preserva_posicao_e_id() {
    let t = TempDb::open();
    t.layer("Liderança");
    t.layer("Implementação");

    let edited =
        t.db.save_layer(&LayerDraft {
            id: Some("lideranca".to_string()),
            name: "Liderança do time".to_string(),
            description: "conduz".to_string(),
            kind: "planejar".to_string(),
            instructions: "planeje".to_string(),
        })
        .unwrap();

    assert_eq!(
        (edited.id.as_str(), edited.position, edited.kind.as_str()),
        ("lideranca", 0, "planejar")
    );
    assert_eq!(layer_order(&t.db), vec!["lideranca", "implementacao"]);
    assert_eq!(
        message_of(t.db.save_layer(&LayerDraft {
            id: Some("nao-existe".to_string()),
            ..layer_draft("X")
        })),
        "A camada nao-existe não existe neste projeto."
    );
}

#[test]
fn instrucoes_de_4001_caracteres_sao_recusadas_e_4000_passam_na_camada_e_no_agente() {
    let t = TempDb::open();
    let team = t.team("Time");
    let layer = t.layer("Implementação");
    let at_limit = "é".repeat(4000);
    let over_limit = "é".repeat(4001);
    let expected = "As instruções aceitam até 4000 caracteres.";

    let saved =
        t.db.save_layer(&LayerDraft {
            instructions: at_limit.clone(),
            ..layer_draft("Revisão")
        })
        .unwrap();
    let placed =
        t.db.place_agent(&PlacementDraft {
            instructions: at_limit.clone(),
            ..placement_draft("vexa", &team.id, &layer.id)
        })
        .unwrap();

    assert_eq!(saved.instructions, at_limit);
    assert_eq!(placed.instructions, at_limit);
    assert_eq!(
        message_of(t.db.save_layer(&LayerDraft {
            instructions: over_limit.clone(),
            ..layer_draft("Pesquisa")
        })),
        expected
    );
    assert_eq!(
        message_of(t.db.place_agent(&PlacementDraft {
            instructions: over_limit,
            ..placement_draft("oracle", &team.id, &layer.id)
        })),
        expected
    );
    assert_eq!(layer_order(&t.db), vec!["implementacao", "revisao"]);
}

#[test]
fn max_instances_0_e_5_sao_recusados_e_1_e_4_passam() {
    let t = TempDb::open();
    let team = t.team("Time");
    let layer = t.layer("Implementação");
    let expected = "O campo max_instances da orquestração deve ficar entre 1 e 4.";

    for instances in [0, 5] {
        let draft = PlacementDraft {
            max_instances: instances,
            ..placement_draft("vexa", &team.id, &layer.id)
        };
        assert_eq!(
            message_of(t.db.place_agent(&draft)),
            expected,
            "com {instances}"
        );
    }
    for instances in [1, 4] {
        let draft = PlacementDraft {
            max_instances: instances,
            ..placement_draft("vexa", &team.id, &layer.id)
        };
        assert_eq!(t.db.place_agent(&draft).unwrap().max_instances, instances);
    }
    assert_eq!(t.db.orchestration().unwrap().placements.len(), 1);
}

#[test]
fn agente_posicionado_duas_vezes_continua_com_uma_so_posicao_a_ultima() {
    let t = TempDb::open();
    let web = t.team("Web");
    let api = t.team("API");
    let build = t.layer("Implementação");
    let review = t.layer("Revisão");

    t.place("vexa", &web.id, &build.id);
    let moved = t.place("vexa", &api.id, &review.id);

    let placements = t.db.orchestration().unwrap().placements;
    assert_eq!(placements.len(), 1);
    assert_eq!(
        (
            placements[0].agent_id.as_str(),
            placements[0].team_id.as_str(),
            placements[0].layer_id.as_str()
        ),
        ("vexa", "api", "revisao")
    );
    assert_eq!(moved.layer_id, "revisao");
}

#[test]
fn agente_so_e_posicionado_em_time_e_camada_que_existem() {
    let t = TempDb::open();
    let team = t.team("Time");
    let layer = t.layer("Implementação");

    assert_eq!(
        message_of(t.db.place_agent(&placement_draft("vexa", "fantasma", &layer.id))),
        "O time fantasma não existe neste projeto."
    );
    assert_eq!(
        message_of(t.db.place_agent(&placement_draft("vexa", &team.id, "fantasma"))),
        "A camada fantasma não existe neste projeto."
    );
    assert!(t.db.orchestration().unwrap().placements.is_empty());
}

#[test]
fn apagar_camada_com_agente_posicionado_e_recusado_e_nada_muda() {
    let t = TempDb::open();
    let team = t.team("Time");
    t.layer("Liderança");
    t.layer("Implementação");
    t.place("vexa", &team.id, "implementacao");

    let error = message_of(t.db.delete_layer("implementacao"));

    assert_eq!(
        error,
        "A camada implementacao tem agentes posicionados: mova os agentes antes de excluí-la."
    );
    let snapshot = t.db.orchestration().unwrap();
    assert_eq!(snapshot.layers.len(), 2);
}

#[test]
fn apagar_time_com_agente_passa_os_cards_ao_sucessor_e_apaga_as_posicoes() {
    let t = TempDb::open();
    let project = t.project("alfa");
    let team = t.team("Time");
    let layer = t.layer("Implementação");
    t.place("vexa", &team.id, &layer.id);
    let card =
        t.db.create_card(
            project,
            &NewCard {
                title: "Ajustar login".to_string(),
                description: None,
                origin: "usuario".to_string(),
                links: Vec::new(),
            },
        )
        .unwrap();
    t.db.conn()
        .unwrap()
        .execute(
            "UPDATE backlog_items SET who = 'vexa' WHERE id = ?1",
            [&card.id],
        )
        .unwrap();

    let notes =
        t.db.delete_team(
            &team.id,
            &Agent::ember(),
            "Ember",
            |_| "Vexa".to_string(),
            1,
        )
        .unwrap();

    let owner: String =
        t.db.conn()
            .unwrap()
            .query_row(
                "SELECT who FROM backlog_items WHERE id = ?1",
                [&card.id],
                |row| row.get(0),
            )
            .unwrap();
    assert_eq!(owner, "ember");
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].project_id, project);
    assert_eq!(
        notes[0].text,
        format!("Ember assumiu o card {} de Vexa", card.id)
    );
    let snapshot = t.db.orchestration().unwrap();
    assert!(snapshot.placements.is_empty());
    assert!(snapshot.teams.is_empty());
    assert_eq!(layer_order(&t.db), vec!["implementacao"]);
}

#[test]
fn apagar_time_inexistente_e_recusado() {
    let t = TempDb::open();

    assert_eq!(
        message_of(t.db.delete_team("fantasma", &Agent::ember(), "Ember", |_| String::new(), 1)),
        "O time fantasma não existe neste projeto."
    );
}

#[test]
fn reordenar_camadas_grava_a_nova_ordem() {
    let t = TempDb::open();
    t.layer("Liderança");
    t.layer("Implementação");
    t.layer("Revisão");

    t.db.reorder_layers(&[
        "implementacao".to_string(),
        "revisao".to_string(),
        "lideranca".to_string(),
    ])
    .unwrap();

    assert_eq!(
        layer_order(&t.db),
        vec!["implementacao", "revisao", "lideranca"]
    );
}

#[test]
fn reordenar_exige_cada_camada_do_projeto_uma_unica_vez() {
    let t = TempDb::open();
    t.layer("Liderança");
    t.layer("Implementação");
    let expected = "A nova ordem precisa listar cada camada do projeto uma única vez.";

    let missing = message_of(t.db.reorder_layers(&["lideranca".to_string()]));
    let repeated =
        message_of(t.db.reorder_layers(&["lideranca".to_string(), "lideranca".to_string()]));
    let unknown =
        message_of(t.db.reorder_layers(&["lideranca".to_string(), "fantasma".to_string()]));

    assert_eq!(
        (missing.as_str(), repeated.as_str(), unknown.as_str()),
        (expected, expected, expected)
    );
    assert_eq!(layer_order(&t.db), vec!["lideranca", "implementacao"]);
}

#[test]
fn list_movements_com_limit_menor_que_1_e_recusado() {
    let t = TempDb::open();
    let project = t.project("alfa");

    assert_eq!(
        message_of(t.db.list_movements(project, 0)),
        "O campo limit da orquestração não aceita o valor \"0\"."
    );
    assert_eq!(
        message_of(t.db.list_movements(project, -3)),
        "O campo limit da orquestração não aceita o valor \"-3\"."
    );
}

#[test]
fn list_movements_devolve_os_mais_recentes_primeiro_respeitando_o_limite_e_o_projeto() {
    let t = TempDb::open();
    let project = t.project("alfa");
    let other = t.project("beta");
    for item in ["item-1", "item-2", "item-3"] {
        t.db.record_handoff(project, &handoff_draft(item, "delegar"))
            .unwrap();
    }
    t.db.record_handoff(other, &handoff_draft("item-9", "delegar"))
        .unwrap();

    let latest_two = t.db.list_movements(project, 2).unwrap();
    let all = t.db.list_movements(project, 50).unwrap();

    let items = |moves: &[Handoff]| -> Vec<Option<String>> {
        moves.iter().map(|m| m.item_id.clone()).collect()
    };
    assert_eq!(
        items(&latest_two),
        vec![Some("item-3".to_string()), Some("item-2".to_string())]
    );
    assert_eq!(all.len(), 3);
    assert_eq!(all[2].item_id.as_deref(), Some("item-1"));
    assert_eq!(
        (
            all[0].from_agent.as_str(),
            all[0].to_agent.as_str(),
            all[0].run_id
        ),
        ("axiom", "vexa", Some(7))
    );
}

#[test]
fn registro_de_movimento_recusa_acao_tipo_e_contagem_invalidos() {
    let t = TempDb::open();
    let project = t.project("alfa");

    assert_eq!(
        message_of(t.db.record_handoff(project, &handoff_draft("item-1", "mandar"))),
        "O campo action da orquestração não aceita o valor \"mandar\"."
    );
    assert_eq!(
        message_of(t.db.record_handoff(
            project,
            &HandoffDraft {
                kind: "livre".to_string(),
                ..handoff_draft("item-1", "delegar")
            }
        )),
        "O campo kind da orquestração não aceita o valor \"livre\"."
    );
    assert_eq!(
        message_of(t.db.record_handoff(
            project,
            &HandoffDraft {
                return_count: -1,
                ..handoff_draft("item-1", "devolver")
            }
        )),
        "O campo return_count da orquestração deve ficar entre 0 e 9223372036854775807."
    );
    assert!(t.db.list_movements(project, 10).unwrap().is_empty());
}

#[test]
fn configuracao_fora_das_faixas_e_recusada_e_dentro_das_faixas_e_gravada() {
    let t = TempDb::open();
    let base = OrchestrationSettings::default();
    let out_of_range = [
        (
            OrchestrationSettings {
                max_parallel: 0,
                ..base.clone()
            },
            "max_parallel",
            1,
            8,
        ),
        (
            OrchestrationSettings {
                max_parallel: 9,
                ..base.clone()
            },
            "max_parallel",
            1,
            8,
        ),
        (
            OrchestrationSettings {
                pair_turn_limit: 0,
                ..base.clone()
            },
            "pair_turn_limit",
            1,
            50,
        ),
        (
            OrchestrationSettings {
                pair_turn_limit: 51,
                ..base.clone()
            },
            "pair_turn_limit",
            1,
            50,
        ),
        (
            OrchestrationSettings {
                task_turn_limit: 4,
                ..base.clone()
            },
            "task_turn_limit",
            5,
            200,
        ),
        (
            OrchestrationSettings {
                task_turn_limit: 201,
                ..base.clone()
            },
            "task_turn_limit",
            5,
            200,
        ),
    ];

    for (settings, field, min, max) in out_of_range {
        assert_eq!(
            message_of(t.db.save_settings(&settings)),
            format!("O campo {field} da orquestração deve ficar entre {min} e {max}.")
        );
    }
    assert_eq!(
        message_of(t.db.save_settings(&OrchestrationSettings {
            human_gate: "talvez".to_string(),
            ..base.clone()
        })),
        "O campo human_gate da orquestração não aceita o valor \"talvez\"."
    );
    assert_eq!(t.db.orchestration().unwrap().settings, base);

    let edge = OrchestrationSettings {
        enforcement: "on".to_string(),
        leader_agent: "oracle".to_string(),
        step_mode: "livre".to_string(),
        human_gate: "nunca".to_string(),
        max_parallel: 8,
        pair_turn_limit: 1,
        task_turn_limit: 200,
        worktree_policy: "sempre".to_string(),
    };
    t.db.save_settings(&edge).unwrap();
    t.db.save_settings(&edge).unwrap();
    assert_eq!(t.db.orchestration().unwrap().settings, edge);
}

#[test]
fn orquestracao_e_a_mesma_nos_dois_projetos_e_os_movimentos_ficam_separados() {
    let t = TempDb::open();
    let alfa = t.project("alfa");
    let beta = t.project("beta");
    t.layer("Implementação");
    t.team("Time");
    t.db.record_handoff(alfa, &handoff_draft("item-1", "delegar"))
        .unwrap();
    t.db.record_handoff(beta, &handoff_draft("item-2", "delegar"))
        .unwrap();

    let snapshot = t.db.orchestration().unwrap();
    let items_of = |project: i64| -> Vec<Option<String>> {
        t.db.list_movements(project, 10)
            .unwrap()
            .into_iter()
            .map(|movement| movement.item_id)
            .collect()
    };

    assert_eq!(layer_order(&t.db), vec!["implementacao"]);
    assert_eq!(snapshot.teams.len(), 1);
    assert_eq!(items_of(alfa), vec![Some("item-1".to_string())]);
    assert_eq!(items_of(beta), vec![Some("item-2".to_string())]);
}

#[test]
fn apagar_o_projeto_apaga_movimentos_e_pedidos_dele_e_mantem_a_orquestracao() {
    let t = TempDb::open();
    let project = t.project("alfa");
    let other = t.project("beta");
    let team = t.team("Time");
    t.layer("Liderança");
    t.layer("Implementação");
    t.place("vexa", &team.id, "implementacao");
    for id in [project, other] {
        t.db.record_handoff(id, &handoff_draft("item-1", "delegar"))
            .unwrap();
        t.db.start_run(id, "vexa", "implementar", None, None, 1)
            .unwrap();
    }

    t.db.delete_project(project).unwrap();

    let count = |table: &str, project_id: i64| -> i64 {
        t.db.conn()
            .unwrap()
            .query_row(
                &format!("SELECT COUNT(*) FROM {table} WHERE project_id = ?1"),
                [project_id],
                |row| row.get(0),
            )
            .unwrap()
    };
    assert_eq!(count("orch_handoffs", project), 0);
    assert_eq!(count("orch_runs", project), 0);
    assert_eq!(count("orch_handoffs", other), 1);
    assert_eq!(count("orch_runs", other), 1);
    let snapshot = t.db.orchestration().unwrap();
    assert_eq!(layer_order(&t.db), vec!["lideranca", "implementacao"]);
    assert_eq!((snapshot.teams.len(), snapshot.placements.len()), (1, 1));
}
