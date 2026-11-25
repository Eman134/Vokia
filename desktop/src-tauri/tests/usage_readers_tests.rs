use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json as AxumJson, Router};
use rusqlite::Connection;
use serde_json::{json, Value as Json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

use crate::usage::model::{
    ProviderPlanUsage, UsageWindow, UsageWindowKind, FIVE_HOUR_MINS, MONTHLY_MINS, WEEKLY_MINS,
};
use crate::usage::read_gate::UsageReadGate;
use crate::usage::readers::claude::{
    parse_oauth_usage_json, parse_rate_limit_event_json, read_claude_usage, ClaudeReaderConfig,
};
use crate::usage::readers::codex::{
    parse_rate_limits_rpc_message, read_codex_usage, read_codex_via_rpc_io, CodexReaderConfig,
};
use crate::usage::readers::gemini::{
    is_gemini_api_key_quota_error, parse_quota_summary_json, read_gemini_usage,
    read_valid_oauth_token, GeminiReaderConfig,
};
use crate::usage::readers::opencode::{
    parse_opencode_usage_json, read_opencode_usage, resolve_opencode_api_key, OpenCodeReaderConfig,
};
use crate::usage::readers::{redact_secrets, resolve_endpoint_url};
use crate::usage::UsageEngine;

const NOW_MS: u64 = 1_764_000_000_000;

fn temp_test_dir(tag: &str) -> PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "vokia-usage-readers-{tag}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

async fn spawn_test_server(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback listener");
    let addr = listener.local_addr().expect("local addr");
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    (format!("http://127.0.0.1:{}", addr.port()), handle)
}

#[test]
fn redacao_remove_tokens_explicitos_bearer_e_prefixos_conhecidos() {
    let secret = "my-ultra-secret-local-token-999";
    let raw = format!(
        "Failed calling API with Authorization: Bearer ya29.a0AfH6SMC_super_secret_token, \
         key={secret}, access_token=\"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.abc.def\", \
         api_key=AIzaSyD-1234567890abcdef, go=oc_live_1234567890"
    );
    let sanitized = redact_secrets(&raw, &[secret]);
    assert!(!sanitized.contains(secret));
    assert!(!sanitized.contains("ya29."));
    assert!(!sanitized.contains("eyJhbGci"));
    assert!(!sanitized.contains("AIzaSyD"));
    assert!(!sanitized.contains("oc_live_"));
    assert!(sanitized.contains("[REDACTED]"));

    // URL overrides must only allow loopback addresses.
    assert!(resolve_endpoint_url(
        "https://opencode.ai/zen/go/v1/usage",
        Some("http://127.0.0.1:8080/usage")
    )
    .is_ok());
    assert!(resolve_endpoint_url(
        "https://opencode.ai/zen/go/v1/usage",
        Some("https://evil.example.com/steal")
    )
    .is_err());
}

#[test]
fn claude_parseia_rate_limit_event_passivo_e_oauth_usage() {
    let line = json!({
        "type": "rate_limit_event",
        "rate_limit_info": {
            "unifiedWindows": {
                "five_hour": { "utilization": 0.64, "resetsAt": 1764018000, "status": "allowed" },
                "seven_day": { "utilization": 0.91, "resetsAt": 1764604800, "status": "rejected" },
                "seven_day_sonnet": { "utilization": 0.35, "resetsAt": 1764604800, "status": "allowed" }
            }
        }
    })
    .to_string();

    let passive = parse_rate_limit_event_json(&line, NOW_MS).expect("passive rate_limit_event");
    assert_eq!(passive.provider_id, "claude");
    assert_eq!(passive.primary.as_ref().unwrap().percent, 64);
    assert_eq!(passive.primary.as_ref().unwrap().kind, UsageWindowKind::FiveHour);
    assert_eq!(passive.secondary.as_ref().unwrap().percent, 91);
    assert!(passive.secondary.as_ref().unwrap().rejected);
    assert_eq!(passive.groups.len(), 1);
    assert_eq!(passive.groups[0].label, "Claude Sonnet");
    assert_eq!(passive.groups[0].window.percent, 35);

    let oauth_body = json!({
        "five_hour": { "utilization": 28.4, "resets_at": "2026-11-25T22:00:00Z" },
        "seven_day": { "utilization": 52.7, "resets_at": "2026-12-01T00:00:00Z" },
        "seven_day_opus": { "utilization": 15.0, "resets_at": "2026-12-01T00:00:00Z" },
        "extra_usage": { "is_enabled": true }
    })
    .to_string();

    let probed = parse_oauth_usage_json(&oauth_body, NOW_MS)
        .expect("oauth parse ok")
        .expect("usage present");
    assert_eq!(probed.primary.as_ref().unwrap().percent, 28);
    assert_eq!(probed.secondary.as_ref().unwrap().percent, 53);
    assert_eq!(probed.credits_available, Some(true));
    assert_eq!(probed.groups.len(), 1);
    assert_eq!(probed.groups[0].label, "Claude Opus");
}

#[test]
fn claude_sonda_espacada_respeita_passivo_cooldown_429_e_token_expirado() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let dir = temp_test_dir("claude");
        let creds_path = dir.join(".credentials.json");
        let secret_token = "sk-ant-oat01-claude-secret-token-value";
        std::fs::write(
            &creds_path,
            json!({
                "claudeAiOauth": {
                    "accessToken": secret_token,
                    "expiresAt": NOW_MS + 3_600_000
                }
            })
            .to_string(),
        )
        .unwrap();

        let hits = Arc::new(AtomicUsize::new(0));
        let hits_state = Arc::clone(&hits);
        let router = Router::new()
            .route(
                "/api/oauth/usage",
                get(|State(counter): State<Arc<AtomicUsize>>, headers: HeaderMap| async move {
                    let count = counter.fetch_add(1, Ordering::SeqCst) + 1;
                    let auth = headers
                        .get("authorization")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");
                    assert_eq!(auth, "Bearer sk-ant-oat01-claude-secret-token-value");
                    assert_eq!(
                        headers.get("anthropic-beta").and_then(|v| v.to_str().ok()),
                        Some("oauth-2025-04-20")
                    );
                    if count == 1 {
                        (
                            StatusCode::OK,
                            AxumJson(json!({
                                "five_hour": { "utilization": 31.0, "resets_at": "2026-11-25T23:00:00Z" },
                                "seven_day": { "utilization": 14.0, "resets_at": "2026-12-01T00:00:00Z" }
                            })),
                        )
                    } else {
                        (
                            StatusCode::TOO_MANY_REQUESTS,
                            AxumJson(json!({ "error": "rate_limit" })),
                        )
                    }
                }),
            )
            .with_state(hits_state);

        let (base_url, server) = spawn_test_server(router).await;
        let config = ClaudeReaderConfig {
            endpoint_url: Some(format!("{base_url}/api/oauth/usage")),
            credentials_paths: vec![creds_path.clone()],
            env_oauth_token: Some(String::new()),
            env_api_key: Some(String::new()),
            probe_cooldown_ms: 15 * 60 * 1_000,
            timeout: Duration::from_secs(5),
        };

        // 1) Passive snapshot within cooldown -> zero HTTP calls.
        let passive = ProviderPlanUsage {
            provider_id: "claude".to_string(),
            label: Some("Claude Code".to_string()),
            primary: Some(UsageWindow::new(40, (NOW_MS / 1_000) + 7_200, FIVE_HOUR_MINS)),
            secondary: Some(UsageWindow::new(20, (NOW_MS / 1_000) + 86_400, WEEKLY_MINS)),
            tertiary: None,
            groups: Vec::new(),
            credits_available: None,
            reset_credits_available: None,
            updated_at_ms: NOW_MS - 60_000,
            read_failed: false,
        };
        let from_passive = read_claude_usage(&config, Some(passive.clone()), NOW_MS)
            .await
            .expect("ok")
            .expect("usage");
        assert_eq!(from_passive.primary.as_ref().unwrap().percent, 40);
        assert_eq!(hits.load(Ordering::SeqCst), 0);

        // 2) App start with no passive snapshot -> performs spaced probe.
        let probed = read_claude_usage(&config, None, NOW_MS)
            .await
            .expect("probe ok")
            .expect("usage");
        assert_eq!(probed.primary.as_ref().unwrap().percent, 31);
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        // 3) Second probe returns HTTP 429 -> UsageReadGate preserves last valid reading and backs off.
        let gate = UsageReadGate::default();
        gate.record_push(probed.clone());
        let after_cooldown = NOW_MS + 20 * 60 * 1_000;
        let outcome = gate
            .read_optional("claude", after_cooldown, || async {
                read_claude_usage(&config, gate.get("claude"), after_cooldown)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await;
        assert!(outcome.in_backoff);
        assert!(outcome.error.as_deref().unwrap().contains("429"));
        assert!(!outcome.error.as_deref().unwrap().contains(secret_token));
        let preserved = outcome.usage.expect("preserves last valid reading");
        assert_eq!(preserved.primary.as_ref().unwrap().percent, 31);
        assert!(preserved.read_failed);

        // 4) Expired local token -> fails without hitting network and never leaks token.
        std::fs::write(
            &creds_path,
            json!({
                "claudeAiOauth": {
                    "accessToken": secret_token,
                    "expiresAt": NOW_MS + 30_000
                }
            })
            .to_string(),
        )
        .unwrap();
        let err = read_claude_usage(&config, None, NOW_MS)
            .await
            .expect_err("expired token must error");
        assert!(!err.message.contains(secret_token));
        assert_eq!(hits.load(Ordering::SeqCst), 2);

        server.abort();
        let _ = std::fs::remove_dir_all(&dir);
    });
}

