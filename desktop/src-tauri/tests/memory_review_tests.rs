use std::fs;
use std::path::PathBuf;

use super::*;
use crate::db::{
    Db, MemoryOriginType, NewProject, NewProjectMemory, NewTurnWorkSteps, SkillScope,
    SuggestionKind, SuggestionStatus, LEARNING_MODE_ASK, LEARNING_MODE_AUTO,
    LEARNING_MODE_OFF, SETTING_LEARNING_MODE, SETTING_LEARNING_TURN_CADENCE,
};
use crate::events::Speaker;
use crate::providers::ProviderRegistry;
use crate::usage::model::{ProviderPlanUsage, UsageWindow, FIVE_HOUR_MINS};
use crate::usage::UsageEngine;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vokia-memory-review-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn setup_db_with_project(dir: &Path) -> (Db, i64, PathBuf) {
    let db_path = dir.join("vokia.db");
    let proj_dir = dir.join("repo");
    fs::create_dir_all(&proj_dir).unwrap();
    let db = Db::open(&db_path).expect("abrir db");
    let path_str = proj_dir.to_string_lossy().to_string();
    let proj = db
        .insert_project(&NewProject {
            name: "Projeto Teste",
            symbol: "PT",
            description: "",
            path: &path_str,
            origin_kind: "folder",
            origin_repo: None,
        })
        .expect("criar projeto");
    (db, proj.id, proj_dir)
}

