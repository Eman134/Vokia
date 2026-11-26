use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::*;
use crate::agents::{AgentDraft, AgentError, AgentRegistry};
use crate::db::{Db, DbError, OrchestrationSettings};
use crate::providers::ProviderRegistry;

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
        role: "Backend Developer".to_string(),
        avatar: "onix".to_string(),
        cli: "claude".to_string(),
        model: "sonnet".to_string(),
        model_tier: None,
        reasoning_effort: None,
        command: "claude".to_string(),
        follow_default_llm: false,
        fallback: Vec::new(),
    }
}

fn placement_layers(db: &Db) -> Vec<(String, String)> {
    db.orchestration()
        .unwrap()
        .placements
        .into_iter()
        .map(|placement| (placement.agent_id, placement.layer_id))
        .collect()
}

fn sorted_pairs(mut list: Vec<(String, String)>) -> Vec<(String, String)> {
    list.sort();
    list
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

    let leader = Agent::axiom();
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
            (leader.as_str(), "time", "lideranca", 1),
            ("vexa", "time", "implementacao", 1),
            ("kairo", "time", "implementacao", 1),
            ("sentry", "time", "revisao", 1),
            ("oracle", "time", "implementacao", 1),
        ]
    );
}

#[test]
fn semente_sem_sentry_no_elenco_nao_posiciona_sentry_e_deixa_revisao_sem_agente() {
    let dir = TempDir::new();
    let leader = Agent::axiom();
    let db = Db::open(&dir.0).unwrap();

    db.seed_orchestration(&build(&team_without(&dir, "sentry")))
        .unwrap();

    assert_eq!(
        placement_layers(&db),
        sorted_pairs(vec![
            pair("vexa", "implementacao"),
            pair(leader.as_str(), "lideranca"),
            pair("kairo", "implementacao"),
            pair("oracle", "implementacao"),
        ])
    );
    assert_eq!(db.orchestration().unwrap().layers.len(), 3);
}

#[test]
fn semente_gravada_duas_vezes_no_mesmo_projeto_nao_duplica_nada() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let seed = build(&full_team(&dir));

    let first = db.seed_orchestration(&seed).unwrap();
    let second = db.seed_orchestration(&seed).unwrap();

    let snapshot = db.orchestration().unwrap();
    assert_eq!((first, second), (true, false));
    assert_eq!(
        (
            snapshot.layers.len(),
            snapshot.teams.len(),
            snapshot.placements.len(),
            snapshot.edges.len()
        ),
        (3, 1, 5, 0)
    );
}

#[test]
fn semente_nao_roda_em_projeto_que_ja_tem_linha_de_configuracao() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let custom = OrchestrationSettings {
        enforcement: "on".to_string(),
        ..OrchestrationSettings::default()
    };
    db.save_settings(&custom).unwrap();

    let wrote = db.seed_orchestration(&build(&full_team(&dir))).unwrap();

    let snapshot = db.orchestration().unwrap();
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
    let seed = Arc::new(build(&full_team(&dir)));

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let (db, seed) = (db.clone(), seed.clone());
            std::thread::spawn(move || db.seed_orchestration(&seed).unwrap())
        })
        .collect();
    let mut wrote: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    wrote.sort();

    assert_eq!(wrote, vec![false, true]);
    assert_eq!(db.orchestration().unwrap().layers.len(), 3);
    assert_eq!(table_rows(&db, "orch_placements"), 5);
}

#[test]
fn semente_com_instrucoes_do_time_acima_do_limite_falha_e_nao_grava_nada() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let mut seed = build(&full_team(&dir));
    seed.team.instructions = Some("x".repeat(4001));

    let error = db.seed_orchestration(&seed).unwrap_err();

    assert!(matches!(
        error,
        DbError::OrchestrationInstructionsTooLong { max: 4000 }
    ));
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
    let mut seed = build(&full_team(&dir));
    seed.placements[0].layer_id = "inexistente".to_string();

    let error = db.seed_orchestration(&seed).unwrap_err();

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
    assert!(db.seed_orchestration(&build(&full_team(&dir))).unwrap());
}

