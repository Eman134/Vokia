use std::sync::atomic::{AtomicBool, Ordering};

use serde::Deserialize;

use crate::db::{Db, NewProject, Project, ORIGIN_FOLDER, ORIGIN_GITHUB};

use super::error::{ErrorCode, ProjectError};
use super::folder::inspect_folder;
use super::github::{create_from_github, GithubOrigin};

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

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Origin {
    Folder { path: String },
    Github(GithubOrigin),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewProjectInput {
    pub(super) name: String,
    pub(super) symbol: String,
    pub(super) description: String,
    origin: Origin,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEdit {
    name: String,
    symbol: String,
    description: String,
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
