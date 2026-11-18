use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::db::projects::NewProject;

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

fn temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "vokia-orchestration-test-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::SeqCst)
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

    fn layer(&self, project: i64, name: &str) -> Layer {
        self.db
            .save_layer(project, &layer_draft(name))
            .expect("a camada deveria ser salva")
    }

    fn team(&self, project: i64, name: &str) -> Team {
        self.db
            .save_team(
                project,
                &TeamDraft {
                    id: None,
                    name: name.to_string(),
                },
            )
            .expect("o time deveria ser salvo")
    }

    fn rule(&self, project: i64, from: &str, to: &str, action: &str) -> Rule {
        self.db
            .save_rule(project, &rule_draft(from, to, action))
            .expect("a seta deveria ser salva")
    }

    fn place(&self, project: i64, agent: &str, team: &str, layer: &str) -> Placement {
        self.db
            .place_agent(project, &placement_draft(agent, team, layer))
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

fn rule_draft(from: &str, to: &str, action: &str) -> RuleDraft {
    RuleDraft {
        id: None,
        from_layer: from.to_string(),
        to_layer: to.to_string(),
        when_event: "tarefa_concluida".to_string(),
        action: action.to_string(),
        kind: "revisar".to_string(),
        exec_mode: "sequencia".to_string(),
        return_limit: 2,
    }
}

fn placement_draft(agent: &str, team: &str, layer: &str) -> PlacementDraft {
    PlacementDraft {
        agent_id: agent.to_string(),
        team_id: team.to_string(),
        layer_id: layer.to_string(),
        instructions: String::new(),
        max_instances: 1,
    }
}

fn handoff_draft(item: &str, action: &str) -> HandoffDraft {
    HandoffDraft {
        run_id: Some(7),
        from_agent: "iara".to_string(),
        to_agent: "davi".to_string(),
        from_layer: Some("lideranca".to_string()),
        to_layer: None,
        action: action.to_string(),
        kind: "implementar".to_string(),
        item_id: Some(item.to_string()),
        return_count: 0,
        message_id: None,
        event: None,
    }
}

fn layer_order(db: &Db, project: i64) -> Vec<String> {
    db.orchestration(project)
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
    let project = t.project("alfa");

    let snapshot = t.db.orchestration(project).unwrap();

    assert_eq!(
        snapshot.settings,
        OrchestrationSettings {
            enforcement: "off".to_string(),
            leader_agent: crate::agents::Agent::ember().as_str().to_string(),
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
    assert!(snapshot.rules.is_empty());
}

#[test]
fn projeto_inexistente_e_recusado_em_toda_leitura_e_escrita() {
    let t = TempDb::open();
    let missing = 999;
    let expected = "O projeto 999 não existe.";

    assert_eq!(message_of(t.db.orchestration(missing)), expected);
    assert_eq!(
        message_of(t.db.save_settings(missing, &OrchestrationSettings::default())),
        expected
    );
    assert_eq!(
        message_of(t.db.save_layer(missing, &layer_draft("Revisão"))),
        expected
    );
    assert_eq!(
        message_of(t.db.save_team(
            missing,
            &TeamDraft {
                id: None,
                name: "Time".into()
            }
        )),
        expected
    );
    assert_eq!(message_of(t.db.delete_layer(missing, "revisao")), expected);
    assert_eq!(message_of(t.db.list_movements(missing, 5)), expected);
    assert_eq!(
        message_of(t.db.record_handoff(missing, &handoff_draft("item-1", "delegar"))),
        expected
    );
}

#[test]
fn nomes_que_dao_o_mesmo_slug_nao_criam_duas_camadas() {
    let t = TempDb::open();
    let project = t.project("alfa");

    let first = t.layer(project, "Revisão");
    let error = message_of(t.db.save_layer(project, &layer_draft("revisao")));

    assert_eq!(first.id, "revisao");
    assert_eq!(
        error,
        "Já existe uma camada com o identificador revisao neste projeto."
    );
    assert_eq!(layer_order(&t.db, project), vec!["revisao"]);
}

#[test]
fn nomes_que_dao_o_mesmo_slug_nao_criam_dois_times() {
    let t = TempDb::open();
    let project = t.project("alfa");

    t.team(project, "Qualidade Técnica");
    let error = message_of(t.db.save_team(
        project,
        &TeamDraft {
            id: None,
            name: "qualidade  tecnica".to_string(),
        },
    ));

    assert_eq!(
        error,
        "Já existe um time com o identificador qualidade-tecnica neste projeto."
    );
}

#[test]
fn camada_chamada_voce_e_recusada_porque_voce_e_o_no_virtual() {
    let t = TempDb::open();
    let project = t.project("alfa");

    let error = message_of(t.db.save_layer(project, &layer_draft("Você")));

    assert_eq!(
        error,
        "\"Você\" é o nó Você e não pode ser o nome de uma camada."
    );
}

#[test]
fn camada_nova_vai_para_o_fim_e_editar_por_id_preserva_posicao_e_id() {
    let t = TempDb::open();
    let project = t.project("alfa");
    t.layer(project, "Liderança");
    t.layer(project, "Implementação");

    let edited =
        t.db.save_layer(
            project,
            &LayerDraft {
                id: Some("lideranca".to_string()),
                name: "Liderança do time".to_string(),
                description: "conduz".to_string(),
                kind: "planejar".to_string(),
                instructions: "planeje".to_string(),
            },
        )
        .unwrap();

    assert_eq!(
        (edited.id.as_str(), edited.position, edited.kind.as_str()),
        ("lideranca", 0, "planejar")
    );
    assert_eq!(
        layer_order(&t.db, project),
        vec!["lideranca", "implementacao"]
    );
    assert_eq!(
        message_of(t.db.save_layer(
            project,
            &LayerDraft {
                id: Some("nao-existe".to_string()),
                ..layer_draft("X")
            }
        )),
        "A camada nao-existe não existe neste projeto."
    );
}

#[test]
fn instrucoes_de_4001_caracteres_sao_recusadas_e_4000_passam_na_camada_e_no_agente() {
    let t = TempDb::open();
    let project = t.project("alfa");
    let team = t.team(project, "Time");
    let layer = t.layer(project, "Implementação");
    let at_limit = "é".repeat(4000);
    let over_limit = "é".repeat(4001);
    let expected = "As instruções aceitam até 4000 caracteres.";

    let saved =
        t.db.save_layer(
            project,
            &LayerDraft {
                instructions: at_limit.clone(),
                ..layer_draft("Revisão")
            },
        )
        .unwrap();
    let placed =
        t.db.place_agent(
            project,
            &PlacementDraft {
                instructions: at_limit.clone(),
                ..placement_draft("davi", &team.id, &layer.id)
            },
        )
        .unwrap();

    assert_eq!(saved.instructions, at_limit);
    assert_eq!(placed.instructions, at_limit);
    assert_eq!(
        message_of(t.db.save_layer(
            project,
            &LayerDraft {
                instructions: over_limit.clone(),
                ..layer_draft("Pesquisa")
            }
        )),
        expected
    );
    assert_eq!(
        message_of(t.db.place_agent(
            project,
            &PlacementDraft {
                instructions: over_limit,
                ..placement_draft("rui", &team.id, &layer.id)
            }
        )),
        expected
    );
    assert_eq!(
        layer_order(&t.db, project),
        vec!["implementacao", "revisao"]
    );
}

#[test]
fn max_instances_0_e_5_sao_recusados_e_1_e_4_passam() {
    let t = TempDb::open();
    let project = t.project("alfa");
    let team = t.team(project, "Time");
    let layer = t.layer(project, "Implementação");
    let expected = "O campo max_instances da orquestração deve ficar entre 1 e 4.";

    for instances in [0, 5] {
        let draft = PlacementDraft {
            max_instances: instances,
            ..placement_draft("davi", &team.id, &layer.id)
        };
        assert_eq!(
            message_of(t.db.place_agent(project, &draft)),
            expected,
            "com {instances}"
        );
    }
    for instances in [1, 4] {
        let draft = PlacementDraft {
            max_instances: instances,
            ..placement_draft("davi", &team.id, &layer.id)
        };
        assert_eq!(
            t.db.place_agent(project, &draft).unwrap().max_instances,
            instances
        );
    }
    assert_eq!(t.db.orchestration(project).unwrap().placements.len(), 1);
}

#[test]
fn agente_posicionado_duas_vezes_continua_com_uma_so_posicao_a_ultima() {
    let t = TempDb::open();
    let project = t.project("alfa");
    let web = t.team(project, "Web");
    let api = t.team(project, "API");
    let build = t.layer(project, "Implementação");
    let review = t.layer(project, "Revisão");

    t.place(project, "davi", &web.id, &build.id);
    let moved = t.place(project, "davi", &api.id, &review.id);

    let placements = t.db.orchestration(project).unwrap().placements;
    assert_eq!(placements.len(), 1);
    assert_eq!(
        (
            placements[0].agent_id.as_str(),
            placements[0].team_id.as_str(),
            placements[0].layer_id.as_str()
        ),
        ("davi", "api", "revisao")
    );
    assert_eq!(moved.layer_id, "revisao");
}

#[test]
fn agente_so_e_posicionado_em_time_e_camada_que_existem_no_projeto() {
    let t = TempDb::open();
    let project = t.project("alfa");
    let other = t.project("beta");
    let team = t.team(project, "Time");
    let layer = t.layer(project, "Implementação");
    let foreign_layer = t.layer(other, "Externa");

    assert_eq!(
        message_of(t.db.place_agent(project, &placement_draft("davi", "fantasma", &layer.id))),
        "O time fantasma não existe neste projeto."
    );
    assert_eq!(
        message_of(t.db.place_agent(
            project,
            &placement_draft("davi", &team.id, &foreign_layer.id)
        )),
        "A camada externa não existe neste projeto."
    );
    assert!(t.db.orchestration(project).unwrap().placements.is_empty());
}

#[test]
fn apagar_camada_com_agente_posicionado_e_recusado_e_nada_muda() {
    let t = TempDb::open();
    let project = t.project("alfa");
    let team = t.team(project, "Time");
    t.layer(project, "Liderança");
    t.layer(project, "Implementação");
    t.rule(project, "lideranca", "implementacao", "delegar");
    t.place(project, "davi", &team.id, "implementacao");

    let error = message_of(t.db.delete_layer(project, "implementacao"));

    assert_eq!(
        error,
        "A camada implementacao tem agentes posicionados: mova os agentes antes de excluí-la."
    );
    let snapshot = t.db.orchestration(project).unwrap();
    assert_eq!(snapshot.layers.len(), 2);
    assert_eq!(snapshot.rules.len(), 1);
}

#[test]
fn apagar_camada_vazia_apaga_as_setas_que_saem_e_as_que_chegam_nela() {
    let t = TempDb::open();
    let project = t.project("alfa");
    t.layer(project, "Liderança");
    t.layer(project, "Implementação");
    t.layer(project, "Revisão");
    t.rule(project, "lideranca", "implementacao", "delegar");
    t.rule(project, "implementacao", "revisao", "delegar");
    t.rule(project, "implementacao", "voce", "escalar");
    t.rule(project, "revisao", "implementacao", "devolver");
    let survivor = t.rule(project, "lideranca", "revisao", "delegar");

    t.db.delete_layer(project, "implementacao").unwrap();

    let snapshot = t.db.orchestration(project).unwrap();
    assert_eq!(layer_order(&t.db, project), vec!["lideranca", "revisao"]);
    assert_eq!(snapshot.rules.len(), 1);
    assert_eq!(snapshot.rules[0].id, survivor.id);
    assert_eq!(
        message_of(t.db.delete_layer(project, "implementacao")),
        "A camada implementacao não existe neste projeto."
    );
}

#[test]
fn apagar_time_com_agente_e_recusado_e_sem_agente_passa() {
    let t = TempDb::open();
    let project = t.project("alfa");
    let team = t.team(project, "Time");
    let layer = t.layer(project, "Implementação");
    t.place(project, "davi", &team.id, &layer.id);

    assert_eq!(
        message_of(t.db.delete_team(project, &team.id)),
        "O time time tem agentes posicionados: mova os agentes antes de excluí-lo."
    );

    let empty = t.team(project, "Vazio");
    t.db.delete_team(project, &empty.id).unwrap();
    let teams: Vec<String> =
        t.db.orchestration(project)
            .unwrap()
            .teams
            .into_iter()
            .map(|team| team.id)
            .collect();
    assert_eq!(teams, vec!["time"]);
}

#[test]
fn seta_duplicada_no_mesmo_par_e_recusada_mesmo_com_outra_acao() {
    let t = TempDb::open();
    let project = t.project("alfa");
    t.layer(project, "Implementação");
    t.layer(project, "Revisão");
    t.rule(project, "implementacao", "revisao", "delegar");

    let error =
        message_of(t.db.save_rule(project, &rule_draft("implementacao", "revisao", "delegar")));

    assert_eq!(error, "Já existe uma seta de implementacao para revisao.");
    assert_eq!(t.db.orchestration(project).unwrap().rules.len(), 1);
}

#[test]
fn par_inverso_nao_e_duplicata_e_editar_a_propria_seta_nao_conflita_consigo() {
    let t = TempDb::open();
    let project = t.project("alfa");
    t.layer(project, "Implementação");
    t.layer(project, "Revisão");
    let forward = t.rule(project, "implementacao", "revisao", "delegar");
    t.rule(project, "revisao", "implementacao", "devolver");

    let edited =
        t.db.save_rule(
            project,
            &RuleDraft {
                id: Some(forward.id),
                exec_mode: "paralelo".to_string(),
                ..rule_draft("implementacao", "revisao", "delegar")
            },
        )
        .unwrap();

    assert_eq!(
        (edited.id, edited.exec_mode.as_str()),
        (forward.id, "paralelo")
    );
    assert_eq!(t.db.orchestration(project).unwrap().rules.len(), 2);
}

#[test]
fn editar_seta_para_um_par_ja_ocupado_e_recusado() {
    let t = TempDb::open();
    let project = t.project("alfa");
    t.layer(project, "Liderança");
    t.layer(project, "Implementação");
    t.layer(project, "Revisão");
    t.rule(project, "lideranca", "implementacao", "delegar");
    let other = t.rule(project, "lideranca", "revisao", "delegar");

    let error = message_of(t.db.save_rule(
        project,
        &RuleDraft {
            id: Some(other.id),
            ..rule_draft("lideranca", "implementacao", "delegar")
        },
    ));

    assert_eq!(error, "Já existe uma seta de lideranca para implementacao.");
}

#[test]
fn seta_de_uma_camada_para_ela_mesma_e_recusada() {
    let t = TempDb::open();
    let project = t.project("alfa");
    t.layer(project, "Implementação");

    let error = message_of(t.db.save_rule(
        project,
        &rule_draft("implementacao", "implementacao", "delegar"),
    ));

    assert_eq!(error, "Uma camada não pode enviar trabalho para ela mesma.");
    assert!(t.db.orchestration(project).unwrap().rules.is_empty());
}

#[test]
fn escalar_so_aponta_para_voce_e_voce_so_recebe_por_escalar() {
    let t = TempDb::open();
    let project = t.project("alfa");
    t.layer(project, "Implementação");
    t.layer(project, "Revisão");

    assert_eq!(
        message_of(t.db.save_rule(project, &rule_draft("implementacao", "revisao", "escalar"))),
        "A regra escalar só pode apontar para Você."
    );
    assert_eq!(
        message_of(t.db.save_rule(project, &rule_draft("implementacao", "voce", "delegar"))),
        "Você só recebe trabalho pela regra escalar."
    );
    assert_eq!(
        message_of(t.db.save_rule(project, &rule_draft("implementacao", "voce", "devolver"))),
        "Você só recebe trabalho pela regra escalar."
    );
    let escalate = t.rule(project, "implementacao", "voce", "escalar");
    assert_eq!(escalate.to_layer, "voce");
    assert_eq!(t.db.orchestration(project).unwrap().rules.len(), 1);
}

#[test]
fn seta_com_camada_de_origem_ou_destino_inexistente_e_recusada() {
    let t = TempDb::open();
    let project = t.project("alfa");
    t.layer(project, "Implementação");

    assert_eq!(
        message_of(t.db.save_rule(project, &rule_draft("fantasma", "implementacao", "delegar"))),
        "A camada fantasma não existe neste projeto."
    );
    assert_eq!(
        message_of(t.db.save_rule(project, &rule_draft("implementacao", "fantasma", "delegar"))),
        "A camada fantasma não existe neste projeto."
    );
}

#[test]
fn devolver_para_camada_posterior_e_recusado_e_para_anterior_passa() {
    let t = TempDb::open();
    let project = t.project("alfa");
    t.layer(project, "Implementação");
    t.layer(project, "Revisão");

    assert_eq!(
        message_of(t.db.save_rule(project, &rule_draft("implementacao", "revisao", "devolver"))),
        "Devolver só vai para uma camada anterior."
    );
    let back = t.rule(project, "revisao", "implementacao", "devolver");
    assert_eq!(
        (back.from_layer.as_str(), back.to_layer.as_str()),
        ("revisao", "implementacao")
    );
}

#[test]
fn return_limit_0_e_6_sao_recusados_e_1_e_5_passam() {
    let t = TempDb::open();
    let project = t.project("alfa");
    t.layer(project, "Implementação");
    t.layer(project, "Revisão");
    let expected = "O campo return_limit da orquestração deve ficar entre 1 e 5.";

    for limit in [0, 6] {
        let draft = RuleDraft {
            return_limit: limit,
            ..rule_draft("revisao", "implementacao", "devolver")
        };
        assert_eq!(
            message_of(t.db.save_rule(project, &draft)),
            expected,
            "com {limit}"
        );
    }
    for limit in [1, 5] {
        let draft = RuleDraft {
            id: None,
            return_limit: limit,
            ..rule_draft("implementacao", "revisao", "delegar")
        };
        let saved = t.db.save_rule(project, &draft).unwrap();
        assert_eq!(saved.return_limit, limit);
        t.db.delete_rule(project, saved.id).unwrap();
    }
}

#[test]
fn seta_com_valor_fora_dos_enums_e_recusada_com_o_campo_no_erro() {
    let t = TempDb::open();
    let project = t.project("alfa");
    t.layer(project, "Implementação");
    t.layer(project, "Revisão");

    let bad_event = RuleDraft {
        when_event: "sempre".to_string(),
        ..rule_draft("implementacao", "revisao", "delegar")
    };
    let bad_action = rule_draft("implementacao", "revisao", "mandar");
    let bad_kind = RuleDraft {
        kind: "livre".to_string(),
        ..rule_draft("implementacao", "revisao", "delegar")
    };
    let bad_mode = RuleDraft {
        exec_mode: "turbo".to_string(),
        ..rule_draft("implementacao", "revisao", "delegar")
    };

    assert_eq!(
        message_of(t.db.save_rule(project, &bad_event)),
        "O campo when_event da orquestração não aceita o valor \"sempre\"."
    );
    assert_eq!(
        message_of(t.db.save_rule(project, &bad_action)),
        "O campo action da orquestração não aceita o valor \"mandar\"."
    );
    assert_eq!(
        message_of(t.db.save_rule(project, &bad_kind)),
        "O campo kind da orquestração não aceita o valor \"livre\"."
    );
    assert_eq!(
        message_of(t.db.save_rule(project, &bad_mode)),
        "O campo exec_mode da orquestração não aceita o valor \"turbo\"."
    );
    assert!(t.db.orchestration(project).unwrap().rules.is_empty());
}

#[test]
fn reordenar_camadas_que_deixa_devolucao_apontando_para_frente_e_recusado_e_desfeito() {
    let t = TempDb::open();
    let project = t.project("alfa");
    t.layer(project, "Liderança");
    t.layer(project, "Implementação");
    t.layer(project, "Revisão");
    t.rule(project, "revisao", "implementacao", "devolver");

    let error = message_of(t.db.reorder_layers(
        project,
        &[
            "revisao".to_string(),
            "implementacao".to_string(),
            "lideranca".to_string(),
        ],
    ));

    assert_eq!(error, "Devolver só vai para uma camada anterior.");
    assert_eq!(
        layer_order(&t.db, project),
        vec!["lideranca", "implementacao", "revisao"]
    );
}

#[test]
fn reordenar_camadas_sem_quebrar_devolucao_grava_a_nova_ordem() {
    let t = TempDb::open();
    let project = t.project("alfa");
    t.layer(project, "Liderança");
    t.layer(project, "Implementação");
    t.layer(project, "Revisão");
    t.rule(project, "revisao", "implementacao", "devolver");

    t.db.reorder_layers(
        project,
        &[
            "implementacao".to_string(),
            "revisao".to_string(),
            "lideranca".to_string(),
        ],
    )
    .unwrap();

    assert_eq!(
        layer_order(&t.db, project),
        vec!["implementacao", "revisao", "lideranca"]
    );
}

#[test]
fn reordenar_exige_cada_camada_do_projeto_uma_unica_vez() {
    let t = TempDb::open();
    let project = t.project("alfa");
    t.layer(project, "Liderança");
    t.layer(project, "Implementação");
    let expected = "A nova ordem precisa listar cada camada do projeto uma única vez.";

    let missing = message_of(t.db.reorder_layers(project, &["lideranca".to_string()]));
    let repeated = message_of(
        t.db.reorder_layers(project, &["lideranca".to_string(), "lideranca".to_string()]),
    );
    let unknown = message_of(
        t.db.reorder_layers(project, &["lideranca".to_string(), "fantasma".to_string()]),
    );

    assert_eq!(
        (missing.as_str(), repeated.as_str(), unknown.as_str()),
        (expected, expected, expected)
    );
    assert_eq!(
        layer_order(&t.db, project),
        vec!["lideranca", "implementacao"]
    );
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
        ("iara", "davi", Some(7))
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
    let project = t.project("alfa");
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
            message_of(t.db.save_settings(project, &settings)),
            format!("O campo {field} da orquestração deve ficar entre {min} e {max}.")
        );
    }
    assert_eq!(
        message_of(t.db.save_settings(
            project,
            &OrchestrationSettings {
                human_gate: "talvez".to_string(),
                ..base.clone()
            }
        )),
        "O campo human_gate da orquestração não aceita o valor \"talvez\"."
    );
    assert_eq!(t.db.orchestration(project).unwrap().settings, base);

    let edge = OrchestrationSettings {
        enforcement: "on".to_string(),
        leader_agent: "rui".to_string(),
        step_mode: "livre".to_string(),
        human_gate: "nunca".to_string(),
        max_parallel: 8,
        pair_turn_limit: 1,
        task_turn_limit: 200,
        worktree_policy: "sempre".to_string(),
    };
    t.db.save_settings(project, &edge).unwrap();
    t.db.save_settings(project, &edge).unwrap();
    assert_eq!(t.db.orchestration(project).unwrap().settings, edge);
}

#[test]
fn orquestracao_de_um_projeto_nao_aparece_no_outro() {
    let t = TempDb::open();
    let project = t.project("alfa");
    let other = t.project("beta");
    t.layer(project, "Implementação");
    t.team(project, "Time");

    let snapshot = t.db.orchestration(other).unwrap();

    assert!(snapshot.layers.is_empty() && snapshot.teams.is_empty());
    t.layer(other, "Implementação");
    assert_eq!(layer_order(&t.db, other), vec!["implementacao"]);
}

#[test]
fn apagar_o_projeto_apaga_camadas_times_setas_e_movimentos() {
    let t = TempDb::open();
    let project = t.project("alfa");
    let team = t.team(project, "Time");
    t.layer(project, "Liderança");
    t.layer(project, "Implementação");
    t.rule(project, "lideranca", "implementacao", "delegar");
    t.place(project, "davi", &team.id, "implementacao");
    t.db.record_handoff(project, &handoff_draft("item-1", "delegar"))
        .unwrap();

    t.db.delete_project(project).unwrap();

    let conn = t.db.conn().unwrap();
    for table in [
        "orch_layers",
        "orch_teams",
        "orch_placements",
        "orch_rules",
        "orch_handoffs",
    ] {
        let rows: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(rows, 0, "{table} deveria estar vazia");
    }
}

#[test]
fn banco_recusa_seta_escalar_para_camada_comum_mesmo_fora_da_api() {
    let t = TempDb::open();
    let project = t.project("alfa");
    t.layer(project, "Implementação");
    t.layer(project, "Revisão");

    let conn = t.db.conn().unwrap();
    let inserted = conn.execute(
        "INSERT INTO orch_rules (project_id, from_layer, to_layer, when_event, action, kind, exec_mode, return_limit, position) \
         VALUES (?1, 'implementacao', 'revisao', 'tarefa_concluida', 'escalar', 'revisar', 'sequencia', 2, 0)",
        [project],
    );

    assert!(
        inserted.is_err(),
        "o CHECK do banco deveria recusar escalar para camada comum"
    );
}
