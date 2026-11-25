use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use chrono::{TimeZone, Utc};

use crate::claude::TurnUsage;
use crate::db::usage_analytics::{
    diff_cumulative_usage, NewUsageRecord, UsageAnalyticsQuery, MAX_RANGE_DAYS,
};
use crate::db::usage_pricing::{estimate_usage_price, TokenCounts};
use crate::db::{Db, DbError, NewProject, ORIGIN_FOLDER};
use crate::events::Speaker;

struct TempDb {
    dir: PathBuf,
    db: Db,
}

impl TempDb {
    fn open() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "vokia-usage-analytics-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let db = Db::open(&dir).expect("abrir banco temporário");
        Self { dir, db }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn utc_ms(year: i32, month: u32, day: u32, hour: u32, min: u32) -> i64 {
    Utc.with_ymd_and_hms(year, month, day, hour, min, 0)
        .single()
        .expect("timestamp UTC válido")
        .timestamp_millis()
}

#[test]
fn metricas_permanecem_intactas_apos_limpar_conversa() {
    let fixture = TempDb::open();
    let dir_str = fixture.dir.to_string_lossy().into_owned();
    let project = fixture
        .db
        .insert_project(&NewProject {
            name: "Projeto Teste",
            path: &dir_str,
            symbol: "code",
            description: "",
            origin_kind: ORIGIN_FOLDER,
            origin_repo: None,
        })
        .expect("criar projeto");

    let occurred_at = utc_ms(2026, 5, 10, 14, 0);
    fixture
        .db
        .insert_message(
            project.id,
            Speaker::from(crate::agents::Agent::from_stored("axiom").expect("axiom")),
            Speaker::User,
            "Resposta que será apagada ao limpar conversa",
            occurred_at as u64,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("inserir mensagem");

    let providers = [
        ("claude", "claude-sonnet-4-6", "axiom"),
        ("codex", "gpt-5.3-codex", "codex-dev"),
        ("opencode", "deepseek-v3", "scout"),
        ("antigravity", "gemini-2.5-pro", "gemini-lead"),
    ];
    for (provider, model, agent) in providers {
        let usage = TurnUsage {
            provider: Some(provider.to_string()),
            model: Some(model.to_string()),
            input_tokens: Some(1_000),
            output_tokens: Some(500),
            cached_input_tokens: Some(250),
            cache_write_tokens: Some(100),
            thought_tokens: Some(if provider == "antigravity" { 300 } else { 0 }),
            cost_usd: None,
            is_fallback: Some(false),
        };
        let record = NewUsageRecord::from_turn_usage(
            occurred_at,
            Some(project.id),
            Some("CARD-1".to_string()),
            agent,
            "card",
            &usage,
            1_200,
        );
        fixture.db.record_usage(&record).expect("gravar uso");
    }

    // Limpa todas as mensagens e sessões do projeto
    fixture
        .db
        .clear_messages(project.id)
        .expect("limpar conversa");
    let history = fixture
        .db
        .load_history(project.id)
        .expect("carregar histórico");
    assert!(history.is_empty(), "histórico de mensagens deveria estar vazio");

    // Estatísticas continuam intactas
    let report = fixture
        .db
        .query_usage_analytics_at(
            utc_ms(2026, 5, 10, 18, 0),
            &UsageAnalyticsQuery {
                days: Some(7),
                timezone: Some("UTC".to_string()),
                project_id: Some(project.id),
                ..Default::default()
            },
        )
        .expect("consultar analytics");

    assert_eq!(report.totals.turns, 4);
    assert_eq!(report.by_provider.len(), 4);
    assert_eq!(report.by_agent.len(), 4);
    assert_eq!(report.by_model.len(), 4);
    assert!(report.totals.cost_usd_estimated.is_some());
}

#[test]
fn agrupamento_por_dia_respeita_fuso_horario_iana_e_offset() {
    let fixture = TempDb::open();
    // 2026-05-10 01:30 UTC == 2026-05-09 22:30 America/Sao_Paulo (-03:00)
    let ts = utc_ms(2026, 5, 10, 1, 30);
    let usage = TurnUsage {
        provider: Some("claude".to_string()),
        model: Some("claude-sonnet-4-6".to_string()),
        input_tokens: Some(2_000),
        output_tokens: Some(400),
        ..Default::default()
    };
    let record = NewUsageRecord::from_turn_usage(ts, Some(1), None, "axiom", "quick", &usage, 800);
    fixture.db.record_usage(&record).expect("gravar uso");

    let now_ms = utc_ms(2026, 5, 10, 12, 0);

    let sp = fixture
        .db
        .query_usage_analytics_at(
            now_ms,
            &UsageAnalyticsQuery {
                days: Some(30),
                timezone: Some("America/Sao_Paulo".to_string()),
                ..Default::default()
            },
        )
        .expect("consulta America/Sao_Paulo");
    assert_eq!(sp.range.offset, "-03:00");
    assert_eq!(sp.daily.len(), 1);
    assert_eq!(sp.daily[0].date, "2026-05-09");
    assert_eq!(sp.daily_by_provider.len(), 1);
    assert_eq!(sp.daily_by_provider[0].date, "2026-05-09");
    assert_eq!(sp.daily_by_provider[0].provider_id, "claude");

    let utc = fixture
        .db
        .query_usage_analytics_at(
            now_ms,
            &UsageAnalyticsQuery {
                days: Some(30),
                timezone: Some("UTC".to_string()),
                ..Default::default()
            },
        )
        .expect("consulta UTC");
    assert_eq!(utc.range.offset, "+00:00");
    assert_eq!(utc.daily.len(), 1);
    assert_eq!(utc.daily[0].date, "2026-05-10");

    let explicit_offset = fixture
        .db
        .query_usage_analytics_at(
            now_ms,
            &UsageAnalyticsQuery {
                days: Some(30),
                timezone: Some("-03:00".to_string()),
                ..Default::default()
            },
        )
        .expect("consulta -03:00");
    assert_eq!(explicit_offset.daily[0].date, "2026-05-09");
}

#[test]
fn thought_tokens_e_cache_write_sao_separados_sem_inflar_input_tokens() {
    let fixture = TempDb::open();
    let ts = utc_ms(2026, 5, 11, 15, 0);
    let usage = TurnUsage {
        provider: Some("antigravity".to_string()),
        model: Some("gemini-2.5-pro".to_string()),
        input_tokens: Some(1_000),
        cached_input_tokens: Some(500),
        cache_write_tokens: Some(200),
        output_tokens: Some(400),
        thought_tokens: Some(2_500),
        cost_usd: None,
        is_fallback: None,
    };
    let record =
        NewUsageRecord::from_turn_usage(ts, Some(1), None, "gemini", "card", &usage, 1_500);
    assert_eq!(record.input_tokens, 1_000);
    assert_eq!(record.cached_input_tokens, 500);
    assert_eq!(record.cache_write_tokens, 200);
    assert_eq!(record.output_tokens, 400);
    assert_eq!(record.thought_tokens, 2_500);

    // Preço esperado gemini-2.5-pro standard:
    // input: 1000 * 1.25 / 1e6 = 0.00125
    // cache_read: 500 * 0.3125 / 1e6 = 0.00015625
    // cache_write: 200 * 1.25 / 1e6 = 0.00025
    // output + thought: (400 + 2500) * 10.00 / 1e6 = 0.029
    // total = 0.03065625
    let expected_cost = 0.03065625_f64;
    let actual_cost = record.cost_usd_estimated.expect("custo estimado");
    assert!(
        (actual_cost - expected_cost).abs() < 1e-9,
        "esperado {expected_cost}, obtido {actual_cost}"
    );
    assert!(
        record
            .price_basis
            .as_deref()
            .is_some_and(|b| b.ends_with(":standard")),
        "esperado tier standard, obtido {:?}",
        record.price_basis
    );

    fixture.db.record_usage(&record).expect("gravar gemini");
    let report = fixture
        .db
        .query_usage_analytics_at(
            ts,
            &UsageAnalyticsQuery {
                days: Some(7),
                timezone: Some("UTC".to_string()),
                ..Default::default()
            },
        )
        .expect("consultar");

    assert_eq!(report.totals.input_tokens, 1_000);
    assert_eq!(report.totals.thought_tokens, 2_500);
    assert_eq!(report.totals.output_tokens, 400);
    assert_eq!(report.totals.cache_write_tokens, 200);
    assert_eq!(report.totals.total_tokens, 4_600);

    // Verifica faixa long-context (> 200k tokens de entrada)
    let long_est = estimate_usage_price(
        "claude-sonnet-4-6",
        TokenCounts {
            input_tokens: 210_000,
            cached_input_tokens: 0,
            cache_write_tokens: 0,
            output_tokens: 1_000,
            thought_tokens: 0,
        },
    )
    .expect("estimativa long-context");
    assert!(long_est.is_long_context);
    assert!(long_est.price_basis.ends_with(":long-context"));
}

#[test]
fn modelo_desconhecido_retorna_none_e_nunca_zero() {
    let estimate = estimate_usage_price(
        "modelo-proprietario-desconhecido-99",
        TokenCounts {
            input_tokens: 5_000,
            cached_input_tokens: 1_000,
            cache_write_tokens: 0,
            output_tokens: 2_000,
            thought_tokens: 0,
        },
    );
    assert!(estimate.is_none(), "modelo desconhecido deve retornar None");

    let fixture = TempDb::open();
    let ts = utc_ms(2026, 5, 11, 16, 0);
    let usage = TurnUsage {
        provider: Some("custom-cli".to_string()),
        model: Some("modelo-proprietario-desconhecido-99".to_string()),
        input_tokens: Some(5_000),
        output_tokens: Some(2_000),
        cost_usd: None,
        ..Default::default()
    };
    let record =
        NewUsageRecord::from_turn_usage(ts, Some(1), None, "worker", "quick", &usage, 900);
    assert!(record.cost_usd_estimated.is_none());
    assert!(record.price_basis.is_none());

    fixture.db.record_usage(&record).expect("gravar uso");
    let report = fixture
        .db
        .query_usage_analytics_at(
            ts,
            &UsageAnalyticsQuery {
                days: Some(7),
                timezone: Some("UTC".to_string()),
                ..Default::default()
            },
        )
        .expect("consultar");

    assert_eq!(report.totals.turns, 1);
    assert_eq!(report.totals.total_tokens, 7_000);
    assert_eq!(report.totals.cost_usd_estimated, None);
    assert_eq!(report.totals.unpriced_turns, 1);
    assert_eq!(report.by_model[0].cost_usd_estimated, None);
    assert_eq!(report.by_model[0].unpriced_turns, 1);
    assert_eq!(report.daily[0].cost_usd_estimated, None);
}

#[test]
fn rejeita_intervalo_maior_que_367_dias_e_aceita_ate_367() {
    let fixture = TempDb::open();
    let now_ms = utc_ms(2026, 5, 11, 12, 0);

    let err_days = fixture
        .db
        .query_usage_analytics_at(
            now_ms,
            &UsageAnalyticsQuery {
                days: Some(MAX_RANGE_DAYS + 1),
                timezone: Some("UTC".to_string()),
                ..Default::default()
            },
        )
        .expect_err("deve rejeitar > 367 dias");
    assert!(
        matches!(err_days, DbError::InvalidUsageQuery(ref msg) if msg.contains("367")),
        "erro inesperado: {err_days}"
    );

    let err_custom = fixture
        .db
        .query_usage_analytics_at(
            now_ms,
            &UsageAnalyticsQuery {
                from: Some("2025-01-01".to_string()),
                to: Some("2026-01-03".to_string()), // 368 dias inclusivos
                timezone: Some("UTC".to_string()),
                ..Default::default()
            },
        )
        .expect_err("deve rejeitar intervalo customizado > 367 dias");
    assert!(
        matches!(err_custom, DbError::InvalidUsageQuery(ref msg) if msg.contains("367")),
        "erro inesperado: {err_custom}"
    );

    let ok_exact = fixture
        .db
        .query_usage_analytics_at(
            now_ms,
            &UsageAnalyticsQuery {
                from: Some("2025-01-01".to_string()),
                to: Some("2026-01-02".to_string()), // 367 dias inclusivos
                timezone: Some("UTC".to_string()),
                ..Default::default()
            },
        )
        .expect("deve aceitar exatamente 367 dias");
    assert_eq!(ok_exact.range.days, 367);
}

#[test]
fn agregacao_por_provedor_agente_modelo_filtros_e_economia() {
    let fixture = TempDb::open();
    let day1 = utc_ms(2026, 5, 10, 12, 0);
    let day2 = utc_ms(2026, 5, 11, 12, 0);

    let r1 = NewUsageRecord::from_turn_usage(
        day1,
        Some(10),
        Some("C-1".to_string()),
        "axiom",
        "card",
        &TurnUsage {
            provider: Some("claude".to_string()),
            model: Some("claude-haiku-4-5".to_string()),
            input_tokens: Some(10_000),
            cached_input_tokens: Some(40_000),
            cache_write_tokens: Some(5_000),
            output_tokens: Some(2_000),
            thought_tokens: None,
            cost_usd: None,
            is_fallback: Some(false),
        },
        1_000,
    );
    let mut r2 = NewUsageRecord::from_turn_usage(
        day1,
        Some(10),
        Some("C-2".to_string()),
        "dev",
        "review",
        &TurnUsage {
            provider: Some("codex".to_string()),
            model: Some("gpt-5.3-codex".to_string()),
            input_tokens: Some(8_000),
            cached_input_tokens: Some(2_000),
            output_tokens: Some(1_500),
            is_fallback: Some(true),
            ..Default::default()
        },
        2_000,
    );
    r2.prompt_id = Some("prompt-dedup-1".to_string());

    let r3 = NewUsageRecord::from_turn_usage(
        day2,
        Some(20),
        None,
        "axiom",
        "triage",
        &TurnUsage {
            provider: Some("claude".to_string()),
            model: Some("claude-sonnet-4-6".to_string()),
            input_tokens: Some(5_000),
            output_tokens: Some(1_000),
            ..Default::default()
        },
        500,
    );

    fixture.db.record_usage(&r1).expect("r1");
    fixture.db.record_usage(&r2).expect("r2");
    // Repetição com mesmo prompt_id não duplica
    fixture.db.record_usage(&r2).expect("r2 duplicado ignorado");
    fixture.db.record_usage(&r3).expect("r3");

    let all = fixture
        .db
        .query_usage_analytics_at(
            day2,
            &UsageAnalyticsQuery {
                days: Some(7),
                timezone: Some("UTC".to_string()),
                ..Default::default()
            },
        )
        .expect("consulta geral");

    assert_eq!(all.totals.turns, 3);
    assert_eq!(all.totals.fallback_turns, 1);
    assert_eq!(all.totals.duration_ms, 3_500);
    assert_eq!(all.by_provider.len(), 2);
    assert_eq!(all.by_agent.len(), 2);
    assert_eq!(all.by_model.len(), 3);
    assert_eq!(all.daily_by_provider.len(), 3);
    assert!(all.savings.cache_savings_usd > 0.0);
    assert!(all.savings.light_tier_savings_usd > 0.0);
    assert!(all.savings.savings_percent > 0.0);
    assert!(all.savings.cache_hit_percent > 0.0);

    // Filtros combinados
    let filtered = fixture
        .db
        .query_usage_analytics_at(
            day2,
            &UsageAnalyticsQuery {
                days: Some(7),
                timezone: Some("UTC".to_string()),
                project_id: Some(10),
                agent_id: Some("axiom".to_string()),
                provider_id: Some("claude".to_string()),
                ..Default::default()
            },
        )
        .expect("consulta filtrada");
    assert_eq!(filtered.totals.turns, 1);
    assert_eq!(filtered.by_agent[0].agent_id, "axiom");
    assert_eq!(filtered.by_model[0].model, "claude-haiku-4-5");

    // Checkpoint cumulativo sem dupla contagem
    let delta = diff_cumulative_usage(
        TokenCounts {
            input_tokens: 1_000,
            cached_input_tokens: 200,
            cache_write_tokens: 50,
            output_tokens: 300,
            thought_tokens: 100,
        },
        TokenCounts {
            input_tokens: 2_500,
            cached_input_tokens: 800,
            cache_write_tokens: 50,
            output_tokens: 900,
            thought_tokens: 450,
        },
    );
    assert_eq!(delta.input_tokens, 1_500);
    assert_eq!(delta.cached_input_tokens, 600);
    assert_eq!(delta.cache_write_tokens, 0);
    assert_eq!(delta.output_tokens, 600);
    assert_eq!(delta.thought_tokens, 350);
}
