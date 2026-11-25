use std::sync::atomic::{AtomicU32, Ordering};

use crate::agents::Agent;
use crate::claude::{ClaudeError, ToolActivity, ToolCategory, ToolStatus, TurnActivity};
use crate::commands::dispatcher;
use crate::db::{
    DbError, DbState, SETTING_CONTINUATION_ENABLED, SETTING_CONTINUATION_MAX_COUNT,
};
use crate::events::VokiaEvent;
use crate::mcp::support::build_app;
use crate::orchestrator::error::FlowError;
use crate::orchestrator::turn_plan::{TurnPlan, TurnPlanStep, TurnPlanStepStatus};
use crate::session_key::SessionKey;
use crate::usage::{
    model::{ProviderPlanUsage, UsageWindow, FIVE_HOUR_MINS},
    UsageEngine,
};

use super::*;

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

fn step(id: &str, text: &str, status: TurnPlanStepStatus) -> TurnPlanStep {
    TurnPlanStep {
        id: id.to_string(),
        text: text.to_string(),
        active_text: None,
        status,
    }
}

fn plan_with(steps: Vec<TurnPlanStep>) -> TurnPlan {
    TurnPlan {
        explanation: None,
        steps,
    }
    .normalize()
}

fn setup_app(name: &str) -> (tauri::App, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "vokia-continuation-{name}-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("diretório temporário");
    let app = build_app(&dir);
    (app, dir)
}

// =========================================================================
// Critério 1: TurnPlan com 2 etapas `pending` dispara continuação `plan`
// listando apenas as 2 etapas abertas.
// =========================================================================

#[test]
fn criterio_1_etapas_pendentes_no_turn_plan_disparam_continuacao_listando_apenas_as_abertas() {
    let mut engine = PlanContinuation::default();
    let plan = plan_with(vec![
        step("s1", "Criar migração do banco", TurnPlanStepStatus::Completed),
        step("s2", "Implementar endpoint REST", TurnPlanStepStatus::Pending),
        step("s3", "Escrever testes de integração", TurnPlanStepStatus::Pending),
    ]);

    let decision = engine.consider(&TurnEndInput {
        reply_text: "Migração criada com sucesso.".to_string(),
        turn_plan: Some(plan),
        ..Default::default()
    });

    match decision {
        ContinuationDecision::Continue {
            trigger,
            attempt,
            max_attempts,
            prompt,
            marker,
        } => {
            assert_eq!(trigger, ContinuationTrigger::Plan);
            assert_eq!(attempt, 1);
            assert_eq!(max_attempts, DEFAULT_CONTINUATION_MAX_COUNT);
            assert_eq!(marker, "Continuação automática 1/5 · etapas pendentes");
            assert!(prompt.contains("Implementar endpoint REST"));
            assert!(prompt.contains("Escrever testes de integração"));
            assert!(!prompt.contains("Criar migração do banco"));
        }
        other => panic!("esperava Continue(Plan), recebeu {other:?}"),
    }
    assert_eq!(engine.count(), 1);
    assert_eq!(engine.plan_count(), 1);
}

#[test]
fn plan_conclui_quando_todas_as_etapas_ficam_completed() {
    let mut engine = PlanContinuation::default();
    let initial = plan_with(vec![
        step("s1", "Etapa 1", TurnPlanStepStatus::InProgress),
        step("s2", "Etapa 2", TurnPlanStepStatus::Pending),
    ]);
    assert!(matches!(
        engine.consider(&TurnEndInput {
            turn_plan: Some(initial),
            ..Default::default()
        }),
        ContinuationDecision::Continue {
            trigger: ContinuationTrigger::Plan,
            attempt: 1,
            ..
        }
    ));

    // Progresso: s1 concluída, resta s2.
    let progressed = plan_with(vec![
        step("s1", "Etapa 1", TurnPlanStepStatus::Completed),
        step("s2", "Etapa 2", TurnPlanStepStatus::InProgress),
    ]);
    assert!(matches!(
        engine.consider(&TurnEndInput {
            turn_plan: Some(progressed),
            ..Default::default()
        }),
        ContinuationDecision::Continue {
            trigger: ContinuationTrigger::Plan,
            attempt: 2,
            ..
        }
    ));

    // Tudo concluído -> Complete.
    let done = plan_with(vec![
        step("s1", "Etapa 1", TurnPlanStepStatus::Completed),
        step("s2", "Etapa 2", TurnPlanStepStatus::Completed),
    ]);
    assert_eq!(
        engine.consider(&TurnEndInput {
            turn_plan: Some(done),
            reply_text: "Todas as etapas concluídas.".to_string(),
            ..Default::default()
        }),
        ContinuationDecision::Complete
    );
}

