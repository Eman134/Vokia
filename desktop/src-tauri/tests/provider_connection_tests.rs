use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::db::parse_usage_pause_threshold;
use crate::provider_auth::{
    classify_auth_diagnostic_error, diagnose_provider_sync, extract_device_code,
    extract_first_url, ExecutableSource, LoginSessionPhase, ProviderDiagnosticState,
    ProviderLoginManager,
};
use crate::provider_env::{
    redact_registered_secrets, validate_env_var_name, ProviderEnvRegistry,
};
use crate::providers::{AuthFlowKind, ProviderAuthProbe, ProviderConfig, ProviderProtocol, ProviderRegistry};
use crate::usage::model::{ProviderPlanUsage, UsageWindow, FIVE_HOUR_MINS};
use crate::usage::UsageEngine;

fn temp_test_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("vokia-{prefix}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn ca1_diagnoses_not_installed_and_installed_signed_out_with_version() {
    let dir = temp_test_dir("diag");
    let missing_cfg = ProviderConfig {
        id: "missing-cli".to_string(),
        label: "Missing CLI".to_string(),
        enabled: true,
        disabled_reason: None,
        protocol: ProviderProtocol::GenericCli,
        default_command: "vokia-nonexistent-binary-094".to_string(),
        default_model: "m1".to_string(),
        models: vec!["m1".to_string()],
        free_models: vec![],
        tiers: crate::providers::ModelTiers {
            basic: "m1".to_string(),
            intermediate: "m1".to_string(),
            advanced: "m1".to_string(),
        },
        supports_mcp: false,
        tool_transport: None,
        supports_sandbox: false,
        has_native_web_search: false,
        reserved_flags: vec![],
        rate_limit_patterns: vec![],
        model_unavailable_patterns: vec![],
        discovery: None,
        generic: None,
        model_grouping: crate::providers::ModelGroupingMode::Flat,
        effort: None,
        auth_probe: Some(ProviderAuthProbe {
            env_vars: vec!["TEST_MISSING_TOKEN_094".to_string()],
            credential_files: vec![],
            free_without_login: false,
        }),
        auth_flows: vec![],
        logout: None,
        env_presets: vec![],
        auth_diagnostics: None,
        install_hint: Some("npm install -g missing".to_string()),
        connect_hint: None,
        badge_code: None,
        display_order: None,
        builtin_seed: false,
    };

    let diag_missing = diagnose_provider_sync(&missing_cfg, None);
    assert!(!diag_missing.installed);
    assert_eq!(diag_missing.state, ProviderDiagnosticState::NotInstalled);

    // Cria um executável/script explícito que responde --version
    #[cfg(windows)]
    let script_path = {
        let p = dir.join("fake-cli.cmd");
        fs::write(&p, "@echo off\r\nif \"%1\"==\"--version\" echo fake-cli 1.2.3\r\n").unwrap();
        p
    };
    #[cfg(not(windows))]
    let script_path = {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join("fake-cli.sh");
        fs::write(&p, "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"fake-cli 1.2.3\"; fi\n").unwrap();
        let mut perms = fs::metadata(&p).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&p, perms).unwrap();
        p
    };

    let mut installed_cfg = missing_cfg;
    installed_cfg.id = "fake-cli".to_string();
    installed_cfg.default_command = script_path.display().to_string();

    let diag_signed_out = diagnose_provider_sync(&installed_cfg, None);
    assert!(diag_signed_out.installed);
    assert_eq!(
        diag_signed_out.state,
        ProviderDiagnosticState::InstalledSignedOut
    );
    assert_eq!(diag_signed_out.version.as_deref(), Some("fake-cli 1.2.3"));
    assert_eq!(
        diag_signed_out.executable_source,
        Some(ExecutableSource::Explicit)
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn ca2_extracts_device_code_and_url_and_handles_free_no_login() {
    let sample = "Open https://auth.openai.com/device and enter code ABCD-1234 to continue.";
    assert_eq!(
        extract_first_url(sample).as_deref(),
        Some("https://auth.openai.com/device")
    );
    assert_eq!(extract_device_code(sample).as_deref(), Some("ABCD-1234"));

    let registry = ProviderRegistry::seeded();
    let opencode = registry.get("opencode").expect("opencode seed");
    let env_reg = ProviderEnvRegistry::in_memory();
    let manager = ProviderLoginManager::default();

    let rt = tokio::runtime::Runtime::new().unwrap();
    let state = rt
        .block_on(manager.start_login(&opencode, Some("free-no-login"), None, &env_reg))
        .expect("free-no-login deve concluir imediatamente");
    assert_eq!(state.phase, LoginSessionPhase::Completed);
    assert_eq!(state.kind, AuthFlowKind::FreeNoLogin);
}

#[test]
fn ca3_sensitive_env_var_never_returns_value_and_is_redacted_in_logs() {
    let dir = temp_test_dir("env-secret");
    let env_reg = ProviderEnvRegistry::load(&dir);
    let secret = "oc_super_secret_token_987654321";

    let saved = env_reg
        .set_var("opencode", "OPENCODE_API_KEY", secret, true)
        .expect("salvar variável sensível");
    assert!(saved.sensitive);
    assert!(saved.stored);
    assert_eq!(saved.status, "saved");
    assert!(saved.value.is_none(), "valor sensível nunca pode retornar ao frontend");

    let listed = env_reg.list_vars("opencode");
    assert_eq!(listed.len(), 1);
    assert!(listed[0].stored);
    assert!(listed[0].value.is_none());

    let resolved = env_reg.resolve_env_for_provider("opencode");
    assert_eq!(resolved.get("OPENCODE_API_KEY").map(String::as_str), Some(secret));

    let log_line = format!("Authorization: Bearer {secret} failed");
    let redacted = redact_registered_secrets(&log_line);
    assert!(!redacted.contains(secret));
    assert!(redacted.contains("[REDACTED]"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn ca4_rejects_reserved_env_var_names() {
    for reserved in ["PATH", "path", "HOME", "VOKIA_URL", "VOKIA_TOKEN", "vokia_token", "VOKIA_CLI"] {
        let err = validate_env_var_name(reserved)
            .expect_err(&format!("variável reservada '{reserved}' deve ser rejeitada"));
        assert!(
            err.contains("reservada"),
            "mensagem inesperada para '{reserved}': {err}"
        );
    }

    let env_reg = ProviderEnvRegistry::in_memory();
    let err = env_reg
        .set_var("claude", "VOKIA_TOKEN", "abc", false)
        .expect_err("set_var deve rejeitar VOKIA_TOKEN");
    assert!(err.contains("reservada"));
}

#[test]
fn ca5_classifies_403_distinctly_from_401_and_incompatible() {
    let registry = ProviderRegistry::seeded();
    let opencode = registry.get("opencode").expect("opencode seed");
    let rules = opencode.auth_diagnostics.as_ref();

    let err_403 = classify_auth_diagnostic_error("HTTP 403 Forbidden: plan inactive", rules);
    assert_eq!(err_403.category, "forbidden_403");
    assert!(err_403.message.contains("403"));
    assert!(
        err_403.message.contains("plano") || err_403.message.contains("rede"),
        "403 deve orientar sobre plano/rede: {}",
        err_403.message
    );

    let err_401 = classify_auth_diagnostic_error("HTTP 401 Unauthorized: invalid_api_key", rules);
    assert_eq!(err_401.category, "unauthorized_401");
    assert!(err_401.message.contains("401"));

    let err_db = classify_auth_diagnostic_error("SQLite database schema version is newer version", rules);
    assert_eq!(err_db.category, "incompatible");
}

#[test]
fn declarative_providers_have_no_mandatory_default_and_respect_usage_pause_threshold() {
    let registry = ProviderRegistry::seeded();
    let providers = registry.list().expect("listar provedores");
    for id in ["claude", "codex", "opencode", "antigravity"] {
        let p = providers.iter().find(|item| item.id == id).expect(id);
        assert!(
            !p.auth_flows.is_empty(),
            "provedor {id} deve declarar auth_flows"
        );
        assert!(p.logout.is_some(), "provedor {id} deve declarar logout");
        assert!(
            !p.env_presets.is_empty(),
            "provedor {id} deve declarar env_presets"
        );
    }

    assert_eq!(parse_usage_pause_threshold("85"), Some(85));
    assert_eq!(parse_usage_pause_threshold("90"), Some(90));
    assert_eq!(parse_usage_pause_threshold("95"), Some(95));
    assert_eq!(parse_usage_pause_threshold("off"), None);

    let engine = UsageEngine::default();
    engine.record_provider_usage(ProviderPlanUsage {
        provider_id: "codex".to_string(),
        label: Some("Codex".to_string()),
        primary: Some(UsageWindow::new(88, 0, FIVE_HOUR_MINS)),
        secondary: None,
        tertiary: None,
        groups: vec![],
        credits_available: None,
        reset_credits_available: None,
        updated_at_ms: 1_000,
        read_failed: false,
    });

    engine.set_pause_threshold(Some(85));
    assert!(engine.is_provider_paused_by_limit("codex", None, 1_000));

    engine.set_pause_threshold(Some(90));
    assert!(!engine.is_provider_paused_by_limit("codex", None, 1_000));

    engine.set_pause_threshold(None);
    assert!(!engine.is_provider_paused_by_limit("codex", None, 1_000));
}
