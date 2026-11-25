use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::json;

use super::*;
use crate::agents::Agent;
use crate::claude::TurnActivity;
use crate::commands::dispatcher;
use crate::commands::turn_plan::{resolve_turn_plan, GetTurnPlanArgs};
use crate::db::{Db, NewProject};
use crate::events::{Speaker, VokiaEvent};

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

struct TempDbDir(PathBuf);

impl TempDbDir {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "vokia-turn-plan-test-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::SeqCst)
        )))
    }

    fn open(&self) -> Db {
        Db::open(&self.0).expect("o banco de teste deveria abrir")
    }
}

impl Drop for TempDbDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn agent(id: &str) -> Agent {
    Agent::from_stored(id).expect("id de agente válido")
}

// ==========================================
// 1. Normalização
// ==========================================

#[test]
fn normalizacao_limita_a_100_etapas_deduplica_ids_e_descarta_vazias() {
    let mut steps = Vec::new();
    steps.push(TurnPlanStep {
        id: "dup".to_string(),
        text: "   ".to_string(),
        active_text: None,
        status: TurnPlanStepStatus::Pending,
    });
    for i in 0..115 {
        steps.push(TurnPlanStep {
            id: if i < 3 {
                "dup".to_string()
            } else if i == 3 {
                "".to_string()
            } else {
                format!("id-{i}")
            },
            text: format!("  Etapa   {i}  "),
            active_text: if i == 0 {
                Some("   Executando   etapa 0  ".to_string())
            } else if i == 1 {
                Some("   ".to_string())
            } else {
                None
            },
            status: TurnPlanStepStatus::Pending,
        });
    }

    let plan = TurnPlan {
        explanation: Some("  Plano   geral  ".to_string()),
        steps,
    }
    .normalize();

    assert_eq!(plan.explanation.as_deref(), Some("Plano geral"));
    assert_eq!(plan.steps.len(), MAX_TURN_PLAN_STEPS);
    assert_eq!(plan.steps[0].id, "dup");
    assert_eq!(plan.steps[0].text, "Etapa 0");
    assert_eq!(
        plan.steps[0].active_text.as_deref(),
        Some("Executando etapa 0")
    );
    assert_eq!(plan.steps[1].id, "dup-2");
    assert_eq!(plan.steps[1].active_text, None);
    assert_eq!(plan.steps[2].id, "dup-3");
    assert_eq!(plan.steps[3].id, "step-4");

    let unique_ids: HashSet<&str> = plan.steps.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(unique_ids.len(), MAX_TURN_PLAN_STEPS);
}

#[test]
fn normalizacao_trunca_textos_e_ids_longos_com_utf8_seguro() {
    let long_utf8 = "á".repeat(600);
    let plan = TurnPlan {
        explanation: Some(long_utf8.clone()),
        steps: vec![TurnPlanStep {
            id: long_utf8.clone(),
            text: long_utf8.clone(),
            active_text: Some(long_utf8),
            status: TurnPlanStepStatus::InProgress,
        }],
    }
    .normalize();

    assert_eq!(
        plan.explanation.as_ref().unwrap().chars().count(),
        MAX_PLAN_EXPLANATION_CHARS
    );
    assert_eq!(plan.steps[0].id.chars().count(), MAX_PLAN_STEP_ID_CHARS);
    assert_eq!(
        plan.steps[0].text.chars().count(),
        MAX_PLAN_STEP_TEXT_CHARS
    );
    assert_eq!(
        plan.steps[0]
            .active_text
            .as_ref()
            .unwrap()
            .chars()
            .count(),
        MAX_PLAN_STEP_TEXT_CHARS
    );
}

// ==========================================
// 2. Claude (TodoWrite, TaskCreate, TaskUpdate, deleted, remap)
// ==========================================