#[test]
fn agente_novo_criado_depois_da_semente_fica_sem_posicao_ate_alguem_posicionar() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let registry = AgentRegistry::load(&dir.0);
    db.seed_orchestration(&build(&registry.team().unwrap()))
        .unwrap();

    registry.create(&draft("Lia"), &ProviderRegistry::seeded()).unwrap();

    let ids: Vec<String> = placement_layers(&db)
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

    let providers = ProviderRegistry::seeded();
    for name in ["Vexa 2", "Vexa-2", "Lia-3"] {
        let error = registry.create(&draft(name), &providers).unwrap_err();
        assert!(matches!(error, AgentError::InvalidName), "{name}");
    }
    assert_eq!(registry.team().unwrap().ids().count(), 6);
}

#[test]
fn migracao_de_agents_json_antigo_sem_axiom_e_vexa_insere_ambos_sem_builtin_e_nao_duplica() {
    let dir = TempDir::new();
    let file = dir.0.join("agents.json");
    let initial_json = serde_json::json!({
        "version": 1,
        "agents": [
            {
                "id": "kairo",
                "name": "Kairo",
                "role": "Backend Developer",
                "avatar": "vesper",
                "cli": "claude",
                "model": "sonnet",
                "builtin": false
            }
        ],
        "instructions": {}
    });
    fs::write(&file, serde_json::to_string_pretty(&initial_json).unwrap()).unwrap();

    // 1º carregamento: migra e insere axiom e vexa com builtin: false
    let registry1 = AgentRegistry::load(&dir.0);
    let team1 = registry1.team().expect("time após migração");

    let axiom_profile = team1.profile(&Agent::axiom()).expect("axiom deve existir");
    assert_eq!(axiom_profile.id.as_str(), "axiom");
    assert_eq!(axiom_profile.name, "Axiom");
    assert_eq!(axiom_profile.role, "Tech Lead");
    assert_eq!(axiom_profile.avatar, "quarzo");
    assert_eq!(axiom_profile.builtin, false);

    let vexa_profile = team1.profile(&Agent::vexa()).expect("vexa deve existir");
    assert_eq!(vexa_profile.id.as_str(), "vexa");
    assert_eq!(vexa_profile.name, "Vexa");
    assert_eq!(vexa_profile.role, "Frontend Developer");
    assert_eq!(vexa_profile.avatar, "lume");
    assert_eq!(vexa_profile.builtin, false);

    let ember_profile = team1.profile(&Agent::ember()).expect("ember deve existir");
    assert_eq!(ember_profile.id.as_str(), "ember");
    assert_eq!(ember_profile.builtin, true);

    // Confere se o arquivo foi reescrito no disco com builtin: false e ids corretos
    let disk_content: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    let disk_agents = disk_content["agents"].as_array().expect("array de agentes");
    assert_eq!(disk_agents.len(), 3); // kairo, axiom, vexa
    let disk_axiom = disk_agents
        .iter()
        .find(|a| a["id"] == "axiom")
        .expect("axiom no json");
    assert_eq!(disk_axiom["builtin"], false);
    let disk_vexa = disk_agents
        .iter()
        .find(|a| a["id"] == "vexa")
        .expect("vexa no json");
    assert_eq!(disk_vexa["builtin"], false);

    // 2º carregamento: não deve duplicar axiom nem vexa
    let registry2 = AgentRegistry::load(&dir.0);
    let team2 = registry2.team().expect("time no segundo carregamento");
    assert_eq!(team2.profiles().len(), 4); // 1 builtin (ember) + 3 persistidos (kairo, axiom, vexa)
    assert_eq!(
        team2.profiles().iter().filter(|p| p.id == Agent::axiom()).count(),
        1
    );
    assert_eq!(
        team2.profiles().iter().filter(|p| p.id == Agent::vexa()).count(),
        1
    );
}

