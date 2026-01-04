use crate::model::CommandSpec;
use anyhow::{anyhow, bail, Context, Result};
use std::path::{Component, Path, PathBuf};

/// Validate a command for "common sense" safety.
///
/// This is a best-effort guardrail. It is not a sandbox.
pub fn validate_command(command: &CommandSpec, workspace_root: &Path, workdir: &Path) -> Result<()> {
    validate_workdir(workspace_root, workdir)?;

    let prog = command.program.to_lowercase();

    // Shells are effectively arbitrary code execution; require explicit opt-in.
    let is_shell = matches!(
        prog.as_str(),
        "sh" | "bash" | "zsh" | "fish" | "dash" | "ksh" | "pwsh" | "powershell"
    );
    if is_shell && !command.allow_shell {
        bail!(
            "shell invocation '{}' is blocked by default. Set allow_shell=true to opt-in.",
            command.program
        );
    }

    // Hard deny list: these are almost always destructive in CI.
    let hard_deny = [
        "sudo",
        "doas",
        "shutdown",
        "reboot",
        "halt",
        "poweroff",
        "mkfs",
        "mkfs.ext4",
        "mkfs.xfs",
        "fdisk",
        "parted",
        "wipefs",
        "dd",
        "mount",
        "umount",
        "chown",
        "chmod",
    ];
    if hard_deny.iter().any(|p| p == &prog) {
        bail!("command '{}' is not allowed by policy", command.program);
    }

    // Special handling for common destructive commands: validate that any target paths
    // are contained within the workspace.
    match prog.as_str() {
        "rm" | "rmdir" => validate_path_args(command, workspace_root, workdir)?,
        "mv" => validate_path_args(command, workspace_root, workdir)?,
        "cp" => validate_path_args(command, workspace_root, workdir)?,
        "git" => validate_git(command)?,
        _ => {}
    }

    Ok(())
}

pub fn validate_workdir(workspace_root: &Path, workdir: &Path) -> Result<()> {
    let ws = canonicalize_best_effort(workspace_root)
        .with_context(|| format!("failed to canonicalize workspace_root: {}", workspace_root.display()))?;
    let wd = canonicalize_best_effort(workdir)
        .with_context(|| format!("failed to canonicalize workdir: {}", workdir.display()))?;

    if !is_within(&ws, &wd) {
        bail!(
            "workdir '{}' is outside workspace_root '{}' (possible symlink escape)",
            wd.display(),
            ws.display()
        );
    }
    Ok(())
}

fn validate_git(command: &CommandSpec) -> Result<()> {
    if command.args.is_empty() {
        return Ok(());
    }
    let sub = command.args[0].to_lowercase();
    if sub == "clean" {
        bail!("'git clean' is blocked by policy (destructive).");
    }
    if sub == "reset" {
        // Block 'git reset --hard'
        if command.args.iter().any(|a| a == "--hard") {
            bail!("'git reset --hard' is blocked by policy (destructive).");
        }
    }
    Ok(())
}

fn validate_path_args(command: &CommandSpec, workspace_root: &Path, workdir: &Path) -> Result<()> {
    // Identify candidate path arguments: for simple tools we treat non-flag args as paths.
    // This is conservative and may reject some valid commands.
    let mut targets: Vec<&str> = Vec::new();
    for arg in &command.args {
        if arg.starts_with('-') {
            continue;
        }
        targets.push(arg.as_str());
    }

    // rm with no args is suspicious
    if command.program.to_lowercase() == "rm" && targets.is_empty() {
        bail!("'rm' with no target paths is not allowed");
    }

    for t in targets {
        let resolved = resolve_under(workdir, t)?;
        // Symlink escape protection (best-effort): if path exists, canonicalize and re-check.
        if resolved.exists() {
            let ws = canonicalize_best_effort(workspace_root)?;
            let c = canonicalize_best_effort(&resolved)?;
            if !is_within(&ws, &c) {
                bail!(
                    "path '{}' resolves outside workspace_root '{}' (symlink escape)",
                    c.display(),
                    ws.display()
                );
            }
        } else {
            // If it doesn't exist, ensure its parent (if exists) is within workspace root.
            if let Some(parent) = resolved.parent() {
                if parent.exists() {
                    let ws = canonicalize_best_effort(workspace_root)?;
                    let cp = canonicalize_best_effort(parent)?;
                    if !is_within(&ws, &cp) {
                        bail!(
                            "target '{}' would be created outside workspace_root '{}' (via parent '{}')",
                            resolved.display(),
                            ws.display(),
                            cp.display()
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

/// Resolve a user-supplied path argument under a working directory.
///
/// Denies absolute paths and `..` traversal that would escape.
pub fn resolve_under(workdir: &Path, user_path: &str) -> Result<PathBuf> {
    let p = Path::new(user_path);
    if p.is_absolute() {
        bail!("absolute paths are not allowed: {}", user_path);
    }
    let joined = workdir.join(p);
    let normalized = normalize_path(&joined)?;
    // Ensure normalized path is within workdir (lexical check).
    let wd_norm = normalize_path(workdir)?;
    if !is_within(&wd_norm, &normalized) {
        bail!(
            "path '{}' escapes working directory '{}'",
            normalized.display(),
            wd_norm.display()
        );
    }
    Ok(normalized)
}

/// Normalize a path lexically (no filesystem access).
pub fn normalize_path(p: &Path) -> Result<PathBuf> {
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::Prefix(_) | Component::RootDir => {
                out.push(comp.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                // Pop only if possible; otherwise keep to avoid turning relative into absolute.
                if !out.pop() {
                    out.push("..");
                }
            }
            Component::Normal(s) => out.push(s),
        }
    }
    Ok(out)
}

pub fn is_within(parent: &Path, child: &Path) -> bool {
    let parent = parent.components().collect::<Vec<_>>();
    let child = child.components().collect::<Vec<_>>();
    child.len() >= parent.len() && child[..parent.len()] == parent[..]
}

/// Canonicalize if possible; if the full path doesn't exist, canonicalize the nearest existing parent.
pub fn canonicalize_best_effort(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return std::fs::canonicalize(path).map_err(|e| anyhow!(e));
    }
    let mut cur = path.to_path_buf();
    while !cur.exists() {
        if !cur.pop() {
            break;
        }
    }
    if cur.as_os_str().is_empty() {
        bail!("cannot canonicalize path '{}'", path.display());
    }
    let canon_parent = std::fs::canonicalize(&cur).map_err(|e| anyhow!(e))?;
    let suffix = path.strip_prefix(&cur).unwrap_or(path);
    Ok(canon_parent.join(suffix))
}