#[test]
fn claude_todo_write_substitui_lista_e_normaliza_campos() {
    let mut state = ClaudePlanState::default();
    let plan = fold_claude_plan_call(
        &mut state,
        "call-1",
        "TodoWrite",
        &json!({
            "explanation": "Implementar plano vivo",
            "todos": [
                { "id": "1", "content": "Ler arquitetura", "status": "completed" },
                { "id": "2", "content": "Criar módulo", "activeForm": "Criando módulo turn_plan.rs", "status": "in_progress" },
                { "id": "3", "content": "Rodar testes", "status": "pending" }
            ]
        }),
    )
    .expect("TodoWrite deve gerar TurnPlan");

    assert_eq!(plan.explanation.as_deref(), Some("Implementar plano vivo"));
    assert_eq!(plan.steps.len(), 3);
    assert_eq!(plan.steps[0].status, TurnPlanStepStatus::Completed);
    assert_eq!(plan.steps[1].status, TurnPlanStepStatus::InProgress);
    assert_eq!(
        plan.steps[1].active_text.as_deref(),
        Some("Criando módulo turn_plan.rs")
    );
    assert_eq!(plan.steps[2].status, TurnPlanStepStatus::Pending);
    assert_eq!(plan.completed_count(), 1);
    assert_eq!(plan.open_steps().len(), 2);
}

#[test]
fn claude_task_create_remap_id_via_tool_result_update_e_deleted() {
    let mut state = ClaudePlanState::default();

    // 1. Cria duas tarefas com IDs temporários
    let p1 = fold_claude_plan_call(
        &mut state,
        "tu-1",
        "TaskCreate",
        &json!({ "subject": "Analisar código", "activeForm": "Analisando código" }),
    )
    .expect("primeiro TaskCreate");
    assert_eq!(p1.steps.len(), 1);
    assert_eq!(p1.steps[0].id, "temp:tu-1");

    let p2 = fold_claude_plan_call(
        &mut state,
        "tu-2",
        "TaskCreate",
        &json!({ "subject": "Escrever testes" }),
    )
    .expect("segundo TaskCreate");
    assert_eq!(p2.steps.len(), 2);
    assert_eq!(p2.steps[1].id, "temp:tu-2");

    // 2. Remapeamento via tool_use_result estruturado (`task.id`)
    let r1 = fold_claude_plan_result(
        &mut state,
        "tu-1",
        Some(&json!("Task #1 created successfully")),
        Some(&json!({ "task": { "id": "1" } })),
        false,
    )
    .expect("remap estruturado");
    assert_eq!(r1.steps[0].id, "1");

    // 3. Remapeamento via texto puro no content (`Task #2 created`)
    let r2 = fold_claude_plan_result(
        &mut state,
        "tu-2",
        Some(&json!("Task #2 created successfully: Escrever testes")),
        None,
        false,
    )
    .expect("remap via texto");
    assert_eq!(r2.steps[1].id, "2");

    // 4. TaskUpdate muda status da tarefa 1 para in_progress e depois completed
    let u1 = fold_claude_plan_call(
        &mut state,
        "tu-3",
        "TaskUpdate",
        &json!({ "taskId": "1", "status": "in_progress" }),
    )
    .expect("TaskUpdate in_progress");
    assert_eq!(u1.steps[0].status, TurnPlanStepStatus::InProgress);

    let u2 = fold_claude_plan_call(
        &mut state,
        "tu-4",
        "TaskUpdate",
        &json!({ "taskId": "1", "status": "completed" }),
    )
    .expect("TaskUpdate completed");
    assert_eq!(u2.steps[0].status, TurnPlanStepStatus::Completed);

    // 5. TaskUpdate com status `deleted` remove a tarefa 2
    let u3 = fold_claude_plan_call(
        &mut state,
        "tu-5",
        "TaskUpdate",
        &json!({ "taskId": "2", "status": "deleted" }),
    )
    .expect("TaskUpdate deleted");
    assert_eq!(u3.steps.len(), 1);
    assert_eq!(u3.steps[0].id, "1");
    assert!(u3.is_all_completed());
}