#[test]
fn axiom_e_vexa_sao_editaveis_e_ember_recusa_alteracao_com_builtin_locked() {
    let dir = TempDir::new();
    let registry = AgentRegistry::load(&dir.0);

    let providers = ProviderRegistry::seeded();
    // Ember não pode ser alterado nem removido
    let ember_draft = AgentDraft {
        name: "Ember Alterado".to_string(),
        role: "Orquestrador Geral".to_string(),
        avatar: "ember".to_string(),
        cli: "claude".to_string(),
        model: "sonnet".to_string(),
        model_tier: None,
        reasoning_effort: None,
        command: "claude".to_string(),
        follow_default_llm: false,
        fallback: Vec::new(),
    };
    let err_update_ember = registry
        .update(&Agent::ember(), &ember_draft, &providers)
        .unwrap_err();
    assert!(
        matches!(err_update_ember, AgentError::BuiltinLocked(ref name) if name == "Ember"),
        "esperava BuiltinLocked('Ember'), obteve {:?}",
        err_update_ember
    );

    let err_remove_ember = registry.remove(&Agent::ember()).unwrap_err();
    assert!(
        matches!(err_remove_ember, AgentError::BuiltinLocked(ref name) if name == "Ember"),
        "esperava BuiltinLocked('Ember'), obteve {:?}",
        err_remove_ember
    );

    // Axiom pode ser editado
    let axiom_draft = AgentDraft {
        name: "Axiom".to_string(),
        role: "Principal Architect".to_string(),
        avatar: "quarzo".to_string(),
        cli: "claude".to_string(),
        model: "opus".to_string(),
        model_tier: None,
        reasoning_effort: None,
        command: "claude".to_string(),
        follow_default_llm: false,
        fallback: Vec::new(),
    };
    let updated_axiom = registry
        .update(&Agent::axiom(), &axiom_draft, &providers)
        .expect("edição do Axiom deve ser permitida");
    assert_eq!(updated_axiom.id, Agent::axiom());
    assert_eq!(updated_axiom.role, "Principal Architect");
    assert_eq!(updated_axiom.model, "opus");
    assert_eq!(updated_axiom.builtin, false);

    // Vexa pode ser editada
    let vexa_draft = AgentDraft {
        name: "Vexa".to_string(),
        role: "Frontend Lead".to_string(),
        avatar: "lume".to_string(),
        cli: "claude".to_string(),
        model: "haiku".to_string(),
        model_tier: None,
        reasoning_effort: None,
        command: "claude".to_string(),
        follow_default_llm: false,
        fallback: Vec::new(),
    };
    let updated_vexa = registry
        .update(&Agent::vexa(), &vexa_draft, &providers)
        .expect("edição da Vexa deve ser permitida");
    assert_eq!(updated_vexa.id, Agent::vexa());
    assert_eq!(updated_vexa.role, "Frontend Lead");
    assert_eq!(updated_vexa.model, "haiku");
    assert_eq!(updated_vexa.builtin, false);
}

#[test]
fn remocao_de_axiom_bloqueada_enquanto_lider_ativo_com_posicao_no_banco() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let registry = AgentRegistry::load(&dir.0);
    let team = registry.team().unwrap();
    db.seed_orchestration(&build(&team)).unwrap();

    let axiom = Agent::axiom();
    let ember = Agent::ember();

    let res = db.remove_agent::<DbError>(&axiom, "Axiom", &ember, "Ember", 1000, || {
        registry.remove(&axiom).map_err(|e| DbError::AgentHasPosition(e.to_string()))
    });

    match res {
        Err(DbError::AgentHasPosition(msg)) => {
            assert!(
                msg.contains("Axiom está posicionado em") || msg.contains("líder"),
                "mensagem inesperada: {msg}"
            );
        }
        Err(other) => panic!("esperava DbError::AgentHasPosition, obteve {:?}", other),
        Ok(_) => panic!("remoção do Axiom posicionado deveria ter falhado"),
    }

    // Garante que Axiom não foi removido do registro
    assert!(registry.team().unwrap().profile(&axiom).is_some());
}

#[test]
fn remocao_de_vexa_sem_posicao_no_banco_tem_sucesso() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let registry = AgentRegistry::load(&dir.0);

    // Sem semear orquestração, Vexa não possui posições nem liderança
    let vexa = Agent::vexa();
    let ember = Agent::ember();
    let res = db.remove_agent::<DbError>(&vexa, "Vexa", &ember, "Ember", 1000, || {
        registry.remove(&vexa).map_err(|e| DbError::AgentHasPosition(e.to_string()))
    });
    assert!(res.is_ok(), "remoção de Vexa sem posição deve ter sucesso");
    let notes = res.unwrap();
    assert!(notes.is_empty());
    assert!(registry.team().unwrap().profile(&vexa).is_none());
}

#[test]
fn migracao_de_agents_json_legado_sem_command_recebe_default_command_do_preset() {
    let dir = TempDir::new();
    let file = dir.0.join("agents.json");
    // agents.json legado sem campo "command"
    let legacy_json = serde_json::json!({
        "version": 1,
        "agents": [
            {
                "id": "kairo",
                "name": "Kairo",
                "role": "Backend Developer",
                "avatar": "vesper",
                "cli": "claude",
                "model": "sonnet",
                "builtin": false
            }
        ],
        "instructions": {}
    });
    fs::write(&file, serde_json::to_string_pretty(&legacy_json).unwrap()).unwrap();

    let registry = AgentRegistry::load(&dir.0);
    let team = registry.team().expect("time legado com migração");

    let kairo = team.profile(&Agent::from_stored("kairo").unwrap()).unwrap();
    assert_eq!(kairo.command, "claude");

    let axiom = team.profile(&Agent::axiom()).unwrap();
    assert_eq!(axiom.command, "claude");

    let vexa = team.profile(&Agent::vexa()).unwrap();
    assert_eq!(vexa.command, "claude");
}