// =========================================================================
// Critério 2: Estagnação — se as mesmas 2 etapas continuam abertas sem avanço,
// detecta estagnação (`Stopped::Stagnated`) e para imediatamente.
// =========================================================================

#[test]
fn criterio_2_detecta_estagnacao_quando_mesmas_etapas_permanecem_abertas() {
    let mut engine = PlanContinuation::default();
    let plan = plan_with(vec![
        step("s1", "Etapa concluída", TurnPlanStepStatus::Completed),
        step("s2", "Corrigir validação", TurnPlanStepStatus::Pending),
        step("s3", "Rodar suíte de testes", TurnPlanStepStatus::Pending),
    ]);

    let first = engine.consider(&TurnEndInput {
        turn_plan: Some(plan.clone()),
        ..Default::default()
    });
    assert!(matches!(
        first,
        ContinuationDecision::Continue {
            trigger: ContinuationTrigger::Plan,
            attempt: 1,
            ..
        }
    ));

    // Segundo turno encerra mantendo exatamente as mesmas 2 etapas abertas.
    let second = engine.consider(&TurnEndInput {
        turn_plan: Some(plan),
        ..Default::default()
    });
    assert_eq!(
        second,
        ContinuationDecision::Stopped {
            reason: ContinuationStopReason::Stagnated,
            trigger: Some(ContinuationTrigger::Plan),
        }
    );
    assert_eq!(engine.count(), 1);
}

// =========================================================================
// Critério 3: Anúncio de trabalho sem execução (PT-BR e EN), <= 400 chars,
// teto de até 2 empurrões `next_step` por cadeia.
// =========================================================================

#[test]
fn criterio_3_detecta_anuncio_pt_br_e_respeita_teto_de_2_empurroes() {
    let mut engine = PlanContinuation::default();
    let input = TurnEndInput {
        reply_text: "Agora vou rodar `cargo test` para verificar.".to_string(),
        executed_action_tool: false,
        ..Default::default()
    };

    let first = engine.consider(&input);
    match first {
        ContinuationDecision::Continue {
            trigger,
            attempt,
            max_attempts,
            marker,
            ..
        } => {
            assert_eq!(trigger, ContinuationTrigger::NextStep);
            assert_eq!(attempt, 1);
            assert_eq!(max_attempts, 5);
            assert_eq!(marker, "Continuação automática 1/5 · próximo passo anunciado");
        }
        other => panic!("esperava Continue(NextStep #1), recebeu {other:?}"),
    }

    let second = engine.consider(&TurnEndInput {
        reply_text: "Deixa eu verificar o arquivo de testes agora.".to_string(),
        executed_action_tool: false,
        ..Default::default()
    });
    assert!(matches!(
        second,
        ContinuationDecision::Continue {
            trigger: ContinuationTrigger::NextStep,
            attempt: 2,
            ..
        }
    ));

    // Terceiro empurrão excede o teto de 2 para `next_step`.
    let third = engine.consider(&TurnEndInput {
        reply_text: "Vou corrigir o erro encontrado.".to_string(),
        executed_action_tool: false,
        ..Default::default()
    });
    assert_eq!(
        third,
        ContinuationDecision::Stopped {
            reason: ContinuationStopReason::NextStepCapReached,
            trigger: Some(ContinuationTrigger::NextStep),
        }
    );
    assert_eq!(engine.next_step_count(), MAX_NEXT_STEP_NUDGES);
}

