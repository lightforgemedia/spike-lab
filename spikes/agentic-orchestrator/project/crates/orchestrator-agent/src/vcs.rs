use anyhow::{anyhow, Context, Result};
use std::path::Path;
use tokio::process::Command;

/// Return current VCS revision id for this workspace.
///
/// Prefers `jj` (commit id), falls back to `git` (sha).
pub async fn current_revision(workspace_root: &Path) -> Result<String> {
    if has_cmd("jj").await {
        let out = Command::new("jj")
            .arg("--repository")
            .arg(workspace_root)
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
            .current_dir(workspace_root)
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

async fn has_cmd(name: &str) -> bool {
    Command::new(name).arg("--version").output().await.is_ok()
}
