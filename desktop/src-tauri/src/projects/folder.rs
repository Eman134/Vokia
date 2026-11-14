use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use super::error::{ErrorCode, ProjectError};
use super::github::github_repo_of;
use super::process::{git_output, locate, GIT_PROGRAM};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderInfo {
    pub(super) path: String,
    name: String,
    branch: Option<String>,
    remote_url: Option<String>,
    pub(super) github_repo: Option<String>,
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