#[test]
fn codex_parseia_rpc_read_updated_creditos_e_reset_credits() {
    let rpc_read = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "result": {
            "rateLimits": {
                "limitId": "codex",
                "limitName": "Codex",
                "primary": {
                    "usedPercent": 62.6,
                    "windowDurationMins": 300,
                    "resetsAt": 1764018000
                },
                "secondary": {
                    "usedPercent": 24.2,
                    "windowDurationMins": 10080,
                    "resetsAt": 1764604800
                },
                "credits": {
                    "hasCredits": true,
                    "unlimited": false
                }
            },
            "rateLimitsByLimitId": {
                "codex": {
                    "limitId": "codex",
                    "primary": { "usedPercent": 62.6, "windowDurationMins": 300, "resetsAt": 1764018000 }
                },
                "codex_spark": {
                    "limitId": "codex_spark",
                    "limitName": "GPT-5.3-Codex-Spark",
                    "primary": { "usedPercent": 88.0, "windowDurationMins": 300, "resetsAt": 1764018000 },
                    "secondary": { "usedPercent": 41.0, "windowDurationMins": 10080, "resetsAt": 1764604800 }
                }
            },
            "rateLimitResetCredits": {
                "availableCount": 3
            }
        }
    });

    let usage = parse_rate_limits_rpc_message(&rpc_read, NOW_MS)
        .expect("parse ok")
        .expect("codex usage");
    assert_eq!(usage.provider_id, "codex");
    assert_eq!(usage.primary.as_ref().unwrap().percent, 63);
    assert_eq!(usage.primary.as_ref().unwrap().kind, UsageWindowKind::FiveHour);
    assert_eq!(usage.secondary.as_ref().unwrap().percent, 24);
    assert_eq!(usage.secondary.as_ref().unwrap().kind, UsageWindowKind::Weekly);
    assert_eq!(usage.credits_available, Some(true));
    assert_eq!(usage.reset_credits_available, Some(3));
    assert_eq!(usage.groups.len(), 2);
    assert_eq!(usage.groups[0].models, vec!["GPT-5.3-Codex-Spark".to_string()]);
    assert_eq!(usage.groups[0].window.percent, 88);
    assert_eq!(usage.groups[1].window.percent, 41);

    // Push event account/rateLimits/updated with unlimited credits and 0 reset credits.
    let push_event = json!({
        "method": "account/rateLimits/updated",
        "params": {
            "rateLimits": {
                "primary": { "usedPercent": 100.0, "windowDurationMins": 300, "resetsAt": 1764018000 },
                "credits": { "hasCredits": false, "unlimited": true }
            },
            "rateLimitResetCredits": { "availableCount": 0 }
        }
    });
    let pushed = parse_rate_limits_rpc_message(&push_event, NOW_MS)
        .expect("push ok")
        .expect("pushed usage");
    assert_eq!(pushed.credits_available, Some(true));
    assert!(pushed.is_using_credits());
    assert_eq!(pushed.reset_credits_available, None);

    // Unauthenticated JSON-RPC error falls back cleanly to Ok(None).
    let unauth = json!({
        "id": 2,
        "error": { "code": -32001, "message": "User is not logged in" }
    });
    assert_eq!(parse_rate_limits_rpc_message(&unauth, NOW_MS).unwrap(), None);
}

