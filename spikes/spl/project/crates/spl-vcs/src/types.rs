use std::path::{Path, PathBuf};

use anyhow::Result;
use spl_core::{TaskId, VcsType};

pub type RevId = String;

#[derive(Clone, Debug)]
pub struct WorkspaceHandle {
    pub path: PathBuf,
    /// Adapter-specific identifier (e.g., git worktree path, jj workspace name).
    pub workspace_id: String,
}

#[derive(Clone, Debug)]
pub struct Patch {
    pub bytes: Vec<u8>,
    pub format: String, // "git"
}

pub trait VcsAdapter: Send + Sync {
    fn vcs_type(&self) -> VcsType;

    fn repo_root_is_clean(&self, repo_root: &Path) -> Result<bool>;

    fn create_workspace(&self, repo_root: &Path, ws_root: &Path, task_id: &TaskId) -> Result<WorkspaceHandle>;

    fn get_base_rev(&self, repo_root: &Path) -> Result<RevId>;

    fn snapshot(&self, ws: &WorkspaceHandle, message: &str) -> Result<RevId>;

    fn export_patch(&self, ws: &WorkspaceHandle, base: &RevId, head: &RevId) -> Result<Patch>;

    fn apply_patch_to_repo_root(&self, repo_root: &Path, patch: &Patch, message: &str) -> Result<RevId>;

    fn cleanup_workspace(&self, repo_root: &Path, ws: WorkspaceHandle) -> Result<()>;
}
