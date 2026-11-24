use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{Listener, Manager};

use super::*;
use crate::agents::Agent;
use crate::commands::dispatcher::dispatch;
use crate::projects::worktree::project_worktrees_dir;
use crate::db::{DbState, NewProject};
use crate::mcp::support::build_app;
use crate::state::AppState;

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

const BASH: &str = "Bash";
const EDIT: &str = "Edit";
const EVENT_WAIT: Duration = Duration::from_secs(5);
const NO_PROMPT_WAIT: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(10);

struct World {
    app: tauri::App,
    dir: PathBuf,
    project_id: i64,
    events: Arc<Mutex<Vec<Value>>>,
    granted: Arc<Mutex<HashSet<String>>>,
    next_request: AtomicU32,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[derive(Clone, Copy)]
enum Reply {
    Allow { always: bool, root: bool },
    Deny,
    NoPromptExpected,
}

fn world() -> World {
    let dir = std::env::temp_dir().join(format!(
        "vokia-permission-broker-test-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::SeqCst)
    ));
    let app = build_app(&dir);
    let project_id = app
        .state::<DbState>()
        .db()
        .expect("banco de teste aberto")
        .insert_project(&NewProject {
            name: "a",
            symbol: "folder",
            description: "",
            path: "C:/projetos/a",
            origin_kind: "folder",
            origin_repo: None,
        })
        .expect("o projeto deveria ser criado")
        .id;
    let events: Arc<Mutex<Vec<Value>>> = Arc::default();
    let sink = events.clone();
    app.handle().listen(EVENT_NAME, move |event| {
        if let Ok(value) = serde_json::from_str::<Value>(event.payload()) {
            sink.lock().unwrap().push(value);
        }
    });
    World {
        app,
        dir,
        project_id,
        events,
        granted: Arc::default(),
        next_request: AtomicU32::new(0),
    }
}

impl World {
    fn db(&self) -> &Db {
        self.app.state::<DbState>().inner().db().expect("banco de teste aberto")
    }

    fn of_type(&self, kind: &str) -> Vec<Value> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| event["type"] == kind)
            .cloned()
            .collect()
    }

    fn pending_prompt(&self, request_id: &str) -> Option<Value> {
        self.of_type("permission").into_iter().find(|event| event["id"] == request_id)
    }

    fn rule_commands(&self) -> Vec<String> {
        self.db()
            .list_permission_rules(self.project_id)
            .unwrap()
            .into_iter()
            .map(|rule| rule.command)
            .collect()
    }

    fn ask(&self, per_session: bool, tool: &str, input: Value, reply: Reply) -> (PermissionVerdict, Option<Value>) {
        let request_id = format!("req-{}", self.next_request.fetch_add(1, Ordering::SeqCst));
        let request = PermissionRequest {
            id: request_id.clone(),
            tool: tool.to_string(),
            input,
        };
        let app = self.app.handle().clone();
        let granted = self.granted.clone();
        let project_id = self.project_id;
        let decision = tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            PermissionGate {
                app: &app,
                broker: &state.permissions,
                agent: Agent::ember(),
                project_id,
                item_id: None,
                granted: &granted,
                grant_per_session: per_session,
            }
            .decide(request)
            .await
        });
        if let Reply::NoPromptExpected = reply {
            let verdict = tauri::async_runtime::block_on(async {
                tokio::time::timeout(NO_PROMPT_WAIT, decision)
                    .await
                    .expect("o pedido não deveria ter ido ao usuário")
                    .expect("a decisão deveria terminar")
            });
            assert!(self.pending_prompt(&request_id).is_none(), "não deveria ter emitido pedido");
            return (verdict, None);
        }
        let waited = std::time::Instant::now();
        let prompt = loop {
            if let Some(prompt) = self.pending_prompt(&request_id) {
                break prompt;
            }
            assert!(waited.elapsed() < EVENT_WAIT, "o pedido {request_id} deveria chegar ao usuário");
            std::thread::sleep(POLL);
        };
        let answer = match reply {
            Reply::Allow { always, root } => json!({ "id": request_id, "allow": true, "always": always, "root": root }),
            _ => json!({ "id": request_id, "allow": false, "always": false }),
        };
        tauri::async_runtime::block_on(dispatch(self.app.handle(), None, "answer_permission", answer))
            .expect("answer_permission reconhecido")
            .expect("a resposta deveria ser aceita");
        let verdict = tauri::async_runtime::block_on(decision).expect("a decisão deveria terminar");
        (verdict, Some(prompt))
    }

    fn bash(&self, per_session: bool, command: &str, reply: Reply) -> (PermissionVerdict, Option<Value>) {
        self.ask(per_session, BASH, json!({ "command": command }), reply)
    }

    fn worktrees_dir(&self, project_id: i64) -> String {
        let data_dir = self.app.path().app_data_dir().expect("pasta de dados do app");
        project_worktrees_dir(&data_dir, project_id).to_string_lossy().replace('\\', "/")
    }
}

