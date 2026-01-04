use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// Kind of stage to execute.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StageKind {
    /// Executes a structured list of commands, captures logs, writes a bundle.
    ExecBlock,
}

/// Declarative workflow definition (graph).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSpec {
    /// Human name.
    pub name: String,
    /// Stages (nodes).
    pub stages: Vec<StageDef>,
    /// Dependency edges.
    ///
    /// Semantics: `from -> to` means **to depends on from**.
    pub edges: Vec<Edge>,
}

/// Stage definition (node).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageDef {
    /// Stable id within workflow.
    pub stage_id: String,
    /// Stage kind.
    pub kind: StageKind,
    /// Configuration for the kind.
    pub config: StageConfig,
}

/// Edge definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
}

/// Stage configuration (kind-specific).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StageConfig {
    ExecBlock(ExecBlockSpec),
}

/// Where/how an exec block should be executed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecutorSpec {
    /// Run commands locally on the agent host.
    Local,
    /// Submit a Slurm batch job which runs the commands.
    Slurm(SlurmSpec),
}

impl Default for ExecutorSpec {
    fn default() -> Self {
        Self::Local
    }
}

/// Slurm submission settings.
///
/// Notes:
/// - This assumes a shared filesystem so the agent and compute node can both
///   write/read the same bundle directory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlurmSpec {
    /// Partition to submit to.
    #[serde(default)]
    pub partition: Option<String>,
    /// Time limit, e.g. "00:10:00".
    #[serde(default)]
    pub time_limit: Option<String>,
    /// CPUs per task.
    #[serde(default)]
    pub cpus_per_task: Option<u32>,
    /// Memory in MB.
    #[serde(default)]
    pub mem_mb: Option<u32>,
    /// Additional raw `sbatch` arguments.
    #[serde(default)]
    pub extra_args: Vec<String>,
    /// Poll interval for `squeue` in milliseconds.
    #[serde(default = "default_slurm_poll_ms")]
    pub poll_ms: u64,
}

fn default_slurm_poll_ms() -> u64 {
    2_000
}

/// Execution block spec: multiple commands with structured args.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecBlockSpec {
    /// Working directory boundary (project workspace root).
    pub workdir: String,

    /// Execution backend.
    #[serde(default)]
    pub executor: ExecutorSpec,

    /// Commands to run in order.
    pub commands: Vec<CommandSpec>,

    /// If true, stop on first non-zero exit.
    #[serde(default = "default_halt_on_error")]
    pub halt_on_error: bool,

    /// If true, allow launching shell entrypoints (bash/sh/powershell/cmd).
    /// Default: false.
    #[serde(default)]
    pub allow_shell: bool,

    /// Environment variables applied to all commands (per-command env overrides apply after).
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

fn default_halt_on_error() -> bool {
    true
}

/// Command spec: program + args. No implicit shell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,

    /// Relative working directory under `ExecBlockSpec.workdir`.
    #[serde(default)]
    pub cwd: Option<String>,

    /// Extra env vars for this command.
    #[serde(default)]
    pub env: BTreeMap<String, String>,

    /// Soft timeout in seconds. Not enforced in v0.
    #[serde(default)]
    pub timeout_sec: Option<u64>,
}

/// Runtime status for a run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Succeeded,
    Failed,
}

/// Runtime status for a stage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    NeedsHuman,
    Skipped,
}

/// Runtime status for a job.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
}

/// High-level record representing a job lease returned to an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobLease {
    pub job_id: String,
    pub lease_token: String,
    pub run_id: String,
    pub stage_id: String,
    pub kind: StageKind,
    pub config: StageConfig,

    /// Lease expiry timestamp (ms).
    pub lease_expires_at_ms: i64,
}

/// Result of executing an exec-block stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecBlockResult {
    pub run_id: String,
    pub stage_id: String,

    /// Bundle root on disk (prototype assumes shared filesystem).
    pub bundle_root: String,

    /// Which executor was used.
    #[serde(default)]
    pub executor: ExecutorSpec,

    /// Slurm job id (if executor is Slurm).
    #[serde(default)]
    pub slurm_job_id: Option<String>,

    /// Extra files written into the bundle (relative paths).
    #[serde(default)]
    pub extra_files: Vec<String>,

    pub started_at_ms: i64,
    pub finished_at_ms: i64,

    pub status: JobStatus,

    pub commands: Vec<CommandResult>,
    pub error: Option<String>,
}

/// Per-command result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub index: usize,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,

    pub started_at_ms: i64,
    pub finished_at_ms: i64,

    pub exit_code: Option<i32>,
    pub status: JobStatus,

    /// Relative paths within bundle root.
    pub stdout_path: String,
    pub stderr_path: String,

    /// Spawn/runtime error string, if any.
    pub error: Option<String>,
}

/// Helper: generate an id string (ULID) for external identifiers.
pub fn ulid_string(id: Ulid) -> String {
    id.to_string()
}
