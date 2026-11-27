use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value as Json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, Mutex as AsyncMutex};

use crate::agents::{replica_id, Agent};
use crate::claude::{Access, ClaudeError, ConsoleLine, ProjectAccess, ToolCategory, ToolStatus};
use crate::orchestrator::adapters::ProviderAdapter;
use crate::orchestrator::session_pool::NO_PID;
use crate::orchestrator::turn_plan::{TurnPlan, TurnPlanStepStatus};
use crate::orchestrator::work_steps::WorkStepCollector;
use crate::permissions::PermissionLevel;
use crate::providers::{ProviderProtocol, ProviderRegistry};
use crate::session_key::SessionKey;
use crate::usage::model::{ProviderPlanUsage, UsageWindowKind};

use super::*;

fn args_of(command: &tokio::process::Command) -> Vec<String> {
    command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

fn project_access<'a>(dir: &'a Path, attachments: &'a Path, level: PermissionLevel) -> Access<'a> {
    Access::Project(ProjectAccess {
        dir,
        attachments,
        extra_dirs: &[],
        level,
        builtin_tools: true,
    })
}

/// Servidor JSON-RPC fake in-memory (sobre `tokio::io::duplex`) que simula o `codex app-server`
/// sem depender do binário real instalado na máquina.
struct FakeAppServer {
    client: Arc<CodexRpcClient>,
    received_methods: Arc<StdMutex<Vec<(String, Json)>>>,
    steer_messages: Arc<StdMutex<Vec<String>>>,
    control_tx: mpsc::UnboundedSender<Json>,
}

impl FakeAppServer {
    fn spawn(hold_turn_until_steer: bool) -> Self {
        let (client_to_server_w, client_to_server_r) = tokio::io::duplex(64 * 1024);
        let (server_to_client_w, server_to_client_r) = tokio::io::duplex(64 * 1024);

        let received_methods = Arc::new(StdMutex::new(Vec::<(String, Json)>::new()));
        let steer_messages = Arc::new(StdMutex::new(Vec::<String>::new()));
        let (control_tx, mut control_rx) = mpsc::unbounded_channel::<Json>();

        let received_clone = Arc::clone(&received_methods);
        let steer_clone = Arc::clone(&steer_messages);
        let writer = Arc::new(AsyncMutex::new(server_to_client_w));
        let writer_ctrl = Arc::clone(&writer);

        // Permite aos testes injetar notificações arbitrárias (ex.: rate limits) no servidor fake.
        tokio::spawn(async move {
            while let Some(msg) = control_rx.recv().await {
                let mut line = serde_json::to_string(&msg).unwrap();
                line.push('\n');
                let mut w = writer_ctrl.lock().await;
                let _ = w.write_all(line.as_bytes()).await;
                let _ = w.flush().await;
            }
        });

        let writer_rpc = Arc::clone(&writer);
        tokio::spawn(async move {
            let mut reader = BufReader::new(client_to_server_r).lines();
            let mut thread_seq: u64 = 0;
            let mut turn_seq: u64 = 0;
            let mut held_turn: Option<(String, String)> = None;

            while let Ok(Some(line)) = reader.next_line().await {
                let Ok(req) = serde_json::from_str::<Json>(line.trim()) else {
                    continue;
                };
                let id = req.get("id").cloned();
                let method = req
                    .get("method")
                    .and_then(Json::as_str)
                    .unwrap_or("")
                    .to_string();
                let params = req.get("params").cloned().unwrap_or(Json::Null);

                if !method.is_empty() {
                    received_clone
                        .lock()
                        .unwrap()
                        .push((method.clone(), params.clone()));
                }

                let send = |msg: Json| {
                    let w = Arc::clone(&writer_rpc);
                    async move {
                        let mut payload = serde_json::to_string(&msg).unwrap();
                        payload.push('\n');
                        let mut guard = w.lock().await;
                        let _ = guard.write_all(payload.as_bytes()).await;
                        let _ = guard.flush().await;
                    }
                };

                match method.as_str() {
                    "initialize" => {
                        if let Some(id) = id {
                            send(json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": { "serverInfo": { "name": "fake-codex", "version": "1.0.0" } }
                            }))
                            .await;
                        }
                    }
                    "initialized" => {}
                    "model/list" => {
                        if let Some(id) = id {
                            send(json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "data": [
                                        { "id": "gpt-6-luna", "hidden": false },
                                        { "id": "gpt-5.6-terra", "hidden": false },
                                        { "id": "internal-hidden", "hidden": true }
                                    ]
                                }
                            }))
                            .await;
                        }
                    }
                    "account/rateLimits/read" => {
                        if let Some(id) = id {
                            send(json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "rateLimits": {
                                        "limitId": "codex",
                                        "primary": {
                                            "usedPercent": 35.0,
                                            "windowDurationMins": 300,
                                            "resetsAt": 1764018000
                                        },
                                        "secondary": {
                                            "usedPercent": 22.0,
                                            "windowDurationMins": 10080,
                                            "resetsAt": 1764604800
                                        },
                                        "credits": {
                                            "hasCredits": true,
                                            "unlimited": false
                                        }
                                    },
                                    "rateLimitResetCredits": {
                                        "availableCount": 2
                                    }
                                }
                            }))
                            .await;
                        }
                    }
                    "thread/start" => {
                        thread_seq += 1;
                        let tid = format!("thr_{thread_seq}");
                        if let Some(id) = id {
                            send(json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": { "thread": { "id": tid } }
                            }))
                            .await;
                        }
                    }
                    "thread/resume" => {
                        let tid = params
                            .get("threadId")
                            .and_then(Json::as_str)
                            .unwrap_or("thr_resumed")
                            .to_string();
                        if let Some(id) = id {
                            send(json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": { "thread": { "id": tid } }
                            }))
                            .await;
                        }
                    }
                    "thread/unsubscribe" | "thread/archive" => {
                        if let Some(id) = id {
                            send(json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {}
                            }))
                            .await;
                        }
                    }
                    "turn/start" => {
                        turn_seq += 1;
                        let turn_id = format!("turn_{turn_seq}");
                        let tid = params
                            .get("threadId")
                            .and_then(Json::as_str)
                            .unwrap_or("")
                            .to_string();
                        let input_text = params
                            .pointer("/input/0/text")
                            .and_then(Json::as_str)
                            .unwrap_or("")
                            .to_string();
                        if let Some(id) = id {
                            send(json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": { "turn": { "id": turn_id, "status": "inProgress" } }
                            }))
                            .await;
                        }
                        if hold_turn_until_steer {
                            held_turn = Some((tid.clone(), turn_id.clone()));
                            send(json!({
                                "jsonrpc": "2.0",
                                "method": "turn/plan/updated",
                                "params": {
                                    "threadId": tid,
                                    "turnId": turn_id,
                                    "explanation": "Aguardando direção",
                                    "plan": [
                                        { "step": "Analisar código", "status": "in_progress" }
                                    ]
                                }
                            }))
                            .await;
                        } else {
                            // Simula fluxo estruturado completo do turno
                            send(json!({
                                "jsonrpc": "2.0",
                                "method": "turn/plan/updated",
                                "params": {
                                    "threadId": tid,
                                    "turnId": turn_id,
                                    "explanation": "Plano do turno",
                                    "plan": [
                                        { "step": "Ler arquivos", "status": "completed" },
                                        { "step": "Aplicar ajuste", "status": "in_progress" }
                                    ]
                                }
                            }))
                            .await;
                            send(json!({
                                "jsonrpc": "2.0",
                                "method": "account/rateLimits/updated",
                                "params": {
                                    "rateLimits": {
                                        "limitId": "codex",
                                        "primary": { "usedPercent": 42.0, "windowDurationMins": 300, "resetsAt": 1764018000 },
                                        "credits": { "hasCredits": true }
                                    },
                                    "rateLimitResetCredits": { "availableCount": 3 }
                                }
                            }))
                            .await;
                            send(json!({
                                "jsonrpc": "2.0",
                                "method": "item/completed",
                                "params": {
                                    "threadId": tid,
                                    "turnId": turn_id,
                                    "item": {
                                        "id": format!("cmd_{turn_seq}"),
                                        "type": "commandExecution",
                                        "command": "cargo test --quiet",
                                        "commandActions": [
                                            { "type": "read", "path": "src/providers.rs" }
                                        ],
                                        "exitCode": 0,
                                        "status": "completed",
                                        "aggregatedOutput": "test result: ok. 12 passed"
                                    }
                                }
                            }))
                            .await;
                            send(json!({
                                "jsonrpc": "2.0",
                                "method": "item/agentMessage/delta",
                                "params": {
                                    "threadId": tid,
                                    "turnId": turn_id,
                                    "itemId": format!("msg_{turn_seq}"),
                                    "delta": format!("Turno {turn_seq}: ")
                                }
                            }))
                            .await;
                            send(json!({
                                "jsonrpc": "2.0",
                                "method": "item/completed",
                                "params": {
                                    "threadId": tid,
                                    "turnId": turn_id,
                                    "item": {
                                        "id": format!("msg_{turn_seq}"),
                                        "type": "agentMessage",
                                        "text": format!("Turno {turn_seq}: resposta para {input_text}")
                                    }
                                }
                            }))
                            .await;
                            send(json!({
                                "jsonrpc": "2.0",
                                "method": "thread/tokenUsage/updated",
                                "params": {
                                    "threadId": tid,
                                    "turnId": turn_id,
                                    "tokenUsage": {
                                        "total": {
                                            "totalTokens": turn_seq * 1_500,
                                            "inputTokens": turn_seq * 1_000,
                                            "cachedInputTokens": turn_seq * 300,
                                            "outputTokens": turn_seq * 400,
                                            "reasoningOutputTokens": turn_seq * 100
                                        },
                                        "last": {
                                            "totalTokens": 1_500,
                                            "inputTokens": 1_000,
                                            "cachedInputTokens": 300,
                                            "outputTokens": 400,
                                            "reasoningOutputTokens": 100
                                        },
                                        "modelContextWindow": 200_000
                                    }
                                }
                            }))
                            .await;
                            send(json!({
                                "jsonrpc": "2.0",
                                "method": "turn/completed",
                                "params": {
                                    "threadId": tid,
                                    "turn": { "id": turn_id, "status": "completed" }
                                }
                            }))
                            .await;
                        }
                    }
                    "turn/steer" => {
                        let steer_text = params
                            .pointer("/input/0/text")
                            .and_then(Json::as_str)
                            .unwrap_or("")
                            .to_string();
                        steer_clone.lock().unwrap().push(steer_text.clone());
                        if let Some(id) = id {
                            send(json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": { "turnId": params.get("expectedTurnId").cloned().unwrap_or(json!("turn_1")) }
                            }))
                            .await;
                        }
                        if let Some((tid, turn_id)) = held_turn.take() {
                            send(json!({
                                "jsonrpc": "2.0",
                                "method": "item/completed",
                                "params": {
                                    "threadId": tid,
                                    "turnId": turn_id,
                                    "item": {
                                        "id": "msg_steered",
                                        "type": "agentMessage",
                                        "text": format!("Ajustado durante o turno: {steer_text}")
                                    }
                                }
                            }))
                            .await;
                            send(json!({
                                "jsonrpc": "2.0",
                                "method": "turn/completed",
                                "params": {
                                    "threadId": tid,
                                    "turn": { "id": turn_id, "status": "completed" }
                                }
                            }))
                            .await;
                        }
                    }
                    "turn/interrupt" => {
                        if let Some(id) = id {
                            send(json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {}
                            }))
                            .await;
                        }
                        if let Some((tid, turn_id)) = held_turn.take() {
                            send(json!({
                                "jsonrpc": "2.0",
                                "method": "turn/completed",
                                "params": {
                                    "threadId": tid,
                                    "turn": { "id": turn_id, "status": "interrupted" }
                                }
                            }))
                            .await;
                        }
                    }
                    _ => {}
                }
            }
        });

        let client = CodexRpcClient::from_io(
            BufReader::new(server_to_client_r),
            client_to_server_w,
            None,
            None,
            Arc::new(StdMutex::new(VecDeque::new())),
        );
        Self {
            client,
            received_methods,
            steer_messages,
            control_tx,
        }
    }

    fn method_names(&self) -> Vec<String> {
        self.received_methods
            .lock()
            .unwrap()
            .iter()
            .map(|(m, _)| m.clone())
            .collect()
    }
}

