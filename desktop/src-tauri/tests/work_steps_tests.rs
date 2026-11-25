use super::*;
use crate::claude::{ToolActivity, ToolCategory, ToolDetail, ToolStatus, TurnActivity};
use crate::db::{
    load_recent_work_steps_safe, work_steps_session_key, Db, DbState, NewProject,
    NewTurnWorkSteps,
};

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vokia-work-steps-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn captura_compacta_turno_com_cargo_test_edicoes_e_mcp_sem_reasoning_cru() {
    let giant_build_dump = format!(
        "{}\nFINAL_TEST_SUMMARY: 42 passed; 0 failed",
        "Compiling crate-xyz v0.1.0\n".repeat(400)
    );
    let activity = TurnActivity {
        thinking_ms: Some(1800),
        thinking_text: Some(
            "REASONING_CRU_SECRETO: vou pensar passo a passo internamente antes de agir".to_string(),
        ),
        current_step_label: None,
        plan: None,
        tools: vec![
            ToolActivity {
                call_id: "t0".into(),
                name: "Read".into(),
                category: ToolCategory::Read,
                target: "src/main.rs".into(),
                status: ToolStatus::Ok,
                description: None,
                added: None,
                removed: None,
                change: None,
                duration_ms: Some(15),
                detail: None,
                result: None,
                exit_code: None,
            },
            ToolActivity {
                call_id: "t1".into(),
                name: "Edit".into(),
                category: ToolCategory::Edit,
                target: "src/a.rs".into(),
                status: ToolStatus::Ok,
                description: None,
                added: Some(10),
                removed: Some(2),
                change: None,
                duration_ms: Some(30),
                detail: None,
                result: None,
                exit_code: None,
            },
            ToolActivity {
                call_id: "t2".into(),
                name: "Write".into(),
                category: ToolCategory::Edit,
                target: "src/b.rs".into(),
                status: ToolStatus::Ok,
                description: None,
                added: Some(25),
                removed: Some(0),
                change: None,
                duration_ms: Some(20),
                detail: None,
                result: None,
                exit_code: None,
            },
            ToolActivity {
                call_id: "t3".into(),
                name: "Bash".into(),
                category: ToolCategory::Command,
                target: "cargo test".into(),
                status: ToolStatus::Ok,
                description: None,
                added: None,
                removed: None,
                change: None,
                duration_ms: Some(2500),
                detail: None,
                result: Some(giant_build_dump),
                exit_code: Some(0),
            },
            ToolActivity {
                call_id: "t4".into(),
                name: "mcp__vokia__read_card".into(),
                category: ToolCategory::Mcp,
                target: String::new(),
                status: ToolStatus::Ok,
                description: None,
                added: None,
                removed: None,
                change: None,
                duration_ms: Some(12),
                detail: None,
                result: None,
                exit_code: None,
            },
            ToolActivity {
                call_id: "t5".into(),
                name: "WebSearch".into(),
                category: ToolCategory::Web,
                target: "rusqlite wal mode".into(),
                status: ToolStatus::Ok,
                description: None,
                added: None,
                removed: None,
                change: None,
                duration_ms: Some(100),
                detail: None,
                result: None,
                exit_code: None,
            },
            ToolActivity {
                call_id: "t6".into(),
                name: "mcp__vokia__app_command_call".into(),
                category: ToolCategory::App,
                target: "list_agents".into(),
                status: ToolStatus::Ok,
                description: None,
                added: None,
                removed: None,
                change: None,
                duration_ms: Some(10),
                detail: Some(ToolDetail::AppCommand {
                    command: "list_agents".into(),
                    domain: Some("agents".into()),
                    subject: None,
                }),
                result: None,
                exit_code: None,
            },
        ],
    };

    let collector = WorkStepCollector::from_turn_activity(&activity);
    assert!(!collector.is_empty());
    let formatted = collector.format().expect("deve gerar bloco [work steps]");

    assert!(formatted.block.starts_with(WORK_STEPS_OPEN));
    assert!(formatted.block.ends_with(WORK_STEPS_CLOSE));
    assert!(
        formatted.block.chars().count() <= MAX_TURN_WORK_STEPS_CHARS,
        "bloco excedeu 4.000 chars: {}",
        formatted.block.chars().count()
    );
    assert_eq!(formatted.omitted_count, 0);
    assert!(formatted.block.contains("files (ok): edit src/a.rs, create src/b.rs"));
    assert!(formatted.block.contains("$ cargo test (exit 0)"));
    assert!(formatted.block.contains("FINAL_TEST_SUMMARY: 42 passed; 0 failed"));
    assert!(formatted.block.contains("tool vokia/read_card (ok)"));
    assert!(formatted.block.contains("searched: rusqlite wal mode"));
    assert!(formatted.block.contains("tool vokia/list_agents (ok)"));
    // Nunca vaza reasoning cru nem o dump gigante inteiro de compilação.
    assert!(!formatted.block.contains("REASONING_CRU_SECRETO"));
    assert!(formatted.block.len() < 2000);
}

