use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU32;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::agents::Agent;
use crate::claude::{Access, ProjectAccess};
use crate::mcp::{
    Binding, ExchangeSlot, McpRelayManager, McpServer, StdioMcpServerConfig, ToolScope,
    VokiaMcpBridge,
};
use crate::orchestrator::adapters::acp::flavor::{
    resolve_flavor, AcpFlavor, AcpSteeringMode, DiscoveredModel, GeminiFlavor, OpenCodeFlavor,
    OPENCODE_DISABLED_TOOLS,
};
use crate::orchestrator::adapters::acp::{
    AcpRuntimePool, AcpSessionStatus, WorkHoldState, WORK_HOLD_IDLE_TIMEOUT,
    WORK_HOLD_MAX_DURATION,
};
use crate::orchestrator::adapters::ProviderAdapter;
use crate::orchestrator::turn_plan::TurnPlanStepStatus;
use crate::permissions::PermissionLevel;
use crate::providers::{ProviderProtocol, ProviderRegistry};
use crate::session_key::SessionKey;

fn temp_test_dir(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vokia-acp-test-{prefix}-{}-{}",
        std::process::id(),
        crate::mcp::random_hex(4).unwrap_or_else(|_| "0".to_string())
    ));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[test]
fn relay_http_loopback_autentica_ponte_vokia_e_encerra_ao_fechar_ultima_sessao() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    tauri::async_runtime::block_on(async {
        let dir = temp_test_dir("relay");
        let vokia_server = McpServer::start().await.expect("McpServer::start");
        let binding = Binding {
            project_id: 1,
            agent: Agent::vexa(),
            agent_name: "Dev".to_string(),
            slot: ExchangeSlot::default(),
            scope: ToolScope::Coordinator,
        };
        let vokia_session = vokia_server.open_headless(binding).expect("open_headless");

        // Servidor MCP stdio mock em Node.js respondendo JSON-RPC `tools/list`.
        let mock_stdio_script = r#"
const rl = require('readline').createInterface({ input: process.stdin });
rl.on('line', (line) => {
  if (!line.trim()) return;
  const req = JSON.parse(line);
  if (req.id !== undefined) {
    process.stdout.write(JSON.stringify({
      jsonrpc: '2.0',
      id: req.id,
      result: { tools: [{ name: 'mock_echo', description: 'echo' }] }
    }) + '\n');
  }
});
"#;
        let stdio_cfg = StdioMcpServerConfig {
            name: "mock_stdio".to_string(),
            command: "node".to_string(),
            args: vec!["-e".to_string(), mock_stdio_script.to_string()],
            env: BTreeMap::new(),
            cwd: Some(dir.clone()),
        };

        let manager = McpRelayManager::new();
        let bridge = VokiaMcpBridge {
            url: vokia_session.url().to_string(),
            token: vokia_session.token().to_string(),
        };

        let guard1 = manager
            .acquire_for_directory(&dir, Some(bridge.clone()), vec![stdio_cfg.clone()])
            .await
            .expect("acquire guard1");
        let guard2 = manager
            .acquire_for_directory(&dir, Some(bridge), vec![stdio_cfg])
            .await
            .expect("acquire guard2");

        assert_eq!(manager.session_count_for(&dir), 2);
        assert_eq!(guard1.port(), guard2.port());
        assert_eq!(guard1.token(), guard2.token());

        let client = reqwest::Client::new();
        // Sem token -> 401
        let unauth = client
            .post(guard1.vokia_url())
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .send()
            .await
            .expect("unauth request");
        assert_eq!(unauth.status(), reqwest::StatusCode::UNAUTHORIZED);

        // Com header Origin -> 403
        let origin_req = client
            .post(guard1.vokia_url())
            .header("Origin", "https://evil.example")
            .header("Authorization", format!("Bearer {}", guard1.token()))
            .json(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
            .send()
            .await
            .expect("origin request");
        assert_eq!(origin_req.status(), reqwest::StatusCode::FORBIDDEN);

        // Ponte do Vokia -> 200
        let vokia_resp = client
            .post(guard1.vokia_url())
            .header("Authorization", format!("Bearer {}", guard1.token()))
            .json(&json!({"jsonrpc":"2.0","id":3,"method":"tools/list"}))
            .send()
            .await
            .expect("vokia bridge request");
        assert_eq!(vokia_resp.status(), reqwest::StatusCode::OK);

        // Servidor MCP stdio via relay -> 200 com `mock_echo`
        let stdio_resp = client
            .post(guard1.url_for("mock_stdio"))
            .header("Authorization", format!("Bearer {}", guard1.token()))
            .json(&json!({"jsonrpc":"2.0","id":4,"method":"tools/list"}))
            .send()
            .await
            .expect("stdio relay request");
        assert_eq!(stdio_resp.status(), reqwest::StatusCode::OK);
        let body: Value = stdio_resp.json().await.expect("stdio json");
        assert_eq!(body["result"]["tools"][0]["name"], "mock_echo");

        let vokia_url = guard1.vokia_url();
        drop(guard1);
        assert_eq!(manager.session_count_for(&dir), 1);

        drop(guard2);
        assert_eq!(manager.session_count_for(&dir), 0);
        assert_eq!(manager.active_directories(), 0);

        // Dá um instante para o axum concluir o graceful shutdown.
        tokio::time::sleep(Duration::from_millis(80)).await;
        let after_close = client
            .post(&vokia_url)
            .json(&json!({"jsonrpc":"2.0","id":5,"method":"tools/list"}))
            .send()
            .await;
        assert!(after_close.is_err(), "relay deveria estar encerrado após fechar última sessão");

        let _ = std::fs::remove_dir_all(&dir);
    });
}

