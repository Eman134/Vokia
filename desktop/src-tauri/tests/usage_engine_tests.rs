use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tauri::Manager;
use tokio::sync::Notify;

use crate::agents::Agent;
use crate::claude::{LimitWindow, RateLimits};
use crate::commands::dispatcher::{command_domain, dispatch, list_commands};
use crate::mcp::support::build_app;
use crate::providers::ModelTier;
use crate::state::AppState;
use crate::usage::limit_gate::{
    UsageLimitGate, UsageLimitReturn, RESET_GRACE_SECS, UNKNOWN_RESET_RECHECK_SECS,
};
use crate::usage::model::{
    PaceStatus, ProviderPlanUsage, ProviderUsageGroup, UsageWindow, UsageWindowKind, DAILY_MINS,
    FIVE_HOUR_MINS, MONTHLY_MINS, WEEKLY_MINS,
};
use crate::usage::read_gate::{backoff_duration_ms, UsageReadGate, CACHE_TTL_MS};

const MIN_MS: u64 = 60_000;
const SEC_MS: u64 = 1_000;

fn sample_usage(provider_id: &str, percent: u8, resets_at_secs: u64, updated_at_ms: u64) -> ProviderPlanUsage {
    ProviderPlanUsage {
        provider_id: provider_id.to_string(),
        label: Some(provider_id.to_string()),
        primary: Some(UsageWindow::new(percent, resets_at_secs, FIVE_HOUR_MINS)),
        secondary: Some(UsageWindow::new(percent / 2, resets_at_secs + 86_400, WEEKLY_MINS)),
        tertiary: None,
        groups: Vec::new(),
        credits_available: None,
        updated_at_ms,
        read_failed: false,
    }
}

#[test]
fn classifica_janelas_com_tolerancia_de_cinco_por_cento() {
    // 5 h = 300 min -> [285, 315]
    assert_eq!(UsageWindowKind::classify(284), UsageWindowKind::Custom(284));
    assert_eq!(UsageWindowKind::classify(285), UsageWindowKind::FiveHour);
    assert_eq!(UsageWindowKind::classify(300), UsageWindowKind::FiveHour);
    assert_eq!(UsageWindowKind::classify(315), UsageWindowKind::FiveHour);
    assert_eq!(UsageWindowKind::classify(316), UsageWindowKind::Custom(316));

    // diária = 1440 min -> [1368, 1512]
    assert_eq!(UsageWindowKind::classify(1367), UsageWindowKind::Custom(1367));
    assert_eq!(UsageWindowKind::classify(1368), UsageWindowKind::Daily);
    assert_eq!(UsageWindowKind::classify(DAILY_MINS), UsageWindowKind::Daily);
    assert_eq!(UsageWindowKind::classify(1512), UsageWindowKind::Daily);
    assert_eq!(UsageWindowKind::classify(1513), UsageWindowKind::Custom(1513));

    // semanal = 10080 min -> [9576, 10584]
    assert_eq!(UsageWindowKind::classify(9575), UsageWindowKind::Custom(9575));
    assert_eq!(UsageWindowKind::classify(9576), UsageWindowKind::Weekly);
    assert_eq!(UsageWindowKind::classify(WEEKLY_MINS), UsageWindowKind::Weekly);
    assert_eq!(UsageWindowKind::classify(10584), UsageWindowKind::Weekly);
    assert_eq!(UsageWindowKind::classify(10585), UsageWindowKind::Custom(10585));

    // mensal = 43200 min -> [41040, 45360]
    assert_eq!(UsageWindowKind::classify(41039), UsageWindowKind::Custom(41039));
    assert_eq!(UsageWindowKind::classify(41040), UsageWindowKind::Monthly);
    assert_eq!(UsageWindowKind::classify(MONTHLY_MINS), UsageWindowKind::Monthly);
    assert_eq!(UsageWindowKind::classify(45360), UsageWindowKind::Monthly);
    assert_eq!(UsageWindowKind::classify(45361), UsageWindowKind::Custom(45361));

    assert_eq!(UsageWindowKind::FiveHour.label(), "5 h");
    assert_eq!(UsageWindowKind::Daily.label(), "diária");
    assert_eq!(UsageWindowKind::Weekly.label(), "semanal");
    assert_eq!(UsageWindowKind::Monthly.label(), "mensal");
    assert_eq!(UsageWindowKind::Custom(120).label(), "2 h");
    assert_eq!(UsageWindowKind::Custom(45).label(), "45 min");
}