#[test]
fn quarenta_comandos_longos_descartam_passos_antigos_e_respeitam_4000_chars() {
    let mut collector = WorkStepCollector::new();
    for i in 1..=40 {
        let output = format!("linha inicial {i}\n{}\nFIM_CMD_{i:02}", "saida ".repeat(250));
        collector.record_command(
            &format!("cargo test --package mod_{i:02} -- --nocapture"),
            Some(0),
            "ok",
            Some(&output),
        );
    }

    assert_eq!(collector.len(), 40);
    for step in &collector.steps {
        assert!(
            step.chars().count() <= MAX_STEP_CHARS,
            "passo excedeu 1.000 chars: {}",
            step.chars().count()
        );
    }

    let formatted = collector.format().expect("deve gerar bloco truncado");
    assert!(
        formatted.block.chars().count() <= MAX_TURN_WORK_STEPS_CHARS,
        "bloco excedeu 4.000 chars: {}",
        formatted.block.chars().count()
    );
    assert!(formatted.omitted_count > 0);
    assert_eq!(formatted.step_count, 40);
    let expected_notice = format!("({} passos anteriores omitidos)", formatted.omitted_count);
    assert!(
        formatted.block.contains(&expected_notice),
        "esperava aviso '{expected_notice}' em:\n{}",
        formatted.block
    );
    // Mantém os comandos mais recentes e descarta o primeiro.
    assert!(formatted.block.contains("mod_40"));
    assert!(formatted.block.contains("FIM_CMD_40"));
    assert!(!formatted.block.contains("mod_01"));
}

#[test]
fn segredos_em_comandos_saidas_notas_e_handoff_sao_redigidos() {
    let mut collector = WorkStepCollector::new();
    collector.record_command(
        "curl -H 'Authorization: Bearer super-secret-bearer-token-999' https://alice:s3cr3tPass@api.example.com/v1?api_key=my_query_key_123",
        Some(0),
        "ok",
        Some(
            "GITHUB_TOKEN=ghp_1234567890abcdefABCDEF1234567890\n\
             OPENAI_API_KEY=sk-proj-secretvalue9876543210\n\
             {\"access_token\":\"json_secret_token_555\"}\n\
             -----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEAsecretkeydata\n-----END RSA PRIVATE KEY-----\n\
             status=ok",
        ),
    );
    collector.record_note("Usando sk-ant-api03-secretkey999999 para validar");
    collector.record_web_search("docs token=topsecret999");

    let formatted = collector.format().expect("deve formatar passos redigidos");
    let block = &formatted.block;

    for leaked in [
        "super-secret-bearer-token-999",
        "s3cr3tPass",
        "my_query_key_123",
        "ghp_1234567890abcdef",
        "sk-proj-secretvalue",
        "json_secret_token_555",
        "MIIEpAIBAAKCAQEAsecretkeydata",
        "sk-ant-api03-secretkey999999",
        "topsecret999",
    ] {
        assert!(
            !block.contains(leaked),
            "segredo '{leaked}' vazou no bloco:\n{block}"
        );
    }
    assert!(block.contains("[oculto]"));
    assert!(block.contains("status=ok"));

    let packet = build_handoff_packet(&HandoffInput {
        from_provider: Some("claude (sonnet)".into()),
        to_provider: Some("codex (gpt-5.3-codex)".into()),
        recent_messages: vec![HandoffMessage {
            author: "user".into(),
            text: "Use API_KEY=very_secret_handoff_key_123".into(),
        }],
        work_steps_blocks: vec![block.clone()],
        ..Default::default()
    });
    assert!(!packet.text.contains("very_secret_handoff_key_123"));
    assert!(packet.text.contains("[oculto]"));
}

