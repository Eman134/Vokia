use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::db::{DbState, Project, ORIGIN_GITHUB};
use crate::projects::{self, FolderTree, TREE_MAX_DEPTH};

use super::error::FlowError;

const NO_FOLDER_MESSAGE: &str =
    "O projeto do pedido não tem uma pasta. Escolha a pasta do projeto para enviar pedidos.";
pub(super) const PROJECT_RULE: &str =
    "Você trabalha na pasta do projeto: use as ferramentas só dentro dela e nunca fora.";
const PROJECT_IDENTITY_PREFIX: &str = "Projeto em contexto: \"";
const PROJECT_IDENTITY_PATH_PREFIX: &str = "\", na pasta ";
const PROJECT_DESCRIPTION_PREFIX: &str = "Descrição do projeto: ";
const PROJECT_DESCRIPTION_LIMIT: usize = 500;
const PROJECT_REPO_PREFIX: &str = "Repositório do projeto no GitHub: ";
const PROJECT_NAME_RULE_PREFIX: &str = "O nome do projeto é só \"";
const PROJECT_NAME_RULE_SUFFIX: &str = "\": mesmo que a pasta, o repositório ou o programa que você roda tenham outro nome, não o use e não proponha renomear nada por causa disso. Quando o usuário falar do projeto, é esse.";
const TRUNCATION_MARK: &str = "…";
const PROJECT_TREE_HEADER_PREFIX: &str = "Estrutura da pasta do projeto, até ";
const PROJECT_TREE_HEADER_SUFFIX: &str = " níveis, um caminho relativo por linha e pastas terminadas em \"/\". É um retrato do momento em que a sessão abriu; pastas de dependência e de build ficam de fora; use as ferramentas se precisar do estado atual:";
const PROJECT_TREE_OMITTED_PREFIX: &str = "… e mais ";
const PROJECT_TREE_OMITTED_LOWER_BOUND_PREFIX: &str = "… e mais de ";
const PROJECT_TREE_OMITTED_SUFFIX: &str = " entradas não listadas.";
const PROJECT_TREE_EMPTY: &str = "A pasta do projeto está vazia.";

pub(super) fn load_project(app: &AppHandle, project_id: i64) -> Result<Project, FlowError> {
    let state = app.state::<DbState>();
    let db = state
        .require()
        .map_err(|error| FlowError::ProjectFolder(error.to_string()))?;
    db.project(project_id)
        .map_err(|error| FlowError::ProjectFolder(error.to_string()))
}

// Projeto fixado e nomes de todos os projetos: o Ember usa para identificar o projeto de um pedido.
pub(super) struct ProjectChoices {
    pub(super) pinned: Option<String>,
    pub(super) names: Vec<String>,
}

pub(super) fn project_choices(app: &AppHandle) -> Result<ProjectChoices, FlowError> {
    let state = app.state::<DbState>();
    let db = state
        .require()
        .map_err(|error| FlowError::ProjectFolder(error.to_string()))?;
    let projects = db
        .list_projects()
        .map_err(|error| FlowError::ProjectFolder(error.to_string()))?;
    Ok(ProjectChoices {
        pinned: projects
            .iter()
            .find(|project| project.pinned)
            .map(|project| project.name.clone()),
        names: projects.into_iter().map(|project| project.name).collect(),
    })
}

pub(super) fn project_dir(app: &AppHandle, project_id: i64) -> Result<PathBuf, FlowError> {
    let project = load_project(app, project_id)?;
    if project.path.trim().is_empty() {
        return Err(FlowError::ProjectFolder(NO_FOLDER_MESSAGE.to_string()));
    }
    projects::ensure_directory(&project.path)
        .map_err(|error| FlowError::ProjectFolder(error.message().to_string()))
}

pub(super) async fn scan_structure(dir: &Path) -> Option<FolderTree> {
    let owned = dir.to_path_buf();
    let scanned = tauri::async_runtime::spawn_blocking(move || projects::folder_tree(&owned)).await;
    let error = match scanned {
        Ok(Ok(tree)) => return Some(tree),
        Ok(Err(error)) => error.to_string(),
        Err(error) => error.to_string(),
    };
    eprintln!(
        "Não foi possível ler a estrutura da pasta do projeto {}: {error}",
        dir.display()
    );
    None
}

pub(super) fn project_context(project: &Project, tree: Option<&FolderTree>) -> String {
    let name = project.name.trim();
    let mut sentences = vec![format!(
        "{PROJECT_IDENTITY_PREFIX}{name}{PROJECT_IDENTITY_PATH_PREFIX}{}.",
        project.path
    )];
    if let Some(description) = project_description(project) {
        sentences.push(format!("{PROJECT_DESCRIPTION_PREFIX}{description}"));
    }
    if project.origin_kind == ORIGIN_GITHUB {
        if let Some(repo) = project
            .origin_repo
            .as_deref()
            .map(str::trim)
            .filter(|repo| !repo.is_empty())
        {
            sentences.push(format!("{PROJECT_REPO_PREFIX}{repo}."));
        }
    }
    sentences.push(format!(
        "{PROJECT_NAME_RULE_PREFIX}{name}{PROJECT_NAME_RULE_SUFFIX}"
    ));
    let mut context = sentences.join(" ");
    if let Some(tree) = tree {
        context.push('\n');
        context.push_str(&project_tree_block(tree));
    }
    context
}

fn project_tree_block(tree: &FolderTree) -> String {
    if tree.entries.is_empty() && tree.omitted == 0 {
        return PROJECT_TREE_EMPTY.to_string();
    }
    let mut lines = vec![format!(
        "{PROJECT_TREE_HEADER_PREFIX}{TREE_MAX_DEPTH}{PROJECT_TREE_HEADER_SUFFIX}"
    )];
    lines.extend(tree.entries.iter().cloned());
    if tree.omitted > 0 {
        let prefix = if tree.omitted_is_lower_bound {
            PROJECT_TREE_OMITTED_LOWER_BOUND_PREFIX
        } else {
            PROJECT_TREE_OMITTED_PREFIX
        };
        lines.push(format!(
            "{prefix}{}{PROJECT_TREE_OMITTED_SUFFIX}",
            tree.omitted
        ));
    }
    lines.join("\n")
}

fn project_description(project: &Project) -> Option<String> {
    let description = project
        .description
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if description.is_empty() {
        return None;
    }
    if description.chars().count() <= PROJECT_DESCRIPTION_LIMIT {
        return Some(description);
    }
    let truncated: String = description
        .chars()
        .take(PROJECT_DESCRIPTION_LIMIT)
        .collect();
    Some(format!("{truncated}{TRUNCATION_MARK}"))
}