#[test]
fn economia_de_tokens_desativa_tool_suggest_e_recommended_plugins_sem_sobrescrever_usuario() {
    // Padrão: ambos os flags são desativados automaticamente
    let cmd_default = build_app_server_command(Path::new("codex"), &[], None);
    let args_default = args_of(&cmd_default);
    assert!(args_default.contains(&"features.tool_suggest=false".to_string()));
    assert!(args_default.contains(&"features.recommended_plugins=false".to_string()));
    assert_eq!(args_default.last().map(String::as_str), Some("app-server"));

    // Usuário definiu explicitamente features.tool_suggest=true via -c
    let user_args = vec![
        "-c".to_string(),
        "features.tool_suggest=true".to_string(),
        "--config=features.recommended_plugins=true".to_string(),
    ];
    assert!(has_user_config_override(&user_args, "features.tool_suggest"));
    assert!(has_user_config_override(
        &user_args,
        "features.recommended_plugins"
    ));

    let cmd_custom = build_app_server_command(Path::new("codex"), &user_args, None);
    let args_custom = args_of(&cmd_custom);
    assert!(
        !args_custom.contains(&"features.tool_suggest=false".to_string()),
        "não deve sobrescrever flag explícito do usuário: {args_custom:?}"
    );
    assert!(
        !args_custom.contains(&"features.recommended_plugins=false".to_string()),
        "não deve sobrescrever flag explícito do usuário: {args_custom:?}"
    );

    // Override parcial: só o flag não definido recebe =false
    let partial = vec!["--config".to_string(), "features.tool_suggest = true".to_string()];
    let extra = token_saving_config_args(&partial);
    assert_eq!(extra, vec!["-c", "features.recommended_plugins=false"]);
}