#[test]
fn codex_executa_handshake_rpc_em_memoria_e_faz_fallback_sem_cli() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (client_read, client_write) = tokio::io::split(client_io);
        let (server_read, mut server_write) = tokio::io::split(server_io);

        let mock_server = tokio::spawn(async move {
            let mut lines = BufReader::new(server_read).lines();
            let init_line = lines.next_line().await.unwrap().unwrap();
            let init_msg: Json = serde_json::from_str(&init_line).unwrap();
            assert_eq!(init_msg["method"], "initialize");
            server_write
                .write_all(b"{\"id\":1,\"result\":{\"userAgent\":\"codex-test\"}}\n")
                .await
                .unwrap();

            let initialized_line = lines.next_line().await.unwrap().unwrap();
            let initialized_msg: Json = serde_json::from_str(&initialized_line).unwrap();
            assert_eq!(initialized_msg["method"], "initialized");

            let read_line = lines.next_line().await.unwrap().unwrap();
            let read_msg: Json = serde_json::from_str(&read_line).unwrap();
            assert_eq!(read_msg["method"], "account/rateLimits/read");

            let reply = json!({
                "id": 2,
                "result": {
                    "rateLimits": {
                        "primary": { "usedPercent": 45, "windowDurationMins": 300, "resetsAt": 1764018000 },
                        "secondary": { "usedPercent": 19, "windowDurationMins": 10080, "resetsAt": 1764604800 },
                        "credits": { "hasCredits": true, "unlimited": false }
                    },
                    "rateLimitResetCredits": { "availableCount": 1 }
                }
            });
            server_write
                .write_all(format!("{reply}\n").as_bytes())
                .await
                .unwrap();
        });

        let usage = read_codex_via_rpc_io(BufReader::new(client_read), client_write, NOW_MS)
            .await
            .expect("rpc ok")
            .expect("usage");
        mock_server.await.unwrap();
        assert_eq!(usage.primary.as_ref().unwrap().percent, 45);
        assert_eq!(usage.secondary.as_ref().unwrap().percent, 19);
        assert_eq!(usage.credits_available, Some(true));
        assert_eq!(usage.reset_credits_available, Some(1));

        // Missing CLI executable -> clean Ok(None).
        let missing_cfg = CodexReaderConfig {
            command: Some("vokia-nonexistent-codex-binary-xyz".to_string()),
            timeout: Duration::from_secs(2),
        };
        assert_eq!(read_codex_usage(&missing_cfg, NOW_MS).await.unwrap(), None);
    });
}

