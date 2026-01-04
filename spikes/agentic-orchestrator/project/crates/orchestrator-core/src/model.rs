use crate::time::EpochMs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A globally unique identifier (ULID as string by convention).
pub type Id = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutorKind {
    Local,
    Slurm,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SlurmSpec {
    /// Slurm partition (aka queue).
    pub partition: Option<String>,
    /// Time limit in Slurm format, e.g. "00:10:00" or "1-00:00:00".
    pub time_limit: Option<String>,
    /// Account to charge.
    pub account: Option<String>,
    /// QOS
    pub qos: Option<String>,
    /// Number of CPUs per task.
    pub cpus_per_task: Option<u32>,
    /// Memory, e.g. "4G".
    pub mem: Option<String>,
    /// Additional raw sbatch arguments (advanced).
    pub extra_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    /// Optional stable identifier for UI / debugging.
    pub name: Option<String>,
    /// Program to execute (no shell expansion).
    pub program: String,
    /// Arguments (no shell expansion).
    #[serde(default)]
    pub args: Vec<String>,
    /// Extra environment variables for this command.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// If true, command failure does not fail the whole exec block.
    #[serde(default)]
    pub allow_failure: bool,
    /// Optional soft timeout (seconds). Agent enforces best-effort.
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecBlockSpec {
    pub label: String,
    pub executor: ExecutorKind,
    /// Extra environment variables for all commands in this exec block.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub slurm: Option<SlurmSpec>,
    pub commands: Vec<CommandSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lease {
    pub agent_id: String,
    pub token: String,
    pub expires_ms: EpochMs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobAssignment {
    pub job_id: Id,
    pub run_id: Id,
    pub stage_id: Id,
    pub lease: Lease,

    /// Absolute path to a bundle directory where logs/artifacts should be written.
    pub bundle_root: String,
    /// Absolute path to the workspace directory to run commands in.
    pub workspace_root: String,

    /// The input repo revision this job is based on (jj commit id or git sha).
    pub input_revision: Option<String>,

    pub exec: ExecBlockSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimRequest {
    pub agent_id: String,
    /// Optional tags like ["slurm", "linux", "gpu"] for scheduling.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimResponse {
    /// If no job is currently available, this is None.
    pub assignment: Option<JobAssignment>,
    /// Server time for client-side skew detection.
    pub server_now_ms: EpochMs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    pub agent_id: String,
    pub job_id: Id,
    pub lease_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    pub ok: bool,
    pub new_expires_ms: Option<EpochMs>,
    pub server_now_ms: EpochMs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandStatus {
    Succeeded,
    Failed,
    TimedOut,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub index: u32,
    pub name: Option<String>,
    pub program: String,
    pub args: Vec<String>,

    pub status: CommandStatus,
    pub exit_code: Option<i32>,

    pub started_ms: EpochMs,
    pub ended_ms: EpochMs,

    pub stdout_path: String,
    pub stderr_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobResultStatus {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub status: JobResultStatus,
    pub started_ms: EpochMs,
    pub ended_ms: EpochMs,
    pub commands: Vec<CommandResult>,

    /// The output repo revision after running (jj commit id or git sha), if available.
    pub output_revision: Option<String>,

    /// Optional string for executor details (e.g. Slurm job id).
    pub executor_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteRequest {
    pub agent_id: String,
    pub job_id: Id,
    pub lease_token: String,
    pub result: JobResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteResponse {
    pub ok: bool,
    pub server_now_ms: EpochMs,
}
