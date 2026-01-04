use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::warn;

/// Return the current VCS revision id for the project.
///
/// Prefers `jj` (commit id), falls back to `git` (sha).
pub async fn current_revision(project_root: &Path) -> Result<String> {
    if has_cmd("jj").await {
        let out = Command::new("jj")
            .arg("--repository")
            .arg(project_root)
            .args(["log", "-r", "@", "--no-graph", "-T", "commit_id"])
            .output()
            .await
            .context("running jj log")?;
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Ok(s);
            }
        }
    }
    if has_cmd("git").await {
        let out = Command::new("git")
            .current_dir(project_root)
            .args(["rev-parse", "HEAD"])
            .output()
            .await
            .context("running git rev-parse")?;
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Ok(s);
            }
        }
    }
    Err(anyhow!("no supported VCS (jj/git) found"))
}

/// Ensure a workspace directory exists and is ready for execution at `input_rev` if provided.
///
/// - For `jj`, we create an additional workspace attached to the project repo:
///   `jj --repository <project> workspace add --name <...> --revision <input_rev> <dest>`
/// - For `git`, we fall back to `git worktree add <dest> <rev>`.
pub async fn prepare_workspace(project_root: &Path, workspace_root: &Path, input_rev: Option<String>) -> Result<()> {
    // If it looks like a workspace already exists, don't destroy it. Just ensure it's not stale.
    if is_nonempty_dir(workspace_root) {
        if has_cmd("jj").await && looks_like_jj_workspace(workspace_root) {
            let _ = Command::new("jj")
                .arg("--repository")
                .arg(workspace_root)
                .args(["workspace", "update-stale"])
                .output()
                .await;
        }
        return Ok(());
    }

    std::fs::create_dir_all(workspace_root)
        .with_context(|| format!("creating workspace dir {}", workspace_root.display()))?;

    if has_cmd("jj").await {
        ensure_jj_repo(project_root).await?;

        let name = workspace_name(workspace_root);

        let mut cmd = Command::new("jj");
        cmd.arg("--repository")
            .arg(project_root)
            .args(["workspace", "add", "--name", &name]);

        if let Some(rev) = input_rev {
            cmd.args(["--revision", &rev]);
        }

        cmd.arg(workspace_root);

        let out = cmd.output().await.context("running jj workspace add")?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            warn!("jj workspace add failed: {stderr}");
        }
        return Ok(());
    }

    if has_cmd("git").await {
        let rev = input_rev.unwrap_or_else(|| "HEAD".to_string());
        let out = Command::new("git")
            .current_dir(project_root)
            .args(["worktree", "add"])
            .arg(workspace_root)
            .arg(&rev)
            .output()
            .await
            .context("running git worktree add")?;
        if !out.status.success() {
            return Err(anyhow!(
                "git worktree add failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        return Ok(());
    }

    Err(anyhow!("no supported VCS (jj/git) found"))
}

async fn ensure_jj_repo(project_root: &Path) -> Result<()> {
    let jj_dir = project_root.join(".jj");
    if jj_dir.exists() {
        return Ok(());
    }
    // If this is a git repo, prefer colocated jj repo.
    let mut cmd = Command::new("jj");
    cmd.current_dir(project_root).args(["git", "init"]);
    let out = cmd.output().await.context("running jj git init")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!("jj git init failed: {stderr}"));
    }
    Ok(())
}

async fn has_cmd(name: &str) -> bool {
    Command::new(name).arg("--version").output().await.is_ok()
}

fn is_nonempty_dir(p: &Path) -> bool {
    if !p.is_dir() {
        return false;
    }
    match std::fs::read_dir(p) {
        Ok(mut it) => it.next().is_some(),
        Err(_) => false,
    }
}

fn looks_like_jj_workspace(p: &Path) -> bool {
    p.join(".jj").exists() || p.join(".jj").is_file()
}

fn workspace_name(workspace_root: &Path) -> String {
    workspace_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("orchestrator-ws")
        .to_string()
}