#[test]
fn calcula_ritmo_de_consumo_e_status_sustentavel_vs_acima() {
    // Janela semanal (10080 min = 604800 s): 60% do tempo restante = 362880 s
    let now_secs = 1_700_000_000u64;
    let resets_at = now_secs + 362_880;
    let now_ms = now_secs * SEC_MS;

    // 80% usada => 20% cota restante vs 60% tempo restante => acima do ritmo sustentável
    let fast = UsageWindow::new(80, resets_at, WEEKLY_MINS);
    let pace_remaining = fast.pace_remaining_percent(now_ms).expect("pace");
    assert!((pace_remaining - 60.0).abs() < 0.01);
    assert!((fast.expected_used_percent(now_ms).unwrap() - 40.0).abs() < 0.01);
    assert_eq!(fast.pace_status(now_ms), Some(PaceStatus::AbovePace));

    // 25% usada => 75% cota restante vs 60% tempo restante => sustentável
    let comfortable = UsageWindow::new(25, resets_at, WEEKLY_MINS);
    assert_eq!(comfortable.pace_status(now_ms), Some(PaceStatus::Sustainable));

    // Janela expirada => 0% tempo restante => sustentável
    assert_eq!(fast.pace_remaining_percent(resets_at * SEC_MS), Some(0.0));
    assert_eq!(fast.pace_status(resets_at * SEC_MS), Some(PaceStatus::Sustainable));

    // Sem reset ou sem duração => sem indicador de ritmo
    assert_eq!(UsageWindow::new(50, 0, WEEKLY_MINS).pace_remaining_percent(now_ms), None);
    assert_eq!(UsageWindow::new(50, resets_at, 0).pace_remaining_percent(now_ms), None);
}

#[test]
fn converte_rate_limits_do_claude_e_trata_estado_usando_creditos() {
    let limits = RateLimits {
        five_hour: LimitWindow {
            utilization: 0.76,
            resets_at: 1_700_010_000,
            rejected: false,
        },
        seven_day: LimitWindow {
            utilization: 0.42,
            resets_at: 1_700_500_000,
            rejected: false,
        },
    };
    let usage = ProviderPlanUsage::from_claude_rate_limits(limits, 1_700_000_000_000);
    assert_eq!(usage.provider_id, "claude");
    assert_eq!(usage.primary.as_ref().unwrap().percent, 76);
    assert_eq!(usage.primary.as_ref().unwrap().kind, UsageWindowKind::FiveHour);
    assert_eq!(usage.secondary.as_ref().unwrap().percent, 42);
    assert_eq!(usage.secondary.as_ref().unwrap().kind, UsageWindowKind::Weekly);
    assert!(!usage.is_exhausted(90));
    assert!(!usage.is_using_credits());

    // Provedor em 100% mas com creditsAvailable = true não bloqueia a fila
    let mut codex = sample_usage("codex", 100, 1_700_010_000, 1_700_000_000_000);
    codex.credits_available = Some(true);
    assert!(codex.is_using_credits());
    assert!(!codex.is_exhausted(90));

    // Grupos de modelos (ex.: Gemini) entram na seleção da janela mais restritiva
    let gemini = ProviderPlanUsage {
        provider_id: "gemini".to_string(),
        label: Some("Gemini".to_string()),
        primary: Some(UsageWindow::new(20, 1_700_010_000, FIVE_HOUR_MINS)),
        secondary: None,
        tertiary: Some(UsageWindow::new(30, 1_702_500_000, MONTHLY_MINS)),
        groups: vec![ProviderUsageGroup {
            id: "pro".to_string(),
            label: "Gemini Pro".to_string(),
            models: vec!["gemini-2.5-pro".to_string()],
            window: UsageWindow::new(88, 1_700_010_000, DAILY_MINS),
        }],
        credits_available: None,
        updated_at_ms: 1_700_000_000_000,
        read_failed: false,
    };
    let worst = gemini
        .most_restrictive_window(1_700_000_000_000)
        .expect("janela mais restritiva");
    assert_eq!(worst.percent, 88);
    assert_eq!(worst.kind, UsageWindowKind::Daily);
}