#[test]
fn recusa_de_comando_invalido_vazio_espacos_aspas_abertas_e_flags_reservadas() {
    let dir = TempDir::new();
    let registry = AgentRegistry::load(&dir.0);
    let providers = ProviderRegistry::seeded();

    let base_draft = |cli: &str, model: &str, cmd: &str| AgentDraft {
        name: "TestBot".to_string(),
        role: "Dev".to_string(),
        avatar: "onix".to_string(),
        cli: cli.to_string(),
        model: model.to_string(),
        model_tier: None,
        reasoning_effort: None,
        command: cmd.to_string(),
        follow_default_llm: false,
        fallback: Vec::new(),
    };

    // 1. Comando vazio e apenas espaços
    let err_vazio = registry.create(&base_draft("claude", "haiku", ""), &providers).unwrap_err();
    assert!(matches!(err_vazio, AgentError::InvalidCommand));

    let err_espacos = registry.create(&base_draft("claude", "haiku", "   \t  "), &providers).unwrap_err();
    assert!(matches!(err_espacos, AgentError::InvalidCommand));

    // 2. Aspas sem fechamento (falha de shlex::split)
    let err_aspas = registry.create(&base_draft("claude", "haiku", "claude \"aspas sem fechar"), &providers).unwrap_err();
    assert!(matches!(err_aspas, AgentError::InvalidCommand));

    // 3. Flags reservadas Claude (inclusive com '=')
    for flag in ["-p", "--tools", "--allowedTools", "--model=sonnet", "--append-system-prompt", "--permission-mode"] {
        let cmd = format!("claude {flag}");
        let err = registry.create(&base_draft("claude", "haiku", &cmd), &providers).unwrap_err();
        match err {
            AgentError::ReservedCommandFlag(found) => {
                let expected_base = flag.split('=').next().unwrap();
                assert_eq!(found, expected_base, "para flag {flag}");
            }
            other => panic!("esperava ReservedCommandFlag para {flag}, obteve {:?}", other),
        }
    }

    // 4. Flags reservadas Codex
    for flag in ["--json", "-m", "--model=gpt-5", "-s", "--sandbox=workspace-write", "-C", "--cd", "--add-dir"] {
        let cmd = format!("codex {flag}");
        let err = registry.create(&base_draft("codex", "gpt-6-luna", &cmd), &providers).unwrap_err();
        assert!(matches!(err, AgentError::ReservedCommandFlag(_)), "para flag codex {flag}");
    }

    // 5. Flags reservadas OpenCode
    for flag in ["--format", "--format=json", "-m", "--model=gpt", "--dir=/tmp", "--auto"] {
        let cmd = format!("opencode {flag}");
        let err = registry.create(&base_draft("opencode", "openai/gpt-6-astra", &cmd), &providers).unwrap_err();
        assert!(matches!(err, AgentError::ReservedCommandFlag(_)), "para flag opencode {flag}");
    }

    // 6. Flags reservadas Antigravity
    for flag in ["-p", "--print", "--output-format", "--sandbox", "--mode=plan", "--dangerously-skip-permissions"] {
        let cmd = format!("agy {flag}");
        let err = registry.create(&base_draft("antigravity", "gemini-4-hydrogen-medium", &cmd), &providers).unwrap_err();
        assert!(matches!(err, AgentError::ReservedCommandFlag(_)), "para flag antigravity {flag}");
    }

    // 7. Flag reservada no meio de outros argumentos
    let cmd_meio = "codex --custom-flag --json --outra-flag";
    let err_meio = registry.create(&base_draft("codex", "gpt-6-luna", cmd_meio), &providers).unwrap_err();
    match err_meio {
        AgentError::ReservedCommandFlag(flag) => assert_eq!(flag, "--json"),
        other => panic!("esperava ReservedCommandFlag, obteve {:?}", other),
    }

    // 8. Comando válido com flags customizadas livres
    let cmd_valido = "codex --custom-feature --dangerously-skip-repo-check";
    let ok = registry.create(&base_draft("codex", "gpt-6-luna", cmd_valido), &providers);
    assert!(ok.is_ok(), "comando válido foi recusado: {:?}", ok.err());
}