#[test]
fn opencode_flavor_desativa_ferramentas_via_opencode_config_temporario_sem_tocar_projeto() {
    let project_dir = temp_test_dir("opencode-cfg");
    let attachments_dir = project_dir.join(".attachments");
    let temp_dir = temp_test_dir("opencode-tmp");
    let user_opencode_json = project_dir.join("opencode.json");
    std::fs::write(&user_opencode_json, r#"{"userSetting":true}"#).unwrap();

    let flavor = OpenCodeFlavor;
    assert_eq!(flavor.steering_mode(), AcpSteeringMode::CustomMethod("_session/steering"));
    assert_eq!(flavor.relist_delay(), Some(Duration::from_secs(5)));

    let relay = crate::mcp::RelayEndpoint {
        vokia_url: "http://127.0.0.1:43210/mcp/vokia".to_string(),
        token: "secret-tok".to_string(),
        server_urls: BTreeMap::from([(
            "vokia".to_string(),
            "http://127.0.0.1:43210/mcp/vokia".to_string(),
        )]),
    };

    let spec = flavor
        .build_spawn_spec(
            Path::new("opencode"),
            &[],
            Access::Project(ProjectAccess {
                dir: &project_dir,
                attachments: &attachments_dir,
                extra_dirs: &[],
                level: PermissionLevel::Liberado,
                builtin_tools: true,
            }),
            Some(&relay),
            &temp_dir,
        )
        .expect("build_spawn_spec");

    assert_eq!(spec.temp_files.len(), 1);
    let cfg_path = &spec.temp_files[0];
    let cfg_text = std::fs::read_to_string(cfg_path).expect("read temp OPENCODE_CONFIG");
    let cfg_json: Value = serde_json::from_str(&cfg_text).expect("parse temp OPENCODE_CONFIG");
    for tool in OPENCODE_DISABLED_TOOLS {
        assert_eq!(cfg_json["tools"][tool], json!(false));
    }
    assert_eq!(
        cfg_json["mcp"]["vokia"]["url"],
        json!("http://127.0.0.1:43210/mcp/vokia")
    );
    assert_eq!(
        std::fs::read_to_string(&user_opencode_json).unwrap(),
        r#"{"userSetting":true}"#
    );

    let _ = std::fs::remove_dir_all(&project_dir);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn opencode_acp_descobre_modelos_com_gratis_no_topo_e_executa_em_todos_niveis_permissao() {
    tauri::async_runtime::block_on(async {
        let project_dir = temp_test_dir("opencode-acp");
        let attachments_dir = project_dir.join(".attachments");
        let mock_acp = project_dir.join("mock-opencode.cjs");
        std::fs::write(
            &mock_acp,
            r#"
const rl = require('readline').createInterface({ input: process.stdin });
let permOutcome = null;
rl.on('line', (line) => {
  if (!line.trim()) return;
  const msg = JSON.parse(line);
  if (msg.method === 'initialize') {
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: msg.id, result: { protocolVersion: 1 } }) + '\n');
  } else if (msg.method === 'model/list') {
    process.stdout.write(JSON.stringify({
      jsonrpc: '2.0',
      id: msg.id,
      result: {
        models: [
          { id: 'openai/gpt-6-astra', isFree: false },
          { id: 'opencode/big-pickle', isFree: true },
          { id: 'google/gemini-free:free', isFree: true }
        ]
      }
    }) + '\n');
  } else if (msg.method === 'session/new') {
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: msg.id, result: { sessionId: 'sess-oc-1' } }) + '\n');
  } else if (msg.method === 'session/set_model') {
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: msg.id, result: {} }) + '\n');
  } else if (msg.method === 'session/prompt') {
    // Emite pedido de permissão ao cliente ACP e aguarda resposta.
    process.stdout.write(JSON.stringify({
      jsonrpc: '2.0',
      id: 9001,
      method: 'session/request_permission',
      params: {
        toolCall: { toolCallId: 'tc-1', title: 'Bash', kind: 'execute', rawInput: { command: 'echo ok' } },
        options: [
          { optionId: 'allow-once', kind: 'allow_once' },
          { optionId: 'reject-once', kind: 'reject_once' }
        ]
      }
    }) + '\n');
    const waitPerm = setInterval(() => {
      if (permOutcome) {
        clearInterval(waitPerm);
        process.stdout.write(JSON.stringify({
          jsonrpc: '2.0',
          method: 'session/update',
          params: {
            update: {
              sessionUpdate: 'agent_message_chunk',
              content: { type: 'text', text: 'perm:' + permOutcome }
            }
          }
        }) + '\n');
        process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: msg.id, result: { stopReason: 'end_turn' } }) + '\n');
        permOutcome = null;
      }
    }, 10);
  } else if (msg.id === 9001 && msg.result) {
    permOutcome = msg.result.outcome.optionId || msg.result.outcome.outcome;
  }
});
"#,
        )
        .unwrap();

        let providers = ProviderRegistry::seeded();
        let mut opencode_cfg = providers.get("opencode").unwrap();
        opencode_cfg.protocol = ProviderProtocol::Acp;

        let pool = AcpRuntimePool::default();
        let args = vec![mock_acp.to_string_lossy().to_string()];

        // 1. Descoberta de modelos coloca gratuitos no topo sem conta
        let models = pool
            .discover_models(Path::new("node"), &args, Some(&opencode_cfg))
            .await
            .expect("discover_models");
        assert_eq!(models[0], "opencode/big-pickle");
        assert_eq!(models[1], "google/gemini-free:free");
        assert_eq!(models[2], "openai/gpt-6-astra");

        // 2. Execução em Leitura, Edição e Liberado
        for (level, expected_opt) in [
            (PermissionLevel::Leitura, "reject-once"),
            (PermissionLevel::Edicao, "allow-once"),
            (PermissionLevel::Liberado, "allow-once"),
        ] {
            let pid = AtomicU32::new(0);
            let reply = pool
                .run_ephemeral_turn(
                    Path::new("node"),
                    &args,
                    "Dev",
                    "opencode/big-pickle",
                    Access::Project(ProjectAccess {
                        dir: &project_dir,
                        attachments: &attachments_dir,
                        extra_dirs: &[],
                        level,
                        builtin_tools: true,
                    }),
                    "teste",
                    Some(&opencode_cfg),
                    None,
                    Vec::new(),
                    None,
                    &pid,
                    None,
                )
                .await
                .expect("run_ephemeral_turn");
            assert_eq!(reply.text, format!("perm:{expected_opt}"));
        }

        let _ = std::fs::remove_dir_all(&project_dir);
    });
}

