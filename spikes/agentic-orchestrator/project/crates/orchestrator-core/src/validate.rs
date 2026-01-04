use crate::model::{CommandSpec, ExecBlockSpec, StageKind, WorkflowSpec};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("workflow has duplicate node id: {0}")]
    DuplicateNodeId(String),
    #[error("workflow edge references missing node: {0}")]
    MissingNode(String),
    #[error("workflow contains a cycle")]
    Cycle,
    #[error("exec_block node {0} is missing exec spec")]
    MissingExec(String),
    #[error("command argv cannot be empty")]
    EmptyArgv,
    #[error("disallowed command: {0}")]
    DisallowedCommand(String),
    #[error("shell execution requires allow_shell=true (found: {0})")]
    ShellNotAllowed(String),
}

pub fn validate_workflow(spec: &WorkflowSpec) -> Result<(), ValidationError> {
    // Unique node IDs
    let mut seen = std::collections::HashSet::new();
    for n in &spec.nodes {
        if !seen.insert(n.id.clone()) {
            return Err(ValidationError::DuplicateNodeId(n.id.clone()));
        }
        if n.kind == StageKind::ExecBlock && n.exec.is_none() {
            return Err(ValidationError::MissingExec(n.id.clone()));
        }
        if let Some(exec) = &n.exec {
            validate_exec_block(exec)?;
        }
    }

    // Edges reference existing nodes
    let ids: std::collections::HashSet<_> = spec.nodes.iter().map(|n| n.id.as_str()).collect();
    for e in &spec.edges {
        if !ids.contains(e.from.as_str()) {
            return Err(ValidationError::MissingNode(e.from.clone()));
        }
        if !ids.contains(e.to.as_str()) {
            return Err(ValidationError::MissingNode(e.to.clone()));
        }
    }

    // Cycle detection (DFS)
    let mut graph: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
    for e in &spec.edges {
        graph.entry(e.from.as_str()).or_default().push(e.to.as_str());
    }
    let mut temp = std::collections::HashSet::new();
    let mut perm = std::collections::HashSet::new();

    fn visit<'a>(
        v: &'a str,
        graph: &std::collections::HashMap<&'a str, Vec<&'a str>>,
        temp: &mut std::collections::HashSet<&'a str>,
        perm: &mut std::collections::HashSet<&'a str>,
    ) -> bool {
        if perm.contains(v) {
            return false;
        }
        if !temp.insert(v) {
            return true; // cycle
        }
        if let Some(ns) = graph.get(v) {
            for &n in ns {
                if visit(n, graph, temp, perm) {
                    return true;
                }
            }
        }
        temp.remove(v);
        perm.insert(v);
        false
    }

    for &node in &ids {
        if visit(node, &graph, &mut temp, &mut perm) {
            return Err(ValidationError::Cycle);
        }
    }

    Ok(())
}

pub fn validate_exec_block(exec: &ExecBlockSpec) -> Result<(), ValidationError> {
    for cmd in &exec.commands {
        validate_command(cmd, exec.allow_shell)?;
    }
    Ok(())
}

pub fn validate_command(cmd: &CommandSpec, allow_shell: bool) -> Result<(), ValidationError> {
    if cmd.argv.is_empty() {
        return Err(ValidationError::EmptyArgv);
    }
    let bin = cmd.argv[0].as_str();

    // Hard denylist for v0
    let deny = [
        "sudo", "shutdown", "reboot", "mkfs", "dd", "kill", "killall",
    ];
    if deny.contains(&bin) {
        return Err(ValidationError::DisallowedCommand(bin.to_string()));
    }

    // Shell is only allowed if allow_shell=true at exec_block level.
    let shell_bins = ["sh", "bash", "zsh", "fish"];
    if shell_bins.contains(&bin) && cmd.argv.get(1).map(|s| s.as_str()) == Some("-c") && !allow_shell
    {
        return Err(ValidationError::ShellNotAllowed(format!("{bin} -c")));
    }

    Ok(())
}
