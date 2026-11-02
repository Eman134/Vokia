use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;
use tokio::time::timeout;

use crate::claude::base_command;
use crate::db::{Db, DbError, NewProject, Project, ORIGIN_FOLDER, ORIGIN_GITHUB};
use crate::orchestrator::FlowError;

const GIT_PROGRAM: &str = "git";
const GH_PROGRAM: &str = "gh";
const CLONE_TIMEOUT: Duration = Duration::from_secs(600);
const INSPECT_TIMEOUT: Duration = Duration::from_secs(30);
const STDERR_MAX_LINES: usize = 10;
const GITHUB_PREFIXES: [&str; 3] = [
    "https://github.com/",
    "git@github.com:",
    "ssh://git@github.com/",
];
const REPO_NOT_FOUND_MARKERS: [&str; 2] = ["Could not resolve to a Repository", "HTTP 404"];

// Espelha as chaves de desktop/src/features/projects/projectIcons.ts.
const PROJECT_ICONS: [&str; 8] = [
    "folder",
    "target",
    "triangle",
    "square",
    "diamond",
    "document",
    "diagram",
    "whiteboard",
];

static CREATE_RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Serialize)]
pub enum ErrorCode {
    GitNotInstalled,
    FolderNotFound,
    NotADirectory,
    GhNotInstalled,
    GhNotAuthenticated,
    InvalidRepoRef,
    RepoNotFound,
    GhFailed,
    DestinationExists,
    CloneFailed,
    CloneTimeout,
    BranchNotFound,
    ProjectAlreadyAdded,
    CreateInProgress,
    InvalidIcon,
    ProjectNotFound,
    ProjectBusy,
    ProjectHasNoFolder,
    Db,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectError {
    code: ErrorCode,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
}

impl ProjectError {
    fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        ProjectError {
            code,
            message: message.into(),
            path: None,
            repo: None,
            branch: None,
        }
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    fn with_repo(mut self, repo: impl Into<String>) -> Self {
        self.repo = Some(repo.into());
        self
    }

    fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }
}

impl From<DbError> for ProjectError {
    fn from(error: DbError) -> Self {
        let message = error.to_string();
        match error {
            DbError::ProjectPathTaken(path) => {
                ProjectError::new(ErrorCode::ProjectAlreadyAdded, message).with_path(path)
            }
            DbError::ProjectNotFound(_) => ProjectError::new(ErrorCode::ProjectNotFound, message),
            DbError::ProjectBusy => ProjectError::new(ErrorCode::ProjectBusy, message),
            DbError::ProjectHasNoFolder => {
                ProjectError::new(ErrorCode::ProjectHasNoFolder, message)
            }
            _ => ProjectError::new(ErrorCode::Db, message),
        }
    }
}