const ALLOW_ONCE: Reply = Reply::Allow { always: false, root: false };
const ALLOW_ROOT: Reply = Reply::Allow { always: false, root: true };
const ALWAYS_ROOT: Reply = Reply::Allow { always: true, root: true };
const ALWAYS_EXACT: Reply = Reply::Allow { always: true, root: false };

#[test]
fn pedido_de_comando_simples_informa_o_comando_raiz_e_composto_nao() {
    let world = world();

    let (_, simple) = world.bash(false, "cd /a", Reply::Deny);
    let (_, composed) = world.bash(false, "cd /a && rm -rf b", Reply::Deny);

    assert_eq!(simple.unwrap()["rootCommand"], "cd");
    assert_eq!(composed.unwrap()["rootCommand"], Value::Null);
}

#[test]
fn pedido_de_arquivo_nao_tem_comando_raiz_mesmo_com_espaco_no_caminho() {
    let world = world();

    let (_, prompt) = world.ask(false, EDIT, json!({ "file_path": "C:/Meus Documentos/a.txt" }), Reply::Deny);

    assert_eq!(prompt.unwrap()["rootCommand"], Value::Null);
}

#[test]
fn permitir_raiz_na_sessao_libera_outro_argumento_do_mesmo_comando_sem_novo_pedido() {
    let world = world();

    let (first, _) = world.bash(true, "cd /a", ALLOW_ROOT);
    let (second, _) = world.bash(true, "cd /b", Reply::NoPromptExpected);

    assert_eq!(first, PermissionVerdict::Allow);
    assert_eq!(second, PermissionVerdict::Allow);
    assert_eq!(world.of_type("permission").len(), 1);
}

#[test]
fn permitir_raiz_na_sessao_nao_libera_comando_composto_nem_outra_raiz() {
    let world = world();
    world.bash(true, "cd /a", ALLOW_ROOT);

    let (composed, composed_prompt) = world.bash(true, "cd /a && rm -rf b", Reply::Deny);
    let (other, other_prompt) = world.bash(true, "rm -rf b", Reply::Deny);

    assert_eq!(composed, PermissionVerdict::Deny(None));
    assert!(composed_prompt.is_some());
    assert_eq!(other, PermissionVerdict::Deny(None));
    assert!(other_prompt.is_some());
}

#[test]
fn permitir_sem_raiz_na_sessao_vale_so_para_o_comando_exato() {
    let world = world();
    world.bash(true, "cd /a", ALLOW_ONCE);

    let (same, _) = world.bash(true, "cd /a", Reply::NoPromptExpected);
    let (different, different_prompt) = world.bash(true, "cd /b", Reply::Deny);

    assert_eq!(same, PermissionVerdict::Allow);
    assert_eq!(different, PermissionVerdict::Deny(None));
    assert!(different_prompt.is_some());
}

#[test]
fn raiz_na_sessao_nao_vaza_para_outra_ferramenta() {
    let world = world();
    world.bash(true, "cd /a", ALLOW_ROOT);

    let (verdict, prompt) = world.ask(true, "PowerShell", json!({ "command": "cd /b" }), Reply::Deny);

    assert_eq!(verdict, PermissionVerdict::Deny(None));
    assert!(prompt.is_some());
}

#[test]
fn sempre_com_raiz_grava_a_regra_raiz_e_o_proximo_pedido_passa_por_regra() {
    let world = world();

    let (verdict, _) = world.bash(false, "cd /a", ALWAYS_ROOT);
    let (next, _) = world.bash(false, "cd /outro/caminho", Reply::NoPromptExpected);

    assert_eq!(verdict, PermissionVerdict::Allow);
    assert_eq!(next, PermissionVerdict::Allow);
    assert_eq!(world.rule_commands(), ["cd *"]);
    assert_eq!(world.of_type("permission-by-rule").len(), 1);
    assert_eq!(world.db().approval_history(world.project_id, BASH, "cd /q", Some("cd")).unwrap().approved, 1);
}

