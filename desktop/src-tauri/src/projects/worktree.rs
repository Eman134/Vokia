use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex as StdMutex, PoisonError};
use std::time::Duration;

use tokio::sync::Mutex as AsyncMutex;

use super::process::{run, tail, RunError, GIT_MISSING_MESSAGE, GIT_PROGRAM};

// Uma trava por pasta de card: réplicas do mesmo card esperam uma só criação; cards diferentes não se bloqueiam.
static ITEM_LOCKS: LazyLock<StdMutex<HashMap<PathBuf, Arc<AsyncMutex<()>>>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

const WORKTREES_DIR: &str = "worktrees";
const BRANCH_PREFIX: &str = "vokia/";
const SETUP_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_PATH_CHARS: usize = 260;

#[cfg(windows)]
const SHELL: (&str, &str) = ("cmd", "/C");
#[cfg(not(windows))]
const SHELL: (&str, &str) = ("sh", "-c");

#[derive(Debug)]
pub enum WorktreeError {
    GitNotInstalled(String),
    NoBaseBranch,
    NameConflict(String),
    PathTooLong(String),
    Git(String),
    Io(String),
    SetupFailed { path: PathBuf, output: String },
    SetupTimedOut(PathBuf),
    UncommittedChanges(PathBuf),
}

impl fmt::Display for WorktreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorktreeError::GitNotInstalled(message) => f.write_str(message),
            WorktreeError::NoBaseBranch => {
                f.write_str("O projeto não está numa branch; o worktree precisa de uma branch base.")
            }
            WorktreeError::NameConflict(name) => write!(
                f,
                "Outro item já usa o nome {name} para branch e pasta; nada foi criado."
            ),
            WorktreeError::PathTooLong(path) => {
                write!(f, "O caminho {path} passa de {MAX_PATH_CHARS} caracteres.")
            }
            WorktreeError::Git(output) => write!(f, "O git recusou o comando:\n{output}"),
            WorktreeError::Io(message) => {
                write!(f, "Não foi possível criar a pasta dos worktrees: {message}")
            }
            WorktreeError::SetupFailed { path, output } => write!(
                f,
                "O comando de preparo falhou em {}; o worktree foi mantido.\n{output}",
                path.display()
            ),
            WorktreeError::SetupTimedOut(path) => write!(
                f,
                "O comando de preparo passou de {} s em {}; o worktree foi mantido.",
                SETUP_TIMEOUT.as_secs(),
                path.display()
            ),
            WorktreeError::UncommittedChanges(path) => write!(
                f,
                "{} tem alterações não commitadas; o worktree foi mantido.",
                path.display()
            ),
        }
    }
}

pub struct Worktree {
    #[cfg_attr(not(test), allow(dead_code))] // lido só nos testes
    pub branch: String,
    pub path: PathBuf,
}


pub enum RemoveOutcome {
    Removed,
    BranchKept { commits: u32 },
}