#[test]
fn gemini_flavor_rotulo_gemini_args_e_preset_gemini_cli_e_retrocompatibilidade() {
    let providers = ProviderRegistry::seeded();
    let agy = providers.get("antigravity").expect("antigravity seed");
    assert_eq!(agy.label, "Gemini");
    assert_eq!(agy.protocol, ProviderProtocol::Acp);

    // Alias retrocompatível "gemini" -> "antigravity"
    let by_alias = providers.get("gemini").expect("gemini alias");
    assert_eq!(by_alias.id, "antigravity");

    // Preset opcional @google/gemini-cli
    let gemini_cli = providers.get("gemini-cli").expect("gemini-cli seed");
    assert_eq!(gemini_cli.default_command, "gemini");
    assert_eq!(gemini_cli.protocol, ProviderProtocol::Acp);

    // Fallback legado continua disponível
    let mut legacy_oc = providers.get("opencode").unwrap();
    legacy_oc.protocol = ProviderProtocol::OpenCodeJson;
    assert!(matches!(ProviderAdapter::from_config(legacy_oc), ProviderAdapter::OpenCode(_)));

    let mut legacy_agy = agy.clone();
    legacy_agy.protocol = ProviderProtocol::AntigravityStream;
    assert!(matches!(ProviderAdapter::from_config(legacy_agy), ProviderAdapter::Antigravity(_)));

    // Verifica --gemini-dir absoluto e --app-data-dir relativo no agy
    let project_dir = temp_test_dir("gemini-args");
    let attachments_dir = project_dir.join(".attachments");
    let flavor = resolve_flavor(Some(&agy), Path::new("agy"));
    assert_eq!(flavor.flavor_id(), "gemini");
    let spec = flavor
        .build_spawn_spec(
            Path::new("agy"),
            &[],
            Access::Project(ProjectAccess {
                dir: &project_dir,
                attachments: &attachments_dir,
                extra_dirs: &[],
                level: PermissionLevel::Edicao,
                builtin_tools: true,
            }),
            None,
            &project_dir,
        )
        .expect("build_spawn_spec");

    let std_args: Vec<String> = spec
        .command
        .as_std()
        .get_args()
        .map(|s| s.to_string_lossy().to_string())
        .collect();
    let gemini_dir_idx = std_args.iter().position(|a| a == "--gemini-dir").unwrap();
    assert!(Path::new(&std_args[gemini_dir_idx + 1]).is_absolute());
    let app_data_idx = std_args.iter().position(|a| a == "--app-data-dir").unwrap();
    assert!(!Path::new(&std_args[app_data_idx + 1]).is_absolute());

    // Preserva ordem de modelos da CLI
    let ordered = GeminiFlavor { is_gemini_cli: false }.sort_models(vec![
        DiscoveredModel { id: "gemini-pro".into(), is_free: false },
        DiscoveredModel { id: "gemini-free".into(), is_free: true },
    ]);
    assert_eq!(ordered, vec!["gemini-pro", "gemini-free"]);

    let _ = std::fs::remove_dir_all(&project_dir);
}