#[test]
fn detecta_anuncios_em_en_e_ignora_respostas_acima_de_400_caracteres() {
    assert!(detect_next_step_announcement(
        "Let me now run the unit tests to verify the fix."
    ));
    assert!(detect_next_step_announcement("Shall I continue?"));
    assert!(detect_next_step_announcement("Vou corrigir o parser agora."));
    assert!(detect_next_step_announcement("Deixa eu verificar o arquivo."));

    // Resposta longa (> 400 chars) nunca dispara `next_step`.
    let long_reply = format!("Agora vou rodar os testes. {}", "detalhe ".repeat(60));
    assert!(long_reply.chars().count() > NEXT_STEP_MAX_CHARS);
    assert!(!detect_next_step_announcement(&long_reply));
}

// =========================================================================
// Critério 4: Exceções obrigatórias — pergunta real aguardando decisão humana,
// frases de aprovação ou `ask_human` pendente NUNCA disparam continuação.
// =========================================================================

#[test]
fn criterio_4_nao_dispara_para_pergunta_real_aprovacao_ou_ask_human() {
    let mut engine = PlanContinuation::default();

    // Pergunta real do critério 4 de 089.md.
    let real_question = "Posso remover a tabela antiga ou prefere manter por compatibilidade?";
    assert!(!detect_next_step_announcement(real_question));
    assert_eq!(
        engine.consider(&TurnEndInput {
            reply_text: real_question.to_string(),
            ..Default::default()
        }),
        ContinuationDecision::Complete
    );

    // Frases de aprovação / confirmação humana.
    for phrase in [
        "Vou aplicar a migração se você aprovar.",
        "Agora vou rodar o script, aguardando sua confirmação.",
        "Deixa eu remover os arquivos antigos se você concordar.",
        "I'll deploy the changes if you approve.",
        "Let me run the migration, waiting for your confirmation.",
        "Qual abordagem você prefere para o cache?",
    ] {
        assert!(
            !detect_next_step_announcement(phrase),
            "não deveria disparar para: {phrase}"
        );
    }

    // Decisão pendente em `ask_human` / `request_decision` bloqueia qualquer gatilho (mesmo com plano aberto).
    let plan = plan_with(vec![step("s1", "Etapa pendente", TurnPlanStepStatus::Pending)]);
    assert_eq!(
        engine.consider(&TurnEndInput {
            reply_text: "Agora vou rodar os testes.".to_string(),
            turn_plan: Some(plan),
            has_pending_human_decision: true,
            ..Default::default()
        }),
        ContinuationDecision::Stopped {
            reason: ContinuationStopReason::PendingHumanDecision,
            trigger: None,
        }
    );
}

// =========================================================================
// Gatilho 3: `finish_check` (até 3 checagens quando falta evidência).
// =========================================================================

#[test]
fn finish_check_dispara_ate_3_vezes_sem_evidencia_e_conclui_com_evidencia() {
    let mut engine = PlanContinuation::new(ContinuationConfig {
        enabled: true,
        max_count: 10,
    });
    let criteria = vec!["C1: cargo test passando sem regressões".to_string()];

    for expected in 1..=MAX_FINISH_CHECK_NUDGES {
        let decision = engine.consider(&TurnEndInput {
            reply_text: "Implementação finalizada.".to_string(),
            pending_verification_items: criteria.clone(),
            has_verification_evidence: false,
            ..Default::default()
        });
        assert!(matches!(
            decision,
            ContinuationDecision::Continue {
                trigger: ContinuationTrigger::FinishCheck,
                attempt,
                ..
            } if attempt == expected
        ));
    }

    // 4ª tentativa bate no teto de 3 checagens.
    assert_eq!(engine.finish_check_count(), MAX_FINISH_CHECK_NUDGES);
    assert_eq!(
        engine.consider(&TurnEndInput {
            reply_text: "Implementação finalizada.".to_string(),
            pending_verification_items: criteria.clone(),
            has_verification_evidence: false,
            ..Default::default()
        }),
        ContinuationDecision::Stopped {
            reason: ContinuationStopReason::FinishCheckCapReached,
            trigger: Some(ContinuationTrigger::FinishCheck),
        }
    );

    // Com evidência concreta de teste/build, conclui diretamente.
    let mut verified_engine = PlanContinuation::default();
    let activity = TurnActivity {
        thinking_ms: None,
        thinking_text: None,
        current_step_label: None,
        plan: None,
        tools: vec![ToolActivity {
            call_id: "c1".to_string(),
            name: "Bash".to_string(),
            category: ToolCategory::Command,
            target: "cargo test --quiet".to_string(),
            status: ToolStatus::Ok,
            description: None,
            added: None,
            removed: None,
            change: None,
            duration_ms: Some(120),
            detail: None,
            result: Some("ok".to_string()),
            exit_code: Some(0),
        }],
    };
    assert!(has_verification_evidence(Some(&activity)));
    assert_eq!(
        verified_engine.consider(&TurnEndInput {
            reply_text: "Testes verdes.".to_string(),
            pending_verification_items: criteria,
            has_verification_evidence: has_verification_evidence(Some(&activity)),
            ..Default::default()
        }),
        ContinuationDecision::Complete
    );
}

