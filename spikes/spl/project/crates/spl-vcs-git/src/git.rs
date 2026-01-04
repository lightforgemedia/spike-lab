use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use spl_core::{TaskId, VcsType};
use spl_vcs::{Patch, RevId, VcsAdapter, WorkspaceHandle};

#[derive(Clone, Debug)]
pub struct GitAdapter {
    pub main_branch: String,
}

impl GitAdapter {
    pub fn new(main_branch: impl Into<String>) -> Self {
        Self { main_branch: main_branch.into() }
    }

    fn run(repo: &Path, args: &[&str]) -> Result<String> {
        let mut cmd = Command::new(args[0]);
        cmd.args(&args[1..]).current_dir(repo);
        let out = cmd.output().with_context(|| format!("run {:?}", args))?;
        if !out.status.success() {
            return Err(anyhow!(
                "command failed: {:?}\nstdout:{}\nstderr:{}",
                args,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn ensure_on_main(&self, repo_root: &Path) -> Result<()> {
        let head = Self::run(repo_root, &["git", "rev-parse", "--abbrev-ref", "HEAD"])?;
        if head != self.main_branch {
            // avoid failing if detached; just checkout
            let _ = Self::run(repo_root, &["git", "checkout", &self.main_branch]);
        }
        Ok(())
    }

    fn has_staged_changes(repo_root: &Path) -> Result<bool> {
        let out = Command::new("git")
            .args(["diff", "--cached", "--name-only"])
            .current_dir(repo_root)
            .output()?;
        Ok(!out.stdout.is_empty())
    }
}

impl VcsAdapter for GitAdapter {
    fn vcs_type(&self) -> VcsType {
        VcsType::Git
    }

    fn repo_root_is_clean(&self, repo_root: &Path) -> Result<bool> {
        let out = Self::run(repo_root, &["git", "status", "--porcelain"])?;
        Ok(out.is_empty())
    }

    fn create_workspace(&self, repo_root: &Path, ws_root: &Path, task_id: &TaskId) -> Result<WorkspaceHandle> {
        std::fs::create_dir_all(ws_root).ok();
        let ws_path = ws_root.join(format!("{}-{}", task_id.as_str(), uuid::Uuid::new_v4()));
        // Detached worktree keeps v0 simple (no branch cleanup required)
        Self::run(repo_root, &["git", "worktree", "add", "--detach", ws_path.to_str().unwrap(), &self.main_branch])?;
        Ok(WorkspaceHandle { path: ws_path, workspace_id: "git-worktree".to_string() })
    }

    fn get_base_rev(&self, repo_root: &Path) -> Result<RevId> {
        let rev = Self::run(repo_root, &["git", "rev-parse", "HEAD"])?;
        Ok(rev)
    }

    fn snapshot(&self, ws: &WorkspaceHandle, message: &str) -> Result<RevId> {
        // if no changes, return HEAD
        let status = Self::run(&ws.path, &["git", "status", "--porcelain"])?;
        if status.is_empty() {
            return Ok(Self::run(&ws.path, &["git", "rev-parse", "HEAD"])?);
        }

        let _ = Self::run(&ws.path, &["git", "add", "-A"])?;
        // commit may fail if nothing staged, but we checked status above
        let _ = Self::run(&ws.path, &["git", "commit", "-m", message])?;
        Ok(Self::run(&ws.path, &["git", "rev-parse", "HEAD"])?)
    }

    fn export_patch(&self, ws: &WorkspaceHandle, base: &RevId, head: &RevId) -> Result<Patch> {
        let bytes = Command::new("git")
            .args(["diff", "--binary", base, head])
            .current_dir(&ws.path)
            .output()
            .with_context(|| "git diff")?;
        if !bytes.status.success() {
            return Err(anyhow!("git diff failed"));
        }
        Ok(Patch { bytes: bytes.stdout, format: "git".into() })
    }

    fn apply_patch_to_repo_root(&self, repo_root: &Path, patch: &Patch, message: &str) -> Result<RevId> {
        self.ensure_on_main(repo_root)?;

        // Apply patch; if empty patch, no-op
        if patch.bytes.is_empty() {
            return Ok(Self::run(repo_root, &["git", "rev-parse", "HEAD"])?);
        }

        // git apply --index reads from stdin
        let mut child = Command::new("git")
            .args(["apply", "--index", "--whitespace=nowarn", "-"])
            .current_dir(repo_root)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .with_context(|| "spawn git apply")?;
        {
            use std::io::Write;
            let mut stdin = child.stdin.take().unwrap();
            stdin.write_all(&patch.bytes)?;
        }
        let out = child.wait_with_output()?;
        if !out.status.success() {
            return Err(anyhow!(
                "git apply failed\nstdout:{}\nstderr:{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ));
        }

        if !Self::has_staged_changes(repo_root)? {
            // patch applied but no changes staged (possible if patch empty-ish)
            return Ok(Self::run(repo_root, &["git", "rev-parse", "HEAD"])?);
        }

        let _ = Self::run(repo_root, &["git", "commit", "-m", message])?;
        Ok(Self::run(repo_root, &["git", "rev-parse", "HEAD"])?)
    }

    fn cleanup_workspace(&self, repo_root: &Path, ws: WorkspaceHandle) -> Result<()> {
        // Best effort removal.
        let _ = Self::run(repo_root, &["git", "worktree", "remove", "--force", ws.path.to_str().unwrap()]);
        let _ = Self::run(repo_root, &["git", "worktree", "prune"]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use spl_vcs::contract::{init_git_repo, run_vcs_contract_suite};

    #[test]
    fn git_adapter_contract() {
        let dir = tempdir().unwrap();
        init_git_repo(dir.path()).unwrap();

        let ws_root = dir.path().join(".spl-ws");
        let adapter = GitAdapter::new("master"); // git init default varies; detect:
        // Use current HEAD branch as main
        let main = GitAdapter::run(dir.path(), &["git","rev-parse","--abbrev-ref","HEAD"]).unwrap();
        let adapter = GitAdapter::new(main);

        run_vcs_contract_suite(&adapter, dir.path(), &ws_root).unwrap();
    }
}