#[test]
fn acp_recupera_sessao_perdida_32602_e_32002_injetando_handoff() {
    tauri::async_runtime::block_on(async {
        let dir = temp_test_dir("acp-recover");
        let mock_acp = dir.join("mock-recover.cjs");
        std::fs::write(
            &mock_acp,
            r#"
const rl = require('readline').createInterface({ input: process.stdin });
rl.on('line', (line) => {
  if (!line.trim()) return;
  const msg = JSON.parse(line);
  if (msg.method === 'initialize') {
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: msg.id, result: { protocolVersion: 1 } }) + '\n');
  } else if (msg.method === 'session/load') {
    process.stdout.write(JSON.stringify({
      jsonrpc: '2.0',
      id: msg.id,
      error: { code: -32602, message: 'session not found' }
    }) + '\n');
  } else if (msg.method === 'session/new') {
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: msg.id, result: { sessionId: 'sess-recreated' } }) + '\n');
  } else if (msg.method === 'session/set_model') {
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: msg.id, result: {} }) + '\n');
  } else if (msg.method === 'session/prompt') {
    const text = msg.params.prompt[0].text;
    process.stdout.write(JSON.stringify({
      jsonrpc: '2.0',
      method: 'session/update',
      params: { update: { sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: text } } }
    }) + '\n');
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: msg.id, result: { stopReason: 'end_turn' } }) + '\n');
  }
});
"#,
        )
        .unwrap();

        let providers = ProviderRegistry::seeded();
        let cfg = providers.get("opencode").unwrap();
        let pool = AcpRuntimePool::default();
        let pid = AtomicU32::new(0);
        let key = SessionKey {
            project_id: 99,
            agent: Agent::vexa(),
        };

        let reply = pool
            .run_persistent_turn(
                key,
                None,
                Path::new("node"),
                &[mock_acp.to_string_lossy().to_string()],
                "Dev",
                "opencode/big-pickle",
                Access::TextOnly,
                "Continue a tarefa",
                "Próximo passo",
                Some("sess-perdida-123"),
                Some(&cfg),
                None,
                Vec::new(),
                None,
                &pid,
                None,
            )
            .await
            .expect("recovered turn");

        assert_eq!(reply.session_id.as_deref(), Some("sess-recreated"));
        assert!(
            reply.text.contains("sess-perdida-123") && reply.text.contains("Continue a tarefa"),
            "handoff deveria conter contexto da sessão perdida: {}",
            reply.text
        );

        pool.archive_all().await;
        let _ = std::fs::remove_dir_all(&dir);
    });
}