// =========================================================================
// Critério 5: Queda abrupta da CLI / falha de compactação -> 1 retomada
// automática preservando `[work steps]` (091).
// =========================================================================

#[test]
fn criterio_5_retoma_uma_vez_apos_queda_da_cli_preservando_work_steps() {
    let mut engine = PlanContinuation::default();
    let work_block = "[work steps]\n- edit src/orchestrator/turns.rs (ok)\n- cmd cargo check --bins (exit=0)\n[end work steps]";

    let err = FlowError::Claude(ClaudeError::ForCli {
        cli: "codex".to_string(),
        inner: Box::new(ClaudeError::Exit {
            code: Some(137),
            message: "killed unexpectedly".to_string(),
        }),
    });
    assert_eq!(classify_turn_failure(&err), TurnFailureKind::ProviderExit);

    let first = engine.consider(&TurnEndInput {
        provider_exited_abruptly: true,
        work_steps_blocks: vec![work_block.to_string()],
        ..Default::default()
    });
    match first {
        ContinuationDecision::Continue {
            trigger,
            attempt,
            prompt,
            marker,
            ..
        } => {
            assert_eq!(trigger, ContinuationTrigger::ProviderExit);
            assert_eq!(attempt, 1);
            assert_eq!(
                marker,
                "Continuação automática 1/5 · retomada após queda da CLI"
            );
            assert!(prompt.contains("[work steps]"));
            assert!(prompt.contains("src/orchestrator/turns.rs"));
            assert!(prompt.contains("[end work steps]"));
        }
        other => panic!("esperava Continue(ProviderExit), recebeu {other:?}"),
    }

    // Segunda queda na mesma cadeia não repete (teto de 1 retomada).
    assert_eq!(engine.recovery_count(), MAX_RECOVERY_NUDGES);
    let second = engine.consider(&TurnEndInput {
        provider_exited_abruptly: true,
        work_steps_blocks: vec![work_block.to_string()],
        ..Default::default()
    });
    assert_eq!(
        second,
        ContinuationDecision::Stopped {
            reason: ContinuationStopReason::RecoveryCapReached,
            trigger: Some(ContinuationTrigger::ProviderExit),
        }
    );
}

#[test]
fn falha_de_compactacao_e_classificada_e_retomada_uma_vez() {
    let err = FlowError::Claude(ClaudeError::Reported(
        "Context compaction failed during turn".to_string(),
    ));
    assert_eq!(classify_turn_failure(&err), TurnFailureKind::Compaction);

    // Erro de limite nunca é classificado como queda recuperável.
    let limit_err = FlowError::Claude(ClaudeError::LimitExhausted);
    assert_eq!(classify_turn_failure(&limit_err), TurnFailureKind::Other);

    let mut engine = PlanContinuation::default();
    let decision = engine.consider(&TurnEndInput {
        compaction_failed: true,
        work_steps_blocks: vec!["[work steps]\n- edit app.rs (ok)\n[end work steps]".to_string()],
        ..Default::default()
    });
    assert!(matches!(
        decision,
        ContinuationDecision::Continue {
            trigger: ContinuationTrigger::Compaction,
            attempt: 1,
            ..
        }
    ));
}

