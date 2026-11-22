use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::db::{root_command, Db, NewProject};

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

const TOOL: &str = "Bash";
const NOW: u64 = 1_000;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        TempDir(std::env::temp_dir().join(format!(
            "vokia-permission-root-test-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::SeqCst)
        )))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn project(db: &Db, name: &str) -> i64 {
    db.insert_project(&NewProject {
        name,
        symbol: "folder",
        description: "",
        path: &format!("C:/projetos/{name}"),
        origin_kind: "folder",
        origin_repo: None,
    })
    .expect("o projeto deveria ser criado")
    .id
}

fn approve(db: &Db, project_id: i64, tool: &str, command: &str) {
    db.record_approval(project_id, tool, command, root_command(command), NOW).unwrap();
}

fn history(db: &Db, project_id: i64, tool: &str, command: &str) -> crate::db::ApprovalHistory {
    db.approval_history(project_id, tool, command, root_command(command)).unwrap()
}

#[test]
fn root_command_devolve_o_primeiro_token_de_comando_simples() {
    assert_eq!(root_command("cd /outro/caminho"), Some("cd"));
    assert_eq!(root_command("  git   status  "), Some("git"));
    assert_eq!(root_command("ls"), Some("ls"));
    assert_eq!(root_command("cargo test --quiet"), Some("cargo"));
}

#[test]
fn root_command_recusa_composicao_redirecionamento_e_atribuicao_de_ambiente() {
    for composed in [
        "cd a && rm -rf b",
        "cd a || rm b",
        "cd a; rm b",
        "cat x | sh",
        "sleep 1 & rm b",
        "echo $(rm b)",
        "echo `rm b`",
        "echo x > /etc/passwd",
        "cat < /etc/passwd",
        "cd a\nrm b",
        "cd a\rrm b",
        "FOO=1 make",
    ] {
        assert_eq!(root_command(composed), None, "{composed:?} não tem raiz segura");
    }
}

#[test]
fn root_command_de_texto_vazio_ou_so_espacos_e_nenhum() {
    assert_eq!(root_command(""), None);
    assert_eq!(root_command("   "), None);
}

#[test]
fn regra_de_raiz_libera_qualquer_argumento_do_mesmo_comando_e_nada_alem_dele() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let project_id = project(&db, "a");
    db.save_permission_rule(project_id, TOOL, "cd *", NOW).unwrap();

    assert!(db.has_permission_rule(project_id, TOOL, "cd /outro/caminho").unwrap());
    assert!(db.has_permission_rule(project_id, TOOL, "cd").unwrap());
    assert!(!db.has_permission_rule(project_id, TOOL, "cdx /outro").unwrap());
    assert!(!db.has_permission_rule(project_id, TOOL, "rm -rf /").unwrap());
}

#[test]
fn regra_de_raiz_nao_libera_comando_composto_nem_com_atribuicao_de_ambiente() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let project_id = project(&db, "a");
    db.save_permission_rule(project_id, TOOL, "cd *", NOW).unwrap();

    for command in ["cd a && rm -rf b", "cd a; rm b", "cd a | sh", "cd $(rm b)", "cd a\nrm b", "FOO=1 cd a"] {
        assert!(!db.has_permission_rule(project_id, TOOL, command).unwrap(), "{command:?}");
    }
}

#[test]
fn regra_de_raiz_respeita_ferramenta_e_projeto() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let project_id = project(&db, "a");
    let other_project = project(&db, "b");
    db.save_permission_rule(project_id, TOOL, "cd *", NOW).unwrap();

    assert!(!db.has_permission_rule(project_id, "Edit", "cd /x").unwrap());
    assert!(!db.has_permission_rule(other_project, TOOL, "cd /x").unwrap());
}

#[test]
fn regra_exata_continua_valendo_so_para_o_comando_completo() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let project_id = project(&db, "a");
    db.save_permission_rule(project_id, TOOL, "npm test", NOW).unwrap();
    db.save_permission_rule(project_id, TOOL, "cd a && make", NOW).unwrap();

    assert!(db.has_permission_rule(project_id, TOOL, "npm test").unwrap());
    assert!(!db.has_permission_rule(project_id, TOOL, "npm run build").unwrap());
    assert!(db.has_permission_rule(project_id, TOOL, "cd a && make").unwrap());
}

#[test]
fn aprovacoes_de_comandos_simples_somam_no_mesmo_comando_raiz() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let project_id = project(&db, "a");
    approve(&db, project_id, TOOL, "cd /a");
    approve(&db, project_id, TOOL, "cd /b");
    approve(&db, project_id, TOOL, "cd /b");

    assert_eq!(history(&db, project_id, TOOL, "cd /nunca/visto").approved, 3);
    assert_eq!(history(&db, project_id, TOOL, "ls").approved, 0);
}

#[test]
fn aprovacao_de_comando_composto_conta_pelo_texto_inteiro_e_nao_infla_a_raiz() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let project_id = project(&db, "a");
    approve(&db, project_id, TOOL, "cd /a");
    approve(&db, project_id, TOOL, "cd /a && rm -rf b");
    approve(&db, project_id, TOOL, "cd /a && rm -rf b");

    assert_eq!(history(&db, project_id, TOOL, "cd /z").approved, 1);
    assert_eq!(history(&db, project_id, TOOL, "cd /a && rm -rf b").approved, 2);
    assert_eq!(history(&db, project_id, TOOL, "cd /a && ls").approved, 0);
}

#[test]
fn historico_de_raiz_separa_ferramenta_e_projeto() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let project_id = project(&db, "a");
    let other_project = project(&db, "b");
    approve(&db, project_id, TOOL, "cd /a");

    assert_eq!(history(&db, project_id, "Edit", "cd /a").approved, 0);
    assert_eq!(history(&db, other_project, TOOL, "cd /a").approved, 0);
}

#[test]
fn negacao_continua_indexada_pelo_comando_completo() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let project_id = project(&db, "a");
    db.record_denial(project_id, TOOL, "rm -rf /tmp/x", 777).unwrap();

    assert_eq!(history(&db, project_id, TOOL, "rm -rf /tmp/x").last_denied_at, Some(777));
    assert_eq!(history(&db, project_id, TOOL, "rm -rf /tmp/y").last_denied_at, None);
}

#[test]
fn aprovar_arquivo_com_espaco_no_caminho_nao_conta_para_outro_arquivo_da_mesma_pasta() {
    let dir = TempDir::new();
    let db = Db::open(&dir.0).unwrap();
    let project_id = project(&db, "a");
    db.record_approval(project_id, "Edit", "C:/Meus Docs/a.txt", None, NOW).unwrap();

    let same = db.approval_history(project_id, "Edit", "C:/Meus Docs/a.txt", None).unwrap();
    let sibling = db.approval_history(project_id, "Edit", "C:/Meus Docs/b.txt", None).unwrap();
    let same_prefix = db.approval_history(project_id, "Edit", "C:/Meus Outros/c.txt", None).unwrap();

    assert_eq!(same.approved, 1);
    assert_eq!(sibling.approved, 0);
    assert_eq!(same_prefix.approved, 0);
}