#[test]
fn claude_task_create_com_erro_no_tool_result_remove_etapa_temporaria() {
    let mut state = ClaudePlanState::default();
    let p = fold_claude_plan_call(
        &mut state,
        "tu-err",
        "TaskCreate",
        &json!({ "subject": "Falha ao criar" }),
    )
    .unwrap();
    assert_eq!(p.steps.len(), 1);

    let after = fold_claude_plan_result(&mut state, "tu-err", Some(&json!("erro")), None, true)
        .expect("erro deve remover etapa temporária");
    assert!(after.is_empty());
}

#[test]
fn claude_begin_new_turn_limpa_etapas_concluidas_e_mantem_abertas() {
    let mut state = ClaudePlanState::default();
    fold_claude_plan_call(
        &mut state,
        "c1",
        "TodoWrite",
        &json!({
            "todos": [
                { "id": "1", "content": "Feita", "status": "completed" },
                { "id": "2", "content": "Ainda aberta", "status": "in_progress" }
            ]
        }),
    );
    state.begin_new_turn();
    let snap = state.snapshot();
    assert_eq!(snap.steps.len(), 1);
    assert_eq!(snap.steps[0].id, "2");
}

// ==========================================
// 3. Codex (`turn/plan/updated` e `todo_list`)
// ==========================================

#[test]
fn codex_turn_plan_updated_extrai_explanation_e_status_camel_case() {
    let notification = json!({
        "jsonrpc": "2.0",
        "method": "turn/plan/updated",
        "params": {
            "turnId": "turn_42",
            "explanation": "Atualizando plano após ler os testes",
            "plan": [
                { "step": "Inspecionar adaptador", "status": "completed" },
                { "step": "Emitir evento ao vivo", "activeText": "Emitindo evento ao vivo", "status": "inProgress" },
                { "step": "Validar cargo test", "status": "pending" }
            ]
        }
    });

    let plan = plan_from_codex_notification(&notification).expect("plano Codex");
    assert_eq!(
        plan.explanation.as_deref(),
        Some("Atualizando plano após ler os testes")
    );
    assert_eq!(plan.steps.len(), 3);
    assert_eq!(plan.steps[0].id, "step-1");
    assert_eq!(plan.steps[0].status, TurnPlanStepStatus::Completed);
    assert_eq!(plan.steps[1].status, TurnPlanStepStatus::InProgress);
    assert_eq!(
        plan.steps[1].active_text.as_deref(),
        Some("Emitindo evento ao vivo")
    );
    assert_eq!(plan.steps[2].status, TurnPlanStepStatus::Pending);
}

#[test]
fn codex_todo_list_jsonl_e_eventos_nao_relacionados() {
    let todo_item = json!({
        "type": "item.updated",
        "item": {
            "id": "todo_1",
            "type": "todo_list",
            "items": [
                { "text": "Criar schema", "completed": true },
                { "text": "Rodar typecheck", "completed": false }
            ]
        }
    });
    let plan = plan_from_codex_notification(&todo_item).expect("todo_list Codex");
    assert_eq!(plan.steps.len(), 2);
    assert_eq!(plan.steps[0].status, TurnPlanStepStatus::Completed);
    assert_eq!(plan.steps[1].status, TurnPlanStepStatus::Pending);

    let unrelated = json!({ "type": "item.completed", "item": { "type": "agent_message", "text": "oi" } });
    assert_eq!(plan_from_codex_notification(&unrelated), None);
}

// ==========================================
// 4. OpenCode e Gemini (ACP `plan`)
// ==========================================