#[test]
fn read_gate_respeita_cache_de_4_minutos_e_invalida_apos_reset() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let gate = UsageReadGate::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let t0 = 1_700_000_000_000u64;
        let reset_at_secs = (t0 / SEC_MS) + 120; // reseta em 2 min

        // Primeira leitura executa fetch
        let c1 = Arc::clone(&calls);
        let r1 = gate
            .read("claude", t0, move || async move {
                c1.fetch_add(1, Ordering::SeqCst);
                Ok(sample_usage("claude", 40, reset_at_secs, t0))
            })
            .await;
        assert!(!r1.from_cache);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        // Dentro de 4 min e antes do reset -> hit de cache
        let c2 = Arc::clone(&calls);
        let r2 = gate
            .read("claude", t0 + 60 * SEC_MS, move || async move {
                c2.fetch_add(1, Ordering::SeqCst);
                Ok(sample_usage("claude", 99, reset_at_secs, t0 + 60 * SEC_MS))
            })
            .await;
        assert!(r2.from_cache);
        assert_eq!(r2.usage.unwrap().primary.unwrap().percent, 40);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        // Quando now >= resetsAt (mesmo ainda dentro dos 4 min) -> fura o cache imediatamente
        let after_reset = reset_at_secs * SEC_MS;
        let c3 = Arc::clone(&calls);
        let r3 = gate
            .read("claude", after_reset, move || async move {
                c3.fetch_add(1, Ordering::SeqCst);
                Ok(sample_usage("claude", 5, reset_at_secs + 18_000, after_reset))
            })
            .await;
        assert!(!r3.from_cache);
        assert_eq!(r3.usage.unwrap().primary.unwrap().percent, 5);
        assert_eq!(calls.load(Ordering::SeqCst), 2);

        // Após 3m59s ainda usa cache; após 4m00s faz nova leitura
        let c4 = Arc::clone(&calls);
        let r4 = gate
            .read("claude", after_reset + CACHE_TTL_MS - 1, move || async move {
                c4.fetch_add(1, Ordering::SeqCst);
                Ok(sample_usage("claude", 9, reset_at_secs + 18_000, after_reset + CACHE_TTL_MS - 1))
            })
            .await;
        assert!(r4.from_cache);
        assert_eq!(calls.load(Ordering::SeqCst), 2);

        let c5 = Arc::clone(&calls);
        let r5 = gate
            .read("claude", after_reset + CACHE_TTL_MS, move || async move {
                c5.fetch_add(1, Ordering::SeqCst);
                Ok(sample_usage("claude", 12, reset_at_secs + 18_000, after_reset + CACHE_TTL_MS))
            })
            .await;
        assert!(!r5.from_cache);
        assert_eq!(r5.usage.unwrap().primary.unwrap().percent, 12);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    });
}

#[test]
fn read_gate_deduplica_chamadas_concorrentes_com_single_flight() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let gate = UsageReadGate::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let now_ms = 1_700_000_000_000u64;

        let mut handles = Vec::new();
        for _ in 0..3 {
            let g = gate.clone();
            let c = Arc::clone(&calls);
            let s = Arc::clone(&started);
            let r = Arc::clone(&release);
            handles.push(tokio::spawn(async move {
                g.read("codex", now_ms, move || async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    s.notify_one();
                    r.notified().await;
                    Ok(sample_usage("codex", 33, (now_ms / SEC_MS) + 3_600, now_ms))
                })
                .await
            }));
        }

        started.notified().await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        release.notify_waiters();

        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.unwrap());
        }

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        for outcome in results {
            assert_eq!(outcome.usage.unwrap().primary.unwrap().percent, 33);
            assert!(outcome.error.is_none());
        }
    });
}

