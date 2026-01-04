use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use spl_core::{TaskId, VcsType};
use spl_vcs::{Patch, RevId, VcsAdapter, WorkspaceHandle};

use spl_vcs_git::GitAdapter;

/// JJ adapter (v0) using jj workspaces + jj snapshots, and **git-first landing**.
///
/// Assumptions:
/// - jj is installed
/// - repository is jj-colocated with git (`.jj/` exists)
/// - mainline is tracked by a jj bookmark name (e.g. `main`)
#[derive(Clone, Debug)]
pub struct JjAdapter {
    pub main_bookmark: String,
    pub git_main_branch: String,
    pub require_colocated: bool,
}

impl JjAdapter {
    pub fn new(main_bookmark: impl Into<String>, git_main_branch: impl Into<String>, require_colocated: bool) -> Self {
        Self {
            main_bookmark: main_bookmark.into(),
            git_main_branch: git_main_branch.into(),
            require_colocated,
        }
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

    fn ensure_jj_available() -> Result<()> {
        let out = Command::new("jj").arg("--version").output();
        match out {
            Ok(o) if o.status.success() => Ok(()),
            _ => Err(anyhow!("jj not found on PATH; install with `brew install jj`")),
        }
    }

    fn ensure_colocated(&self, repo_root: &Path) -> Result<()> {
        if self.require_colocated {
            let jj_dir = repo_root.join(".jj");
            if !jj_dir.exists() {
                return Err(anyhow!(
                    "jj colocated repo required but .jj not found. Run: `jj git init --colocate`"
                ));
            }
        }
        Ok(())
    }

    fn jj_commit_id(repo: &Path, revset: &str) -> Result<String> {
        // commit_id is a template keyword documented by jj templates.
        // Use --no-graph to simplify parsing.
        let out = Self::run(repo, &["jj", "log", "-r", revset, "--no-graph", "-T", "commit_id"])?;
        Ok(out.lines().next().unwrap_or("").trim().to_string())
    }

    fn best_effort_jj_git_import(repo_root: &Path) {
        // In colocated repos this may be unnecessary; keep best-effort for v0.
        let _ = Command::new("jj").args(["git", "import"]).current_dir(repo_root).output();
    }

    fn git_lander(&self) -> GitAdapter {
        GitAdapter::new(self.git_main_branch.clone())
    }
}

impl VcsAdapter for JjAdapter {
    fn vcs_type(&self) -> VcsType {
        VcsType::Jj
    }

    fn repo_root_is_clean(&self, repo_root: &Path) -> Result<bool> {
        Self::ensure_jj_available()?;
        self.ensure_colocated(repo_root)?;
        // Use underlying git cleanliness (filesystem changes show up there too).
        let git = self.git_lander();
        git.repo_root_is_clean(repo_root)
    }

    fn create_workspace(&self, repo_root: &Path, ws_root: &Path, task_id: &TaskId) -> Result<WorkspaceHandle> {
        Self::ensure_jj_available()?;
        self.ensure_colocated(repo_root)?;
        std::fs::create_dir_all(ws_root).ok();

        let ws_name = format!("spl-{}-{}", task_id.as_str(), uuid::Uuid::new_v4().simple());
        let ws_path = ws_root.join(format!("{}-ws", ws_name));

        // `jj workspace add --name <name> -r <bookmark> <dest>`
        let bookmark = self.main_bookmark.as_str();
        Self::run(repo_root, &[
            "jj","workspace","add",
            "--name",&ws_name,
            "-r",bookmark,
            ws_path.to_str().unwrap()
        ])?;

        Ok(WorkspaceHandle { path: ws_path, workspace_id: ws_name })
    }

    fn get_base_rev(&self, repo_root: &Path) -> Result<RevId> {
        Self::ensure_jj_available()?;
        self.ensure_colocated(repo_root)?;
        let id = Self::jj_commit_id(repo_root, &self.main_bookmark)?;
        if id.is_empty() {
            return Err(anyhow!("failed to resolve jj main bookmark {}", self.main_bookmark));
        }
        Ok(id)
    }

    fn snapshot(&self, ws: &WorkspaceHandle, message: &str) -> Result<RevId> {
        Self::ensure_jj_available()?;
        // If no changes in filesystem, return current working-copy commit id.
        let status = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&ws.path)
            .output()
            .ok()
            .map(|o| o.stdout)
            .unwrap_or_default();
        if status.is_empty() {
            return Self::jj_commit_id(&ws.path, "@");
        }

        let _ = Self::run(&ws.path, &["jj", "commit", "-m", message])?;
        // The committed change should be @- (previous working-copy commit).
        let id = Self::jj_commit_id(&ws.path, "@-")?;
        if id.is_empty() {
            return Err(anyhow!("failed to get jj commit_id for @- after commit"));
        }
        Ok(id)
    }

    fn export_patch(&self, ws: &WorkspaceHandle, base: &RevId, head: &RevId) -> Result<Patch> {
        Self::ensure_jj_available()?;
        // Export git-format patch: `jj diff --git --from <base> --to <head>`
        let out = Command::new("jj")
            .args(["diff", "--git", "--from", base, "--to", head])
            .current_dir(&ws.path)
            .output()
            .with_context(|| "jj diff --git")?;
        if !out.status.success() {
            return Err(anyhow!(
                "jj diff failed\nstdout:{}\nstderr:{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        Ok(Patch { bytes: out.stdout, format: "git".into() })
    }

    fn apply_patch_to_repo_root(&self, repo_root: &Path, patch: &Patch, message: &str) -> Result<RevId> {
        Self::ensure_jj_available()?;
        self.ensure_colocated(repo_root)?;
        // git-first deterministic landing:
        let git = self.git_lander();
        let landed = git.apply_patch_to_repo_root(repo_root, patch, message)?;
        Self::best_effort_jj_git_import(repo_root);
        Ok(landed)
    }

    fn cleanup_workspace(&self, repo_root: &Path, ws: WorkspaceHandle) -> Result<()> {
        Self::ensure_jj_available()?;
        self.ensure_colocated(repo_root)?;
        // `jj workspace forget <name>` then remove directory
        let _ = Self::run(repo_root, &["jj", "workspace", "forget", &ws.workspace_id]);
        let _ = std::fs::remove_dir_all(&ws.path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use spl_vcs::contract::{init_git_repo, run_vcs_contract_suite};

    fn jj_available() -> bool {
        Command::new("jj").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
    }

    #[test]
    fn jj_adapter_contract_if_available() {
        if !jj_available() {
            eprintln!("jj not available; skipping");
            return;
        }

        let dir = tempdir().unwrap();
        init_git_repo(dir.path()).unwrap();

        // Initialize jj colocated repo
        let out = Command::new("jj")
            .args(["git", "init", "--colocate"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        if !out.status.success() {
            eprintln!("jj git init failed; skipping\n{}", String::from_utf8_lossy(&out.stderr));
            return;
        }

        // Determine current git branch created by init_git_repo
        let main = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let main_branch = String::from_utf8_lossy(&main.stdout).trim().to_string();

        let ws_root = dir.path().join(".spl-ws");
        let adapter = JjAdapter::new("main", main_branch, true);

        // In a new jj repo, main bookmark may not exist; create it if needed.
        let _ = Command::new("jj")
            .args(["bookmark", "create", "main", "-r", "@"])
            .current_dir(dir.path())
            .output();

        run_vcs_contract_suite(&adapter, dir.path(), &ws_root).unwrap();
    }
}
