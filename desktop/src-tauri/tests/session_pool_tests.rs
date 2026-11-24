use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::agents::{replica_id, Agent, AgentRegistry, Team};
use crate::db::{OrchestrationSettings, OrchestrationSnapshot, Team as OrchTeam};
use crate::orchestrator::agent_session::Place;
use crate::orchestrator::fallback::{now_millis_u64, LlmTarget, OverrideReason, TemporaryOverride};
use crate::permissions::PermissionLevel;
use crate::providers::{ProviderProtocol, ProviderRegistry, ToolTransport};
use crate::state::AppState;

const PROJECT: i64 = 1;
const WAIT: Duration = Duration::from_secs(2);
const BLOCKED_PROBE: Duration = Duration::from_millis(150);
static TEST_DIR_COUNTER: AtomicU32 = AtomicU32::new(0);

fn base_agent() -> Agent {
    Agent::from_stored("vexa").expect("agente")
}

fn test_team() -> (Team, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "vokia-pool-tests-{}-{}",
        std::process::id(),
        TEST_DIR_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("cria diretório temporário para time de teste");
    let team = AgentRegistry::load(&dir)
        .team()
        .expect("time inicial padrão");
    (team, dir)
}

#[test]
fn duas_instancias_do_mesmo_agente_seguram_a_sessao_ao_mesmo_tempo() {
    tauri::async_runtime::block_on(async {
        let pool = SessionPool::default();
        let mut base_exchange = ExchangeSessions::new(&pool, PROJECT);
        let mut replica_exchange = ExchangeSessions::new(&pool, PROJECT);

        let base = base_exchange.hold(&base_agent()).await;
        let replica =
            tokio::time::timeout(WAIT, replica_exchange.hold(&replica_id(&base_agent(), 2)))
                .await
                .expect("a réplica não pode esperar a base");

        assert!(!Arc::ptr_eq(&base.slot, &replica.slot));
    });
}

#[test]
fn a_mesma_instancia_em_duas_trocas_espera_a_primeira_soltar() {
    tauri::async_runtime::block_on(async {
        let pool = SessionPool::default();
        let mut first = ExchangeSessions::new(&pool, PROJECT);
        let mut second = ExchangeSessions::new(&pool, PROJECT);

        let held = first.hold(&base_agent()).await;
        let blocked = tokio::time::timeout(BLOCKED_PROBE, second.hold(&base_agent())).await;
        assert!(blocked.is_err(), "a mesma instância deveria esperar");

        drop(held);
        tokio::time::timeout(WAIT, second.hold(&base_agent()))
            .await
            .expect("solta a primeira, a segunda assume");
    });
}