#[test]
fn acp_plan_entries_e_notificacao_session_update_normalizam_etapas() {
    let entries = vec![
        json!({ "content": "Mapear arquivos", "priority": "high", "status": "completed" }),
        json!({ "id": "p2", "content": "Implementar UI", "activeForm": "Implementando UI", "status": "in_progress" }),
        json!({ "content": "   ", "status": "pending" }),
        json!({ "id": "p2", "content": "Executar verificação", "status": "pending" }),
    ];

    let plan = plan_from_acp_entries(&entries);
    assert_eq!(plan.steps.len(), 3);
    assert_eq!(plan.steps[0].id, "step-1");
    assert_eq!(plan.steps[0].status, TurnPlanStepStatus::Completed);
    assert_eq!(plan.steps[1].id, "p2");
    assert_eq!(plan.steps[1].status, TurnPlanStepStatus::InProgress);
    assert_eq!(
        plan.steps[1].active_text.as_deref(),
        Some("Implementando UI")
    );
    assert_eq!(plan.steps[2].id, "p2-2");
    assert_eq!(plan.steps[2].status, TurnPlanStepStatus::Pending);

    let rpc = json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {
            "sessionId": "sess-1",
            "update": {
                "sessionUpdate": "plan",
                "explanation": "Roteiro ACP",
                "entries": [
                    { "content": "Etapa A", "status": "inProgress" },
                    { "content": "Etapa B", "status": "pending" }
                ]
            }
        }
    });
    let from_rpc = plan_from_acp_notification(&rpc).expect("notificação ACP");
    assert_eq!(from_rpc.explanation.as_deref(), Some("Roteiro ACP"));
    assert_eq!(from_rpc.steps.len(), 2);
    assert_eq!(from_rpc.steps[0].status, TurnPlanStepStatus::InProgress);

    let agy_step = json!({
        "event": "step_update",
        "step_update": {
            "step_type": "plan",
            "steps": [
                { "title": "Verificar build", "status": "completed" }
            ]
        }
    });
    let from_agy = plan_from_acp_notification(&agy_step).expect("step_update plan");
    assert_eq!(from_agy.steps.len(), 1);
    assert_eq!(from_agy.steps[0].text, "Verificar build");
    assert_eq!(from_agy.steps[0].status, TurnPlanStepStatus::Completed);
}

// ==========================================
// 5. Evento, Store, Persistência e comando `get_turn_plan`
// ==========================================

#[test]
fn evento_turn_plan_updated_serializa_no_contrato_esperado() {
    let event = VokiaEvent::TurnPlanUpdated {
        project_id: 7,
        agent: agent("hefesto"),
        card_id: Some("PIX-9".to_string()),
        turn_id: "turn-12".to_string(),
        plan: TurnPlan {
            explanation: Some("Plano".to_string()),
            steps: vec![TurnPlanStep {
                id: "1".to_string(),
                text: "Fazer algo".to_string(),
                active_text: Some("Fazendo algo".to_string()),
                status: TurnPlanStepStatus::InProgress,
            }],
        },
    };
    let val = serde_json::to_value(event).unwrap();
    assert_eq!(
        val,
        json!({
            "type": "turn-plan-updated",
            "projectId": 7,
            "agent": "hefesto",
            "cardId": "PIX-9",
            "turnId": "turn-12",
            "plan": {
                "explanation": "Plano",
                "steps": [
                    {
                        "id": "1",
                        "text": "Fazer algo",
                        "activeText": "Fazendo algo",
                        "status": "in_progress"
                    }
                ]
            }
        })
    );
}