#[test]
fn sempre_sem_raiz_grava_so_a_regra_exata() {
    let world = world();
    world.bash(false, "cd /a", ALWAYS_EXACT);

    assert_eq!(world.rule_commands(), ["cd /a"]);
    assert!(!world.db().has_permission_rule(world.project_id, BASH, "cd /b").unwrap());
}

#[test]
fn raiz_em_comando_composto_e_ignorada_e_grava_a_regra_exata() {
    let world = world();
    world.bash(false, "cd /a && make", ALWAYS_ROOT);

    assert_eq!(world.rule_commands(), ["cd /a && make"]);
    assert!(!world.db().has_permission_rule(world.project_id, BASH, "cd /x").unwrap());
}

#[test]
fn raiz_em_pedido_de_arquivo_com_espaco_no_caminho_nao_vira_regra_de_prefixo() {
    let world = world();
    let path = "C:/Meus Documentos/a.txt";
    world.ask(false, EDIT, json!({ "file_path": path }), ALWAYS_ROOT);

    assert_eq!(world.rule_commands(), [path]);
    assert!(!world.db().has_permission_rule(world.project_id, EDIT, "C:/Meus Documentos/b.txt").unwrap());
}

#[test]
fn contador_do_pedido_agrupa_as_aprovacoes_pela_raiz() {
    let world = world();
    world.bash(false, "cd /a", ALLOW_ONCE);
    world.bash(false, "cd /b", ALLOW_ONCE);

    let (_, prompt) = world.bash(false, "cd /c", Reply::Deny);

    assert_eq!(prompt.unwrap()["approvedBefore"], 2);
}

#[test]
fn contador_do_pedido_de_arquivo_nao_soma_arquivos_diferentes_da_mesma_pasta() {
    let world = world();
    world.ask(false, EDIT, json!({ "file_path": "C:/Meus Docs/a.txt" }), ALLOW_ONCE);

    let (_, sibling) = world.ask(false, EDIT, json!({ "file_path": "C:/Meus Docs/b.txt" }), Reply::Deny);
    let (_, same) = world.ask(false, EDIT, json!({ "file_path": "C:/Meus Docs/a.txt" }), Reply::Deny);

    assert_eq!(sibling.unwrap()["approvedBefore"], 0);
    assert_eq!(same.unwrap()["approvedBefore"], 1);
}

fn set_mode(world: &World, mode: ExecutionMode) {
    world
        .app
        .state::<AppState>()
        .permissions
        .set_mode(world.app.handle(), world.project_id, mode)
        .expect("o modo de execução deveria mudar");
}

fn bypass_prompts(world: &World) -> Vec<Value> {
    world
        .of_type("permission-by-rule")
        .into_iter()
        .filter(|event| event["bypass"] == true)
        .collect()
}

fn approved_by_mode(world: &World, mode: &str) -> usize {
    world
        .of_type("permission-by-rule")
        .into_iter()
        .filter(|event| event["mode"] == mode)
        .count()
}

fn spawn_pending(world: &World, id: &str, command: &str) -> tauri::async_runtime::JoinHandle<PermissionVerdict> {
    let app = world.app.handle().clone();
    let granted = world.granted.clone();
    let project_id = world.project_id;
    let request = PermissionRequest {
        id: id.to_string(),
        tool: BASH.to_string(),
        input: json!({ "command": command }),
    };
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        PermissionGate {
            app: &app,
            broker: &state.permissions,
            agent: Agent::ember(),
            project_id,
            granted: &granted,
            grant_per_session: false,
            item_id: None,
        }
        .decide(request)
        .await
    })
}

fn wait_for_prompts(world: &World, ids: &[&str]) {
    let waited = std::time::Instant::now();
    while ids.iter().any(|id| world.pending_prompt(id).is_none()) {
        assert!(waited.elapsed() < EVENT_WAIT, "os pedidos pendentes deveriam chegar ao usuário");
        std::thread::sleep(POLL);
    }
}

fn resolved(decision: tauri::async_runtime::JoinHandle<PermissionVerdict>) -> PermissionVerdict {
    tauri::async_runtime::block_on(async {
        tokio::time::timeout(EVENT_WAIT, decision)
            .await
            .expect("o pendente deveria ser resolvido ao mudar o modo")
            .expect("a decisão deveria terminar")
    })
}