#[test]
fn preload_targets_inclui_ember_e_lideres_deduplicados_sem_pausados() {
    let (team, dir) = test_team();
    let axiom = Agent::from_stored("axiom").expect("axiom");
    let vexa = Agent::from_stored("vexa").expect("vexa");

    let snapshot = OrchestrationSnapshot {
        settings: OrchestrationSettings {
            leader_agent: "axiom".to_string(),
            ..Default::default()
        },
        layers: Vec::new(),
        teams: vec![
            OrchTeam {
                id: "team-1".into(),
                name: "Team 1".into(),
                position: 0,
                leader_agent: Some("axiom".into()),
                instructions: String::new(),
                final_validation: String::new(),
                start_x: None,
                start_y: None,
                end_x: None,
                end_y: None,
                judge_full_flow: false,
            },
            OrchTeam {
                id: "team-2".into(),
                name: "Team 2".into(),
                position: 1,
                leader_agent: Some("vexa".into()),
                instructions: String::new(),
                final_validation: String::new(),
                start_x: None,
                start_y: None,
                end_x: None,
                end_y: None,
                judge_full_flow: false,
            },
            OrchTeam {
                id: "team-3".into(),
                name: "Team 3".into(),
                position: 2,
                leader_agent: Some("fantasma".into()),
                instructions: String::new(),
                final_validation: String::new(),
                start_x: None,
                start_y: None,
                end_x: None,
                end_y: None,
                judge_full_flow: false,
            },
        ],
        placements: Vec::new(),
        edges: Vec::new(),
        gateways: Vec::new(),
        task_stages: Vec::new(),
    };

    // Caso 1: Vexa pausado -> Ember e Axiom deduplicados (fantasma ignorado por não estar no Team)
    let targets_paused = preload_targets(Some(&snapshot), &team, &[vexa.clone()]);
    assert_eq!(
        targets_paused,
        vec![Agent::ember(), axiom.clone()],
        "vexa pausado deve ser excluído da pré-carga"
    );

    // Caso 2: Nenhum pausado -> Ember, Axiom e Vexa deduplicados
    let targets_unpaused = preload_targets(Some(&snapshot), &team, &[]);
    assert_eq!(
        targets_unpaused,
        vec![Agent::ember(), axiom, vexa],
        "todos os líderes válidos e não pausados devem ser incluídos sem duplicatas"
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn preload_candidate_aceita_primario_claude_sem_cooldown() {
    let (team, dir) = test_team();
    let vexa = Agent::from_stored("vexa").expect("vexa");
    let profile = team.profile(&vexa).expect("perfil de vexa");
    let providers = ProviderRegistry::seeded();
    let state = AppState::new(Ok(PathBuf::from("claude")));

    let active = LlmTarget {
        cli: "claude".into(),
        model_tier: None,
        model: Some("claude-sonnet-4-5".into()),
    };

    let candidate = preload_candidate(
        &vexa,
        profile,
        &providers,
        &state,
        Some(&active),
        &[],
        PermissionLevel::Edicao,
    );

    let candidate = candidate.expect("deve aceitar claude primário");
    assert_eq!(candidate.cli, "claude");
    assert_eq!(candidate.model, "claude-sonnet-4-5");
    assert_eq!(candidate.provider.protocol, ProviderProtocol::ClaudeLive);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn preload_candidate_trata_primario_stateless_identificando_protocolo() {
    let (team, dir) = test_team();
    let vexa = Agent::from_stored("vexa").expect("vexa");
    let profile = team.profile(&vexa).expect("perfil de vexa");
    let providers = ProviderRegistry::seeded();
    let state = AppState::new(Ok(PathBuf::from("claude")));

    let active = LlmTarget {
        cli: "codex".into(),
        model_tier: None,
        model: Some("gpt-5-codex".into()),
    };
    let fallback = vec![LlmTarget {
        cli: "claude".into(),
        model_tier: None,
        model: Some("claude-sonnet-4-5".into()),
    }];

    let candidate = preload_candidate(
        &vexa,
        profile,
        &providers,
        &state,
        Some(&active),
        &fallback,
        PermissionLevel::Edicao,
    );

    let candidate = candidate.expect("deve resolver o primário configurado");
    assert_eq!(candidate.cli, "codex");
    assert_eq!(candidate.provider.protocol, ProviderProtocol::CodexJsonl);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn preload_candidate_regras_de_fallback_e_cooldown() {
    let (team, dir) = test_team();
    let vexa = Agent::from_stored("vexa").expect("vexa");
    let profile = team.profile(&vexa).expect("perfil de vexa");
    let providers = ProviderRegistry::seeded();
    let state = AppState::new(Ok(PathBuf::from("claude")));

    // Caso (a): Primário codex em cooldown + fallback claude -> seleciona claude (ClaudeLive)
    let active_codex = LlmTarget {
        cli: "codex".into(),
        model_tier: None,
        model: Some("gpt-5-codex".into()),
    };
    let fallback_claude = vec![LlmTarget {
        cli: "claude".into(),
        model_tier: None,
        model: Some("claude-sonnet-4-5".into()),
    }];
    state
        .cooldowns
        .lock()
        .unwrap()
        .mark("codex", "gpt-5-codex", Duration::from_secs(120));

    let candidate_fallback_claude = preload_candidate(
        &vexa,
        profile,
        &providers,
        &state,
        Some(&active_codex),
        &fallback_claude,
        PermissionLevel::Edicao,
    );
    let candidate_fallback_claude =
        candidate_fallback_claude.expect("fallback para claude disponível");
    assert_eq!(candidate_fallback_claude.cli, "claude");
    assert_eq!(candidate_fallback_claude.model, "haiku");
    assert_eq!(
        candidate_fallback_claude.provider.protocol,
        ProviderProtocol::ClaudeLive
    );

    // Se o perfil base (haiku) também estiver em cooldown, recorre ao global_fallback (sonnet)
    state
        .cooldowns
        .lock()
        .unwrap()
        .mark("claude", "haiku", Duration::from_secs(120));
    let candidate_global_fallback = preload_candidate(
        &vexa,
        profile,
        &providers,
        &state,
        Some(&active_codex),
        &fallback_claude,
        PermissionLevel::Edicao,
    );
    let candidate_global_fallback =
        candidate_global_fallback.expect("fallback global para sonnet disponível");
    assert_eq!(candidate_global_fallback.cli, "claude");
    assert_eq!(candidate_global_fallback.model, "claude-sonnet-4-5");
    assert_eq!(
        candidate_global_fallback.provider.protocol,
        ProviderProtocol::ClaudeLive
    );

    // Caso (b): Primário claude/sonnet em cooldown + fallback claude/haiku -> seleciona fallback
    let state2 = AppState::new(Ok(PathBuf::from("claude")));
    let active_sonnet = LlmTarget {
        cli: "claude".into(),
        model_tier: None,
        model: Some("claude-sonnet-4-5".into()),
    };
    state2
        .cooldowns
        .lock()
        .unwrap()
        .mark("claude", "claude-sonnet-4-5", Duration::from_secs(120));

    let candidate_haiku = preload_candidate(
        &vexa,
        profile,
        &providers,
        &state2,
        Some(&active_sonnet),
        &[],
        PermissionLevel::Edicao,
    );
    let candidate_haiku = candidate_haiku.expect("fallback para perfil base claude/haiku");
    assert_eq!(candidate_haiku.cli, "claude");
    assert_eq!(candidate_haiku.model, "haiku");
    assert_eq!(
        candidate_haiku.provider.protocol,
        ProviderProtocol::ClaudeLive
    );

    // Caso (c): Todos os candidatos em cooldown -> retorna None
    state2
        .cooldowns
        .lock()
        .unwrap()
        .mark("claude", "haiku", Duration::from_secs(120));

    let candidate_none = preload_candidate(
        &vexa,
        profile,
        &providers,
        &state2,
        Some(&active_sonnet),
        &[],
        PermissionLevel::Edicao,
    );
    assert!(
        candidate_none.is_none(),
        "sem candidatos disponíveis fora de cooldown"
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn preload_candidate_aceita_override_temporario_e_respeita_cooldown() {
    let (team, dir) = test_team();
    let vexa = Agent::from_stored("vexa").expect("vexa");
    let profile = team.profile(&vexa).expect("perfil de vexa");
    let providers = ProviderRegistry::seeded();
    let state = AppState::new(Ok(PathBuf::from("claude")));

    let now = now_millis_u64();
    let temp_override = TemporaryOverride {
        agent: vexa.clone(),
        cli: "claude".into(),
        model: "claude-opus-4-5".into(),
        model_tier: None,
        command: "claude".into(),
        original_cli: "claude".into(),
        original_model: "claude-sonnet-4-5".into(),
        original_tier: None,
        reason: OverrideReason::ManualTemp,
        applied_at: now,
        expires_at: Some(now + 60_000),
    };

    state
        .temporary_overrides
        .lock()
        .unwrap()
        .insert(vexa.clone(), temp_override);

    let candidate = preload_candidate(
        &vexa,
        profile,
        &providers,
        &state,
        None,
        &[],
        PermissionLevel::Edicao,
    );
    let candidate = candidate.expect("override temporário ativo");
    assert_eq!(candidate.cli, "claude");
    assert_eq!(candidate.model, "claude-opus-4-5");

    // Se o modelo do override estiver em cooldown, deve pular o override
    state
        .cooldowns
        .lock()
        .unwrap()
        .mark("claude", "claude-opus-4-5", Duration::from_secs(120));
    let candidate_after_cooldown = preload_candidate(
        &vexa,
        profile,
        &providers,
        &state,
        None,
        &[],
        PermissionLevel::Edicao,
    );
    let candidate_after_cooldown = candidate_after_cooldown.expect("cai para o perfil base");
    assert_ne!(candidate_after_cooldown.model, "claude-opus-4-5");

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn preload_candidate_respeita_has_tools_do_ember_e_sandbox_em_liberado() {
    let (team, dir) = test_team();
    let ember = Agent::ember();
    let ember_profile = team.profile(&ember).expect("perfil do ember");
    let providers = ProviderRegistry::seeded();
    let state = AppState::new(Ok(PathBuf::from("claude")));

    // 1. Provedor customizado sem ferramentas (ToolTransport::None) não serve para Ember
    let mut no_tools_provider = providers.get("claude").expect("claude").clone();
    no_tools_provider.id = "claude-no-tools".into();
    no_tools_provider.tool_transport = Some(ToolTransport::None);
    providers.save_provider(no_tools_provider).unwrap();

    let no_tools_target = LlmTarget {
        cli: "claude-no-tools".into(),
        model_tier: None,
        model: Some("claude-sonnet-4-5".into()),
    };

    let candidate_ember = preload_candidate(
        &ember,
        ember_profile,
        &providers,
        &state,
        Some(&no_tools_target),
        &[],
        PermissionLevel::Edicao,
    );
    // Deve pular claude-no-tools e cair no perfil padrão do ember
    let candidate_ember = candidate_ember.expect("ember encontra provedor válido");
    assert_ne!(candidate_ember.cli, "claude-no-tools");
    assert!(candidate_ember.provider.has_tools());

    // 2. Provedor sem suporte a sandbox (opencode) é recusado no nível Liberado
    let axiom = Agent::from_stored("axiom").expect("axiom");
    let axiom_profile = team.profile(&axiom).expect("perfil de axiom");
    let opencode_target = LlmTarget {
        cli: "opencode".into(),
        model_tier: None,
        model: Some("openai/gpt-6-astra".into()),
    };
    let fallback_claude = vec![LlmTarget {
        cli: "claude".into(),
        model_tier: None,
        model: Some("claude-sonnet-4-5".into()),
    }];

    let candidate_liberado = preload_candidate(
        &axiom,
        axiom_profile,
        &providers,
        &state,
        Some(&opencode_target),
        &fallback_claude,
        PermissionLevel::Liberado,
    );
    let candidate_liberado = candidate_liberado.expect("candidato em nível Liberado");
    assert_eq!(
        candidate_liberado.cli, "claude",
        "opencode (sem sandbox) deve ser filtrado no nível Liberado"
    );
    assert!(candidate_liberado.provider.supports_sandbox);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn capacidade_de_precarga_respeita_limite_max_live_sessions() {
    assert!(
        SessionPool::has_preload_capacity(0),
        "pool vazio tem capacidade"
    );
    assert!(
        SessionPool::has_preload_capacity(MAX_LIVE_SESSIONS - 1),
        "com limite - 1 ainda tem capacidade de pré-carga"
    );
    assert!(
        !SessionPool::has_preload_capacity(MAX_LIVE_SESSIONS),
        "com limite atingido não pode abrir nova pré-carga"
    );
    assert!(
        !SessionPool::has_preload_capacity(MAX_LIVE_SESSIONS + 1),
        "acima do limite não pode pré-carregar"
    );
}

#[test]
fn agent_session_adopt_e_is_unused_preload() {
    use std::time::Instant;

    let mut session = AgentSession::default();
    assert!(!session.is_unused_preload());

    session.place = Some(Place::for_preload(PathBuf::from("/test/project")));
    session.provider_key = Some(("claude".into(), "claude-sonnet-4-5".into()));
    assert!(session.last_used.is_none());
    assert_eq!(session.messages, 0);

    session.last_used = Some(Instant::now());
    assert!(!session.is_unused_preload());
}

// O cmd.exe ignora as flags da CLI e fica vivo enquanto o stdin estiver aberto, como o claude ocioso.
#[cfg(windows)]
fn idle_stand_in() -> crate::claude::LiveSession {
    crate::claude::LiveSession::open(
        std::path::Path::new("cmd.exe"),
        &[],
        "",
        "haiku",
        crate::claude::Access::TextOnly,
        None,
        None,
    )
    .expect("abre o processo substituto")
}

#[cfg(windows)]
fn live_slot(pool: &SessionPool, agent: &Agent, last_used: Option<std::time::Instant>) -> Arc<SessionSlot> {
    let slot = pool.slot(crate::session_key::SessionKey {
        project_id: PROJECT,
        agent: agent.clone(),
    });
    let mut session = slot.session.try_lock().expect("slot recém-criado está livre");
    session.adopt_preload(
        idle_stand_in(),
        Some("sessao-anterior".into()),
        Place::for_preload(PathBuf::from("/test/project")),
        ("claude".into(), "haiku".into()),
    );
    session.last_used = last_used;
    drop(session);
    slot
}

#[cfg(windows)]
#[test]
fn reap_idle_fecha_sessao_usada_parada_e_preserva_o_resume() {
    tauri::async_runtime::block_on(async {
        let pool = SessionPool::default();
        let slot = live_slot(&pool, &base_agent(), Some(std::time::Instant::now()));
        assert!(slot.is_live());

        pool.reap_idle(Duration::ZERO).await;

        let session = slot.session.lock().await;
        assert!(session.live.is_none(), "processo ocioso deveria ter sido encerrado");
        assert!(!slot.is_live(), "pid deveria ter sido zerado");
        assert_eq!(
            session.session_id.as_deref(),
            Some("sessao-anterior"),
            "a próxima mensagem precisa do session_id para o --resume"
        );
    });
}

#[cfg(windows)]
#[test]
fn reap_idle_fecha_precarga_nunca_usada() {
    tauri::async_runtime::block_on(async {
        let pool = SessionPool::default();
        let slot = live_slot(&pool, &base_agent(), None);

        pool.reap_idle(Duration::ZERO).await;

        assert!(slot.session.lock().await.live.is_none());
    });
}

#[cfg(windows)]
#[test]
fn reap_idle_mantem_sessao_recente_e_sessao_em_uso() {
    tauri::async_runtime::block_on(async {
        let pool = SessionPool::default();
        let recent = live_slot(&pool, &base_agent(), Some(std::time::Instant::now()));
        let busy = live_slot(&pool, &replica_id(&base_agent(), 2), Some(std::time::Instant::now()));
        let mut exchange = ExchangeSessions::new(&pool, PROJECT);
        let held = exchange.hold(&replica_id(&base_agent(), 2)).await;

        pool.reap_idle(SESSION_IDLE_LIMIT).await;
        assert!(recent.session.lock().await.live.is_some(), "sessão recente não está ociosa");

        pool.reap_idle(Duration::ZERO).await;
        assert!(busy.is_live(), "sessão com mensagem em andamento não pode ser encerrada");
        drop(held);

        recent.session.lock().await.close_live().await;
        busy.session.lock().await.close_live().await;
    });
}