#[test]
fn protocolo_dois_turnos_consecutivos_reutilizam_app_server_e_retomam_thread_id() {
    tauri::async_runtime::block_on(async {
        let server = FakeAppServer::spawn(false);
        server
            .client
            .initialize(Duration::from_secs(5))
            .await
            .expect("handshake initialize/initialized");

        let pool = CodexIdleSessions::with_config(CodexIdleConfig {
            thread_idle_timeout: Duration::from_secs(300),
            server_idle_timeout: Duration::from_secs(1800),
            handshake_timeout: Duration::from_secs(5),
            rpc_timeout: Duration::from_secs(5),
        });
        let program = PathBuf::from("fake-codex");
        let server_key = ServerKey::new(&program, &[]);
        pool.register_client_for_test(server_key.clone(), Arc::clone(&server.client), Instant::now())
            .await;

        let key = SessionKey {
            project_id: 7,
            agent: Agent::from_stored("kairo").unwrap(),
        };
        let pid = AtomicU32::new(NO_PID);
        let dir = Path::new("C:/test/repo");
        let att = Path::new("C:/test/att");

        // Turno 1: cria thread via thread/start
        let reply1 = pool
            .run_persistent_turn(
                key.clone(),
                Some("CARD-92".to_string()),
                &program,
                &[],
                "Engenheiro",
                "gpt-6-luna",
                project_access(dir, att, PermissionLevel::Edicao),
                "Primeira mensagem",
                "Seguimento 1",
                None,
                None,
                None,
                None,
                &pid,
                None,
            )
            .await
            .expect("primeiro turno deve concluir");
        assert_eq!(reply1.session_id.as_deref(), Some("thr_1"));
        assert!(reply1.text.contains("Primeira mensagem"));
        let usage1 = reply1.usage.as_ref().expect("usage no turno 1");
        assert_eq!(usage1.input_tokens, Some(1_000));
        assert_eq!(usage1.output_tokens, Some(400));
        assert_eq!(usage1.context_tokens, Some(1_500));
        assert_eq!(usage1.context_window, Some(200_000));

        // Simula descarga por ociosidade de 5 minutos -> envia thread/unsubscribe mantendo thread_id
        let after_5_min = Instant::now() + Duration::from_secs(301);
        let reap = pool.reap_idle(after_5_min, |_| false).await;
        assert_eq!(reap.unsubscribed_threads, vec!["thr_1".to_string()]);
        assert_eq!(reap.closed_servers, 0, "servidor deve ficar vivo por 30 min");
        let snap = pool.thread_snapshot(&key).expect("thread preservada");
        assert_eq!(snap.state, ThreadLifecycleState::Unloaded);
        assert_eq!(snap.thread_id, "thr_1");

        // Turno 2: reutiliza o MESMO app-server e chama thread/resume com thr_1
        let reply2 = pool
            .run_persistent_turn(
                key.clone(),
                Some("CARD-92".to_string()),
                &program,
                &[],
                "Engenheiro",
                "gpt-6-luna",
                project_access(dir, att, PermissionLevel::Edicao),
                "Primeira mensagem",
                "Segunda mensagem",
                Some("thr_1"),
                None,
                None,
                None,
                &pid,
                None,
            )
            .await
            .expect("segundo turno deve retomar thread");
        assert_eq!(reply2.session_id.as_deref(), Some("thr_1"));
        assert!(reply2.text.contains("Segunda mensagem"));
        // Delta de tokens sem dupla contagem (total passou de 1000 -> 2000, delta = 1000)
        let usage2 = reply2.usage.as_ref().expect("usage no turno 2");
        assert_eq!(usage2.input_tokens, Some(1_000));
        assert_eq!(usage2.output_tokens, Some(400));

        let methods = server.method_names();
        assert!(methods.contains(&"initialize".to_string()));
        assert!(methods.contains(&"initialized".to_string()));
        assert_eq!(methods.iter().filter(|m| m.as_str() == "thread/start").count(), 1);
        assert_eq!(
            methods
                .iter()
                .filter(|m| m.as_str() == "thread/unsubscribe")
                .count(),
            1
        );
        assert_eq!(
            methods.iter().filter(|m| m.as_str() == "thread/resume").count(),
            1
        );

        // Após 30 minutos ociosos sem threads protegidas, o servidor fecha
        let after_30_min = Instant::now() + Duration::from_secs(1805);
        let reap_server = pool.reap_idle(after_30_min, |_| false).await;
        assert_eq!(reap_server.closed_servers, 1);
        assert_eq!(
            pool.server_state(&server_key).await,
            ServerLifecycleState::Closed
        );
    });
}

