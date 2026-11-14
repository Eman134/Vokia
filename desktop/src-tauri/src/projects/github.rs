use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::db::{Db, NewProject, Project, ORIGIN_GITHUB};

use super::error::{ErrorCode, ProjectError};
use super::folder::ensure_directory;
use super::manage::NewProjectInput;
use super::process::{git_output, locate, run, tail, RunError, GIT_PROGRAM};

const GH_PROGRAM: &str = "gh";
const CLONE_TIMEOUT: Duration = Duration::from_secs(600);
const INSPECT_TIMEOUT: Duration = Duration::from_secs(30);
const GITHUB_PREFIXES: [&str; 3] = [
    "https://github.com/",
    "git@github.com:",
    "ssh://git@github.com/",
];
const REPO_NOT_FOUND_MARKERS: [&str; 2] = ["Could not resolve to a Repository", "HTTP 404"];

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

pub(super) async fn create_from_github(
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
pub(super) fn github_repo_of(remote_url: &str) -> Option<String> {
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