// =========================================================================
// Critério 6: Cancelamento imediato (nova mensagem / Parar / pausa) e
// respeito imediato à pausa por limite de uso (90% / UsageLimitGate 083).
// =========================================================================

#[test]
fn criterio_6_cancela_imediatamente_por_nova_mensagem_ou_pausa_e_respeita_limite_de_uso() {
    let registry = ContinuationRegistry::default();
    let key = SessionKey {
        project_id: 7,
        agent: Agent::axiom(),
    };
    let token = registry.register(&key);
    assert!(!token.is_cancelled());

    // Simula usuário enviando nova mensagem ou pausando o agente.
    registry.cancel_agent(7, &Agent::axiom());
    assert!(token.is_cancelled());

    // Cancelamento direto por sessão também descarta a cadeia ativa.
    let session_token = registry.register(&key);
    assert!(!session_token.is_cancelled());
    registry.cancel_session(&key);
    assert!(session_token.is_cancelled());

    let mut engine = PlanContinuation::default();
    assert_eq!(engine.config(), ContinuationConfig::default());
    assert!(!engine.is_cancelled());
    let open_plan = plan_with(vec![step("s1", "Continuar tarefa", TurnPlanStepStatus::Pending)]);
    assert_eq!(
        engine.consider(&TurnEndInput {
            turn_plan: Some(open_plan.clone()),
            cancelled: token.is_cancelled(),
            ..Default::default()
        }),
        ContinuationDecision::Stopped {
            reason: ContinuationStopReason::Cancelled,
            trigger: None,
        }
    );
    assert!(engine.is_cancelled());

    let mut direct_cancel = PlanContinuation::default();
    direct_cancel.cancel();
    assert!(direct_cancel.is_cancelled());

    // Verifica limite de uso (90% em 083) de forma agnóstica a provedor (ex.: codex, claude, gemini).
    let usage = UsageEngine::default();
    let now_ms = 1_700_000_000_000u64;
    usage.record_provider_usage(ProviderPlanUsage {
        provider_id: "codex".to_string(),
        label: Some("Codex".to_string()),
        primary: Some(UsageWindow::new(90, 0, FIVE_HOUR_MINS)),
        secondary: None,
        tertiary: None,
        groups: Vec::new(),
        credits_available: None,
        reset_credits_available: None,
        updated_at_ms: now_ms,
        read_failed: false,
    });
    assert!(usage.is_provider_paused_by_limit("codex", Some("gpt-5"), now_ms));

    // Outro provedor sem cota esgotada segue liberado.
    assert!(!usage.is_provider_paused_by_limit("gemini", Some("gemini-2.5-pro"), now_ms));

    // Limite ativo por modelo no UsageLimitGate também pausa imediatamente.
    usage.mark_limit_reached("gemini", "gemini-2.5-pro", Some(1_700_003_600), now_ms);
    assert!(usage.is_provider_paused_by_limit("gemini", Some("gemini-2.5-pro"), now_ms));

    let mut limit_engine = PlanContinuation::default();
    assert_eq!(
        limit_engine.consider(&TurnEndInput {
            turn_plan: Some(open_plan),
            usage_limit_paused: usage.is_provider_paused_by_limit("codex", Some("gpt-5"), now_ms),
            ..Default::default()
        }),
        ContinuationDecision::Stopped {
            reason: ContinuationStopReason::UsageLimitPaused,
            trigger: None,
        }
    );
}

