use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

static NEXT_DIR: AtomicU32 = AtomicU32::new(0);
const MAIN_BRANCH: &str = "main";
const SHARED_FILE: &str = "shared.txt";
const ORIGINAL: &str = "original\n";

struct Sandbox {
    root: PathBuf,
    project: PathBuf,
    worktrees: PathBuf,
}

impl Sandbox {
    fn new() -> Sandbox {
        let root = std::env::temp_dir().join(format!(
            "vokia-worktree-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::SeqCst)
        ));
        let project = root.join("projeto");
        fs::create_dir_all(&project).expect("pasta do projeto");
        Sandbox {
            worktrees: root.join("worktrees"),
            project,
            root,
        }
    }

    fn with_repo() -> Sandbox {
        let sandbox = Sandbox::new();
        git_in(&sandbox.project, &["init", "-b", MAIN_BRANCH]);
        git_in(&sandbox.project, &["config", "core.autocrlf", "false"]);
        fs::write(sandbox.project.join(SHARED_FILE), ORIGINAL).expect("arquivo base");
        git_in(&sandbox.project, &["add", "."]);
        commit(&sandbox.project, "base");
        sandbox
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn git_in(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git instalado");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn commit(dir: &Path, message: &str) {
    git_in(
        dir,
        &[
            "-c",
            "user.name=Teste",
            "-c",
            "user.email=teste@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            message,
        ],
    );
}

fn create_item(sandbox: &Sandbox, item: &str) -> Result<Worktree, WorktreeError> {
    tauri::async_runtime::block_on(create(&sandbox.project, &sandbox.worktrees, item, None))
}

fn ensure_item(sandbox: &Sandbox, item: &str) -> Result<Worktree, WorktreeError> {
    tauri::async_runtime::block_on(ensure(&sandbox.project, &sandbox.worktrees, item, None))
}

fn remove_item(sandbox: &Sandbox, worktree: &Worktree) -> Result<RemoveOutcome, WorktreeError> {
    tauri::async_runtime::block_on(remove(
        &sandbox.project,
        &worktree.path,
        &worktree.branch,
        MAIN_BRANCH,
    ))
}

fn branches(sandbox: &Sandbox) -> Vec<String> {
    git_in(&sandbox.project, &["branch", "--format=%(refname:short)"])
        .lines()
        .map(str::to_string)
        .collect()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).expect("arquivo legível")
}

#[test]
fn criar_gera_branch_e_pasta_do_item_sem_tocar_na_pasta_principal() {
    let sandbox = Sandbox::with_repo();

    let worktree = create_item(&sandbox, "card-1").expect("worktree");

    assert_eq!(worktree.branch, "vokia/card-1");
    assert_eq!(worktree.path, sandbox.worktrees.join("card-1"));
    assert_eq!(
        git_in(&worktree.path, &["branch", "--show-current"]),
        "vokia/card-1"
    );
    assert_eq!(read(&worktree.path.join(SHARED_FILE)), ORIGINAL);
    assert_eq!(
        git_in(&sandbox.project, &["branch", "--show-current"]),
        MAIN_BRANCH
    );
    assert_eq!(git_in(&sandbox.project, &["status", "--porcelain"]), "");
}

#[test]
fn id_com_caracteres_fora_do_permitido_vira_nome_seguro_de_branch_e_pasta() {
    let sandbox = Sandbox::with_repo();

    let worktree = create_item(&sandbox, "feat/login x:y").expect("worktree");

    assert_eq!(worktree.branch, "vokia/feat-login-x-y");
    assert_eq!(worktree.path, sandbox.worktrees.join("feat-login-x-y"));
    assert!(worktree.path.is_dir());
}

#[test]
fn mesmo_id_duas_vezes_e_conflito_e_a_primeira_pasta_continua_intacta() {
    let sandbox = Sandbox::with_repo();
    let first = create_item(&sandbox, "card-1").expect("worktree");

    let second = create_item(&sandbox, "card-1");

    assert!(
        matches!(&second, Err(WorktreeError::NameConflict(name)) if name == "card-1"),
        "{:?}", second.as_ref().err()
    );
    assert!(first.path.is_dir());
}

#[test]
fn ids_que_viram_o_mesmo_nome_seguro_conflitam_em_vez_de_dividir_a_pasta() {
    let sandbox = Sandbox::with_repo();
    create_item(&sandbox, "a/b").expect("worktree");

    let second = create_item(&sandbox, "a b");

    assert!(
        matches!(second, Err(WorktreeError::NameConflict(_))),
        "{:?}", second.as_ref().err()
    );
}

#[test]
fn branch_ja_existente_com_o_nome_do_item_e_conflito_e_nada_e_criado() {
    let sandbox = Sandbox::with_repo();
    git_in(&sandbox.project, &["branch", "vokia/card-9"]);

    let result = create_item(&sandbox, "card-9");

    assert!(
        matches!(result, Err(WorktreeError::NameConflict(_))),
        "{:?}", result.as_ref().err()
    );
    assert!(!sandbox.worktrees.join("card-9").exists());
}

#[test]
fn caminho_acima_do_limite_e_recusado_antes_de_criar() {
    let sandbox = Sandbox::with_repo();
    let long_id = "x".repeat(300);

    let result = create_item(&sandbox, &long_id);

    assert!(
        matches!(result, Err(WorktreeError::PathTooLong(_))),
        "{:?}", result.as_ref().err()
    );
    assert!(!sandbox.worktrees.exists());
}

#[test]
fn projeto_sem_git_e_recusado_sem_criar_pasta_de_worktrees() {
    let sandbox = Sandbox::new();

    let result = create_item(&sandbox, "card-1");

    assert!(matches!(result, Err(WorktreeError::Git(_))), "{:?}", result.as_ref().err());
    assert!(!sandbox.worktrees.exists());
}

#[test]
fn ensure_reaproveita_a_pasta_do_mesmo_item_sem_recriar() {
    let sandbox = Sandbox::with_repo();
    let first = ensure_item(&sandbox, "card-1").expect("primeira");
    fs::write(first.path.join("trabalho.txt"), "em andamento").unwrap();

    let second = ensure_item(&sandbox, "card-1").expect("segunda");

    assert_eq!(second.path, first.path);
    assert_eq!(second.branch, "vokia/card-1");
    assert!(second.path.join("trabalho.txt").is_file());
}

#[test]
fn remover_worktree_limpo_e_sem_commits_apaga_pasta_e_branch() {
    let sandbox = Sandbox::with_repo();
    let worktree = create_item(&sandbox, "card-1").expect("worktree");

    let outcome = remove_item(&sandbox, &worktree).expect("remoção");

    assert!(matches!(outcome, RemoveOutcome::Removed));
    assert!(!worktree.path.exists());
    assert_eq!(branches(&sandbox), vec![MAIN_BRANCH.to_string()]);
}

#[test]
fn remover_worktree_com_commit_guarda_a_branch_e_conta_os_commits() {
    let sandbox = Sandbox::with_repo();
    let worktree = create_item(&sandbox, "card-1").expect("worktree");
    fs::write(worktree.path.join("novo.txt"), "feito").unwrap();
    git_in(&worktree.path, &["add", "."]);
    commit(&worktree.path, "trabalho do item");

    let outcome = remove_item(&sandbox, &worktree).expect("remoção");

    assert!(
        matches!(&outcome, RemoveOutcome::BranchKept { branch, commits: 1 } if branch == "vokia/card-1")
    );
    // Commit fora da base: pasta e branch ficam como estão, para o trabalho não se perder.
    assert!(worktree.path.exists());
    assert!(branches(&sandbox).contains(&"vokia/card-1".to_string()));
}

fn cleanup_done(sandbox: &Sandbox, item: &str) -> Result<CleanupOutcome, WorktreeError> {
    tauri::async_runtime::block_on(super::cleanup_done(&sandbox.project, &sandbox.worktrees, item))
}

#[test]
fn concluir_card_integrado_e_limpo_remove_pasta_e_branch() {
    let sandbox = Sandbox::with_repo();
    let worktree = create_item(&sandbox, "card-1").expect("worktree");

    let outcome = cleanup_done(&sandbox, "card-1").expect("limpeza");

    assert_eq!(outcome, CleanupOutcome::Removed);
    assert!(!worktree.path.exists());
    assert_eq!(branches(&sandbox), vec![MAIN_BRANCH.to_string()]);
}

#[test]
fn concluir_card_com_commit_fora_da_main_mantem_pasta_e_branch_com_o_motivo() {
    let sandbox = Sandbox::with_repo();
    let worktree = create_item(&sandbox, "card-2").expect("worktree");
    fs::write(worktree.path.join("novo.txt"), "feito").unwrap();
    git_in(&worktree.path, &["add", "."]);
    commit(&worktree.path, "trabalho do item");

    let outcome = cleanup_done(&sandbox, "card-2").expect("limpeza");

    assert!(
        matches!(&outcome, CleanupOutcome::Kept(reason) if reason.contains("1 commit")),
        "{outcome:?}"
    );
    assert!(worktree.path.exists());
    assert!(branches(&sandbox).contains(&"vokia/card-2".to_string()));
}

#[test]
fn concluir_card_com_alteracao_nao_commitada_mantem_tudo() {
    let sandbox = Sandbox::with_repo();
    let worktree = create_item(&sandbox, "card-3").expect("worktree");
    fs::write(worktree.path.join(SHARED_FILE), "editado\n").unwrap();

    let outcome = cleanup_done(&sandbox, "card-3").expect("limpeza");

    assert_eq!(
        outcome,
        CleanupOutcome::Kept("o worktree tem alterações não commitadas".to_string())
    );
    assert_eq!(read(&worktree.path.join(SHARED_FILE)), "editado\n");
    assert!(branches(&sandbox).contains(&"vokia/card-3".to_string()));
}

#[test]
fn concluir_card_sem_worktree_nem_branch_nao_faz_nada() {
    let sandbox = Sandbox::with_repo();

    let outcome = cleanup_done(&sandbox, "sem-worktree").expect("limpeza");

    assert_eq!(outcome, CleanupOutcome::Absent);
    assert_eq!(branches(&sandbox), vec![MAIN_BRANCH.to_string()]);
}

#[test]
fn remover_worktree_com_alteracao_nao_commitada_e_recusado_e_nada_se_perde() {
    let sandbox = Sandbox::with_repo();
    let worktree = create_item(&sandbox, "card-1").expect("worktree");
    fs::write(worktree.path.join(SHARED_FILE), "editado\n").unwrap();

    let outcome = remove_item(&sandbox, &worktree);

    assert!(
        matches!(outcome, Err(WorktreeError::UncommittedChanges(_))),
        "{:?}", outcome.as_ref().err()
    );
    assert_eq!(read(&worktree.path.join(SHARED_FILE)), "editado\n");
}

#[test]
fn dois_worktrees_editam_o_mesmo_arquivo_sem_interferir_um_no_outro_nem_na_principal() {
    let sandbox = Sandbox::with_repo();
    let first = create_item(&sandbox, "card-1").expect("primeiro");
    let second = create_item(&sandbox, "card-2").expect("segundo");

    fs::write(first.path.join(SHARED_FILE), "do card 1\n").unwrap();
    fs::write(second.path.join(SHARED_FILE), "do card 2\n").unwrap();

    assert_eq!(read(&first.path.join(SHARED_FILE)), "do card 1\n");
    assert_eq!(read(&second.path.join(SHARED_FILE)), "do card 2\n");
    assert_eq!(read(&sandbox.project.join(SHARED_FILE)), ORIGINAL);
    assert_eq!(git_in(&sandbox.project, &["status", "--porcelain"]), "");
}