#[test]
fn comando_get_turn_plan_le_memoria_filtra_por_card_e_faz_fallback_no_banco() {
    let store = TurnPlanStore::default();
    let hefesto = agent("hefesto");
    let live_plan = TurnPlan {
        explanation: None,
        steps: vec![
            TurnPlanStep {
                id: "1".to_string(),
                text: "Passo ativo".to_string(),
                active_text: Some("Executando passo ativo".to_string()),
                status: TurnPlanStepStatus::InProgress,
            },
        ],
    };

    store.upsert(TurnPlanSnapshot {
        project_id: 1,
        agent: hefesto.clone(),
        card_id: Some("CARD-1".to_string()),
        turn_id: "turn-1".to_string(),
        plan: live_plan.clone(),
        updated_at: 1000,
        active: true,
    });

    // Consulta ativa em memória (com e sem project_id)
    let found = resolve_turn_plan(
        &store,
        None,
        &GetTurnPlanArgs {
            agent: hefesto.clone(),
            project_id: Some(1),
            card_id: Some("CARD-1".to_string()),
        },
    )
    .unwrap()
    .expect("deve retornar plano vivo");
    assert!(found.active);
    assert_eq!(found.turn_id, "turn-1");
    assert_eq!(found.plan, live_plan);

    let found_no_project = resolve_turn_plan(
        &store,
        None,
        &GetTurnPlanArgs {
            agent: hefesto.clone(),
            project_id: None,
            card_id: None,
        },
    )
    .unwrap()
    .expect("consulta sem projectId encontra o mais recente");
    assert_eq!(found_no_project.project_id, 1);

    // Encerramento do turno marca `active: false`
    store.mark_turn_finished(1, &hefesto, "turn-1");
    let finished = store.get(&hefesto, Some(1), None).unwrap();
    assert!(!finished.active);

    // Limpeza do projeto remove da memória
    store.forget_project(1);
    assert!(store.get(&hefesto, Some(1), None).is_none());

    // Fallback no banco (TurnActivity persistido após o turno/reinício)
    let temp = TempDbDir::new();
    let db = temp.open();
    let project = db
        .insert_project(&NewProject {
            name: "Projeto Plano",
            symbol: "folder",
            description: "",
            path: "C:/projetos/plano",
            origin_kind: "folder",
            origin_repo: None,
        })
        .unwrap();
    let persisted_plan = TurnPlan {
        explanation: Some("Persistido".to_string()),
        steps: vec![TurnPlanStep {
            id: "s1".to_string(),
            text: "Etapa concluída".to_string(),
            active_text: None,
            status: TurnPlanStepStatus::Completed,
        }],
    };
    let activity = TurnActivity {
        thinking_ms: Some(250),
        thinking_text: None,
        tools: Vec::new(),
        current_step_label: None,
        plan: Some(persisted_plan.clone()),
    };
    db.insert_message(
        project.id,
        Speaker::Agent(hefesto.clone()),
        Speaker::User,
        "Tudo pronto.",
        2000,
        None,
        Some("CARD-2"),
        Some(&activity),
        None,
        None,
    )
    .unwrap();

    let from_db = resolve_turn_plan(
        &store,
        Some(&db),
        &GetTurnPlanArgs {
            agent: hefesto.clone(),
            project_id: Some(project.id),
            card_id: Some("CARD-2".to_string()),
        },
    )
    .unwrap()
    .expect("deve recuperar plano persistido no histórico");
    assert!(!from_db.active);
    assert_eq!(from_db.card_id.as_deref(), Some("CARD-2"));
    assert_eq!(from_db.plan, persisted_plan);

    // Card diferente não encontra
    let wrong_card = resolve_turn_plan(
        &store,
        Some(&db),
        &GetTurnPlanArgs {
            agent: hefesto,
            project_id: Some(project.id),
            card_id: Some("CARD-OUTRO".to_string()),
        },
    )
    .unwrap();
    assert!(wrong_card.is_none());
}

#[test]
fn comando_get_turn_plan_esta_registrado_no_dispatcher() {
    assert_eq!(dispatcher::command_domain("get_turn_plan"), Some("orchestration"));
    let commands = dispatcher::list_commands();
    let meta = commands
        .iter()
        .find(|cmd| cmd["name"].as_str() == Some("get_turn_plan"))
        .expect("get_turn_plan deve constar em list_commands");
    assert_eq!(meta["inputSchema"]["required"], json!(["agent"]));
}