// Id com caractere fora de [A-Za-z0-9._-] vira "-" para caber em nome de branch e de pasta.
fn safe_name(item_id: &str) -> String {
    item_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn find_git() -> Result<PathBuf, WorktreeError> {
    which::which(GIT_PROGRAM)
        .map_err(|_| WorktreeError::GitNotInstalled(GIT_MISSING_MESSAGE.to_string()))
}

fn spawn_error(error: RunError) -> WorktreeError {
    match error {
        RunError::Spawn(error) => {
            WorktreeError::GitNotInstalled(format!("Não foi possível executar o git: {error}"))
        }
        RunError::TimedOut => WorktreeError::Git("O git não respondeu.".to_string()),
    }
}

async fn git(git: &Path, dir: &Path, args: &[&str]) -> Result<String, WorktreeError> {
    let output = run(git, args, Some(dir), None).await.map_err(spawn_error)?;
    if !output.status.success() {
        return Err(WorktreeError::Git(tail(&output)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn branch_exists(git_path: &Path, dir: &Path, branch: &str) -> Result<bool, WorktreeError> {
    let reference = format!("refs/heads/{branch}");
    let output = run(
        git_path,
        &["rev-parse", "--verify", "--quiet", &reference],
        Some(dir),
        None,
    )
    .await
    .map_err(spawn_error)?;
    Ok(output.status.success())
}

async fn run_setup(path: &Path, command: &str) -> Result<(), WorktreeError> {
    let output = run(
        Path::new(SHELL.0),
        &[SHELL.1, command],
        Some(path),
        Some(SETUP_TIMEOUT),
    )
    .await
    .map_err(|error| match error {
        RunError::TimedOut => WorktreeError::SetupTimedOut(path.to_path_buf()),
        RunError::Spawn(error) => WorktreeError::SetupFailed {
            path: path.to_path_buf(),
            output: error.to_string(),
        },
    })?;
    if output.status.success() {
        return Ok(());
    }
    Err(WorktreeError::SetupFailed {
        path: path.to_path_buf(),
        output: tail(&output),
    })
}

// Pasta onde ficam os worktrees de um projeto; o agente trabalha e lista dentro dela.
pub(crate) fn project_worktrees_dir(data_dir: &Path, project_id: i64) -> PathBuf {
    data_dir.join(WORKTREES_DIR).join(project_id.to_string())
}

pub async fn create(
    project_dir: &Path,
    worktrees_root: &Path,
    item_id: &str,
    setup_command: Option<&str>,
) -> Result<Worktree, WorktreeError> {
    let git_path = find_git()?;
    let name = safe_name(item_id);
    let branch = format!("{BRANCH_PREFIX}{name}");
    let path = worktrees_root.join(&name);
    if path.as_os_str().len() > MAX_PATH_CHARS {
        return Err(WorktreeError::PathTooLong(path.display().to_string()));
    }
    if path.exists() || branch_exists(&git_path, project_dir, &branch).await? {
        return Err(WorktreeError::NameConflict(name));
    }
    let base = git(&git_path, project_dir, &["branch", "--show-current"]).await?;
    if base.is_empty() {
        return Err(WorktreeError::NoBaseBranch);
    }
    std::fs::create_dir_all(worktrees_root).map_err(|error| WorktreeError::Io(error.to_string()))?;
    let path_arg = path.to_string_lossy();
    git(
        &git_path,
        project_dir,
        &["worktree", "add", "-b", &branch, &path_arg, &base],
    )
    .await?;
    if let Some(command) = setup_command.map(str::trim).filter(|text| !text.is_empty()) {
        // A pasta que fica depois de um preparo falho seria reaproveitada pelo ensure como se estivesse pronta.
        if let Err(error) = run_setup(&path, command).await {
            let rollback_steps: [&[&str]; 2] = [
                &["worktree", "remove", "--force", &path_arg],
                &["branch", "-D", &branch],
            ];
            for args in rollback_steps {
                if let Err(rollback) = git(&git_path, project_dir, args).await {
                    eprintln!("Não foi possível desfazer o worktree de {item_id} após o preparo falhar: {rollback}");
                }
            }
            return Err(error);
        }
    }
    Ok(Worktree { branch, path })
}

fn item_lock(path: &Path) -> Arc<AsyncMutex<()>> {
    let mut locks = ITEM_LOCKS.lock().unwrap_or_else(PoisonError::into_inner);
    Arc::clone(locks.entry(path.to_path_buf()).or_default())
}

// Mesmo card, mesma pasta: a trava faz a segunda chamada esperar a primeira; depois reaproveita o worktree criado.
pub async fn ensure(
    project_dir: &Path,
    worktrees_root: &Path,
    item_id: &str,
    setup_command: Option<&str>,
) -> Result<Worktree, WorktreeError> {
    let name = safe_name(item_id);
    let path = worktrees_root.join(&name);
    let lock = item_lock(&path);
    let _guard = lock.lock().await;
    if path.is_dir() {
        return Ok(Worktree {
            branch: format!("{BRANCH_PREFIX}{name}"),
            path,
        });
    }
    create(project_dir, worktrees_root, item_id, setup_command).await
}

// Só sai o que está integrado à base e sem alterações: trabalho não commitado ou commit fora da base devolve a
// decisão a quem pediu, e pasta e branch ficam como estavam.
pub async fn remove(
    project_dir: &Path,
    worktree_path: &Path,
    branch: &str,
    base: &str,
) -> Result<RemoveOutcome, WorktreeError> {
    let git_path = find_git()?;
    let worktree_exists = worktree_path.is_dir();
    if worktree_exists {
        let pending = git(&git_path, worktree_path, &["status", "--porcelain"]).await?;
        if !pending.is_empty() {
            return Err(WorktreeError::UncommittedChanges(worktree_path.to_path_buf()));
        }
    }
    let commits = if branch_exists(&git_path, project_dir, branch).await? {
        let range = format!("{base}..{branch}");
        let count = git(&git_path, project_dir, &["rev-list", "--count", &range]).await?;
        count
            .parse::<u32>()
            .map_err(|_| WorktreeError::Git(format!("contagem de commits inválida: {count}")))?
    } else {
        0
    };
    if commits > 0 {
        return Ok(RemoveOutcome::BranchKept { commits });
    }
    if worktree_exists {
        let path_arg = worktree_path.to_string_lossy();
        git(
            &git_path,
            project_dir,
            &["worktree", "remove", "--force", &path_arg],
        )
        .await?;
    }
    if branch_exists(&git_path, project_dir, branch).await? {
        git(&git_path, project_dir, &["branch", "-D", branch]).await?;
    }
    Ok(RemoveOutcome::Removed)
}

// Resultado da limpeza feita ao concluir um card: nada havia, saiu, ou ficou com o motivo.
#[derive(Debug, PartialEq, Eq)]
pub enum CleanupOutcome {
    Absent,
    Removed,
    Kept(String),
}

// Limpa o worktree e a branch vokia/<item> de um card concluído, contra a branch atual do projeto (a base).
// Nunca apaga trabalho: pendência e commit fora da base mantêm tudo e voltam como motivo.
pub async fn cleanup_done(
    project_dir: &Path,
    worktrees_root: &Path,
    item_id: &str,
) -> Result<CleanupOutcome, WorktreeError> {
    let name = safe_name(item_id);
    let path = worktrees_root.join(&name);
    let branch = format!("{BRANCH_PREFIX}{name}");
    let git_path = find_git()?;
    if !path.exists() && !branch_exists(&git_path, project_dir, &branch).await? {
        return Ok(CleanupOutcome::Absent);
    }
    let base = git(&git_path, project_dir, &["branch", "--show-current"]).await?;
    if base.is_empty() {
        return Ok(CleanupOutcome::Kept(
            "o projeto não está numa branch base".to_string(),
        ));
    }
    match remove(project_dir, &path, &branch, &base).await {
        Ok(RemoveOutcome::Removed) => Ok(CleanupOutcome::Removed),
        Ok(RemoveOutcome::BranchKept { commits }) => Ok(CleanupOutcome::Kept(format!(
            "a branch {branch} tem {commits} commit(s) que não estão em {base}"
        ))),
        Err(WorktreeError::UncommittedChanges(_)) => Ok(CleanupOutcome::Kept(
            "o worktree tem alterações não commitadas".to_string(),
        )),
        Err(error) => Err(error),
    }
}

// Varredura de worktrees mortos: poda referências órfãs no git e limpa pastas cujo card já foi concluído, arquivado ou removido.
pub async fn sweep_dead_worktrees(
    project_dir: &Path,
    worktrees_root: &Path,
    active_item_ids: &HashSet<String>,
) -> Result<Vec<String>, WorktreeError> {
    if !project_dir.is_dir() {
        return Ok(Vec::new());
    }
    let git_path = find_git()?;
    let _ = git(&git_path, project_dir, &["worktree", "prune"]).await;
    if !worktrees_root.is_dir() {
        return Ok(Vec::new());
    }
    let active_safe: HashSet<String> = active_item_ids.iter().map(|id| safe_name(id)).collect();
    let entries = match std::fs::read_dir(worktrees_root) {
        Ok(entries) => entries,
        Err(_) => return Ok(Vec::new()),
    };
    let mut removed = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if active_safe.contains(&name) {
            continue;
        }
        if matches!(cleanup_done(project_dir, worktrees_root, &name).await, Ok(CleanupOutcome::Removed)) {
            removed.push(name);
        }
    }
    let _ = git(&git_path, project_dir, &["worktree", "prune"]).await;
    Ok(removed)
}

#[cfg(test)]
#[path = "../../tests/worktree_tests.rs"]
mod tests;