#[test]
fn opencode_resolve_chave_em_ordem_somente_leitura_incluindo_sqlite_db() {
    let dir = temp_test_dir("opencode-keys");
    let db_path = dir.join("opencode.db");
    let auth_path = dir.join("auth.json");

    // Seed temporary opencode.db with credential table.
    let writer_conn = Connection::open(&db_path).expect("create temp sqlite");
    writer_conn
        .execute_batch(
            "CREATE TABLE credential (provider TEXT PRIMARY KEY, data TEXT NOT NULL);
             INSERT INTO credential(provider, data) VALUES ('opencode-go', '{\"type\":\"api\",\"key\":\"oc_from_db_go\"}');
             INSERT INTO credential(provider, data) VALUES ('opencode', '{\"type\":\"api\",\"key\":\"oc_from_db_std\"}');",
        )
        .expect("seed credential table");

    std::fs::write(
        &auth_path,
        json!({
            "opencode-go": { "type": "api", "key": "oc_from_auth_go" },
            "opencode": { "type": "api", "key": "oc_from_auth_std" }
        })
        .to_string(),
    )
    .unwrap();

    // 1. Explicit key wins over everything.
    let mut cfg = OpenCodeReaderConfig {
        endpoint_url: None,
        explicit_api_key: Some("oc_explicit".to_string()),
        db_paths: vec![db_path.clone()],
        auth_json_paths: vec![auth_path.clone()],
        env_api_key: Some("oc_from_env".to_string()),
        timeout: Duration::from_secs(5),
    };
    assert_eq!(resolve_opencode_api_key(&cfg).as_deref(), Some("oc_explicit"));

    // 2. opencode.db (read-only + busy_timeout=100) wins over auth.json and env even while writer_conn is open.
    cfg.explicit_api_key = None;
    assert_eq!(
        resolve_opencode_api_key(&cfg).as_deref(),
        Some("oc_from_db_go")
    );

    // 3. Without opencode.db -> auth.json (opencode-go) wins over OPENCODE_API_KEY.
    cfg.db_paths.clear();
    assert_eq!(
        resolve_opencode_api_key(&cfg).as_deref(),
        Some("oc_from_auth_go")
    );

    // 4. Without opencode-go in auth.json -> OPENCODE_API_KEY wins over auth.json (opencode).
    std::fs::write(
        &auth_path,
        json!({
            "opencode": { "type": "api", "key": "oc_from_auth_std" }
        })
        .to_string(),
    )
    .unwrap();
    assert_eq!(
        resolve_opencode_api_key(&cfg).as_deref(),
        Some("oc_from_env")
    );

    // 5. Without OPENCODE_API_KEY -> auth.json (opencode).
    cfg.env_api_key = Some(String::new());
    assert_eq!(
        resolve_opencode_api_key(&cfg).as_deref(),
        Some("oc_from_auth_std")
    );

    // 6. Without any key (free mode) -> None.
    cfg.auth_json_paths.clear();
    assert_eq!(resolve_opencode_api_key(&cfg), None);

    drop(writer_conn);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opencode_go_http_converte_janelas_e_silencia_401_403_e_modo_gratuito() {
    // Monthly more restrictive than weekly -> monthly becomes secondary, weekly becomes tertiary.
    let monthly_tighter = parse_opencode_usage_json(
        &json!({
            "rolling": { "usagePercent": 42.2, "resetInSec": 7200 },
            "weekly": { "usagePercent": 35.0, "resetInSec": 259200 },
            "monthly": { "usagePercent": 78.6, "resetInSec": 1209600 }
        })
        .to_string(),
        NOW_MS,
    )
    .unwrap()
    .unwrap();
    assert_eq!(monthly_tighter.primary.as_ref().unwrap().percent, 42);
    assert_eq!(
        monthly_tighter.primary.as_ref().unwrap().duration_mins,
        FIVE_HOUR_MINS
    );
    assert_eq!(monthly_tighter.secondary.as_ref().unwrap().percent, 79);
    assert_eq!(
        monthly_tighter.secondary.as_ref().unwrap().duration_mins,
        MONTHLY_MINS
    );
    assert_eq!(
        monthly_tighter.secondary.as_ref().unwrap().kind,
        UsageWindowKind::Monthly
    );
    assert_eq!(monthly_tighter.tertiary.as_ref().unwrap().percent, 35);
    assert_eq!(
        monthly_tighter.tertiary.as_ref().unwrap().duration_mins,
        WEEKLY_MINS
    );

    // Weekly more restrictive than monthly -> weekly becomes secondary, monthly becomes tertiary.
    let weekly_tighter = parse_opencode_usage_json(
        &json!({
            "rolling": { "usagePercent": 10.0, "resetInSec": 3600 },
            "weekly": { "usagePercent": 82.0, "resetInSec": 200000 },
            "monthly": { "usagePercent": 50.0, "resetInSec": 900000 }
        })
        .to_string(),
        NOW_MS,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        weekly_tighter.secondary.as_ref().unwrap().kind,
        UsageWindowKind::Weekly
    );
    assert_eq!(weekly_tighter.secondary.as_ref().unwrap().percent, 82);
    assert_eq!(
        weekly_tighter.tertiary.as_ref().unwrap().kind,
        UsageWindowKind::Monthly
    );
    assert_eq!(weekly_tighter.tertiary.as_ref().unwrap().percent, 50);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let router = Router::new().route(
            "/zen/go/v1/usage",
            get(|headers: HeaderMap| async move {
                let auth = headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                match auth {
                    "Bearer oc_valid_go_key_123456" => (
                        StatusCode::OK,
                        AxumJson(json!({
                            "rolling": { "usagePercent": 25.0, "resetInSec": 5400 },
                            "weekly": { "usagePercent": 40.0, "resetInSec": 300000 },
                            "monthly": { "usagePercent": 60.0, "resetInSec": 1000000 }
                        })),
                    ),
                    "Bearer oc_free_401" => (
                        StatusCode::UNAUTHORIZED,
                        AxumJson(json!({ "error": "unauthorized" })),
                    ),
                    "Bearer oc_free_403" => (
                        StatusCode::FORBIDDEN,
                        AxumJson(json!({ "error": "forbidden" })),
                    ),
                    _ => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        AxumJson(json!({ "error": "boom" })),
                    ),
                }
            }),
        );
        let (base_url, server) = spawn_test_server(router).await;
        let endpoint = format!("{base_url}/zen/go/v1/usage");

        // Valid Go key
        let ok_cfg = OpenCodeReaderConfig {
            endpoint_url: Some(endpoint.clone()),
            explicit_api_key: Some("oc_valid_go_key_123456".to_string()),
            db_paths: Vec::new(),
            auth_json_paths: Vec::new(),
            env_api_key: Some(String::new()),
            timeout: Duration::from_secs(5),
        };
        let usage = read_opencode_usage(&ok_cfg, NOW_MS)
            .await
            .expect("ok")
            .expect("go usage");
        assert_eq!(usage.provider_id, "opencode");
        assert_eq!(usage.primary.as_ref().unwrap().percent, 25);
        assert_eq!(usage.secondary.as_ref().unwrap().percent, 60);
        assert_eq!(usage.tertiary.as_ref().unwrap().percent, 40);

        // HTTP 401 and 403 -> silent Ok(None)
        for free_key in ["oc_free_401", "oc_free_403"] {
            let free_cfg = OpenCodeReaderConfig {
                explicit_api_key: Some(free_key.to_string()),
                ..ok_cfg.clone()
            };
            assert_eq!(read_opencode_usage(&free_cfg, NOW_MS).await.unwrap(), None);
        }

        // No key (free mode) -> silent Ok(None)
        let no_key_cfg = OpenCodeReaderConfig {
            explicit_api_key: None,
            ..ok_cfg.clone()
        };
        assert_eq!(read_opencode_usage(&no_key_cfg, NOW_MS).await.unwrap(), None);

        server.abort();
    });
}