#[test]
fn read_gate_aplica_backoff_exponencial_e_mantem_ultima_leitura_valida() {
    assert_eq!(backoff_duration_ms(1), 5 * MIN_MS);
    assert_eq!(backoff_duration_ms(2), 10 * MIN_MS);
    assert_eq!(backoff_duration_ms(3), 20 * MIN_MS);
    assert_eq!(backoff_duration_ms(4), 40 * MIN_MS);
    assert_eq!(backoff_duration_ms(5), 60 * MIN_MS);
    assert_eq!(backoff_duration_ms(6), 60 * MIN_MS);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let gate = UsageReadGate::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let t0 = 1_700_000_000_000u64;
        let resets_at = (t0 / SEC_MS) + 300;

        // Leitura válida inicial
        let initial = gate
            .read("codex", t0, || async {
                Ok(sample_usage("codex", 55, resets_at, t0))
            })
            .await;
        assert_eq!(initial.usage.unwrap().primary.unwrap().percent, 55);

        // Sequência de falhas HTTP 429: 5 -> 10 -> 20 -> 40 -> 60 -> 60 min
        let expected_mins = [5u64, 10, 20, 40, 60, 60];
        let mut cursor = t0 + CACHE_TTL_MS;
        for (idx, wait_min) in expected_mins.into_iter().enumerate() {
            let c = Arc::clone(&calls);
            let fail = gate
                .read("codex", cursor, move || async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err("HTTP 429 Too Many Requests".to_string())
                })
                .await;
            assert!(fail.in_backoff);
            assert_eq!(fail.backoff_until_ms, Some(cursor + wait_min * MIN_MS));
            // Última leitura válida permanece visível com read_failed = true
            let cached = fail.usage.expect("mantém última leitura válida");
            assert_eq!(cached.primary.unwrap().percent, 55);
            assert!(cached.read_failed);
            assert_eq!(calls.load(Ordering::SeqCst), idx + 1);

            // Polling no meio do backoff (mesmo com now >= resetsAt) NÃO consulta externamente
            let mid = cursor + (wait_min * MIN_MS) - 1;
            let c_mid = Arc::clone(&calls);
            let skipped = gate
                .read("codex", mid, move || async move {
                    c_mid.fetch_add(1, Ordering::SeqCst);
                    Ok(sample_usage("codex", 10, resets_at + 10_000, mid))
                })
                .await;
            assert!(skipped.in_backoff);
            assert!(skipped.from_cache);
            assert!(skipped.usage.unwrap().read_failed);
            assert_eq!(calls.load(Ordering::SeqCst), idx + 1);

            cursor += wait_min * MIN_MS;
        }

        // Invalidação por troca de credencial fura backoff imediatamente
        gate.invalidate("codex");
        let recovered = gate
            .read("codex", cursor - 30 * MIN_MS, || async {
                Ok(sample_usage("codex", 15, resets_at + 20_000, cursor))
            })
            .await;
        assert!(!recovered.in_backoff);
        assert!(!recovered.from_cache);
        let usage = recovered.usage.unwrap();
        assert_eq!(usage.primary.unwrap().percent, 15);
        assert!(!usage.read_failed);
    });
}

#[test]
fn limit_gate_isola_por_provedor_modelo_e_retorna_apos_reset_mais_30s() {
    let mut gate = UsageLimitGate::default();
    let now_ms = 1_700_000_000_000u64;
    let resets_at = 1_700_000_600u64; // +600s
    let expected_release_ms = (resets_at + RESET_GRACE_SECS) * SEC_MS;

    let entry = gate.mark_reached("claude", "sonnet", Some(resets_at), now_ms);
    assert_eq!(entry.release_at_ms, expected_release_ms);

    // Isolamento: apenas (claude, sonnet) fica bloqueado
    assert!(gate.is_limited("claude", "sonnet", now_ms + 1_000));
    assert!(!gate.is_limited("claude", "haiku", now_ms + 1_000));
    assert!(!gate.is_limited("codex", "gpt-6-luna", now_ms + 1_000));
    assert!(!gate.is_limited("opencode", "openai/gpt-6-astra", now_ms + 1_000));

    // Failover transparente só quando !acted e há backup
    assert!(UsageLimitGate::can_failover(false, true));
    assert!(!UsageLimitGate::can_failover(true, true));
    assert!(!UsageLimitGate::can_failover(false, false));

    // Registra usage_limit_return para voltar ao modelo original
    let kairo = Agent::from_stored("kairo").expect("kairo");
    gate.register_return(UsageLimitReturn {
        agent: kairo.clone(),
        original_provider_id: "claude".to_string(),
        original_model: "sonnet".to_string(),
        original_tier: Some(ModelTier::Intermediate),
        fallback_provider_id: "codex".to_string(),
        fallback_model: "gpt-6-luna".to_string(),
        release_at_ms: expected_release_ms,
    });

    // 1 segundo antes de resetsAt + 30s continua bloqueado
    assert!(gate.is_limited("claude", "sonnet", expected_release_ms - 1_000));
    let before = gate.release_expired(expected_release_ms - 1_000);
    assert!(before.released_keys.is_empty());
    assert!(before.returns.is_empty());
    assert!(gate.get_return(&kairo).is_some());

    // Em resetsAt + 30s libera a fila e devolve o agente ao modelo original
    let released = gate.release_expired(expected_release_ms);
    assert_eq!(
        released.released_keys,
        vec![("claude".to_string(), "sonnet".to_string())]
    );
    assert_eq!(released.returns.len(), 1);
    assert_eq!(released.returns[0].agent, kairo);
    assert_eq!(released.returns[0].original_provider_id, "claude");
    assert_eq!(released.returns[0].original_model, "sonnet");
    assert!(!gate.is_limited("claude", "sonnet", expected_release_ms));
    assert!(gate.get_return(&kairo).is_none());

    // Sem resetsAt conhecido agenda rechecagem em 15 minutos
    let unknown = gate.mark_reached("codex", "gpt-6-luna", None, now_ms);
    assert_eq!(
        unknown.release_at_ms,
        now_ms + UNKNOWN_RESET_RECHECK_SECS * SEC_MS
    );

    // Confirmação explícita de renovação libera imediatamente
    gate.register_return(UsageLimitReturn {
        agent: kairo.clone(),
        original_provider_id: "codex".to_string(),
        original_model: "gpt-6-luna".to_string(),
        original_tier: None,
        fallback_provider_id: "claude".to_string(),
        fallback_model: "sonnet".to_string(),
        release_at_ms: unknown.release_at_ms,
    });
    let confirmed = gate.confirm_renewed("codex", None);
    assert_eq!(confirmed.released_keys.len(), 1);
    assert_eq!(confirmed.returns.len(), 1);
    assert!(!gate.is_limited("codex", "gpt-6-luna", now_ms + 1_000));
}