#[test]
fn idle_pool_nunca_descarrega_com_aprovacao_pergunta_terminal_background_ou_guarda_externa() {
    tauri::async_runtime::block_on(async {
        let server = FakeAppServer::spawn(false);
        server.client.initialize(Duration::from_secs(5)).await.unwrap();

        let pool = CodexIdleSessions::with_config(CodexIdleConfig {
            thread_idle_timeout: Duration::from_secs(300),
            server_idle_timeout: Duration::from_secs(1800),
            handshake_timeout: Duration::from_secs(5),
            rpc_timeout: Duration::from_secs(5),
        });
        let program = PathBuf::from("fake-codex");
        let server_key = ServerKey::new(&program, &[]);
        pool.register_client_for_test(server_key.clone(), Arc::clone(&server.client), Instant::now())
            .await;

        let key = SessionKey {
            project_id: 1,
            agent: Agent::from_stored("vexa").unwrap(),
        };
        let pid = AtomicU32::new(NO_PID);
        pool.run_persistent_turn(
            key.clone(),
            None,
            &program,
            &[],
            "",
            "gpt-6-luna",
            Access::TextOnly,
            "Oi",
            "Oi",
            None,
            None,
            None,
            None,
            &pid,
            None,
        )
        .await
        .unwrap();

        let far_future = Instant::now() + Duration::from_secs(2_400);

        // 1. Aprovação pendente -> não desinscreve nem fecha servidor
        pool.set_thread_guards_for_test(&key, 1, 0, 0);
        let r1 = pool.reap_idle(far_future, |_| false).await;
        assert!(r1.unsubscribed_threads.is_empty());
        assert_eq!(r1.closed_servers, 0);

        // 2. Pergunta aberta -> não desinscreve nem fecha servidor
        pool.set_thread_guards_for_test(&key, 0, 1, 0);
        let r2 = pool.reap_idle(far_future, |_| false).await;
        assert!(r2.unsubscribed_threads.is_empty());
        assert_eq!(r2.closed_servers, 0);

        // 3. Terminal em background -> não desinscreve nem fecha servidor
        pool.set_thread_guards_for_test(&key, 0, 0, 1);
        let r3 = pool.reap_idle(far_future, |_| false).await;
        assert!(r3.unsubscribed_threads.is_empty());
        assert_eq!(r3.closed_servers, 0);

        // 4. Guarda externa (ex.: permissão ou decisão pendente no AppState) -> bloqueia
        pool.set_thread_guards_for_test(&key, 0, 0, 0);
        let r4 = pool.reap_idle(far_future, |k| k == &key).await;
        assert!(r4.unsubscribed_threads.is_empty());
        assert_eq!(r4.closed_servers, 0);

        // 5. Sem travas -> descarrega thread e encerra servidor após 30 min
        let r5 = pool.reap_idle(far_future, |_| false).await;
        assert_eq!(r5.unsubscribed_threads, vec!["thr_1".to_string()]);
        assert_eq!(r5.closed_servers, 1);
    });
}