#[test]
fn gemini_antigravity_le_token_local_duas_etapas_http_e_trata_fracao_omitida() {
    assert!(is_gemini_api_key_quota_error("HTTP 429 RESOURCE_EXHAUSTED"));
    assert!(is_gemini_api_key_quota_error("Quota exceeded: limit=0 for model"));
    assert!(!is_gemini_api_key_quota_error("invalid argument"));

    // Direct parser test including omitted remainingFraction with valid resetTime => 100% used (0% remaining).
    let parsed = parse_quota_summary_json(
        &json!({
            "groups": [
                {
                    "groupId": "gemini-models",
                    "displayName": "Gemini Models",
                    "models": ["gemini-4-hydrogen-medium", "gemini-3.8-flash-medium"],
                    "buckets": [
                        {
                            "bucketId": "five-hour-bucket",
                            "label": "5-hour limit",
                            "remainingFraction": 0.72,
                            "resetTime": "2026-11-25T22:00:00Z"
                        },
                        {
                            "bucketId": "weekly-bucket",
                            "label": "Weekly quota",
                            "resetTime": "2026-12-01T00:00:00Z"
                        }
                    ]
                },
                {
                    "groupId": "gemini-argon",
                    "displayName": "Gemini 4 Argon",
                    "models": ["gemini-4-argon-medium"],
                    "buckets": [
                        {
                            "bucketId": "5h-argon",
                            "label": "5h rolling",
                            "remainingFraction": 0.45,
                            "resetTime": "2026-11-25T21:30:00Z"
                        }
                    ]
                }
            ]
        })
        .to_string(),
        NOW_MS,
    )
    .expect("parse ok")
    .expect("gemini usage");

    assert_eq!(parsed.provider_id, "antigravity");
    assert_eq!(parsed.groups.len(), 3);
    // Group 1 5h: 1 - 0.72 = 28%
    assert_eq!(parsed.groups[0].window.percent, 28);
    assert_eq!(parsed.groups[0].window.kind, UsageWindowKind::FiveHour);
    // Group 1 weekly with omitted remainingFraction + valid resetTime => 100% used, rejected = true
    assert_eq!(parsed.groups[1].window.percent, 100);
    assert!(parsed.groups[1].window.rejected);
    assert_eq!(parsed.groups[1].window.kind, UsageWindowKind::Weekly);
    // Group 2 5h: 1 - 0.45 = 55% => becomes top-level primary (most restrictive 5h)
    assert_eq!(parsed.primary.as_ref().unwrap().percent, 55);
    assert_eq!(parsed.secondary.as_ref().unwrap().percent, 100);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let dir = temp_test_dir("gemini");
        let token_path = dir.join("antigravity-oauth-token");
        let secret_access_token = "ya29.a0AfH6SMC_antigravity_secret_token";

        // 1. Token expiring within <= 60s must be rejected without leaking token.
        std::fs::write(
            &token_path,
            json!({
                "access_token": secret_access_token,
                "expiry_date": NOW_MS + 60_000
            })
            .to_string(),
        )
        .unwrap();
        let expired_err = read_valid_oauth_token(&[token_path.clone()], NOW_MS)
            .expect_err("token at exact +60s boundary must fail");
        assert!(!expired_err.message.contains(secret_access_token));

        // 2. Valid token (> now + 60s) -> executes loadCodeAssist + retrieveUserQuotaSummary.
        std::fs::write(
            &token_path,
            json!({
                "access_token": secret_access_token,
                "expiry_date": NOW_MS + 300_000
            })
            .to_string(),
        )
        .unwrap();

        let router = Router::new()
            .route(
                "/v1internal:loadCodeAssist",
                post(|headers: HeaderMap, AxumJson(body): AxumJson<Json>| async move {
                    assert_eq!(
                        headers.get("authorization").and_then(|v| v.to_str().ok()),
                        Some("Bearer ya29.a0AfH6SMC_antigravity_secret_token")
                    );
                    assert_eq!(
                        headers.get("user-agent").and_then(|v| v.to_str().ok()),
                        Some("antigravity/1.104.0")
                    );
                    assert_eq!(body["metadata"]["pluginType"], "GEMINI");
                    (
                        StatusCode::OK,
                        AxumJson(json!({ "cloudaicompanionProject": "proj-vokia-123" })),
                    )
                }),
            )
            .route(
                "/v1internal:retrieveUserQuotaSummary",
                post(|headers: HeaderMap, AxumJson(body): AxumJson<Json>| async move {
                    assert_eq!(
                        headers.get("authorization").and_then(|v| v.to_str().ok()),
                        Some("Bearer ya29.a0AfH6SMC_antigravity_secret_token")
                    );
                    assert_eq!(body["project"], "proj-vokia-123");
                    (
                        StatusCode::OK,
                        AxumJson(json!({
                            "groups": [{
                                "groupId": "gemini-models",
                                "displayName": "Gemini Models",
                                "models": ["gemini-4-hydrogen-medium"],
                                "buckets": [{
                                    "bucketId": "five_hour",
                                    "remainingFraction": 0.80,
                                    "resetTime": "2026-11-25T23:00:00Z"
                                }]
                            }]
                        })),
                    )
                }),
            );

        let (base_url, server) = spawn_test_server(router).await;
        let mut cfg = GeminiReaderConfig {
            load_code_assist_url: Some(format!("{base_url}/v1internal:loadCodeAssist")),
            retrieve_quota_url: Some(format!("{base_url}/v1internal:retrieveUserQuotaSummary")),
            token_paths: vec![token_path.clone()],
            env_gemini_api_key: Some(String::new()),
            user_agent: "antigravity/1.104.0".to_string(),
            timeout: Duration::from_secs(5),
        };

        let usage = read_gemini_usage(&cfg, NOW_MS)
            .await
            .expect("gemini ok")
            .expect("gemini usage");
        assert_eq!(usage.provider_id, "antigravity");
        assert_eq!(usage.primary.as_ref().unwrap().percent, 20);
        assert_eq!(usage.groups.len(), 1);

        // 3. Direct GEMINI_API_KEY active -> skips OAuth quota silently returning Ok(None).
        cfg.env_gemini_api_key = Some("AIzaSyDirectPayAsYouGoKey123456".to_string());
        assert_eq!(read_gemini_usage(&cfg, NOW_MS).await.unwrap(), None);

        server.abort();
        let _ = std::fs::remove_dir_all(&dir);
    });
}