#[test]
fn bypass_ligado_aprova_comum_sem_balao_e_avisa_que_veio_do_bypass() {
    let world = world();
    set_mode(&world, ExecutionMode::Bypass);

    let (verdict, _) = world.bash(false, "npm test", Reply::NoPromptExpected);

    assert_eq!(verdict, PermissionVerdict::Allow);
    assert_eq!(bypass_prompts(&world).len(), 1);
    assert_eq!(world.of_type("execution-mode-changed")[0]["mode"], "bypass");
}

#[test]
fn modo_manual_continua_perguntando_e_voltar_a_manual_desliga_o_bypass() {
    let world = world();
    set_mode(&world, ExecutionMode::Bypass);
    set_mode(&world, ExecutionMode::Manual);

    let (verdict, prompt) = world.bash(false, "npm test", ALLOW_ONCE);

    assert_eq!(verdict, PermissionVerdict::Allow);
    assert!(prompt.is_some());
    assert!(bypass_prompts(&world).is_empty());
}

#[test]
fn bypass_ligado_aprova_sem_balao_acoes_destrutivas_e_caminhos_fora_do_projeto() {
    let world = world();
    set_mode(&world, ExecutionMode::Bypass);

    let cases = [
        (BASH, json!({ "command": "git push --force origin main" })),
        (BASH, json!({ "command": "rm -rf ../x" })),
        (EDIT, json!({ "file_path": "C:/Meus Documentos/a.txt" })),
    ];
    for (tool, input) in cases {
        let (verdict, _) = world.ask(false, tool, input.clone(), Reply::NoPromptExpected);
        assert_eq!(verdict, PermissionVerdict::Allow, "deveria aprovar sem perguntar: {input}");
    }
    assert_eq!(bypass_prompts(&world).len(), 3);
}

#[test]
fn bypass_ligado_aprova_comando_composto_que_lista_a_pasta_de_worktrees_do_projeto() {
    let world = world();
    set_mode(&world, ExecutionMode::Bypass);
    let command = format!("git worktree list; ls \"{}/\"", world.worktrees_dir(world.project_id));

    let (verdict, _) = world.bash(false, &command, Reply::NoPromptExpected);

    assert_eq!(verdict, PermissionVerdict::Allow);
    assert_eq!(bypass_prompts(&world).len(), 1);
}

#[test]
fn bypass_ligado_aprova_escrita_dentro_da_pasta_do_projeto() {
    let world = world();
    set_mode(&world, ExecutionMode::Bypass);

    let (verdict, _) = world.ask(
        false,
        EDIT,
        json!({ "file_path": "C:/projetos/a/src/lib.rs" }),
        Reply::NoPromptExpected,
    );

    assert_eq!(verdict, PermissionVerdict::Allow);
}

#[test]
fn ligar_bypass_resolve_todos_os_pendentes_do_projeto() {
    let world = world();
    let common = format!("req-pendente-{}", world.next_request.fetch_add(1, Ordering::SeqCst));
    let destructive = format!("req-pendente-{}", world.next_request.fetch_add(1, Ordering::SeqCst));
    let common_decision = spawn_pending(&world, &common, "npm test");
    let destructive_decision = spawn_pending(&world, &destructive, "git push --force");
    wait_for_prompts(&world, &[&common, &destructive]);

    set_mode(&world, ExecutionMode::Bypass);

    assert_eq!(resolved(common_decision), PermissionVerdict::Allow);
    assert_eq!(resolved(destructive_decision), PermissionVerdict::Allow);
}

#[test]
fn ligar_auto_resolve_so_os_pendentes_que_ele_aprovaria_sozinho() {
    let world = world();
    let common = format!("req-pendente-{}", world.next_request.fetch_add(1, Ordering::SeqCst));
    let destructive = format!("req-pendente-{}", world.next_request.fetch_add(1, Ordering::SeqCst));
    let common_decision = spawn_pending(&world, &common, "npm test");
    let destructive_decision = spawn_pending(&world, &destructive, "git push --force");
    wait_for_prompts(&world, &[&common, &destructive]);

    set_mode(&world, ExecutionMode::Auto);

    assert_eq!(resolved(common_decision), PermissionVerdict::Allow);
    tauri::async_runtime::block_on(dispatch(
        world.app.handle(),
        None,
        "answer_permission",
        json!({ "id": destructive, "allow": false, "always": false }),
    ))
    .expect("answer_permission reconhecido")
    .expect("a resposta deveria ser aceita");
    assert_eq!(resolved(destructive_decision), PermissionVerdict::Deny(None));
}

