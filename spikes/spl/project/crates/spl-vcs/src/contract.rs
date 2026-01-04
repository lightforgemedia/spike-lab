use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use spl_core::TaskId;

use crate::types::{VcsAdapter};

/// Shared adapter contract suite. This is intentionally small and runs against both git and jj adapters.
pub fn run_vcs_contract_suite(adapter: &dyn VcsAdapter, repo_root: &Path, ws_root: &Path) -> Result<()> {
    if !adapter.repo_root_is_clean(repo_root)? {
        return Err(anyhow!("repo root must be clean for contract tests"));
    }

    let task_id = TaskId::from_str("pt-contract");
    let ws = adapter.create_workspace(repo_root, ws_root, &task_id)?;
    // Make a change in workspace
    let test_file = ws.path.join("contract.txt");
    std::fs::write(&test_file, "hello")?;

    let base = adapter.get_base_rev(repo_root)?;
    let head = adapter.snapshot(&ws, "contract snapshot")?;
    let patch = adapter.export_patch(&ws, &base, &head)?;
    if patch.bytes.is_empty() {
        return Err(anyhow!("expected non-empty patch"));
    }

    // Apply patch and ensure repo root changed
    let before = adapter.get_base_rev(repo_root)?;
    let after = adapter.apply_patch_to_repo_root(repo_root, &patch, "contract land")?;
    let after2 = adapter.get_base_rev(repo_root)?;
    // "after" should match "after2" for deterministic landing
    if after != after2 {
        return Err(anyhow!("expected landed rev to match repo mainline rev"));
    }
    if before == after {
        return Err(anyhow!("expected repo to advance after landing"));
    }

    adapter.cleanup_workspace(repo_root, ws)?;
    Ok(())
}

/// Initialize a minimal git repo fixture with one commit.
pub fn init_git_repo(dir: &Path) -> Result<()> {
    run(dir, &["git", "init"])?;
    run(dir, &["git", "config", "user.email", "spl@example.com"])?;
    run(dir, &["git", "config", "user.name", "spl"])?;
    std::fs::write(dir.join("README.md"), "fixture")?;
    run(dir, &["git", "add", "."])?;
    run(dir, &["git", "commit", "-m", "init"])?;
    Ok(())
}

fn run(dir: &Path, args: &[&str]) -> Result<()> {
    let mut cmd = Command::new(args[0]);
    cmd.args(&args[1..]).current_dir(dir);
    let out = cmd.output().with_context(|| format!("run {:?}", args))?;
    if !out.status.success() {
        return Err(anyhow!("command failed: {:?}\nstdout:{}\nstderr:{}",
            args,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}
