use std::path::Path;
use std::process::Stdio;

/// Minimal adapter for `jj` workspace management.
///
/// The daemon uses this for *lightweight* repo ops:
/// - ensure the repo is initialized for jj
/// - create a per-run workspace directory
///
/// Heavy work (builds/tests/etc.) stays on agents.

/// Ensure the repo at `project_root` has an initialized `.jj` directory.
///
/// This uses `jj git init --colocate` to adopt jj in an existing Git repo.
pub async fn ensure_jj_initialized(project_root: &Path) -> anyhow::Result<()> {
    // If a .jj dir exists, assume initialized.
    if project_root.join(".jj").exists() {
        return Ok(());
    }

    // Try to initialize jj in colocated git mode.
    // This is the typical way to adopt jj in an existing git repo.
    let out = tokio::process::Command::new("jj")
        .args(["git", "init", "--colocate"])
        .current_dir(project_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !out.status.success() {
        anyhow::bail!(
            "jj git init --colocate failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

/// Ensure a working copy exists at `destination` and is registered as a jj workspace.
pub async fn ensure_workspace(
    project_root: &Path,
    destination: &Path,
    name: &str,
) -> anyhow::Result<()> {
    // If destination already looks like a jj workspace, accept it.
    if destination.join(".jj").exists() {
        return Ok(());
    }

    let parent = destination
        .parent()
        .ok_or_else(|| anyhow::anyhow!("workspace destination has no parent"))?;
    tokio::fs::create_dir_all(parent).await?;

    // Create a new workspace. If it already exists, treat as idempotent.
    let out = tokio::process::Command::new("jj")
        .args(["workspace", "add", "--name", name])
        .arg(destination)
        .current_dir(project_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        // Common idempotency case: directory already exists or workspace exists.
        if destination.exists() {
            tracing::warn!(
                workspace = %destination.display(),
                "jj workspace add failed but destination exists; proceeding: {}",
                stderr.trim()
            );
            return Ok(());
        }

        anyhow::bail!("jj workspace add failed: {}", stderr.trim());
    }

    Ok(())
}
