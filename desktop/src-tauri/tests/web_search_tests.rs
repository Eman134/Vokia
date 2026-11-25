use serde_json::json;

use crate::agents::{Agent, AgentProfile};
use crate::commands::providers::ProviderConnectionStatus;
use crate::db::{Db, SETTING_WEB_SEARCH};
use crate::mcp::{tools, ToolScope};
use crate::orchestrator::prompts::{role_with_options, web_search_prompt_hint, ToolPromptMode};
use crate::orchestrator::step_labels::{label_for_action, label_for_tool, StepAction};
use crate::orchestrator::web_search::{
    contains_captcha_marker, has_configured_web_search_backend, load_web_search_settings,
    parse_search_response, sanitize_query, select_auxiliary_provider, step_label_for_query,
    McpSearchConnectorConfig, WebSearchMode, WebSearchSettings, MAX_WEB_SEARCH_RESPONSE_CHARS,
    NO_SEARCH_BACKEND_ERROR,
};
use crate::orchestrator::work_steps::{WorkStep, WorkStepCollector};
use crate::providers::{initial_providers, ModelTier};

fn sample_profile(cli: &str) -> AgentProfile {
    AgentProfile {
        id: Agent::from_stored("kairo").unwrap(),
        name: "Kairo".to_string(),
        role: "Backend Developer".to_string(),
        avatar: "vesper".to_string(),
        cli: cli.to_string(),
        model: "default".to_string(),
        model_tier: Some(ModelTier::Intermediate),
        reasoning_effort: None,
        follow_default_llm: false,
        fallback: Vec::new(),
        command: String::new(),
        builtin: true,
        extra_instructions: String::new(),
    }
}

#[test]
fn criterion_1_native_providers_default_true_and_no_extra_tokens_in_prompt_or_tools_list() {
    let providers = initial_providers();
    let claude = providers.iter().find(|p| p.id == "claude").unwrap();
    let codex = providers.iter().find(|p| p.id == "codex").unwrap();
    let gemini = providers.iter().find(|p| p.id == "antigravity").unwrap();
    let opencode = providers.iter().find(|p| p.id == "opencode").unwrap();

    assert!(claude.has_native_web_search);
    assert!(codex.has_native_web_search);
    assert!(gemini.has_native_web_search);
    assert!(!opencode.has_native_web_search);

    // tools/list não deve incluir web_search (economia de prompt cache)
    for scope in [ToolScope::Coordinator, ToolScope::Worker] {
        let defs = tools::definitions_for_scope(scope);
        assert!(
            !defs.iter().any(|d| d["name"].as_str() == Some("web_search")),
            "web_search não deve entrar no tools/list padrão"
        );
    }

    // Provedores com has_native_web_search = true não recebem dica extra no prompt mesmo com backend ativo
    for (cli, mode) in [
        ("claude", ToolPromptMode::Mcp),
        ("codex", ToolPromptMode::Mcp),
        ("antigravity", ToolPromptMode::VokiaCli),
    ] {
        let profile = sample_profile(cli);
        let with_native = role_with_options(
            &profile,
            None,
            "Elenco: Kairo.",
            None,
            "Projeto: Teste.",
            mode,
            ToolScope::Worker,
            None,
            true,
            true,
        );
        let without_backend = role_with_options(
            &profile,
            None,
            "Elenco: Kairo.",
            None,
            "Projeto: Teste.",
            mode,
            ToolScope::Worker,
            None,
            true,
            false,
        );
        assert_eq!(with_native, without_backend);
        assert!(!with_native.contains("web_search"));
    }

    // Provedor sem busca nativa (OpenCode) recebe instrução concisa quando backend está configurado
    let opencode_profile = sample_profile("opencode");
    let opencode_prompt = role_with_options(
        &opencode_profile,
        None,
        "Elenco: Kairo.",
        None,
        "Projeto: Teste.",
        ToolPromptMode::VokiaCli,
        ToolScope::Worker,
        None,
        false,
        true,
    );
    assert!(opencode_prompt.contains("vokia-cli cmd web_search"));
    assert_eq!(
        web_search_prompt_hint(ToolPromptMode::VokiaCli, true, true),
        ""
    );
}

