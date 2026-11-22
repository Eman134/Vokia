use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex as StdMutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::db::{DbState, Project, ORIGIN_GITHUB};
use crate::projects::{self, FolderTree};

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
// Orçamento do bloco no prompt: bem menor que o da leitura da pasta, que segue com os limites de projects::folder_tree.
const PROMPT_TREE_MAX_DEPTH: usize = 2;
const PROMPT_TREE_MAX_ENTRIES: usize = 60;
const TREE_CACHE_TTL: Duration = Duration::from_secs(30);

static TREE_CACHE: OnceLock<StdMutex<HashMap<PathBuf, (Instant, FolderTree)>>> = OnceLock::new();

fn tree_cache() -> &'static StdMutex<HashMap<PathBuf, (Instant, FolderTree)>> {
    TREE_CACHE.get_or_init(Default::default)
}

fn cached_tree(dir: &Path) -> Option<FolderTree> {
    let cache = tree_cache().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let (scanned_at, tree) = cache.get(dir)?;
    (scanned_at.elapsed() < TREE_CACHE_TTL).then(|| tree.clone())
}

fn store_tree(dir: PathBuf, tree: &FolderTree) {
    let mut cache = tree_cache().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.insert(dir, (Instant::now(), tree.clone()));
}
const PROMPT_TREE_MAX_CHARS: usize = 1800;
const RESERVED_FOOTER_CHARS: usize = 64;

pub(super) fn load_project(app: &AppHandle, project_id: i64) -> Result<Project, FlowError> {
    let state = app.state::<DbState>();
    let db = state
        .require()
        .map_err(|error| FlowError::ProjectFolder(error.to_string()))?;
    db.project(project_id)
        .map_err(|error| FlowError::ProjectFolder(error.to_string()))
}

// Projeto fixado e rótulos de todos os projetos: o Ember usa para identificar o projeto de um pedido.
pub(super) struct ProjectChoices {
    pub(super) pinned: Option<String>,
    pub(super) labels: Vec<String>,
}

// Nomes não são únicos: o rótulo leva o id, e a escolha volta por ele.
const PROJECT_LABEL_ID_PREFIX: &str = " (#";
const PROJECT_LABEL_ID_SUFFIX: &str = ")";

pub(super) fn project_label(name: &str, id: i64) -> String {
    format!("{name}{PROJECT_LABEL_ID_PREFIX}{id}{PROJECT_LABEL_ID_SUFFIX}")
}

fn project_id_of_label(answer: &str) -> Option<i64> {
    let (_, digits) = answer
        .strip_suffix(PROJECT_LABEL_ID_SUFFIX)?
        .rsplit_once(PROJECT_LABEL_ID_PREFIX)?;
    digits.parse().ok()
}

fn list_projects(app: &AppHandle) -> Result<Vec<Project>, FlowError> {
    let state = app.state::<DbState>();
    let db = state
        .require()
        .map_err(|error| FlowError::ProjectFolder(error.to_string()))?;
    db.list_projects()
        .map_err(|error| FlowError::ProjectFolder(error.to_string()))
}

pub(super) fn project_choices(app: &AppHandle) -> Result<ProjectChoices, FlowError> {
    let projects = list_projects(app)?;
    Ok(ProjectChoices {
        pinned: projects
            .iter()
            .find(|project| project.pinned)
            .map(|project| project_label(&project.name, project.id)),
        labels: projects
            .iter()
            .map(|project| project_label(&project.name, project.id))
            .collect(),
    })
}

// Projeto escolhido numa decisão, pelo id do rótulo; None quando o texto não é o rótulo de um projeto existente.
pub(super) fn project_id_from_choice(
    app: &AppHandle,
    answer: &str,
) -> Result<Option<i64>, FlowError> {
    let Some(id) = project_id_of_label(answer) else {
        return Ok(None);
    };
    Ok(list_projects(app)?
        .into_iter()
        .find(|project| project.id == id && project_label(&project.name, project.id) == answer)
        .map(|project| project.id))
}

pub(super) fn project_dir(app: &AppHandle, project_id: i64) -> Result<PathBuf, FlowError> {
    project_dir_of(&load_project(app, project_id)?)
}

pub(super) fn project_dir_of(project: &Project) -> Result<PathBuf, FlowError> {
    if project.path.trim().is_empty() {
        return Err(FlowError::ProjectFolder(NO_FOLDER_MESSAGE.to_string()));
    }
    projects::ensure_directory(&project.path)
        .map_err(|error| FlowError::ProjectFolder(error.message().to_string()))
}

pub(super) async fn scan_structure(dir: &Path) -> Option<FolderTree> {
    if let Some(tree) = cached_tree(dir) {
        return Some(tree);
    }
    let owned = dir.to_path_buf();
    let scanned = tauri::async_runtime::spawn_blocking(move || projects::folder_tree(&owned)).await;
    let error = match scanned {
        Ok(Ok(tree)) => {
            store_tree(dir.to_path_buf(), &tree);
            return Some(tree);
        }
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

// O Ember só roteia pedidos: a árvore de arquivos não entra no prompt dele.
pub(super) fn project_context_without_tree(project: &Project) -> String {
    project_context(project, None)
}

fn entry_depth(entry: &str) -> usize {
    let trimmed = entry.trim_end_matches('/');
    if trimmed.is_empty() {
        return 0;
    }
    trimmed.chars().filter(|&c| c == '/').count() + 1
}

fn project_tree_block(tree: &FolderTree) -> String {
    if tree.entries.is_empty() && tree.omitted == 0 {
        return PROJECT_TREE_EMPTY.to_string();
    }
    let header = format!(
        "{PROJECT_TREE_HEADER_PREFIX}{PROMPT_TREE_MAX_DEPTH}{PROJECT_TREE_HEADER_SUFFIX}"
    );
    let mut lines = vec![header];
    let mut kept = 0usize;
    let mut used_chars = lines[0].chars().count();
    let mut cut_by_budget = 0usize;
    for entry in &tree.entries {
        let line_chars = entry.chars().count() + 1;
        if entry_depth(entry) > PROMPT_TREE_MAX_DEPTH
            || kept >= PROMPT_TREE_MAX_ENTRIES
            || used_chars + line_chars + RESERVED_FOOTER_CHARS > PROMPT_TREE_MAX_CHARS
        {
            cut_by_budget += 1;
            continue;
        }
        kept += 1;
        used_chars += line_chars;
        lines.push(entry.clone());
    }
    let omitted = tree.omitted + cut_by_budget;
    if omitted > 0 {
        let prefix = if tree.omitted_is_lower_bound {
            PROJECT_TREE_OMITTED_LOWER_BOUND_PREFIX
        } else {
            PROJECT_TREE_OMITTED_PREFIX
        };
        lines.push(format!("{prefix}{omitted}{PROJECT_TREE_OMITTED_SUFFIX}"));
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

#[cfg(test)]
#[path = "project_context_tests.rs"]
mod tests;