#[test]
fn presets_contem_claude_codex_opencode_e_antigravity_com_defaults_e_modelos_corretos() {
    let presets = crate::agents::presets();
    assert_eq!(presets.len(), 5);

    let claude = presets.iter().find(|p| p.id == "claude").expect("preset claude");
    assert_eq!(claude.default_command, "claude");
    assert_eq!(claude.default_model, "haiku");
    assert_eq!(claude.models, vec!["haiku", "sonnet", "opus"]);

    let codex = presets.iter().find(|p| p.id == "codex").expect("preset codex");
    assert_eq!(codex.default_command, "codex");
    assert_eq!(codex.default_model, "gpt-6-luna");
    assert!(codex.models.iter().any(|m| m == "gpt-6-luna"));
    assert!(codex.models.iter().any(|m| m == "gpt-5.6-terra"));
    assert!(codex.models.iter().any(|m| m == "gpt-5.6-luna"));

    let opencode = presets.iter().find(|p| p.id == "opencode").expect("preset opencode");
    assert_eq!(opencode.default_command, "opencode");
    assert_eq!(opencode.default_model, "openai/gpt-6-astra");
    assert!(opencode.models.iter().any(|m| m == "openai/gpt-6-astra"));
    assert!(opencode.models.iter().any(|m| m == "openai/gpt-5.6-luna"));
    assert!(opencode.models.iter().any(|m| m == "openai/gpt-5.4-mini"));

    let agy = presets.iter().find(|p| p.id == "antigravity").expect("preset antigravity");
    assert_eq!(agy.default_command, "agy");
    assert_eq!(agy.default_model, "gemini-4-hydrogen-medium");
    assert!(agy.models.iter().any(|m| m == "gemini-4-hydrogen-medium"));
    assert!(agy.models.iter().any(|m| m == "gemini-4-neon-medium"));
    assert!(agy.models.iter().any(|m| m == "gemini-4-argon-medium"));
    assert!(agy.models.iter().any(|m| m == "gemini-3.8-flash-medium"));
}

#[test]
fn ember_bloqueado_contra_alteracao_de_cli_para_codex_ou_outro_provedor() {
    let dir = TempDir::new();
    let registry = AgentRegistry::load(&dir.0);
    let providers = ProviderRegistry::seeded();

    let ember_draft = AgentDraft {
        name: "Ember".to_string(),
        role: "Orquestrador geral".to_string(),
        avatar: "ember".to_string(),
        cli: "codex".to_string(),
        model: "gpt-6-luna".to_string(),
        model_tier: None,
        reasoning_effort: None,
        command: "codex".to_string(),
        follow_default_llm: false,
        fallback: Vec::new(),
    };

    let err = registry.update(&Agent::ember(), &ember_draft, &providers).unwrap_err();
    match err {
        AgentError::BuiltinLocked(name) => assert_eq!(name, "Ember"),
        other => panic!("esperava BuiltinLocked('Ember'), obteve {:?}", other),
    }
}