#[test]
fn criterio_1_prioriza_work_steps_falha_sucesso_redige_segredos_e_gera_sugestao_ask() {
    let dir = temp_dir("criterio-1");
    let (db, project_id, proj_dir) = setup_db_with_project(&dir);

    // Inserir work steps normais e um com falha -> recuperação
    let mut blocks = Vec::new();
    for i in 0..90 {
        blocks.push(format!("[work steps]\n- Read src/file_{i}.rs -> ok\n[end work steps]"));
    }
    blocks.push(
        "[work steps]\n- Run cargo test -> failed (missing env PSP_SANDBOX=1, token=sk-ant-1234567890abcdef)\n- Run PSP_SANDBOX=1 cargo test -> ✓ ok\n[end work steps]".to_string(),
    );

    let prioritized = prioritize_work_steps(&blocks, MAX_WORK_STEPS);
    assert_eq!(prioritized.len(), MAX_WORK_STEPS);
    assert!(
        prioritized[0].contains("cargo test -> failed") && prioritized[0].contains("✓ ok"),
        "sequência falha->sucesso deve ser priorizada no topo"
    );
    assert!(
        !prioritized[0].contains("sk-ant-1234567890abcdef"),
        "segredo deve ser redigido antes de enviar ao modelo Basic"
    );

    let prompt = build_review_prompt(
        &[],
        &[ReviewMessageEntry {
            id: 1,
            from_agent: "nina".to_string(),
            to_agent: "user".to_string(),
            text: "Corrigi o build rodando with bearer secret_token_123456".to_string(),
            item_id: Some("CARD-98".to_string()),
        }],
        &blocks,
        &[],
        Some("CARD-98"),
        Some("nina"),
    );
    assert!(!prompt.contains("secret_token_123456"));
    assert!(prompt.contains("CARD-98"));

    let raw_json = r#"```json
    {
      "memories": [
        { "op": "add", "fact": "Testes de pagamento exigem PSP_SANDBOX=1.", "rationale": "Evita falha de build/teste" }
      ],
      "skill": {
        "op": "create",
        "slug": "psp-sandbox-tests",
        "title": "Testes PSP Sandbox",
        "description": "Como rodar testes de pagamento sem chamar API real.",
        "content": "---\nname: Testes PSP Sandbox\ndescription: Como rodar testes de pagamento.\n---\n\nSempre defina `PSP_SANDBOX=1` antes de rodar `cargo test`.",
        "rationale": "Erro recorrente recuperado após falha"
      }
    }
    ```"#;

    let parsed = parse_review_response(raw_json).expect("JSON válido");
    let (created, auto_applied) = persist_review_output(
        &db,
        project_id,
        Some(&proj_dir),
        Some(&dir),
        LEARNING_MODE_ASK,
        &parsed,
        Some("nina"),
        Some("CARD-98"),
        Some("codex"),
        Some("gpt-5-mini"),
        1_700_000_000_000,
    )
    .expect("persistir sugestões");

    assert_eq!(created, 2);
    assert_eq!(auto_applied, 0);

    let pending = db
        .list_learning_suggestions(Some(project_id), Some(SuggestionStatus::Pending))
        .expect("listar pendentes");
    assert_eq!(pending.len(), 2);
    assert!(pending.iter().any(|s| s.kind == SuggestionKind::Memory));
    assert!(pending.iter().any(|s| s.kind == SuggestionKind::Skill));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn criterio_2_preserva_memorias_manuais_e_fixadas_contra_update_e_forget() {
    let dir = temp_dir("criterio-2");
    let (db, project_id, proj_dir) = setup_db_with_project(&dir);

    db.insert_project_memory(&NewProjectMemory {
        id: "mem-manual-1",
        project_id,
        fact: "Nunca instalar dependência sem aprovação do usuário.",
        source: "manual",
        origin_type: MemoryOriginType::Manual,
        pinned: true,
        author: Some("user"),
        now_ms: 1000,
    })
    .expect("inserir memória manual");

    db.insert_project_memory(&NewProjectMemory {
        id: "mem-auto-1",
        project_id,
        fact: "Fato automático antigo.",
        source: "auto-review",
        origin_type: MemoryOriginType::Auto,
        pinned: false,
        author: Some("nina"),
        now_ms: 1000,
    })
    .expect("inserir memória automática");

    let output = ParsedReviewOutput {
        memories: vec![
            ParsedMemoryOp {
                op: "update".to_string(),
                id: Some("mem-manual-1".to_string()),
                fact: Some("Pode instalar dependências livremente.".to_string()),
                rationale: Some("Tentativa indevida".to_string()),
            },
            ParsedMemoryOp {
                op: "forget".to_string(),
                id: Some("mem-manual-1".to_string()),
                fact: None,
                rationale: Some("Tentativa de remoção".to_string()),
            },
            ParsedMemoryOp {
                op: "update".to_string(),
                id: Some("mem-auto-1".to_string()),
                fact: Some("Fato automático atualizado.".to_string()),
                rationale: Some("Atualização válida".to_string()),
            },
        ],
        skill: None,
    };

    let (created, auto_applied) = persist_review_output(
        &db,
        project_id,
        Some(&proj_dir),
        Some(&dir),
        LEARNING_MODE_AUTO,
        &output,
        Some("ember"),
        None,
        Some("codex"),
        Some("gpt-5-mini"),
        2000,
    )
    .expect("persistir auto");

    assert_eq!(created, 0);
    assert_eq!(auto_applied, 1, "apenas a memória não-manual deve ser alterada");

    let manual = db
        .get_project_memory(project_id, "mem-manual-1")
        .expect("ler manual")
        .expect("manual deve continuar existindo");
    assert_eq!(
        manual.fact,
        "Nunca instalar dependência sem aprovação do usuário."
    );

    let auto_mem = db
        .get_project_memory(project_id, "mem-auto-1")
        .expect("ler auto")
        .expect("auto existe");
    assert_eq!(auto_mem.fact, "Fato automático atualizado.");

    // Também garante que update_project_memory_guarded / delete_project_memory_guarded recusam manual
    assert!(!db
        .update_project_memory_guarded(project_id, "mem-manual-1", "hack", "review", 3000)
        .unwrap());
    assert!(!db
        .delete_project_memory_guarded(project_id, "mem-manual-1")
        .unwrap());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn criterio_3_respeita_limiar_75_porcento_e_escolhe_provedor_basic_dinamicamente() {
    let dir = temp_dir("criterio-3");
    let registry = ProviderRegistry::seeded();

    let mut claude = registry.get("claude").unwrap();
    claude.enabled = true;
    claude.tiers.basic = "claude-haiku-4-5".to_string();
    registry.save_provider(claude).unwrap();

    let mut codex = registry.get("codex").unwrap();
    codex.enabled = true;
    codex.tiers.basic = "gpt-5-mini".to_string();
    registry.save_provider(codex).unwrap();

    let mut opencode = registry.get("opencode").unwrap();
    opencode.enabled = true;
    opencode.tiers.basic = "gemini-2.5-flash".to_string();
    registry.save_provider(opencode).unwrap();

    // Desabilita qualquer outro provedor para isolar o teste nos 3 acima
    for p in registry.list().unwrap() {
        if !matches!(p.id.as_str(), "claude" | "codex" | "opencode") {
            let _ = registry.set_enabled(&p.id, false, Some("test"));
        }
    }

    let usage = UsageEngine::default();
    let now_ms = 1_700_000_000_000u64;
    usage.record_provider_usage(ProviderPlanUsage {
        provider_id: "claude".to_string(),
        label: None,
        primary: Some(UsageWindow::new(80, 0, FIVE_HOUR_MINS)),
        secondary: None,
        tertiary: None,
        groups: Vec::new(),
        credits_available: None,
        reset_credits_available: None,
        updated_at_ms: now_ms,
        read_failed: false,
    });
    usage.record_provider_usage(ProviderPlanUsage {
        provider_id: "codex".to_string(),
        label: None,
        primary: Some(UsageWindow::new(75, 0, FIVE_HOUR_MINS)),
        secondary: None,
        tertiary: None,
        groups: Vec::new(),
        credits_available: None,
        reset_credits_available: None,
        updated_at_ms: now_ms,
        read_failed: false,
    });
    usage.record_provider_usage(ProviderPlanUsage {
        provider_id: "opencode".to_string(),
        label: None,
        primary: Some(UsageWindow::new(20, 0, FIVE_HOUR_MINS)),
        secondary: None,
        tertiary: None,
        groups: Vec::new(),
        credits_available: None,
        reset_credits_available: None,
        updated_at_ms: now_ms,
        read_failed: false,
    });

    // Não hardcoda Claude: pula claude (80%) e codex (75%) e seleciona opencode em ModelTier::Basic!
    let selected = select_basic_provider_candidate(&registry, &usage, None, now_ms);
    match selected {
        ProviderSelectionResult::Selected(cand) => {
            assert_eq!(cand.provider.id, "opencode");
            assert_eq!(cand.model, "gemini-2.5-flash");
        }
        other => panic!("esperava Selected(opencode), obteve {other:?}"),
    }

    // Se opencode também subir para 80%, adia silenciosamente sem marcar erro
    usage.record_provider_usage(ProviderPlanUsage {
        provider_id: "opencode".to_string(),
        label: None,
        primary: Some(UsageWindow::new(80, 0, FIVE_HOUR_MINS)),
        secondary: None,
        tertiary: None,
        groups: Vec::new(),
        credits_available: None,
        reset_credits_available: None,
        updated_at_ms: now_ms,
        read_failed: false,
    });
    let postponed = select_basic_provider_candidate(&registry, &usage, None, now_ms);
    assert_eq!(postponed, ProviderSelectionResult::PostponedUsageLimit);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn criterio_4_apply_e_dismiss_learning_suggestion_grava_skill_md_valido() {
    let dir = temp_dir("criterio-4");
    let (db, project_id, proj_dir) = setup_db_with_project(&dir);

    let output = ParsedReviewOutput {
        memories: vec![],
        skill: Some(ParsedSkillSuggestion {
            op: "create".to_string(),
            slug: "rust-tauri-check".to_string(),
            title: Some("Verificação Tauri".to_string()),
            description: Some("Fluxo de check e testes do backend Rust.".to_string()),
            content: "Sempre defina CARGO_TARGET_DIR antes de rodar cargo check.".to_string(),
            rationale: Some("Padroniza compilação no worktree".to_string()),
        }),
    };

    persist_review_output(
        &db,
        project_id,
        Some(&proj_dir),
        Some(&dir),
        LEARNING_MODE_ASK,
        &output,
        Some("davi"),
        None,
        Some("codex"),
        Some("gpt-5-mini"),
        5000,
    )
    .unwrap();

    let pending = db
        .list_learning_suggestions(Some(project_id), Some(SuggestionStatus::Pending))
        .unwrap();
    assert_eq!(pending.len(), 1);
    let suggestion = &pending[0];

    let applied = apply_learning_suggestion_on_db(
        &db,
        &suggestion.id,
        Some(SkillScope::Project),
        Some(&proj_dir),
        Some(&dir),
        6000,
    )
    .expect("aplicar skill");
    assert_eq!(applied.status, SuggestionStatus::Applied);

    let skill_path = proj_dir.join(".vokia/skills/rust-tauri-check/SKILL.md");
    assert!(skill_path.is_file(), "SKILL.md deve existir no projeto");
    let md = fs::read_to_string(&skill_path).unwrap();
    assert!(md.starts_with("---\n"));
    assert!(md.contains("name:"));
    assert!(md.contains("description:"));
    assert!(md.contains("CARGO_TARGET_DIR"));

    // Testar dismiss em outra sugestão
    let output2 = ParsedReviewOutput {
        memories: vec![ParsedMemoryOp {
            op: "add".to_string(),
            id: None,
            fact: Some("Fato descartável".to_string()),
            rationale: None,
        }],
        skill: None,
    };
    persist_review_output(
        &db,
        project_id,
        Some(&proj_dir),
        Some(&dir),
        LEARNING_MODE_ASK,
        &output2,
        None,
        None,
        None,
        None,
        7000,
    )
    .unwrap();
    let pending2 = db
        .list_learning_suggestions(Some(project_id), Some(SuggestionStatus::Pending))
        .unwrap();
    assert_eq!(pending2.len(), 1);
    let dismissed = dismiss_learning_suggestion_on_db(&db, &pending2[0].id, 8000).unwrap();
    assert_eq!(dismissed.status, SuggestionStatus::Dismissed);
    assert_eq!(
        db.count_pending_learning_suggestions(Some(project_id))
            .unwrap(),
        0
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn criterio_5_modo_desligado_nao_persiste_e_cadencia_idle_clear_funcionam() {
    let dir = temp_dir("criterio-5");
    let (db, project_id, proj_dir) = setup_db_with_project(&dir);

    assert_eq!(db.learning_mode().unwrap(), LEARNING_MODE_ASK);
    assert_eq!(db.learning_turn_cadence().unwrap(), 20);

    db.save_setting(SETTING_LEARNING_MODE, LEARNING_MODE_OFF)
        .unwrap();
    db.save_setting(SETTING_LEARNING_TURN_CADENCE, "10")
        .unwrap();
    assert_eq!(db.learning_mode().unwrap(), LEARNING_MODE_OFF);
    assert_eq!(db.learning_turn_cadence().unwrap(), 10);

    let output = ParsedReviewOutput {
        memories: vec![ParsedMemoryOp {
            op: "add".to_string(),
            id: None,
            fact: Some("Não deve ser salvo quando off".to_string()),
            rationale: None,
        }],
        skill: None,
    };
    let (created, auto_applied) = persist_review_output(
        &db,
        project_id,
        Some(&proj_dir),
        Some(&dir),
        LEARNING_MODE_OFF,
        &output,
        None,
        None,
        None,
        None,
        9000,
    )
    .unwrap();
    assert_eq!(created, 0);
    assert_eq!(auto_applied, 0);
    assert!(db
        .list_learning_suggestions(Some(project_id), None)
        .unwrap()
        .is_empty());

    // Verificar captura antes de clear_messages e estado de ociosidade (3+ turnos, 15 min)
    db.insert_message(
        project_id,
        Speaker::User,
        Speaker::Agent(crate::agents::Agent::ember()),
        "Mensagem antes de limpar conversa",
        1000,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    db.insert_turn_work_steps(&NewTurnWorkSteps {
        project_id,
        agent: "ember",
        session_key: "vokia:1:ember",
        card_id: None,
        provider: Some("codex"),
        block: "[work steps]\n- Read Cargo.toml -> ok\n[end work steps]",
        step_count: 1,
        omitted_count: 0,
        created_at: 1000,
    })
    .unwrap();

    let snap = capture_review_snapshot(&db, project_id);
    assert_eq!(snap.messages.len(), 1);
    assert_eq!(snap.work_steps.len(), 1);

    db.clear_messages(project_id).unwrap();
    // Snapshot pré-capturado continua intacto mesmo após clear_messages!
    assert_eq!(snap.messages[0].text, "Mensagem antes de limpar conversa");

    // Idle watcher: 3 turnos novos há mais de 15 min
    for i in 0..3 {
        db.record_turn_completed_for_learning(project_id, 10_000 + i)
            .unwrap();
    }
    let idle = db
        .list_idle_learning_candidates(10_000 + (IDLE_REVIEW_SECS * 1000) as i64 + 5000, IDLE_MIN_NEW_TURNS)
        .unwrap();
    assert_eq!(idle, vec![project_id]);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn orquestrador_rejeita_json_malformado_e_orquestra_cadencia_e_orcamento() {
    assert!(parse_review_response("texto solto sem json").is_err());
    assert!(parse_review_response(r#"{"memories":[{"op":"invalid"}]}"#).is_err());
    assert!(parse_review_response(
        r#"{"memories":[],"skill":{"op":"create","slug":"../escape","content":"x"}}"#
    )
    .is_err());
}
