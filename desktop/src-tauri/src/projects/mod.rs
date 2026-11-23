mod error;
mod folder;
mod github;
mod manage;
mod process;
mod tree;
pub(crate) mod worktree;

pub use error::ProjectError;
pub(crate) use folder::ensure_directory;
pub use folder::{inspect_folder, list_git_refs, pick_folder, FolderInfo, GitRefs};
pub use github::{inspect_github_repo, GithubRepoInfo};
pub use manage::{create_project, pin_project, update_project, NewProjectInput, ProjectEdit};
pub(crate) use tree::{folder_tree, FolderTree};
