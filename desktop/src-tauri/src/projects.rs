use std::collections::VecDeque;
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

const TREE_EXCLUDED_DIRS: [&str; 10] = [
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".maestri",
    ".vokia",
    ".venv",
    "__pycache__",
    ".next",
];
pub(crate) const TREE_MAX_DEPTH: usize = 3;
const TREE_MAX_ENTRIES: usize = 150;
const TREE_MAX_CHARS: usize = 4000;
const TREE_SCAN_LIMIT: usize = 2000;

#[derive(Debug)]
pub(crate) struct FolderTree {
    pub entries: Vec<String>,
    pub omitted: usize,
    pub omitted_is_lower_bound: bool,
}

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

pub(crate) fn folder_tree(dir: &Path) -> std::io::Result<FolderTree> {
    let mut walk = TreeWalk::default();
    let mut queue = VecDeque::new();
    walk.list_children(&[], read_children(dir)?, &mut queue);
    while let Some(components) = queue.pop_front() {
        if walk.scan_exhausted {
            break;
        }
        let path = components
            .iter()
            .fold(dir.to_path_buf(), |path, name| path.join(name));
        if let Ok(children) = read_children(&path) {
            walk.list_children(&components, children, &mut queue);
        }
    }
    Ok(walk.finish())
}

struct Child {
    name: String,
    is_dir: bool,
}

fn read_children(dir: &Path) -> std::io::Result<Vec<Child>> {
    let mut children = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let is_dir = entry.file_type()?.is_dir();
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_dir && TREE_EXCLUDED_DIRS.contains(&name.as_str()) {
            continue;
        }
        children.push(Child { name, is_dir });
    }
    children.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(children)
}

#[derive(Default)]
struct TreeWalk {
    listed: Vec<(Vec<String>, String)>,
    chars: usize,
    read: usize,
    omitted: usize,
    scan_exhausted: bool,
}

impl TreeWalk {
    fn list_children(
        &mut self,
        prefix: &[String],
        children: Vec<Child>,
        queue: &mut VecDeque<Vec<String>>,
    ) {
        for Child { name, is_dir } in children {
            if self.read == TREE_SCAN_LIMIT {
                self.scan_exhausted = true;
                return;
            }
            self.read += 1;
            let mut components = prefix.to_vec();
            components.push(name);
            let line = tree_line(&components, is_dir);
            let line_chars = line.chars().count() + 1;
            if self.listed.len() >= TREE_MAX_ENTRIES || self.chars + line_chars > TREE_MAX_CHARS {
                self.omitted += 1;
                continue;
            }
            self.chars += line_chars;
            if is_dir && components.len() < TREE_MAX_DEPTH {
                queue.push_back(components.clone());
            }
            self.listed.push((components, line));
        }
    }

    fn finish(mut self) -> FolderTree {
        self.listed.sort_by(|a, b| a.0.cmp(&b.0));
        FolderTree {
            entries: self.listed.into_iter().map(|(_, line)| line).collect(),
            omitted: self.omitted,
            omitted_is_lower_bound: self.scan_exhausted,
        }
    }
}