#[test]
fn cadastrar_e_editar_comando_customizado_com_persistencia_e_restaurar_padrao() {
    let dir = TempDir::new();
    let file = dir.0.join("agents.json");
    let registry = AgentRegistry::load(&dir.0);
    let providers = ProviderRegistry::seeded();

    // 1. Cadastrar agente com comando customizado
    let draft_atlas = AgentDraft {
        name: "Atlas".to_string(),
        role: "Researcher".to_string(),
        avatar: "onix".to_string(),
        cli: "codex".to_string(),
        model: "gpt-6-luna".to_string(),
        model_tier: None,
        reasoning_effort: None,
        command: "codex --custom-flag --another-opt".to_string(),
        follow_default_llm: false,
        fallback: Vec::new(),
    };
    let created = registry.create(&draft_atlas, &providers).expect("criar atlas");
    assert_eq!(created.command, "codex --custom-flag --another-opt");

    // 2. Verificar persistência em disco após reload
    let registry_reloaded = AgentRegistry::load(&dir.0);
    let atlas_profile = registry_reloaded
        .team()
        .unwrap()
        .profile(&created.id)
        .expect("atlas deve existir após reload")
        .clone();
    assert_eq!(atlas_profile.command, "codex --custom-flag --another-opt");

    // 3. Simular "Restaurar padrão" no NewAgentDialog: atualiza para o defaultCommand do preset ("codex")
    let draft_restaurado = AgentDraft {
        command: "codex".to_string(),
        ..draft_atlas
    };
    let updated = registry_reloaded.update(&created.id, &draft_restaurado, &providers).expect("atualizar atlas");
    assert_eq!(updated.command, "codex");

    // Conferir disco
    let disk_text = fs::read_to_string(&file).unwrap();
    assert!(disk_text.contains("\"command\": \"codex\""));

    // 4. Axiom e Vexa editáveis com comando customizado
    let axiom_draft = AgentDraft {
        name: "Axiom".to_string(),
        role: "Tech Lead".to_string(),
        avatar: "quarzo".to_string(),
        cli: "claude".to_string(),
        model: "sonnet".to_string(),
        model_tier: None,
        reasoning_effort: None,
        command: "claude --dangerously-skip-permissions".to_string(),
        follow_default_llm: false,
        fallback: Vec::new(),
    };
    let updated_axiom = registry_reloaded.update(&Agent::axiom(), &axiom_draft, &providers).expect("update axiom com custom command");
    assert_eq!(updated_axiom.command, "claude --dangerously-skip-permissions");

    let vexa_draft = AgentDraft {
        name: "Vexa".to_string(),
        role: "Frontend Developer".to_string(),
        avatar: "lume".to_string(),
        cli: "codex".to_string(),
        model: "gpt-6-luna".to_string(),
        model_tier: None,
        reasoning_effort: None,
        command: "codex --custom-vexa".to_string(),
        follow_default_llm: false,
        fallback: Vec::new(),
    };
    let updated_vexa = registry_reloaded.update(&Agent::vexa(), &vexa_draft, &providers).expect("update vexa com codex");
    assert_eq!(updated_vexa.command, "codex --custom-vexa");
    assert_eq!(updated_vexa.cli, "codex");
}

#[test]
fn read_or_seed_com_roster_migrated_true_nao_reinjeta_axiom_nem_vexa_apos_remocao() {
    let dir = TempDir::new();
    let file = dir.0.join("agents.json");

    // 1. Arquivo com rosterMigrated: true onde o usuário removeu Axiom e Vexa
    let migrated_json = serde_json::json!({
        "version": 1,
        "rosterMigrated": true,
        "agents": [
            {
                "id": "kairo",
                "name": "Kairo",
                "role": "Backend Developer",
                "avatar": "vesper",
                "cli": "claude",
                "model": "sonnet",
                "command": "claude",
                "builtin": false
            }
        ],
        "instructions": {}
    });
    fs::write(&file, serde_json::to_string_pretty(&migrated_json).unwrap()).unwrap();

    let registry = AgentRegistry::load(&dir.0);
    let team = registry.team().expect("time após carregamento com rosterMigrated: true");

    // Axiom e Vexa NÃO devem ser reinjetados
    assert!(
        team.profile(&Agent::axiom()).is_none(),
        "Axiom não deveria ser reinjetado quando rosterMigrated é true"
    );
    assert!(
        team.profile(&Agent::vexa()).is_none(),
        "Vexa não deveria ser reinjetada quando rosterMigrated é true"
    );
    assert!(team.profile(&Agent::from_stored("kairo").unwrap()).is_some());
    assert!(team.profile(&Agent::ember()).is_some());
    assert_eq!(team.profiles().len(), 2); // 1 builtin (ember) + 1 persistido (kairo)

    // 2. Arquivo legado sem rosterMigrated (ou rosterMigrated: false): DEVE migrar e injetar Axiom e Vexa
    let dir_legacy = TempDir::new();
    let file_legacy = dir_legacy.0.join("agents.json");
    let unmigrated_json = serde_json::json!({
        "version": 1,
        "rosterMigrated": false,
        "agents": [
            {
                "id": "kairo",
                "name": "Kairo",
                "role": "Backend Developer",
                "avatar": "vesper",
                "cli": "claude",
                "model": "sonnet",
                "command": "claude",
                "builtin": false
            }
        ],
        "instructions": {}
    });
    fs::write(&file_legacy, serde_json::to_string_pretty(&unmigrated_json).unwrap()).unwrap();

    let registry_legacy = AgentRegistry::load(&dir_legacy.0);
    let team_legacy = registry_legacy.team().expect("time após migração");
    assert!(team_legacy.profile(&Agent::axiom()).is_some());
    assert!(team_legacy.profile(&Agent::vexa()).is_some());
}