#[test]
fn criterion_2_auxiliary_provider_selection_is_dynamic_and_response_is_concise_with_urls() {
    let providers = initial_providers();
    // Apenas Gemini (antigravity) conectado e habilitado — não deve depender de hardcode de Codex/Claude
    let statuses = vec![
        ("claude".to_string(), false, ProviderConnectionStatus::Missing),
        ("codex".to_string(), false, ProviderConnectionStatus::Missing),
        ("opencode".to_string(), true, ProviderConnectionStatus::Connected),
        (
            "antigravity".to_string(),
            true,
            ProviderConnectionStatus::Connected,
        ),
    ];

    let selected = select_auxiliary_provider(&providers, &statuses, None).unwrap();
    assert_eq!(selected.id, "antigravity");

    // Se usuário customizar um provedor GenericCli com has_native_web_search = true, ele também é elegível
    let mut custom_list = vec![providers
        .iter()
        .find(|p| p.id == "opencode")
        .unwrap()
        .clone()];
    let mut custom = custom_list[0].clone();
    custom.id = "custom-search".to_string();
    custom.label = "Custom Search".to_string();
    custom.has_native_web_search = true;
    custom_list.push(custom);

    let custom_statuses = vec![
        ("opencode".to_string(), true, ProviderConnectionStatus::Connected),
        (
            "custom-search".to_string(),
            true,
            ProviderConnectionStatus::Connected,
        ),
    ];
    let selected_custom = select_auxiliary_provider(&custom_list, &custom_statuses, None).unwrap();
    assert_eq!(selected_custom.id, "custom-search");

    // Resposta parseada mantém resumo conciso, URLs verificáveis e teto seguro de caracteres
    let long_summary = "a".repeat(MAX_WEB_SEARCH_RESPONSE_CHARS + 500);
    let raw = json!({
        "summary": long_summary,
        "sources": [
            { "title": "Tokio Docs", "url": "https://tokio.rs/tokio/tutorial" },
            { "title": "Invalid", "url": "file:///etc/passwd" },
            { "title": "Tokio Docs Dup", "url": "https://tokio.rs/tokio/tutorial" },
            { "title": "Docs.rs", "url": "https://docs.rs/tokio" }
        ]
    })
    .to_string();
    let parsed = parse_search_response(
        "tokio runtime",
        &raw,
        "auxiliary:antigravity",
        &step_label_for_query("tokio runtime"),
    );
    assert!(parsed.truncated);
    assert!(parsed.summary.chars().count() <= MAX_WEB_SEARCH_RESPONSE_CHARS);
    assert_eq!(parsed.sources.len(), 2);
    assert_eq!(parsed.sources[0].url, "https://tokio.rs/tokio/tutorial");
    assert_eq!(parsed.sources[1].url, "https://docs.rs/tokio");

    // Sanitização remove segredos da query
    let sanitized = sanitize_query("buscar sk-proj-secret1234567890 rust").unwrap();
    assert!(!sanitized.contains("sk-proj-secret"));
}

#[test]
fn criterion_3_no_backend_configured_returns_actionable_error_and_persists_settings() {
    let providers = initial_providers();
    let none_connected = vec![
        ("claude".to_string(), false, ProviderConnectionStatus::Missing),
        ("codex".to_string(), false, ProviderConnectionStatus::Missing),
        ("opencode".to_string(), true, ProviderConnectionStatus::Connected),
        (
            "antigravity".to_string(),
            false,
            ProviderConnectionStatus::SignedOut,
        ),
    ];

    let err = select_auxiliary_provider(&providers, &none_connected, None).unwrap_err();
    assert_eq!(err, NO_SEARCH_BACKEND_ERROR);

    let default_settings = WebSearchSettings::default();
    assert!(!has_configured_web_search_backend(
        &default_settings,
        &providers,
        &none_connected
    ));

    // Persistência em SQLite via SETTING_WEB_SEARCH
    let db_dir = std::env::temp_dir().join(format!(
        "vokia-web-search-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&db_dir);
    let db = Db::open(&db_dir).unwrap();
    let mcp_settings = WebSearchSettings {
        mode: WebSearchMode::McpConnector,
        auxiliary_provider_id: None,
        mcp_connector: Some(McpSearchConnectorConfig {
            preset: Some("brave".to_string()),
            server_name: "brave-search".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@modelcontextprotocol/server-brave-search".to_string()],
            url: None,
            tool_name: Some("brave_web_search".to_string()),
            env: Default::default(),
        }),
    };
    db.save_setting(
        SETTING_WEB_SEARCH,
        &serde_json::to_string(&mcp_settings).unwrap(),
    )
    .unwrap();
    let loaded = load_web_search_settings(&db);
    assert_eq!(loaded, mcp_settings);
    assert!(has_configured_web_search_backend(
        &loaded,
        &providers,
        &none_connected
    ));
    drop(db);
    let _ = std::fs::remove_dir_all(&db_dir);

    // Marcadores anti-bot/CAPTCHA são detectados para bloqueio explícito
    assert!(contains_captcha_marker("Please complete the Cloudflare Turnstile CAPTCHA"));
    assert!(!contains_captcha_marker("Documentação oficial do Rust 2024"));
}

#[test]
fn criterion_4_step_labels_and_work_steps_integrate_web_search() {
    let direct = label_for_tool("web_search", &json!({ "query": "tokio spawn" })).unwrap();
    assert_eq!(direct, "Pesquisando na web: 'tokio spawn'");

    let mcp_direct =
        label_for_tool("mcp__vokia__web_search", &json!({ "query": "serde rename" })).unwrap();
    assert_eq!(mcp_direct, "Pesquisando na web: 'serde rename'");

    let via_app_cmd = label_for_tool(
        "mcp__vokia__app_command_call",
        &json!({
            "name": "web_search",
            "arguments": { "query": "tauri v2" }
        }),
    )
    .unwrap();
    assert_eq!(via_app_cmd, "Pesquisando na web: 'tauri v2'");

    let action_label = label_for_action(StepAction::WebSearch {
        query: Some("vue 3 composables"),
    })
    .unwrap();
    assert_eq!(action_label, "Pesquisando na web: 'vue 3 composables'");

    let mut collector = WorkStepCollector::new();
    collector.record(WorkStep::WebSearch {
        query: "rust async trait".to_string(),
    });
    let formatted = collector.format().unwrap();
    assert!(formatted.block.contains("searched: rust async trait"));
}