fn tree_line(components: &[String], is_dir: bool) -> String {
    let mut line = components.join("/");
    if is_dir {
        line.push('/');
    }
    line
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

#[cfg(test)]
mod folder_tree_tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    const OVERSIZED_ENTRY_COUNT: usize = 200;
    const SCAN_OVERFLOW_ENTRY_COUNT: usize = 2100;
    const LONG_NAME_LEN: usize = 100;
    const LONG_NAME_COUNT: usize = 100;
    const EVERYONE_SID: &str = "*S-1-1-0";

    static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let id = NEXT_DIR.fetch_add(1, Ordering::SeqCst);
            let path =
                std::env::temp_dir().join(format!("vokia-folder-tree-{}-{id}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn dir(&self, relative: &str) {
            fs::create_dir_all(self.0.join(relative)).unwrap();
        }

        fn file(&self, relative: &str) {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "x").unwrap();
        }

        fn tree(&self) -> FolderTree {
            folder_tree(&self.0).unwrap()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            restore_access(&self.0);
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(windows)]
    fn deny_listing(dir: &Path) {
        let status = std::process::Command::new("icacls")
            .arg(dir)
            .args(["/deny", &format!("{EVERYONE_SID}:(RD)")])
            .output()
            .unwrap()
            .status;
        assert!(status.success());
    }

    #[cfg(windows)]
    fn restore_access(dir: &Path) {
        let _ = std::process::Command::new("icacls")
            .arg(dir)
            .args(["/remove:d", EVERYONE_SID, "/t", "/c"])
            .output();
    }

    #[cfg(unix)]
    fn deny_listing(dir: &Path) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o000)).unwrap();
    }

    #[cfg(unix)]
    fn restore_access(dir: &Path) {
        use std::os::unix::fs::PermissionsExt;
        fn walk(dir: &Path) {
            let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
            if let Ok(read) = fs::read_dir(dir) {
                for entry in read.flatten() {
                    if entry.path().is_dir() {
                        walk(&entry.path());
                    }
                }
            }
        }
        walk(dir);
    }

    #[test]
    fn lists_three_levels_in_tree_order_with_slash_separator() {
        let root = TempDir::new();
        root.file("zz.txt");
        root.file("README.md");
        root.file("docs/roadmap.md");
        root.file("desktop/src/a.txt");
        root.dir("desktop/src/features");
        let tree = root.tree();
        assert_eq!(
            tree.entries,
            [
                "README.md",
                "desktop/",
                "desktop/src/",
                "desktop/src/a.txt",
                "desktop/src/features/",
                "docs/",
                "docs/roadmap.md",
                "zz.txt",
            ]
        );
        assert_eq!(tree.omitted, 0);
        assert!(!tree.omitted_is_lower_bound);
        assert!(tree.entries.iter().all(|line| !line.contains('\\')));
    }

    #[test]
    fn excluded_folders_at_any_level_are_neither_listed_nor_counted() {
        let root = TempDir::new();
        root.file("keep.txt");
        for excluded in TREE_EXCLUDED_DIRS {
            root.file(&format!("{excluded}/inside.txt"));
            root.file(&format!("src/{excluded}/inside.txt"));
        }
        root.file("src/main.rs");
        let tree = root.tree();
        assert_eq!(tree.entries, ["keep.txt", "src/", "src/main.rs"]);
        assert_eq!(tree.omitted, 0);
    }

    #[test]
    fn excluded_names_only_hide_folders_not_files() {
        let root = TempDir::new();
        root.file("target");
        root.file("build");
        assert_eq!(root.tree().entries, ["build", "target"]);
    }

    #[test]
    fn excluded_folder_contents_do_not_consume_the_limits() {
        let root = TempDir::new();
        for index in 0..SCAN_OVERFLOW_ENTRY_COUNT {
            root.file(&format!("node_modules/pkg-{index}.js"));
        }
        root.file("app.ts");
        let tree = root.tree();
        assert_eq!(tree.entries, ["app.ts"]);
        assert_eq!(tree.omitted, 0);
        assert!(!tree.omitted_is_lower_bound);
    }

    #[test]
    fn level_four_is_cut_and_level_three_folder_has_no_children() {
        let root = TempDir::new();
        root.file("a/b/c/d/deep.txt");
        root.file("a/b/c/side.txt");
        let tree = root.tree();
        assert_eq!(tree.entries, ["a/", "a/b/", "a/b/c/"]);
        assert_eq!(tree.omitted, 0);
    }

    #[test]
    fn more_than_150_entries_keeps_150_and_counts_the_excess() {
        let root = TempDir::new();
        for index in 0..OVERSIZED_ENTRY_COUNT {
            root.file(&format!("f{index:04}.txt"));
        }
        let tree = root.tree();
        assert_eq!(tree.entries.len(), TREE_MAX_ENTRIES);
        assert_eq!(tree.omitted, OVERSIZED_ENTRY_COUNT - TREE_MAX_ENTRIES);
        assert!(!tree.omitted_is_lower_bound);
        assert_eq!(tree.entries[0], "f0000.txt");
        assert_eq!(tree.entries[TREE_MAX_ENTRIES - 1], "f0149.txt");
    }

    #[test]
    fn exactly_150_entries_omits_nothing() {
        let root = TempDir::new();
        for index in 0..TREE_MAX_ENTRIES {
            root.file(&format!("f{index:04}.txt"));
        }
        let tree = root.tree();
        assert_eq!(tree.entries.len(), TREE_MAX_ENTRIES);
        assert_eq!(tree.omitted, 0);
    }

    #[test]
    fn more_than_2000_entries_flags_a_lower_bound() {
        let root = TempDir::new();
        for index in 0..SCAN_OVERFLOW_ENTRY_COUNT {
            root.file(&format!("f{index:04}.txt"));
        }
        let tree = root.tree();
        assert_eq!(tree.entries.len(), TREE_MAX_ENTRIES);
        assert!(tree.omitted_is_lower_bound);
        assert_eq!(tree.omitted, TREE_SCAN_LIMIT - TREE_MAX_ENTRIES);
    }

    #[test]
    fn character_cap_cuts_long_names_before_150_entries() {
        let root = TempDir::new();
        for index in 0..LONG_NAME_COUNT {
            let name = format!("{index:03}{}", "n".repeat(LONG_NAME_LEN - 3));
            root.file(&name);
        }
        let tree = root.tree();
        let line_chars = LONG_NAME_LEN + 1;
        let expected = TREE_MAX_CHARS / line_chars;
        assert_eq!(tree.entries.len(), expected);
        assert_eq!(tree.omitted, LONG_NAME_COUNT - expected);
        let total: usize = tree
            .entries
            .iter()
            .map(|line| line.chars().count() + 1)
            .sum();
        assert!(total <= TREE_MAX_CHARS);
    }

    #[test]
    fn accented_and_spaced_names_are_kept_as_is() {
        let root = TempDir::new();
        root.file("ação com espaço/relatório final.txt");
        assert_eq!(
            root.tree().entries,
            ["ação com espaço/", "ação com espaço/relatório final.txt"]
        );
    }

    #[test]
    fn empty_folder_has_no_entries_and_nothing_omitted() {
        let root = TempDir::new();
        let tree = root.tree();
        assert!(tree.entries.is_empty());
        assert_eq!(tree.omitted, 0);
        assert!(!tree.omitted_is_lower_bound);
    }

    #[test]
    fn missing_root_is_an_error() {
        let root = TempDir::new();
        assert!(folder_tree(&root.0.join("nao-existe")).is_err());
    }

    #[test]
    fn root_that_is_a_file_is_an_error() {
        let root = TempDir::new();
        root.file("arquivo.txt");
        assert!(folder_tree(&root.0.join("arquivo.txt")).is_err());
    }

    #[test]
    fn unreadable_subfolder_is_skipped_and_the_rest_is_listed() {
        let root = TempDir::new();
        root.file("a-ok/inside.txt");
        root.file("locked/secret.txt");
        root.file("z.txt");
        deny_listing(&root.0.join("locked"));
        let tree = root.tree();
        assert_eq!(
            tree.entries,
            ["a-ok/", "a-ok/inside.txt", "locked/", "z.txt"]
        );
    }

    #[cfg(windows)]
    fn link_dir(target: &Path, link: &Path) {
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap()
            .status;
        assert!(status.success());
    }

    #[cfg(unix)]
    fn link_dir(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    #[test]
    fn linked_folder_is_listed_without_slash_and_not_opened() {
        let root = TempDir::new();
        root.file("real/inside.txt");
        link_dir(&root.0.join("real"), &root.0.join("link"));
        assert_eq!(root.tree().entries, ["link", "real/", "real/inside.txt"]);
    }
}