#[test]
fn comandos_de_uso_estao_expostos_no_dispatcher_e_respeitam_single_flight() {
    assert_eq!(command_domain("get_plan_usage"), Some("usage"));
    assert_eq!(command_domain("refresh_plan_usage"), Some("usage"));

    let cmds = list_commands();
    let get_meta = cmds
        .iter()
        .find(|cmd| cmd["name"] == "get_plan_usage")
        .expect("get_plan_usage listado");
    assert_eq!(get_meta["domain"], "usage");
    assert!(get_meta["inputSchema"]["properties"]["providerId"].is_object());

    let dir = std::env::temp_dir().join(format!(
        "vokia-usage-cmd-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let app = build_app(&dir);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let state = app.state::<AppState>();
        let now_ms = crate::usage::now_millis();
        state
            .usage
            .record_provider_usage(sample_usage("claude", 42, (now_ms / SEC_MS) + 7_200, now_ms));

        let codex_calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&codex_calls);
        state.usage.register_reader(
            "codex",
            Arc::new(move || {
                let c = Arc::clone(&counter);
                Box::pin(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(15)).await;
                    let ts = crate::usage::now_millis();
                    Ok(sample_usage("codex", 18, (ts / SEC_MS) + 7_200, ts))
                })
            }),
        );

        // 3 chamadas simultâneas a get_plan_usage / refresh_plan_usage => apenas 1 consulta externa ao codex
        let h1 = app.handle().clone();
        let h2 = app.handle().clone();
        let h3 = app.handle().clone();
        let (r1, r2, r3) = tokio::join!(
            dispatch(&h1, None, "get_plan_usage", json!({})),
            dispatch(&h2, None, "refresh_plan_usage", json!({})),
            dispatch(&h3, None, "get_plan_usage", json!({ "providerId": "codex" })),
        );

        assert_eq!(codex_calls.load(Ordering::SeqCst), 1);

        let all = r1.expect("comando registrado").expect("ok");
        let providers = all["providers"].as_array().expect("providers array");
        assert_eq!(providers.len(), 2);
        assert!(providers.iter().any(|p| p["providerId"] == "claude"));
        assert!(providers.iter().any(|p| p["providerId"] == "codex"));

        let only_codex = r3.expect("comando registrado").expect("ok");
        let filtered = only_codex["providers"].as_array().expect("providers array");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0]["providerId"], "codex");
        assert_eq!(filtered[0]["primary"]["percent"], 18);

        let refreshed = r2.expect("comando registrado").expect("ok");
        assert_eq!(refreshed["providers"].as_array().unwrap().len(), 2);
    });

    let _ = std::fs::remove_dir_all(&dir);
}