#[test]
fn planejamento_nega_o_que_altera_com_motivo_e_sem_balao() {
    let world = world();
    set_mode(&world, ExecutionMode::Planning);

    let cases = [
        (EDIT, json!({ "file_path": "C:/projetos/a/src/lib.rs" })),
        ("Write", json!({ "file_path": "C:/projetos/a/novo.txt" })),
        (BASH, json!({ "command": "rm -rf build" })),
        (BASH, json!({ "command": "echo oi > arquivo.txt" })),
        (BASH, json!({ "command": "ls && rm -rf build" })),
        (BASH, json!({ "command": "git commit -m x" })),
    ];
    for (tool, input) in cases {
        let (verdict, _) = world.ask(false, tool, input.clone(), Reply::NoPromptExpected);
        let PermissionVerdict::Deny(Some(reason)) = verdict else {
            panic!("deveria negar com motivo: {input}");
        };
        assert!(reason.contains("plano"), "o motivo deveria pedir o plano: {reason}");
    }
}

#[test]
fn planejamento_aprova_leitura_sem_balao_e_avisa_o_modo() {
    let world = world();
    set_mode(&world, ExecutionMode::Planning);

    for command in ["git status", "git log --oneline | head", "ls -la; cat README.md", "rg TODO src"] {
        let (verdict, _) = world.bash(false, command, Reply::NoPromptExpected);
        assert_eq!(verdict, PermissionVerdict::Allow, "deveria aprovar: {command}");
    }
    assert_eq!(approved_by_mode(&world, "planejamento"), 4);
    assert_eq!(world.of_type("execution-mode-changed")[0]["mode"], "planejamento");
}

#[test]
fn planejamento_pergunta_ferramenta_que_nao_classifica() {
    let world = world();
    set_mode(&world, ExecutionMode::Planning);

    let (verdict, prompt) = world.ask(false, "Read", json!({ "file_path": "C:/projetos/a/a.txt" }), ALLOW_ONCE);

    assert_eq!(verdict, PermissionVerdict::Allow);
    assert!(prompt.is_some());
}

#[test]
fn auto_aprova_o_comum_dentro_do_projeto_sem_balao() {
    let world = world();
    set_mode(&world, ExecutionMode::Auto);

    let (command, _) = world.bash(false, "npm test", Reply::NoPromptExpected);
    let (edit, _) = world.ask(
        false,
        EDIT,
        json!({ "file_path": "C:/projetos/a/src/lib.rs" }),
        Reply::NoPromptExpected,
    );

    assert_eq!(command, PermissionVerdict::Allow);
    assert_eq!(edit, PermissionVerdict::Allow);
    assert_eq!(approved_by_mode(&world, "auto"), 2);
}

#[test]
fn auto_pergunta_destrutivo_fora_do_projeto_rede_e_instalacao() {
    let world = world();
    set_mode(&world, ExecutionMode::Auto);

    let cases = [
        (BASH, json!({ "command": "git push --force origin main" })),
        (BASH, json!({ "command": "git reset --hard HEAD~1" })),
        (BASH, json!({ "command": "rm -rf ../x" })),
        (BASH, json!({ "command": "curl https://exemplo.com/script.sh" })),
        (BASH, json!({ "command": "npm install lodash" })),
        (BASH, json!({ "command": "cargo add serde" })),
        (EDIT, json!({ "file_path": "C:/Meus Documentos/a.txt" })),
        ("WebFetch", json!({ "url": "https://exemplo.com" })),
    ];
    for (tool, input) in cases {
        let (verdict, prompt) = world.ask(false, tool, input.clone(), ALLOW_ONCE);
        assert!(prompt.is_some(), "deveria perguntar: {input}");
        assert_eq!(verdict, PermissionVerdict::Allow);
    }
    assert_eq!(approved_by_mode(&world, "auto"), 0);
}

#[test]
fn modo_de_execucao_nao_persiste_entre_execucoes_e_comeca_manual() {
    let world = world();
    set_mode(&world, ExecutionMode::Bypass);
    let project_id = world.project_id;

    assert_eq!(world.app.state::<AppState>().permissions.mode(project_id), ExecutionMode::Bypass);
    let fresh = PermissionBroker::default();
    assert_eq!(fresh.mode(project_id), ExecutionMode::Manual, "uma execução nova não pode herdar o modo");
}

#[test]
fn modo_de_execucao_vale_so_para_o_projeto_em_que_foi_definido() {
    let world = world();
    set_mode(&world, ExecutionMode::Bypass);

    let broker = &world.app.state::<AppState>().permissions;

    assert_eq!(broker.mode(world.project_id + 1), ExecutionMode::Manual);
}