#[test]
fn thread_archive_enviado_ao_remover_agente_encerrar_projeto_ou_limpar_worktree() {
    tauri::async_runtime::block_on(async {
        let server = FakeAppServer::spawn(false);
        server.client.initialize(Duration::from_secs(5)).await.unwrap();

        let pool = CodexIdleSessions::default();
        let program = PathBuf::from("fake-codex");
        pool.register_client_for_test(
            ServerKey::new(&program, &[]),
            Arc::clone(&server.client),
            Instant::now(),
        )
        .await;

        let pid = AtomicU32::new(NO_PID);
        let kairo = Agent::from_stored("kairo").unwrap();
        let kairo_rep = replica_id(&kairo, 2);
        let vexa = Agent::from_stored("vexa").unwrap();
        let att = Path::new("C:/test/att");

        // Thread 1: kairo no worktree CARD-10
        pool.run_persistent_turn(
            SessionKey { project_id: 1, agent: kairo.clone() },
            Some("CARD-10".to_string()),
            &program,
            &[],
            "",
            "gpt-6-luna",
            project_access(Path::new("C:/worktrees/1/CARD-10"), att, PermissionLevel::Edicao),
            "t1",
            "t1",
            None,
            None,
            None,
            None,
            &pid,
            None,
        )
        .await
        .unwrap();

        // Thread 2: réplica kairo-2 no projeto 1
        pool.run_persistent_turn(
            SessionKey { project_id: 1, agent: kairo_rep.clone() },
            Some("CARD-11".to_string()),
            &program,
            &[],
            "",
            "gpt-6-luna",
            project_access(Path::new("C:/worktrees/1/CARD-11"), att, PermissionLevel::Edicao),
            "t2",
            "t2",
            None,
            None,
            None,
            None,
            &pid,
            None,
        )
        .await
        .unwrap();

        // Thread 3: vexa no projeto 2
        pool.run_persistent_turn(
            SessionKey { project_id: 2, agent: vexa.clone() },
            None,
            &program,
            &[],
            "",
            "gpt-6-luna",
            project_access(Path::new("C:/projects/p2"), att, PermissionLevel::Edicao),
            "t3",
            "t3",
            None,
            None,
            None,
            None,
            &pid,
            None,
        )
        .await
        .unwrap();

        // 1. Limpeza de worktree arquiva thread do worktree (libera lock de pasta no Windows)
        let archived_wt = pool
            .archive_worktree(1, "CARD-10", Some(Path::new("C:/worktrees/1/CARD-10")))
            .await;
        assert_eq!(archived_wt, 1);

        // 2. Exclusão de agente arquiva réplicas remanescentes do agente
        let archived_agent = pool.archive_agent(&kairo).await;
        assert_eq!(archived_agent, 1);

        // 3. Encerramento do projeto 2 arquiva thread do projeto 2
        let archived_project = pool.archive_project(2).await;
        assert_eq!(archived_project, 1);

        let archived_ids: Vec<String> = server
            .received_methods
            .lock()
            .unwrap()
            .iter()
            .filter(|(m, _)| m == "thread/archive")
            .filter_map(|(_, p)| p.get("threadId").and_then(Json::as_str).map(str::to_string))
            .collect();
        assert_eq!(archived_ids, vec!["thr_1", "thr_2", "thr_3"]);
    });
}