impl From<FlowError> for ProjectError {
    fn from(error: FlowError) -> Self {
        ProjectError::new(ErrorCode::Db, error.to_string())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderInfo {
    path: String,
    name: String,
    branch: Option<String>,
    remote_url: Option<String>,
    github_repo: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubRepoInfo {
    full_name: String,
    name: String,
    default_branch: Option<String>,
    is_fork: bool,
    url: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CloneMode {
    Link,
    Fork,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubOrigin {
    repo: String,
    mode: CloneMode,
    parent_dir: String,
    branch: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Origin {
    Folder { path: String },
    Github(GithubOrigin),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewProjectInput {
    name: String,
    symbol: String,
    description: String,
    origin: Origin,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEdit {
    name: String,
    symbol: String,
    description: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhRepoView {
    name_with_owner: String,
    name: String,
    default_branch_ref: Option<GhBranch>,
    is_fork: bool,
    url: String,
}

#[derive(Deserialize)]
struct GhBranch {
    name: String,
}

struct RepoRef {
    owner: String,
    name: String,
}

impl RepoRef {
    fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

enum RunError {
    Spawn(std::io::Error),
    TimedOut,
}

struct CreateGuard;

impl CreateGuard {
    fn acquire() -> Result<CreateGuard, ProjectError> {
        CREATE_RUNNING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map(|_| CreateGuard)
            .map_err(|_| {
                ProjectError::new(
                    ErrorCode::CreateInProgress,
                    "Já existe uma criação de projeto em andamento.",
                )
            })
    }
}

impl Drop for CreateGuard {
    fn drop(&mut self) {
        CREATE_RUNNING.store(false, Ordering::SeqCst);
    }
}

pub fn pick_folder(app: &AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .blocking_pick_folder()
        .and_then(|path| path.into_path().ok())
        .map(|path| path.to_string_lossy().into_owned())
}

pub async fn inspect_folder(raw: &str) -> Result<FolderInfo, ProjectError> {
    let dir = ensure_directory(raw)?;
    let path = dir.to_string_lossy().into_owned();
    let (branch, remote_url) = folder_git_details(&path).await;
    let name = dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());
    let github_repo = remote_url.as_deref().and_then(github_repo_of);
    Ok(FolderInfo {
        path,
        name,
        branch,
        remote_url,
        github_repo,
    })
}

// Pasta local não precisa de git: qualquer falha aqui só deixa ramo e remoto vazios.
async fn folder_git_details(path: &str) -> (Option<String>, Option<String>) {
    let Ok(git) = locate(
        GIT_PROGRAM,
        ErrorCode::GitNotInstalled,
        "O git não está instalado ou não está no PATH.",
    ) else {
        return (None, None);
    };
    let at_root = matches!(
        git_output(&git, path, &["rev-parse", "--show-prefix"]).await,
        Ok(Some(prefix)) if prefix.is_empty()
    );
    if !at_root {
        return (None, None);
    }
    let branch = git_output(&git, path, &["branch", "--show-current"])
        .await
        .ok()
        .flatten()
        .filter(|branch| !branch.is_empty());
    let remote_url = git_output(&git, path, &["remote", "get-url", "origin"])
        .await
        .ok()
        .flatten();
    (branch, remote_url)
}

pub async fn inspect_github_repo(raw: &str) -> Result<GithubRepoInfo, ProjectError> {
    let gh = locate_gh()?;
    ensure_authenticated(&gh).await?;
    let repo = parse_repo_ref(raw).ok_or_else(invalid_repo_ref)?;
    let full_name = repo.full_name();
    let output = run(
        &gh,
        &[
            "repo",
            "view",
            &full_name,
            "--json",
            "nameWithOwner,name,defaultBranchRef,isFork,url",
        ],
        None,
        Some(INSPECT_TIMEOUT),
    )
    .await
    .map_err(gh_run_error)?;
    if !output.status.success() {
        let stderr = tail(&output);
        let code = if REPO_NOT_FOUND_MARKERS
            .iter()
            .any(|marker| stderr.contains(marker))
        {
            ProjectError::new(
                ErrorCode::RepoNotFound,
                format!("O repositório {full_name} não existe ou você não tem acesso."),
            )
        } else {
            ProjectError::new(ErrorCode::GhFailed, format!("O gh falhou: {stderr}"))
        };
        return Err(code.with_repo(full_name));
    }
    let view: GhRepoView = serde_json::from_slice(&output.stdout).map_err(|error| {
        ProjectError::new(
            ErrorCode::GhFailed,
            format!("Resposta inesperada do gh: {error}"),
        )
    })?;
    Ok(GithubRepoInfo {
        full_name: view.name_with_owner,
        name: view.name,
        default_branch: view.default_branch_ref.map(|branch| branch.name),
        is_fork: view.is_fork,
        url: view.url,
    })
}

fn ensure_known_icon(symbol: &str) -> Result<(), ProjectError> {
    if PROJECT_ICONS.contains(&symbol) {
        return Ok(());
    }
    Err(ProjectError::new(
        ErrorCode::InvalidIcon,
        "O ícone do projeto não está no catálogo.",
    ))
}

pub fn update_project(db: &Db, id: i64, edit: ProjectEdit) -> Result<Project, ProjectError> {
    let symbol = edit.symbol.trim();
    ensure_known_icon(symbol)?;
    Ok(db.update_project(id, &edit.name, symbol, &edit.description)?)
}

pub fn pin_project(db: &Db, id: Option<i64>) -> Result<(), ProjectError> {
    Ok(db.pin_project(id)?)
}

pub async fn create_project(db: &Db, input: NewProjectInput) -> Result<Project, ProjectError> {
    let _guard = CreateGuard::acquire()?;
    ensure_known_icon(input.symbol.trim())?;
    match &input.origin {
        Origin::Folder { path } => {
            let info = inspect_folder(path).await?;
            let project = NewProject {
                name: &input.name,
                symbol: &input.symbol,
                description: &input.description,
                path: &info.path,
                origin_kind: if info.github_repo.is_some() {
                    ORIGIN_GITHUB
                } else {
                    ORIGIN_FOLDER
                },
                origin_repo: info.github_repo.as_deref(),
            };
            Ok(db.insert_project(&project)?)
        }
        Origin::Github(origin) => create_from_github(db, &input, origin).await,
    }
}

async fn create_from_github(
    db: &Db,
    input: &NewProjectInput,
    origin: &GithubOrigin,
) -> Result<Project, ProjectError> {
    let gh = locate_gh()?;
    locate(
        GIT_PROGRAM,
        ErrorCode::GitNotInstalled,
        "O git não está instalado ou não está no PATH.",
    )?;
    let repo = parse_repo_ref(&origin.repo).ok_or_else(invalid_repo_ref)?;
    let full_name = repo.full_name();
    let parent = ensure_directory(&origin.parent_dir)?;
    let destination = parent.join(&repo.name);
    let destination_text = destination.to_string_lossy().into_owned();
    if destination.exists() {
        return Err(ProjectError::new(
            ErrorCode::DestinationExists,
            format!("A pasta de destino {destination_text} já existe."),
        )
        .with_path(destination_text));
    }
    let project = NewProject {
        name: &input.name,
        symbol: &input.symbol,
        description: &input.description,
        path: &destination_text,
        origin_kind: ORIGIN_GITHUB,
        origin_repo: Some(&full_name),
    };
    project.validate()?;
    let branch = origin
        .branch
        .as_deref()
        .map(str::trim)
        .filter(|branch| !branch.is_empty());
    if let Some(branch) = branch {
        if !is_valid_branch(branch) {
            return Err(branch_not_found(&full_name, branch));
        }
    }
    clone_repo(&gh, origin.mode, &full_name, &parent, &destination_text).await?;
    if let Some(branch) = branch {
        if let Err(error) = switch_branch(&destination_text, &full_name, branch).await {
            discard(&destination);
            return Err(error);
        }
    }
    db.insert_project(&project).map_err(|error| {
        discard(&destination);
        ProjectError::from(error)
    })
}

async fn clone_repo(
    gh: &Path,
    mode: CloneMode,
    full_name: &str,
    parent: &Path,
    destination: &str,
) -> Result<(), ProjectError> {
    let result = match mode {
        CloneMode::Link => {
            run(
                gh,
                &["repo", "clone", full_name, destination],
                None,
                Some(CLONE_TIMEOUT),
            )
            .await
        }
        CloneMode::Fork => {
            run(
                gh,
                &["repo", "fork", full_name, "--clone"],
                Some(parent),
                Some(CLONE_TIMEOUT),
            )
            .await
        }
    };
    let failure = match result {
        Ok(output) if output.status.success() => return Ok(()),
        Ok(output) => ProjectError::new(
            ErrorCode::CloneFailed,
            format!("Não foi possível clonar {full_name}: {}", tail(&output)),
        ),
        Err(RunError::TimedOut) => ProjectError::new(
            ErrorCode::CloneTimeout,
            format!("O clone de {full_name} passou de 10 minutos e foi cancelado."),
        ),
        Err(RunError::Spawn(error)) => ProjectError::new(
            ErrorCode::CloneFailed,
            format!("Não foi possível clonar {full_name}: {error}"),
        ),
    };
    discard(Path::new(destination));
    Err(failure.with_repo(full_name))
}

async fn switch_branch(
    destination: &str,
    full_name: &str,
    branch: &str,
) -> Result<(), ProjectError> {
    let git = locate(
        GIT_PROGRAM,
        ErrorCode::GitNotInstalled,
        "O git não está instalado ou não está no PATH.",
    )?;
    match git_output(&git, destination, &["switch", branch]).await? {
        Some(_) => Ok(()),
        None => Err(branch_not_found(full_name, branch)),
    }
}

fn branch_not_found(full_name: &str, branch: &str) -> ProjectError {
    ProjectError::new(
        ErrorCode::BranchNotFound,
        format!("A branch {branch} não existe em {full_name}."),
    )
    .with_repo(full_name)
    .with_branch(branch)
}

fn is_valid_branch(branch: &str) -> bool {
    !branch.starts_with('-') && !branch.chars().any(|c| c.is_whitespace() || c.is_control())
}

// O erro original é o que importa ao usuário; falha ao limpar não o substitui.
fn discard(destination: &Path) {
    let _ = fs::remove_dir_all(destination);
}

// Léxico, sem tocar no disco: caminhos equivalentes precisam gerar a mesma chave de duplicidade.
// Caminho relativo volta só aparado, para continuar sendo recusado como FolderNotFound.
// A raiz de unidade ("C:\") só é absoluta com a barra final, então ela é preservada.
fn normalize_path(raw: &str) -> String {
    let trimmed = raw.trim();
    let stripped = trimmed.trim_end_matches(['/', '\\']);
    let path = if Path::new(stripped).is_absolute() {
        Path::new(stripped)
    } else {
        Path::new(trimmed)
    };
    if !path.is_absolute() {
        return stripped.to_string();
    }
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            other => resolved.push(other.as_os_str()),
        }
    }
    resolved.to_string_lossy().into_owned()
}

pub(crate) fn ensure_directory(raw: &str) -> Result<PathBuf, ProjectError> {
    let path = normalize_path(raw);
    let dir = PathBuf::from(&path);
    if !dir.is_absolute() {
        return Err(ProjectError::new(
            ErrorCode::FolderNotFound,
            "Informe o caminho completo da pasta.",
        )
        .with_path(path));
    }
    if !dir.exists() {
        return Err(ProjectError::new(
            ErrorCode::FolderNotFound,
            format!("A pasta {path} não existe."),
        )
        .with_path(path));
    }
    if !dir.is_dir() {
        return Err(ProjectError::new(
            ErrorCode::NotADirectory,
            format!("{path} não é uma pasta."),
        )
        .with_path(path));
    }
    Ok(dir)
}

fn locate(program: &str, code: ErrorCode, message: &str) -> Result<PathBuf, ProjectError> {
    which::which(program).map_err(|_| ProjectError::new(code, message))
}

fn locate_gh() -> Result<PathBuf, ProjectError> {
    locate(
        GH_PROGRAM,
        ErrorCode::GhNotInstalled,
        "O GitHub CLI (gh) não está instalado ou não está no PATH.",
    )
}

async fn ensure_authenticated(gh: &Path) -> Result<(), ProjectError> {
    let output = run(gh, &["auth", "status"], None, Some(INSPECT_TIMEOUT))
        .await
        .map_err(gh_run_error)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ProjectError::new(
            ErrorCode::GhNotAuthenticated,
            "O GitHub CLI não está autenticado. Rode gh auth login.",
        ))
    }
}

fn invalid_repo_ref() -> ProjectError {
    ProjectError::new(
        ErrorCode::InvalidRepoRef,
        "Referência de repositório inválida: use owner/nome ou o link do GitHub.",
    )
}

fn gh_run_error(error: RunError) -> ProjectError {
    match error {
        RunError::TimedOut => {
            ProjectError::new(ErrorCode::GhFailed, "O gh não respondeu em 30 segundos.")
        }
        RunError::Spawn(error) => ProjectError::new(
            ErrorCode::GhFailed,
            format!("Não foi possível executar o gh: {error}"),
        ),
    }
}

fn parse_repo_ref(raw: &str) -> Option<RepoRef> {
    let trimmed = raw.trim().trim_end_matches('/');
    let bare = GITHUB_PREFIXES
        .iter()
        .find_map(|prefix| trimmed.strip_prefix(prefix))
        .unwrap_or(trimmed);
    let bare = bare.strip_suffix(".git").unwrap_or(bare);
    let (owner, name) = bare.split_once('/')?;
    let valid_owner = owner
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric())
        && owner.chars().all(is_ref_char);
    let valid_name =
        !name.is_empty() && name != "." && name != ".." && name.chars().all(is_ref_char);
    (valid_owner && valid_name).then(|| RepoRef {
        owner: owner.to_string(),
        name: name.to_string(),
    })
}

// Só URLs com prefixo do GitHub viram repo: caminhos relativos (`a/b`) não podem ser tratados como owner/nome.
fn github_repo_of(remote_url: &str) -> Option<String> {
    let trimmed = remote_url.trim();
    if !GITHUB_PREFIXES
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
    {
        return None;
    }
    parse_repo_ref(trimmed).map(|repo| repo.full_name())
}

fn is_ref_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')
}

async fn git_output(git: &Path, dir: &str, args: &[&str]) -> Result<Option<String>, ProjectError> {
    let mut full = vec!["-C", dir];
    full.extend_from_slice(args);
    let output = run(git, &full, None, None)
        .await
        .map_err(|error| match error {
            RunError::Spawn(error) => ProjectError::new(
                ErrorCode::GitNotInstalled,
                format!("Não foi possível executar o git: {error}"),
            ),
            RunError::TimedOut => {
                ProjectError::new(ErrorCode::GitNotInstalled, "O git não respondeu.")
            }
        })?;
    Ok(output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string()))
}

async fn run(
    program: &Path,
    args: &[&str],
    dir: Option<&Path>,
    limit: Option<Duration>,
) -> Result<Output, RunError> {
    let mut command = base_command(program);
    command
        .args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .kill_on_drop(true);
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    let output = command.output();
    match limit {
        Some(limit) => timeout(limit, output)
            .await
            .map_err(|_| RunError::TimedOut)?
            .map_err(RunError::Spawn),
        None => output.await.map_err(RunError::Spawn),
    }
}

fn tail(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let text = if stderr.trim().is_empty() {
        String::from_utf8_lossy(&output.stdout)
    } else {
        stderr
    };
    let lines: Vec<&str> = text.trim().lines().collect();
    lines[lines.len().saturating_sub(STDERR_MAX_LINES)..].join("\n")
}