#[test]
fn pacote_de_handoff_deterministico_respeita_teto_32k_e_orcamentos() {
    let mut collector = WorkStepCollector::new();
    collector.record_command("cargo check --bins", Some(0), "ok", Some("Finished dev profile"));
    collector.record_files(
        "ok",
        &[
            FileChange {
                kind: FileChangeKind::Edit,
                path: "src/orchestrator/turns.rs".into(),
            },
            FileChange {
                kind: FileChangeKind::Create,
                path: "src/orchestrator/work_steps.rs".into(),
            },
        ],
    );
    let steps_block = collector.format().unwrap().block;

    let huge_message = format!(
        "INICIO_CRITICO_DA_MENSAGEM {} FIM_CRITICO_DA_MENSAGEM",
        "conteudo repetitivo intermediario ".repeat(1_200)
    );
    let older_messages: Vec<HandoffMessage> = (1..=30)
        .map(|i| HandoffMessage {
            author: "axiom".into(),
            text: format!("Decisao antiga #{i}: {}", "detalhe historico ".repeat(25)),
        })
        .collect();
    let recent_messages = vec![
        HandoffMessage {
            author: "user".into(),
            text: "Mensagem recente curta 1".into(),
        },
        HandoffMessage {
            author: "kairo".into(),
            text: huge_message,
        },
    ];

    let input = HandoffInput {
        from_provider: Some("claude (sonnet)".into()),
        to_provider: Some("codex (gpt-5.3-codex)".into()),
        reason: Some("limite de uso".into()),
        objective: Some("Implementar requisito 091 de work steps e handoff".into()),
        current_request: Some("Finalizar integração e rodar testes".into()),
        card_summary: Some("Card item-091: Captura compacta de etapas de trabalho".into()),
        open_plan_steps: vec![
            "[ ] c1: Criar work_steps.rs".into(),
            "[ ] c2: Integrar handoff no fallback".into(),
        ],
        recent_messages,
        older_messages,
        work_steps_blocks: vec![steps_block],
        history_pointers: Vec::new(),
    };

    let packet_a = build_handoff_packet(&input);
    let packet_b = build_handoff_packet(&input);

    assert_eq!(packet_a, packet_b, "pacote deve ser 100% determinístico");
    assert!(
        packet_a.total_chars <= HANDOFF_MAX_CHARS,
        "pacote excedeu 32.000 chars: {}",
        packet_a.total_chars
    );
    assert!(packet_a.continuity_chars <= HANDOFF_CONTINUITY_BUDGET);
    assert!(packet_a.work_steps_chars <= HANDOFF_WORK_STEPS_BUDGET);
    assert!(packet_a.older_chars <= HANDOFF_OLDER_BUDGET);
    assert!(packet_a.text.contains("troca_provedor: claude (sonnet) -> codex (gpt-5.3-codex) (limite de uso)"));
    assert!(packet_a.text.contains("etapas_abertas:"));
    assert!(packet_a.text.contains("[ ] c1: Criar work_steps.rs"));
    assert!(packet_a.text.contains("history_search"));
    assert!(packet_a.text.contains("history_read"));
    assert!(packet_a.text.contains(WORK_STEPS_OPEN));
    assert!(packet_a.text.contains("$ cargo check --bins (exit 0)"));
    assert!(packet_a.text.contains("INICIO_CRITICO_DA_MENSAGEM"));
    assert!(packet_a.text.contains("FIM_CRITICO_DA_MENSAGEM"));
    assert!(packet_a.text.contains("excertos_anteriores:"));
}

#[test]
fn handoff_reaproveita_sobra_de_orcamento_para_mais_texto_recente_e_compacta_bloco_grande() {
    // Cria um bloco [work steps] próximo de 4.000 chars para validar compactação dentro do teto de 2.000.
    let mut collector = WorkStepCollector::new();
    for i in 1..=20 {
        collector.record_command(
            &format!("cargo test test_case_{i:02}"),
            Some(0),
            "ok",
            Some(&"resultado detalhado ".repeat(25)),
        );
    }
    let big_block = collector.format().unwrap().block;
    assert!(big_block.chars().count() > HANDOFF_WORK_STEPS_BUDGET);

    // Sem older_messages, a sobra de 4.000 + sobra de continuidade deve ir para mensagens recentes.
    let long_recent = format!("START_RECENT {} END_RECENT", "x".repeat(26_500));
    let packet = build_handoff_packet(&HandoffInput {
        to_provider: Some("gemini".into()),
        recent_messages: vec![HandoffMessage {
            author: "user".into(),
            text: long_recent,
        }],
        work_steps_blocks: vec![big_block],
        ..Default::default()
    });

    assert!(packet.total_chars <= HANDOFF_MAX_CHARS);
    assert!(packet.work_steps_chars <= HANDOFF_WORK_STEPS_BUDGET);
    assert!(
        packet.recent_chars > HANDOFF_RECENT_BUDGET,
        "deveria reaproveitar sobra de orçamento para texto recente (obteve {})",
        packet.recent_chars
    );
    assert!(packet.text.contains("START_RECENT"));
    assert!(packet.text.contains("END_RECENT"));
    assert!(packet.text.contains(WORK_STEPS_OPEN));
    assert!(packet.text.contains(WORK_STEPS_CLOSE));
    assert!(packet.text.contains("test_case_20"));
}