#[test]
fn acp_emite_plan_turn_plan_e_step_labels_e_work_hold_e_status_restart() {
    tauri::async_runtime::block_on(async {
        let dir = temp_test_dir("acp-plan");
        let mock_acp = dir.join("mock-plan.cjs");
        std::fs::write(
            &mock_acp,
            r#"
const rl = require('readline').createInterface({ input: process.stdin });
let count = 0;
rl.on('line', (line) => {
  if (!line.trim()) return;
  const msg = JSON.parse(line);
  if (msg.method === 'initialize') {
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: msg.id, result: { protocolVersion: 1 } }) + '\n');
  } else if (msg.method === 'session/new') {
    count += 1;
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: msg.id, result: { sessionId: 'sess-plan-' + count } }) + '\n');
  } else if (msg.method === 'session/set_model') {
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: msg.id, result: {} }) + '\n');
  } else if (msg.method === 'session/prompt') {
    process.stdout.write(JSON.stringify({
      jsonrpc: '2.0',
      method: 'session/update',
      params: {
        update: {
          sessionUpdate: 'plan',
          entries: [
            { id: '1', content: 'Ler configuração', status: 'completed' },
            { id: '2', content: 'Aplicar correção', status: 'in_progress' }
          ]
        }
      }
    }) + '\n');
    process.stdout.write(JSON.stringify({
      jsonrpc: '2.0',
      method: 'session/update',
      params: { update: { sessionUpdate: 'agent_thought_chunk', content: { type: 'text', text: 'Analisando...' } } }
    }) + '\n');
    process.stdout.write(JSON.stringify({
      jsonrpc: '2.0',
      method: 'session/update',
      params: {
        update: {
          sessionUpdate: 'tool_call',
          toolCallId: 'c1',
          title: 'Read',
          kind: 'read',
          status: 'completed',
          rawInput: { file_path: 'src/main.rs' }
        }
      }
    }) + '\n');
    process.stdout.write(JSON.stringify({
      jsonrpc: '2.0',
      method: 'session/update',
      params: { update: { sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: 'Feito!' } } }
    }) + '\n');
    process.stdout.write(JSON.stringify({
      jsonrpc: '2.0',
      id: msg.id,
      result: { stopReason: 'end_turn', wakesAfterPromptEnds: true }
    }) + '\n');
  }
});
"#,
        )
        .unwrap();

        let providers = ProviderRegistry::seeded();
        let cfg = providers.get("antigravity").unwrap();
        let pool = AcpRuntimePool::default();
        let pid = AtomicU32::new(0);
        let key = SessionKey {
            project_id: 77,
            agent: Agent::vexa(),
        };
        assert_eq!(pool.session_status(&key), AcpSessionStatus::Stopped);

        let reply = pool
            .run_persistent_turn(
                key.clone(),
                None,
                Path::new("node"),
                &[mock_acp.to_string_lossy().to_string()],
                "Dev",
                "gemini-4-hydrogen-medium",
                Access::TextOnly,
                "Execute o plano",
                "Continue",
                None,
                Some(&cfg),
                None,
                Vec::new(),
                None,
                &pid,
                None,
            )
            .await
            .expect("turn with plan");

        assert_eq!(reply.text, "Feito!");
        assert_eq!(reply.activity.thinking_text.as_deref(), Some("Analisando..."));
        assert_eq!(reply.activity.tools.len(), 1);
        let plan = reply.activity.plan.expect("TurnPlan populated");
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].status, TurnPlanStepStatus::Completed);
        assert_eq!(plan.steps[1].status, TurnPlanStepStatus::InProgress);

        // `wakesAfterPromptEnds: true` mantém a sessão em `WorkHold`
        assert_eq!(pool.session_status(&key), AcpSessionStatus::WorkHold);

        // WorkHold não expira em 10 min, mas expira após 30 min sem atividade ou 4 h máximo
        let now = Instant::now();
        let hold = WorkHoldState::new(now);
        assert!(!hold.is_expired(now + Duration::from_secs(10 * 60)));
        assert!(hold.is_expired(now + WORK_HOLD_IDLE_TIMEOUT));
        assert!(hold.is_expired(now + WORK_HOLD_MAX_DURATION));

        // Reap dentro da janela mantém WorkHold; restart_session encerra e volta a Stopped
        let reaped = pool.reap_idle(Instant::now() + Duration::from_secs(600), |_| false).await;
        assert!(reaped.is_empty());
        assert!(pool.restart_session(&key).await);
        assert_eq!(pool.session_status(&key), AcpSessionStatus::Stopped);

        let _ = std::fs::remove_dir_all(&dir);
    });
}
