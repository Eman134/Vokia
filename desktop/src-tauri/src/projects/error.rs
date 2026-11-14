use serde::Serialize;

use crate::db::DbError;
use crate::orchestrator::FlowError;

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
    pub(super) fn new(code: ErrorCode, message: impl Into<String>) -> Self {
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

    pub(super) fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub(super) fn with_repo(mut self, repo: impl Into<String>) -> Self {
        self.repo = Some(repo.into());
        self
    }

    pub(super) fn with_branch(mut self, branch: impl Into<String>) -> Self {
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