#[test]
fn respeita_teto_global_e_desativacao_via_configuracao() {
    let mut disabled = PlanContinuation::new(ContinuationConfig {
        enabled: false,
        max_count: 5,
    });
    assert_eq!(
        disabled.consider(&TurnEndInput {
            reply_text: "Agora vou rodar os testes.".to_string(),
            ..Default::default()
        }),
        ContinuationDecision::Stopped {
            reason: ContinuationStopReason::Disabled,
            trigger: None,
        }
    );

    let mut capped = PlanContinuation::new(ContinuationConfig {
        enabled: true,
        max_count: 2,
    });
    for i in 1..=2 {
        let plan = plan_with(vec![step(
            &format!("s{i}"),
            &format!("Etapa {i}"),
            TurnPlanStepStatus::Pending,
        )]);
        assert!(matches!(
            capped.consider(&TurnEndInput {
                turn_plan: Some(plan),
                ..Default::default()
            }),
            ContinuationDecision::Continue { attempt, max_attempts: 2, .. } if attempt == i
        ));
    }

    let plan3 = plan_with(vec![step("s3", "Etapa 3", TurnPlanStepStatus::Pending)]);
    assert_eq!(
        capped.consider(&TurnEndInput {
            turn_plan: Some(plan3),
            ..Default::default()
        }),
        ContinuationDecision::Stopped {
            reason: ContinuationStopReason::MaxCountReached,
            trigger: Some(ContinuationTrigger::Plan),
        }
    );
}

#[test]
fn settings_persiste_e_valida_continuation_enabled_e_max_count_e_expoe_no_dispatcher() {
    let (app, dir) = setup_app("settings");
    {
        let db_state = app.state::<DbState>();
        let db = db_state.require().unwrap();

        // Padrões corretos: enabled = true, maxCount = 5.
        assert!(db.continuation_enabled().unwrap());
        assert_eq!(db.continuation_max_count().unwrap(), 5);
        let loaded = db.load_settings().unwrap();
        assert_eq!(
            loaded.get(SETTING_CONTINUATION_ENABLED).map(String::as_str),
            Some("true")
        );
        assert_eq!(
            loaded
                .get(SETTING_CONTINUATION_MAX_COUNT)
                .map(String::as_str),
            Some("5")
        );

        let cfg = load_continuation_config(app.handle());
        assert_eq!(
            cfg,
            ContinuationConfig {
                enabled: true,
                max_count: 5
            }
        );

        // Atualização via save_setting.
        db.save_setting(SETTING_CONTINUATION_ENABLED, "false").unwrap();
        db.save_setting(SETTING_CONTINUATION_MAX_COUNT, "8").unwrap();
        let updated = load_continuation_config(app.handle());
        assert_eq!(
            updated,
            ContinuationConfig {
                enabled: false,
                max_count: 8
            }
        );

        // Valores inválidos são recusados.
        assert!(matches!(
            db.save_setting(SETTING_CONTINUATION_ENABLED, "sim"),
            Err(DbError::InvalidSetting(_, _))
        ));
        assert!(matches!(
            db.save_setting(SETTING_CONTINUATION_MAX_COUNT, "-1"),
            Err(DbError::InvalidSetting(_, _))
        ));
        assert!(matches!(
            db.save_setting(SETTING_CONTINUATION_MAX_COUNT, "999"),
            Err(DbError::InvalidSetting(_, _))
        ));

        // Dispatcher expõe get_settings / set_setting documentando as chaves.
        let cmds = dispatcher::list_commands_filtered(Some("settings"));
        let set_cmd = cmds
            .iter()
            .find(|c| c["name"] == "set_setting")
            .expect("set_setting exposto");
        let desc = set_cmd["description"].as_str().unwrap_or("");
        assert!(desc.contains("continuationEnabled"));
        assert!(desc.contains("continuationMaxCount"));

        // Serialização do evento discreto de continuação.
        let evt = VokiaEvent::TurnContinuation {
            project_id: 1,
            agent: Agent::axiom(),
            trigger: ContinuationTrigger::Plan,
            attempt: 1,
            max_attempts: 5,
            marker: format_continuation_marker(1, 5, ContinuationTrigger::Plan),
            time: 12345,
            channel_id: None,
            item_id: Some("card-1".to_string()),
        };
        let json = serde_json::to_value(&evt).unwrap();
        assert_eq!(json["type"], "turn-continuation");
        assert_eq!(json["marker"], "Continuação automática 1/5 · etapas pendentes");
        assert_eq!(json["attempt"], 1);
        assert_eq!(json["maxAttempts"], 5);
        assert_eq!(json["trigger"], "plan");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