#[test]
fn steering_entrega_correcao_no_turno_ativo_e_interrupt_cancela_turno() {
    tauri::async_runtime::block_on(async {
        let server = FakeAppServer::spawn(true);
        server.client.initialize(Duration::from_secs(5)).await.unwrap();

        let pool = Arc::new(CodexIdleSessions::default());
        let program = PathBuf::from("fake-codex");
        pool.register_client_for_test(
            ServerKey::new(&program, &[]),
            Arc::clone(&server.client),
            Instant::now(),
        )
        .await;

        let key = SessionKey {
            project_id: 3,
            agent: Agent::from_stored("axiom").unwrap(),
        };

        // Sem turno ativo -> steer retorna Ok(false)
        assert!(!pool.steer_turn(&key, "antes do turno").await.unwrap());

        // Inicia turno que fica ativo aguardando steering
        let pool_turn = Arc::clone(&pool);
        let key_turn = key.clone();
        let prog_turn = program.clone();
        let turn_handle = tokio::spawn(async move {
            let pid = AtomicU32::new(NO_PID);
            pool_turn
                .run_persistent_turn(
                    key_turn,
                    None,
                    &prog_turn,
                    &[],
                    "",
                    "gpt-6-luna",
                    Access::TextOnly,
                    "Inicie a tarefa",
                    "Inicie a tarefa",
                    None,
                    None,
                    None,
                    None,
                    &pid,
                    None,
                )
                .await
        });

        // Aguarda o turno entrar em execução
        for _ in 0..50 {
            if pool
                .thread_snapshot(&key)
                .and_then(|s| s.active_turn_id)
                .is_some()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Envia correção (steering) enquanto o turno ainda está rodando
        let steered = pool
            .steer_turn(&key, "foca primeiro nos testes unitários")
            .await
            .expect("steer deve suceder");
        assert!(steered, "correção deveria ter entrado no turno ativo");

        let reply = turn_handle.await.unwrap().expect("turno deve concluir");
        assert!(reply.text.contains("foca primeiro nos testes unitários"));
        assert_eq!(
            server.steer_messages.lock().unwrap().as_slice(),
            &["foca primeiro nos testes unitários".to_string()]
        );

        // Testa turn/interrupt durante turno ativo
        let pool_int = Arc::clone(&pool);
        let key_int = key.clone();
        let prog_int = program.clone();
        let interrupt_handle = tokio::spawn(async move {
            let pid = AtomicU32::new(NO_PID);
            pool_int
                .run_persistent_turn(
                    key_int,
                    None,
                    &prog_int,
                    &[],
                    "",
                    "gpt-6-luna",
                    Access::TextOnly,
                    "Tarefa longa",
                    "Tarefa longa",
                    Some("thr_1"),
                    None,
                    None,
                    None,
                    &pid,
                    None,
                )
                .await
        });

        for _ in 0..50 {
            if pool
                .thread_snapshot(&key)
                .and_then(|s| s.active_turn_id)
                .is_some()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        assert!(pool.interrupt_turn(&key).await.unwrap());
        let interrupted_res = interrupt_handle.await.unwrap();
        assert!(matches!(interrupted_res, Err(ClaudeError::Reported(msg)) if msg.contains("interrompido")));
    });
}

#[test]
fn model_list_e_rate_limits_read_funcionam_via_rpc() {
    tauri::async_runtime::block_on(async {
        let server = FakeAppServer::spawn(false);
        server.client.initialize(Duration::from_secs(5)).await.unwrap();

        let pool = CodexIdleSessions::default();
        let program = PathBuf::from("fake-codex");
        pool.register_client_for_test(
            ServerKey::new(&program, &[]),
            Arc::clone(&server.client),
            Instant::now(),
        )
        .await;

        let models = pool.list_models(&program, &[]).await.unwrap();
        assert_eq!(models, vec!["gpt-6-luna", "gpt-5.6-terra"]);

        let usage = pool
            .read_rate_limits(&program, &[], 1_764_000_000_000)
            .await
            .unwrap()
            .expect("cota do codex");
        assert_eq!(usage.provider_id, "codex");
        assert_eq!(usage.primary.as_ref().unwrap().percent, 35);
        assert_eq!(usage.primary.as_ref().unwrap().kind, UsageWindowKind::FiveHour);
        assert_eq!(usage.secondary.as_ref().unwrap().percent, 22);
        assert_eq!(usage.secondary.as_ref().unwrap().kind, UsageWindowKind::Weekly);
        assert_eq!(usage.credits_available, Some(true));
        assert_eq!(usage.reset_credits_available, Some(2));
        drop(server.control_tx);
    });
}

#[test]
fn coletor_de_eventos_alimenta_turn_plan_cotas_step_labels_work_steps_e_contexto() {
    let providers = ProviderRegistry::seeded();
    let codex_cfg = providers.get("codex").unwrap();

    let mut collector = CodexTurnCollector::new(
        Some(&codex_cfg),
        "gpt-6-luna",
        CumulativeTokenCheckpoint::default(),
    );

    let mut captured_plans: Vec<TurnPlan> = Vec::new();
    let mut captured_labels: Vec<String> = Vec::new();
    let mut captured_usage: Vec<ProviderPlanUsage> = Vec::new();
    let mut captured_console: Vec<ConsoleLine> = Vec::new();

    let events = vec![
        json!({
            "method": "turn/plan/updated",
            "params": {
                "explanation": "Migrando adaptador",
                "plan": [
                    { "id": "p1", "step": "Criar cliente JSON-RPC", "status": "completed" },
                    { "id": "p2", "step": "Integrar pool ocioso", "status": "in_progress" }
                ]
            }
        }),
        json!({
            "method": "account/rateLimits/updated",
            "params": {
                "rateLimits": {
                    "limitId": "codex",
                    "primary": { "usedPercent": 61.0, "windowDurationMins": 300, "resetsAt": 1764018000 },
                    "credits": { "hasCredits": true }
                },
                "rateLimitResetCredits": { "availableCount": 4 }
            }
        }),
        json!({
            "method": "item/started",
            "params": {
                "item": {
                    "id": "cmd-1",
                    "type": "commandExecution",
                    "command": "cat desktop/src-tauri/src/providers.rs",
                    "commandActions": [
                        { "type": "read", "path": "desktop/src-tauri/src/providers.rs" }
                    ]
                }
            }
        }),
        json!({
            "method": "item/commandExecution/outputDelta",
            "params": {
                "itemId": "cmd-1",
                "delta": "pub enum ProviderProtocol\n"
            }
        }),
        json!({
            "method": "item/completed",
            "params": {
                "item": {
                    "id": "cmd-1",
                    "type": "commandExecution",
                    "command": "cargo test --quiet",
                    "exitCode": 0,
                    "status": "completed",
                    "aggregatedOutput": "ok 42 passed"
                }
            }
        }),
        json!({
            "method": "item/completed",
            "params": {
                "item": {
                    "id": "fc-1",
                    "type": "fileChange",
                    "status": "completed",
                    "changes": [
                        { "path": "src/orchestrator/adapters/codex_app_server.rs", "kind": "create" }
                    ]
                }
            }
        }),
        json!({
            "method": "item/completed",
            "params": {
                "item": {
                    "id": "mcp-1",
                    "type": "mcpToolCall",
                    "server": "vokia",
                    "tool": "read_card",
                    "status": "completed"
                }
            }
        }),
        json!({
            "method": "item/completed",
            "params": {
                "item": {
                    "id": "ws-1",
                    "type": "webSearch",
                    "query": "codex app-server json-rpc",
                    "status": "completed"
                }
            }
        }),
        json!({
            "method": "item/completed",
            "params": {
                "item": {
                    "id": "msg-1",
                    "type": "agentMessage",
                    "text": "Adaptador implementado com sucesso."
                }
            }
        }),
        json!({
            "method": "thread/tokenUsage/updated",
            "params": {
                "tokenUsage": {
                    "total": {
                        "totalTokens": 12_000,
                        "inputTokens": 9_000,
                        "cachedInputTokens": 4_000,
                        "outputTokens": 2_500,
                        "reasoningOutputTokens": 500
                    },
                    "last": {
                        "totalTokens": 4_200,
                        "inputTokens": 3_400,
                        "cachedInputTokens": 1_200,
                        "outputTokens": 800,
                        "reasoningOutputTokens": 150
                    },
                    "modelContextWindow": 200_000
                }
            }
        }),
        json!({
            "method": "turn/completed",
            "params": { "turn": { "id": "turn-1", "status": "completed" } }
        }),
    ];

    {
        let mut on_plan = |p: TurnPlan| captured_plans.push(p);
        let mut on_activity = |a: TurnActivity| {
            if let Some(label) = a.current_step_label {
                captured_labels.push(label);
            }
        };
        let mut on_rate_limits = |u: ProviderPlanUsage| captured_usage.push(u);
        let mut on_console = |c: ConsoleLine| captured_console.push(c);
        let mut cbs = TurnEventCallbacks {
            on_plan: &mut on_plan,
            on_activity: &mut on_activity,
            on_rate_limits: &mut on_rate_limits,
            on_console: &mut on_console,
        };
        for event in &events {
            collector.process_notification(event, 1_764_000_000_000, &mut cbs);
        }
    }

    // 1. Plano vivo (088)
    assert_eq!(captured_plans.len(), 1);
    assert_eq!(captured_plans[0].steps.len(), 2);
    assert_eq!(captured_plans[0].steps[0].status, TurnPlanStepStatus::Completed);
    assert_eq!(captured_plans[0].steps[1].status, TurnPlanStepStatus::InProgress);

    // 2. Cotas e créditos (084/083)
    assert_eq!(captured_usage.len(), 1);
    assert_eq!(captured_usage[0].primary.as_ref().unwrap().percent, 61);
    assert_eq!(captured_usage[0].credits_available, Some(true));
    assert_eq!(captured_usage[0].reset_credits_available, Some(4));

    // 3. Rótulos humanos a partir de commandActions (090)
    assert!(
        captured_labels.iter().any(|l| l == "Lendo providers.rs"),
        "esperava 'Lendo providers.rs', veio: {captured_labels:?}"
    );
    assert!(!captured_console.is_empty());

    // 4. Reply final, contexto e [work steps] (091)
    let (reply, checkpoint) = collector.into_reply("thr_ev".to_string(), Some(&codex_cfg)).unwrap();
    assert_eq!(reply.text, "Adaptador implementado com sucesso.");
    assert_eq!(reply.session_id.as_deref(), Some("thr_ev"));
    assert_eq!(checkpoint.input_tokens, 9_000);
    assert_eq!(checkpoint.output_tokens, 2_500);

    let turn_usage = reply.usage.as_ref().expect("TurnUsage presente");
    assert_eq!(turn_usage.context_tokens, Some(4_200));
    assert_eq!(turn_usage.context_window, Some(200_000));
    assert_eq!(turn_usage.input_tokens, Some(9_000));
    assert_eq!(turn_usage.cached_input_tokens, Some(4_000));
    assert_eq!(turn_usage.thought_tokens, Some(500));

    assert!(reply
        .activity
        .tools
        .iter()
        .any(|t| t.category == ToolCategory::Command && t.status == ToolStatus::Ok && t.exit_code == Some(0)));

    let work_steps = WorkStepCollector::from_turn_activity(&reply.activity)
        .format()
        .expect("bloco [work steps]");
    assert!(work_steps.block.contains("[work steps]"));
    assert!(work_steps.block.contains("cargo test --quiet"));
    assert!(work_steps.block.contains("codex_app_server.rs"));
    assert!(work_steps.block.contains("vokia/read_card"));
    assert!(work_steps.block.contains("codex app-server json-rpc"));
}

#[test]
fn diagnosticos_pt_br_para_binario_antigo_backfill_code_mode_host_e_fallback_jsonl() {
    let providers = ProviderRegistry::seeded();
    let mut codex = providers.get("codex").unwrap();

    // 1. Binário antigo / incompatível -> orienta atualização ou fallback para codex-jsonl
    let err_old = classify_app_server_error(
        "error: unrecognized subcommand 'app-server'",
        "gpt-6-luna",
        Some(&codex),
    );
    let msg_old = err_old.to_string();
    assert!(
        msg_old.contains("codex app-server") && msg_old.contains("codex-jsonl"),
        "mensagem inesperada: {msg_old}"
    );

    // 2. Timeout no state db backfill -> mensagem acionável em PT-BR
    let err_backfill = classify_app_server_error(
        "timed out waiting for state db backfill",
        "gpt-6-luna",
        Some(&codex),
    );
    let msg_backfill = err_backfill.to_string();
    assert!(
        msg_backfill.contains("banco local de estado") && msg_backfill.contains("codex-jsonl"),
        "mensagem inesperada: {msg_backfill}"
    );

    // 3. Ausência de codex-code-mode-host -> mensagem clara em PT-BR
    let err_host = classify_app_server_error(
        "failed to spawn codex-code-mode-host: No such file or directory",
        "gpt-6-luna",
        Some(&codex),
    );
    let msg_host = err_host.to_string();
    assert!(
        msg_host.contains("codex-code-mode-host") && msg_host.contains("gpt-6-luna"),
        "mensagem inesperada: {msg_host}"
    );

    // Detecção preventiva de binário incompleto para modelo code-mode
    let empty_dir = std::env::temp_dir().join(format!("vokia-empty-codex-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&empty_dir);
    assert!(detect_missing_code_mode_host(&empty_dir, false, "gpt-6-luna").is_some());
    assert!(detect_missing_code_mode_host(&empty_dir, true, "gpt-6-luna").is_none());
    assert!(detect_missing_code_mode_host(&empty_dir, false, "gpt-5.6-terra").is_none());
    let _ = std::fs::remove_dir_all(&empty_dir);

    // 4. Fallback explícito para CodexJsonl via configuração do provedor
    assert_eq!(codex.protocol, ProviderProtocol::CodexAppServer);
    assert!(matches!(
        ProviderAdapter::from_config(codex.clone()),
        ProviderAdapter::CodexAppServer(_)
    ));
    codex.protocol = ProviderProtocol::CodexJsonl;
    assert!(matches!(
        ProviderAdapter::from_config(codex),
        ProviderAdapter::Codex(_)
    ));
}

#[test]
fn shutdown_all_sync_encerra_arvore_de_processos_sem_deixar_orfaos() {
    tauri::async_runtime::block_on(async {
        #[cfg(windows)]
        let cmd = {
            let mut c = crate::claude::base_command(Path::new("cmd.exe"));
            c.args(["/C", "ping -n 30 127.0.0.1 >nul"]);
            c.stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            c
        };
        #[cfg(not(windows))]
        let cmd = {
            let mut c = crate::claude::base_command(Path::new("sh"));
            c.args(["-c", "sleep 30"]);
            c.process_group(0);
            c.stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            c
        };

        let mut child = crate::silent_process::spawn_silent(cmd).expect("spawn processo dummy");
        let pid = child.id().expect("pid do processo dummy");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let client = CodexRpcClient::from_io(
            BufReader::new(stdout),
            stdin,
            Some(child),
            Some(pid),
            Arc::new(StdMutex::new(VecDeque::new())),
        );
        let pool = CodexIdleSessions::default();
        let key = ServerKey::new(Path::new("dummy"), &[]);
        pool.register_client_for_test(key.clone(), Arc::clone(&client), Instant::now())
            .await;

        assert!(client.is_alive());
        pool.shutdown_all_sync();
        assert!(!client.is_alive());
        assert_eq!(pool.server_state(&key).await, ServerLifecycleState::Closed);
    });
}

#[test]
fn adapter_runtime_pool_delega_steer_interrupt_e_responde_requisicao_de_aprovacao_do_servidor() {
    tauri::async_runtime::block_on(async {
        let runtime_pool = crate::orchestrator::adapters::AdapterRuntimePool::default();
        let idle_key = SessionKey {
            project_id: 99,
            agent: Agent::from_stored("kairo").unwrap(),
        };
        assert!(!runtime_pool.steer_turn(&idle_key, "sem turno").await.unwrap());
        assert!(!runtime_pool.interrupt_turn(&idle_key).await.unwrap());

        // Simula requisição server -> client de aprovação durante um turno ativo
        let (c2s_w, c2s_r) = tokio::io::duplex(16 * 1024);
        let (mut s2c_w, s2c_r) = tokio::io::duplex(16 * 1024);
        let approval_reply = Arc::new(StdMutex::new(None::<Json>));
        let approval_reply_srv = Arc::clone(&approval_reply);

        tokio::spawn(async move {
            let mut reader = BufReader::new(c2s_r).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let Ok(msg) = serde_json::from_str::<Json>(line.trim()) else {
                    continue;
                };
                if msg.get("method").is_none() && msg.get("id").and_then(Json::as_u64) == Some(77) {
                    *approval_reply_srv.lock().unwrap() = Some(msg);
                    let done = json!({
                        "jsonrpc": "2.0",
                        "method": "turn/completed",
                        "params": { "threadId": "thr_ap", "turn": { "id": "turn_ap", "status": "completed" } }
                    });
                    let _ = s2c_w.write_all(format!("{done}\n").as_bytes()).await;
                    let _ = s2c_w.flush().await;
                    continue;
                }
                let id = msg.get("id").cloned().unwrap_or(Json::Null);
                let method = msg.get("method").and_then(Json::as_str).unwrap_or("");
                let resp = match method {
                    "initialize" => Some(json!({ "jsonrpc": "2.0", "id": id, "result": {} })),
                    "thread/start" => {
                        Some(json!({ "jsonrpc": "2.0", "id": id, "result": { "thread": { "id": "thr_ap" } } }))
                    }
                    "turn/start" => {
                        let start_resp = json!({ "jsonrpc": "2.0", "id": id, "result": { "turn": { "id": "turn_ap" } } });
                        let _ = s2c_w.write_all(format!("{start_resp}\n").as_bytes()).await;
                        let approval_req = json!({
                            "jsonrpc": "2.0",
                            "id": 77,
                            "method": "item/commandExecution/requestApproval",
                            "params": { "threadId": "thr_ap", "command": "rm -rf /tmp/x" }
                        });
                        let _ = s2c_w.write_all(format!("{approval_req}\n").as_bytes()).await;
                        let _ = s2c_w.flush().await;
                        None
                    }
                    _ => None,
                };
                if let Some(r) = resp {
                    let _ = s2c_w.write_all(format!("{r}\n").as_bytes()).await;
                    let _ = s2c_w.flush().await;
                }
            }
        });

        let client = CodexRpcClient::from_io(
            BufReader::new(s2c_r),
            c2s_w,
            None,
            None,
            Arc::new(StdMutex::new(VecDeque::new())),
        );
        client.initialize(Duration::from_secs(5)).await.unwrap();
        let pool = CodexIdleSessions::default();
        let program = PathBuf::from("fake-approval");
        pool.register_client_for_test(ServerKey::new(&program, &[]), client, Instant::now())
            .await;

        let pid = AtomicU32::new(NO_PID);
        pool.run_persistent_turn(
            idle_key.clone(),
            None,
            &program,
            &[],
            "",
            "gpt-6-luna",
            Access::TextOnly,
            "teste aprovacao",
            "teste aprovacao",
            None,
            None,
            None,
            None,
            &pid,
            None,
        )
        .await
        .expect("turno conclui apos responder aprovacao");

        let reply_msg = approval_reply
            .lock()
            .unwrap()
            .clone()
            .expect("resposta JSON-RPC de aprovacao enviada ao servidor");
        assert_eq!(reply_msg.pointer("/result/decision").and_then(Json::as_str), Some("decline"));
        assert_eq!(pool.thread_snapshot(&idle_key).unwrap().pending_approvals, 0);
    });
}
