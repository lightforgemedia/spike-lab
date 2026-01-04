use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use crate::model::{CommandSpec, ExecBlockSpec};

/// Validation decision for an exec block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Allow,
    Warn,
    Block,
}

/// Result of validating an exec block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationOutcome {
    pub decision: Decision,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub violations: Vec<String>,
}

impl ValidationOutcome {
    pub fn allow() -> Self {
        Self {
            decision: Decision::Allow,
            warnings: vec![],
            violations: vec![],
        }
    }

    pub fn warn(mut self, msg: impl Into<String>) -> Self {
        if self.decision != Decision::Block {
            self.decision = Decision::Warn;
        }
        self.warnings.push(msg.into());
        self
    }

    pub fn block(mut self, msg: impl Into<String>) -> Self {
        self.decision = Decision::Block;
        self.violations.push(msg.into());
        self
    }
}

/// Validate an exec block spec.
///
/// This is *guardrails*, not a sandbox. It aims to prevent accidental foot-guns
/// and make intent explicit.
///
/// Rules:
/// - Shell entrypoints are blocked by default unless `allow_shell=true`.
/// - Common destructive tools are blocked or constrained to relative paths.
pub fn validate_exec_block(spec: &ExecBlockSpec) -> ValidationOutcome {
    let mut out = ValidationOutcome::allow();

    // Workdir must be non-empty.
    if spec.workdir.trim().is_empty() {
        out = out.block("workdir must not be empty");
        return out;
    }

    // Commands must exist.
    if spec.commands.is_empty() {
        out = out.block("exec block must contain at least one command");
        return out;
    }

    for (idx, cmd) in spec.commands.iter().enumerate() {
        out = out_merge(out, validate_command(idx, spec, cmd));
        if out.decision == Decision::Block {
            // Keep going to accumulate violations? v0 stops early.
            // We'll stop early to reduce noise.
            break;
        }
    }

    out
}

fn out_merge(mut base: ValidationOutcome, other: ValidationOutcome) -> ValidationOutcome {
    // If either blocks -> block.
    if base.decision == Decision::Block || other.decision == Decision::Block {
        base.decision = Decision::Block;
    } else if base.decision == Decision::Warn || other.decision == Decision::Warn {
        base.decision = Decision::Warn;
    }
    base.warnings.extend(other.warnings);
    base.violations.extend(other.violations);
    base
}

fn validate_command(index: usize, block: &ExecBlockSpec, cmd: &CommandSpec) -> ValidationOutcome {
    let mut out = ValidationOutcome::allow();

    let prog = cmd.program.to_lowercase();

    // Shell entrypoints: too opaque to parse reliably.
    let is_shell = matches!(
        prog.as_str(),
        "sh" | "bash" | "zsh" | "fish" | "cmd" | "cmd.exe" | "powershell" | "pwsh"
    );

    if is_shell && !block.allow_shell {
        return out.block(format!(
            "cmd[{index}]: shell entrypoint '{}' is blocked (set allow_shell=true to override)",
            cmd.program
        ));
    }

    // Path boundary checks for cwd.
    if let Some(cwd) = &cmd.cwd {
        if !is_safe_rel_path(cwd) {
            out = out.block(format!(
                "cmd[{index}]: cwd '{cwd}' is not a safe relative path (no absolute paths or '..')"
            ));
            return out;
        }
    }

    // Heuristic destructive command handling.
    match prog.as_str() {
        "rm" | "rmdir" | "unlink" => {
            // Allow only if all non-flag args are safe relative paths.
            let paths = non_flag_args(&cmd.args);
            if paths.is_empty() {
                out = out.warn(format!(
                    "cmd[{index}]: '{prog}' with no explicit paths (might default to cwd)"
                ));
            }
            for p in paths {
                if !is_safe_rel_path(&p) {
                    out = out.block(format!(
                        "cmd[{index}]: '{prog}' path '{p}' is not allowed (must be relative and must not contain '..')"
                    ));
                    return out;
                }
            }
            out = out.warn(format!(
                "cmd[{index}]: destructive tool '{prog}' allowed only for safe relative paths within workdir '{}'",
                block.workdir
            ));
        }
        "mv" | "cp" | "chmod" | "chown" | "ln" => {
            // These are potentially destructive. We can't fully validate semantics, but we can reject obvious escapes.
            for a in non_flag_args(&cmd.args) {
                if !is_safe_rel_path(&a) {
                    out = out.block(format!(
                        "cmd[{index}]: '{prog}' arg '{a}' is not a safe relative path"
                    ));
                    return out;
                }
            }
            out = out.warn(format!("cmd[{index}]: tool '{prog}' is audited; ensure it only touches paths under workdir"));
        }
        "dd" | "mkfs" | "shutdown" | "reboot" => {
            out = out.block(format!("cmd[{index}]: '{prog}' is blocked by policy"));
        }
        _ => {}
    }

    // Workdir boundary note.
    if !Path::new(&block.workdir).is_absolute() {
        out = out.warn(format!(
            "cmd[{index}]: workdir '{}' is not absolute; boundaries are enforced lexically in v0",
            block.workdir
        ));
    }

    out
}

fn non_flag_args(args: &[String]) -> Vec<String> {
    args.iter()
        .filter(|a| !a.starts_with('-'))
        .cloned()
        .collect()
}

/// Returns true if `p` is a "safe" relative path:
/// - not absolute
/// - does not contain ParentDir (`..`)
pub fn is_safe_rel_path(p: &str) -> bool {
    let path = Path::new(p);
    if path.is_absolute() {
        return false;
    }
    for c in path.components() {
        if matches!(c, Component::ParentDir) {
            return false;
        }
        if matches!(c, Component::RootDir | Component::Prefix(_)) {
            return false;
        }
    }
    true
}
