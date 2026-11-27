use std::sync::atomic::{AtomicBool, Ordering};

use serde::Deserialize;

use crate::db::{Db, NewProject, Project, ORIGIN_FOLDER, ORIGIN_GITHUB};

use super::error::{ErrorCode, ProjectError};
use super::folder::inspect_folder;
use super::github::{create_from_github, GithubOrigin};

// Espelha as chaves de desktop/src/features/projects/projectIcons.ts.
const PROJECT_ICONS: [&str; 24] = [
    "folder",
    "circle",
    "target",
    "triangle",
    "square",
    "diamond",
    "document",
    "diagram",
    "whiteboard",
    "star",
    "heart",
    "bolt",
    "flag",
    "bookmark",
    "hexagon",
    "cross",
    "moon",
    "cloud",
    "leaf",
    "bars",
    "lock",
    "bell",
    "mountain",
    "sun",
];
// Espelha as chaves de desktop/src/features/projects/projectColors.ts.
const PROJECT_COLORS: [&str; 8] = [
    "rose", "peach", "yellow", "green", "teal", "blue", "purple", "pink",
];
const SETUP_COMMAND_MAX_CHARS: usize = 300;
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
    color: Option<String>,
    origin: Origin,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEdit {
    name: String,
    symbol: String,
    description: String,
    color: String,
    setup_command: Option<String>,
    #[serde(default)]
    teardown_command: Option<String>,
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

fn ensure_known_color(color: &str) -> Result<(), ProjectError> {
    if PROJECT_COLORS.contains(&color) {
        return Ok(());
    }
    Err(ProjectError::new(
        ErrorCode::InvalidColor,
        "A cor do projeto não está na paleta.",
    ))
}

fn normalize_lifecycle_command(
    raw: Option<String>,
    label: &str,
) -> Result<Option<String>, ProjectError> {
    let Some(trimmed) = raw.as_deref().map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(None);
    };
    if trimmed.chars().count() > SETUP_COMMAND_MAX_CHARS {
        return Err(ProjectError::new(
            ErrorCode::SetupCommandTooLong,
            format!("O comando de {label} passa de {SETUP_COMMAND_MAX_CHARS} caracteres."),
        ));
    }
    Ok(Some(trimmed.to_string()))
}

pub fn update_project(db: &Db, id: i64, edit: ProjectEdit) -> Result<Project, ProjectError> {
    let symbol = edit.symbol.trim();
    ensure_known_icon(symbol)?;
    ensure_known_color(&edit.color)?;
    let setup_command = normalize_lifecycle_command(edit.setup_command, "preparo")?;
    let teardown_command = normalize_lifecycle_command(edit.teardown_command, "encerramento")?;
    Ok(db.update_project(
        id,
        &edit.name,
        symbol,
        &edit.description,
        &edit.color,
        setup_command.as_deref(),
        teardown_command.as_deref(),
    )?)
}

pub fn pin_project(db: &Db, id: Option<i64>) -> Result<(), ProjectError> {
    Ok(db.pin_project(id)?)
}

pub async fn create_project(db: &Db, input: NewProjectInput) -> Result<Project, ProjectError> {
    let _guard = CreateGuard::acquire()?;
    ensure_known_icon(input.symbol.trim())?;
    if let Some(color) = input.color.as_deref() {
        ensure_known_color(color)?;
    }
    let project = match &input.origin {
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
            db.insert_project(&project)?
        }
        Origin::Github(origin) => create_from_github(db, &input, origin).await?,
    };
    // insert_project grava o projeto sem cor, por isso a cor entra num segundo passo.
    match input.color.as_deref() {
        Some(color) => Ok(db.update_project(
            project.id,
            &project.name,
            &project.symbol,
            &project.description,
            color,
            None,
            None,
        )?),
        None => Ok(project),
    }
}

#[cfg(test)]
#[path = "../../tests/project_color_tests.rs"]
mod tests;
