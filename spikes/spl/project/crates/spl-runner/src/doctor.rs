use anyhow::{anyhow, Context, Result};
use std::path::Path;

use crate::util::run_cmd;
use crate::Config;
use spl_core::VcsType;

pub fn doctor(repo_root: &Path, cfg: &Config) -> Result<()> {
    // must be repo root
    let top = run_cmd(repo_root, "git", &["rev-parse", "--show-toplevel"]).context("git rev-parse")?;
    if Path::new(&top) != repo_root {
        return Err(anyhow!(
            "must run from repo root. expected={}, got={}",
            Path::new(&top).display(),
            repo_root.display()
        ));
    }

    // clean working tree
    let porcelain = run_cmd(repo_root, "git", &["status", "--porcelain"])?;
    if !porcelain.is_empty() {
        return Err(anyhow!("git working tree is dirty; commit or stash changes first"));
    }

    match cfg.vcs_type() {
        VcsType::Git => {
            // ok (git already checked)
            Ok(())
        }
        VcsType::Jj => {
            // ensure jj exists
            let out = std::process::Command::new("jj").arg("--version").output();
            match out {
                Ok(o) if o.status.success() => {}
                _ => return Err(anyhow!("jj not found on PATH; install with `brew install jj`")),
            }
            if cfg.vcs.jj_require_colocated.unwrap_or(true) && !repo_root.join(".jj").exists() {
                return Err(anyhow!("jj colocated repo required but .jj not found. Run: `jj git init --colocate`"));
            }
            Ok(())
        }
    }
}