#[test]
fn persistencia_sqlite_paginacao_isolamento_e_tolerancia_a_corrupcao() {
    let dir = temp_dir("sqlite");
    let db = Db::open(&dir).expect("abre banco de teste");
    let project = db
        .insert_project(&NewProject {
            name: "proj",
            symbol: "folder",
            description: "",
            path: "C:/tmp/proj-work-steps",
            origin_kind: "folder",
            origin_repo: None,
        })
        .expect("insere projeto");

    let key_card_1 = work_steps_session_key(project.id, "kairo", Some("item-1"));
    let key_card_2 = work_steps_session_key(project.id, "kairo", Some("item-2"));
    assert_ne!(key_card_1, key_card_2);

    for turn in 1..=3 {
        let mut collector = WorkStepCollector::new();
        collector.record_command(&format!("echo turno-{turn}"), Some(0), "ok", None);
        let formatted = collector.format().unwrap();
        db.insert_turn_work_steps(&NewTurnWorkSteps {
            project_id: project.id,
            agent: "kairo",
            session_key: &key_card_1,
            card_id: Some("item-1"),
            provider: Some("claude"),
            block: &formatted.block,
            step_count: formatted.step_count,
            omitted_count: formatted.omitted_count,
            created_at: 1_000 + turn as i64,
        })
        .expect("insere work steps");
    }

    // Outro card não vaza na consulta do card 1.
    db.insert_turn_work_steps(&NewTurnWorkSteps {
        project_id: project.id,
        agent: "kairo",
        session_key: &key_card_2,
        card_id: Some("item-2"),
        provider: Some("codex"),
        block: "[work steps]\n$ echo outro-card (exit 0)\n[end work steps]",
        step_count: 1,
        omitted_count: 0,
        created_at: 2_000,
    })
    .unwrap();

    // Paginação (mais recente -> mais antigo) para history_read.
    let page = db
        .list_turn_work_steps(project.id, "kairo", &key_card_1, 2, 1)
        .unwrap();
    assert_eq!(page.len(), 2);
    assert!(page[0].block.contains("turno-2"));
    assert!(page[1].block.contains("turno-1"));

    // Insere linhas corrompidas/malformadas diretamente no SQLite: leitura tolerante pula só elas.
    {
        let conn = db.conn().unwrap();
        conn.execute(
            "INSERT INTO turn_work_steps (project_id, agent, session_key, card_id, provider, block, step_count, omitted_count, created_at) \
             VALUES (?1, 'kairo', ?2, 'item-1', 'claude', 'lixo-corrompido-sem-marcadores', 1, 0, 1004)",
            rusqlite::params![project.id, &key_card_1],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO turn_work_steps (project_id, agent, session_key, card_id, provider, block, step_count, omitted_count, created_at) \
             VALUES (?1, 'kairo', ?2, 'item-1', 'claude', X'FFFE00', 1, 0, 1005)",
            rusqlite::params![project.id, &key_card_1],
        )
        .unwrap();
    }

    let loaded = db.load_recent_work_steps_tolerant(project.id, "kairo", &key_card_1, 10);
    assert_eq!(loaded.len(), 3, "deve ignorar linhas corrompidas e manter as 3 válidas");
    assert!(loaded[0].block.contains("turno-1"));
    assert!(loaded[2].block.contains("turno-3"));
    assert!(loaded.iter().all(|r| !r.block.contains("outro-card")));

    // Tabela ausente/corrompida nunca trava nem falha.
    {
        let conn = db.conn().unwrap();
        conn.execute("DROP TABLE turn_work_steps", []).unwrap();
    }
    let after_drop = db.load_recent_work_steps_tolerant(project.id, "kairo", &key_card_1, 10);
    assert!(after_drop.is_empty());

    // Arquivo de banco corrompido (DbState::open falha) -> helper seguro devolve vazio e handoff segue.
    let corrupt_dir = temp_dir("corrupt");
    std::fs::write(corrupt_dir.join("vokia.db"), b"isto nao e um arquivo sqlite valido").unwrap();
    let corrupt_state = DbState::open(&corrupt_dir);
    assert!(corrupt_state.db().is_none());
    let safe_blocks: Vec<String> = load_recent_work_steps_safe(
        corrupt_state.db(),
        project.id,
        "kairo",
        &key_card_1,
        10,
    )
    .into_iter()
    .map(|r| r.block)
    .collect();
    assert!(safe_blocks.is_empty());

    let packet = build_handoff_packet(&HandoffInput {
        objective: Some("Seguir conversa mesmo sem work steps".into()),
        work_steps_blocks: safe_blocks,
        ..Default::default()
    });
    assert!(packet.text.contains("Seguir conversa mesmo sem work steps"));

    drop(db);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&corrupt_dir);
}
