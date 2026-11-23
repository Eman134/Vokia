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

fn set_bypass(world: &World, enabled: bool) {
    world
        .app
        .state::<AppState>()
        .permissions
        .set_bypass(world.app.handle(), world.project_id, enabled)
        .expect("o modo bypass deveria mudar");
}

fn bypass_prompts(world: &World) -> Vec<Value> {
    world
        .of_type("permission-by-rule")
        .into_iter()
        .filter(|event| event["bypass"] == true)
        .collect()
}

#[test]
fn bypass_ligado_aprova_comum_sem_balao_e_avisa_que_veio_do_bypass() {
    let world = world();
    set_bypass(&world, true);

    let (verdict, _) = world.bash(false, "npm test", Reply::NoPromptExpected);

    assert_eq!(verdict, PermissionVerdict::Allow);
    assert_eq!(bypass_prompts(&world).len(), 1);
    assert_eq!(world.of_type("bypass-changed")[0]["enabled"], true);
}

#[test]
fn bypass_desligado_continua_perguntando() {
    let world = world();

    let (verdict, prompt) = world.bash(false, "npm test", ALLOW_ONCE);

    assert_eq!(verdict, PermissionVerdict::Allow);
    assert!(prompt.is_some());
    assert!(bypass_prompts(&world).is_empty());
}

#[test]
fn bypass_ligado_mantem_perguntando_as_acoes_destrutivas_e_os_caminhos_fora_do_projeto() {
    let world = world();
    set_bypass(&world, true);

    let cases = [
        (BASH, json!({ "command": "git push --force origin main" })),
        (BASH, json!({ "command": "git reset --hard" })),
        (BASH, json!({ "command": "rm -rf C:/outra/pasta" })),
        (BASH, json!({ "command": "cat ../segredo.txt" })),
        (EDIT, json!({ "file_path": "C:/Meus Documentos/a.txt" })),
    ];
    for (tool, input) in cases {
        let (verdict, prompt) = world.ask(false, tool, input.clone(), ALLOW_ONCE);
        assert!(prompt.is_some(), "deveria perguntar: {input}");
        assert_eq!(verdict, PermissionVerdict::Allow);
    }
}

#[test]
fn bypass_ligado_aprova_comando_composto_que_lista_a_pasta_de_worktrees_do_projeto() {
    let world = world();
    set_bypass(&world, true);
    let command = format!("git worktree list; ls \"{}/\"", world.worktrees_dir(world.project_id));

    let (verdict, _) = world.bash(false, &command, Reply::NoPromptExpected);

    assert_eq!(verdict, PermissionVerdict::Allow);
    assert_eq!(bypass_prompts(&world).len(), 1);
}

#[test]
fn bypass_ligado_continua_perguntando_a_pasta_de_worktrees_de_outro_projeto() {
    let world = world();
    set_bypass(&world, true);
    let other = world.worktrees_dir(world.project_id + 1);

    let (_, prompt) = world.bash(false, &format!("ls \"{other}/\""), ALLOW_ONCE);

    assert!(prompt.is_some());
}

#[test]
fn bypass_ligado_aprova_escrita_dentro_da_pasta_do_projeto() {
    let world = world();
    set_bypass(&world, true);

    let (verdict, _) = world.ask(
        false,
        EDIT,
        json!({ "file_path": "C:/projetos/a/src/lib.rs" }),
        Reply::NoPromptExpected,
    );

    assert_eq!(verdict, PermissionVerdict::Allow);
}

#[test]
fn ligar_bypass_resolve_pendentes_nao_destrutivos_e_deixa_destrutivo_para_o_usuario() {
    let world = world();
    let common = format!("req-pendente-{}", world.next_request.fetch_add(1, Ordering::SeqCst));
    let destructive = format!("req-pendente-{}", world.next_request.fetch_add(1, Ordering::SeqCst));
    let spawn = |id: String, command: &str| {
        let app = world.app.handle().clone();
        let granted = world.granted.clone();
        let project_id = world.project_id;
        let request = PermissionRequest {
            id,
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
    };
    let common_decision = spawn(common.clone(), "npm test");
    let destructive_decision = spawn(destructive.clone(), "git push --force");
    let waited = std::time::Instant::now();
    while world.pending_prompt(&common).is_none() || world.pending_prompt(&destructive).is_none() {
        assert!(waited.elapsed() < EVENT_WAIT, "os pedidos pendentes deveriam chegar ao usuário");
        std::thread::sleep(POLL);
    }

    set_bypass(&world, true);

    let common_verdict = tauri::async_runtime::block_on(async {
        tokio::time::timeout(EVENT_WAIT, common_decision)
            .await
            .expect("o pendente comum deveria ser resolvido ao ligar")
            .expect("a decisão deveria terminar")
    });
    assert_eq!(common_verdict, PermissionVerdict::Allow);

    tauri::async_runtime::block_on(dispatch(
        world.app.handle(),
        None,
        "answer_permission",
        json!({ "id": destructive, "allow": false, "always": false }),
    ))
    .expect("answer_permission reconhecido")
    .expect("a resposta deveria ser aceita");
    let destructive_verdict = tauri::async_runtime::block_on(destructive_decision).expect("a decisão deveria terminar");
    assert_eq!(destructive_verdict, PermissionVerdict::Deny(None));
}

#[test]
fn bypass_nao_persiste_entre_execucoes_e_comeca_desligado() {
    let world = world();
    set_bypass(&world, true);
    let project_id = world.project_id;

    assert!(world.app.state::<AppState>().permissions.bypass_enabled(project_id));
    let fresh = PermissionBroker::default();
    assert!(!fresh.bypass_enabled(project_id), "uma execução nova não pode herdar o bypass");
}

#[test]
fn deteccao_de_acao_destrutiva_separa_o_que_pede_aprovacao() {
    use crate::permissions::is_destructive;
    let root = Some("C:/projetos/a");
    let worktrees = Some("C:/dados/worktrees/1");
    let destructive = |input: &Value| is_destructive(input, root, worktrees);
    let command = |text: &str| json!({ "command": text });

    assert!(!destructive(&command("npm test")));
    assert!(!destructive(&command("git push origin main")));
    assert!(!destructive(&command("rm -rf build")));
    assert!(!destructive(&command("rm -rf C:/projetos/a/dist")));
    assert!(destructive(&command("git push --force origin main")));
    assert!(destructive(&command("git push -fu origin main")));
    assert!(destructive(&command("git reset --hard HEAD~1")));
    assert!(destructive(&command("git clean -fd")));
    assert!(destructive(&command("rm -rf C:/outra/pasta")));
    assert!(destructive(&command("cat ../segredo.txt")));
    assert!(destructive(&json!({ "file_path": "C:/Meus Documentos/a.txt" })));
    assert!(!destructive(&json!({ "file_path": "C:/projetos/a/src/lib.rs" })));
    assert!(!destructive(&command("ls \"C:/dados/worktrees/1/\"")));
    assert!(!destructive(&json!({ "file_path": "C:/dados/worktrees/1/item-119/src/lib.rs" })));
    assert!(destructive(&command("ls C:/dados/worktrees/2/")));
    assert!(is_destructive(&command("npm test"), None, worktrees), "sem pasta do projeto nada é seguro");
}