#[test]
fn engine_com_read_optional_nao_entra_em_backoff_no_none_e_preserva_ultimo_valido_em_erro() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let engine = UsageEngine::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);

        engine.register_optional_reader(
            "opencode",
            Arc::new(move || {
                let c = Arc::clone(&counter);
                Box::pin(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok(None)
                })
            }),
        );

        let r1 = engine.refresh_provider("opencode", NOW_MS).await;
        assert!(r1.usage.is_none());
        assert!(!r1.in_backoff);
        assert!(r1.error.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        // Cached Ok(None) within 4 min does not re-hit reader.
        let r2 = engine.refresh_provider("opencode", NOW_MS + 60_000).await;
        assert!(r2.from_cache);
        assert!(!r2.in_backoff);
        assert!(r2.error.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        // On error, UsageReadGate preserves last valid snapshot and activates backoff.
        engine.record_provider_usage(ProviderPlanUsage {
            provider_id: "antigravity".to_string(),
            label: Some("Gemini".to_string()),
            primary: Some(UsageWindow::new(33, (NOW_MS / 1_000) + 3_600, FIVE_HOUR_MINS)),
            secondary: None,
            tertiary: None,
            groups: Vec::new(),
            credits_available: None,
            reset_credits_available: None,
            updated_at_ms: NOW_MS,
            read_failed: false,
        });
        engine.register_optional_reader(
            "antigravity",
            Arc::new(|| {
                Box::pin(async {
                    Err(crate::usage::UsageReadError::redacted(
                        "Token ya29.secret_should_not_leak expirado",
                        &["ya29.secret_should_not_leak"],
                    ))
                })
            }),
        );

        let failed = engine
            .refresh_provider("antigravity", NOW_MS + 5 * 60 * 1_000)
            .await;
        assert!(failed.in_backoff);
        assert!(!failed.error.as_deref().unwrap().contains("ya29."));
        let preserved = failed.usage.expect("last valid snapshot preserved");
        assert_eq!(preserved.primary.as_ref().unwrap().percent, 33);
        assert!(preserved.read_failed);
    });
}
