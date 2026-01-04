use anyhow::{Context, Result};
use clap::Parser;
use orchestrator_core::{
    now_ms, validate_command, validate_workdir, ClaimRequest, ClaimResponse, CommandResult,
    CommandStatus, CompleteRequest, ExecBlockSpec, ExecutorKind, HeartbeatRequest, JobAssignment,
    JobResult, JobResultStatus,
};
use reqwest::Client;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::{sleep, timeout};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

mod slurm;
mod vcs;

#[derive(Debug, Parser)]
#[command(name = "orchestrator-agent", version, about = "Agent runner for the orchestrator")]
struct Cli {
    /// Daemon base URL, e.g. http://127.0.0.1:8080
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    daemon_url: String,

    /// Agent identifier (stable string). If omitted, a random UUID is used.
    #[arg(long)]
    agent_id: Option<String>,

    /// Poll interval when no job is available.
    #[arg(long, default_value_t = 2)]
    poll_interval_seconds: u64,

    /// Heartbeat interval while a job is running.
    #[arg(long, default_value_t = 10)]
    heartbeat_interval_seconds: u64,

    /// Optional agent capability tags, e.g. --cap slurm --cap gpu
    #[arg(long = "cap")]
    capabilities: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let agent_id = cli
        .agent_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let client = Client::new();

    info!("agent_id={agent_id} starting; daemon={}", cli.daemon_url);

    loop {
        let claim = ClaimRequest {
            agent_id: agent_id.clone(),
            capabilities: cli.capabilities.clone(),
        };

        let resp = client
            .post(format!("{}/v1/agent/claim", cli.daemon_url))
            .json(&claim)
            .send()
            .await
            .context("claim request")?
            .error_for_status()
            .context("claim status")?
            .json::<ClaimResponse>()
            .await
            .context("claim decode")?;

        if resp.assignment.is_none() {
            sleep(Duration::from_secs(cli.poll_interval_seconds)).await;
            continue;
        }

        let assignment = resp.assignment.unwrap();
        info!(
            "claimed job {} (run={}, stage={}, executor={:?})",
            assignment.job_id, assignment.run_id, assignment.stage_id, assignment.exec.executor
        );

        if let Err(e) = run_job(&client, &cli.daemon_url, &agent_id, &assignment, cli.heartbeat_interval_seconds).await {
            warn!("job {} execution error: {e:?}", assignment.job_id);
            // Best-effort: try to report failure (without per-command details)
            let ended = now_ms();
            let result = JobResult {
                status: JobResultStatus::Failed,
                started_ms: ended,
                ended_ms: ended,
                commands: vec![],
                output_revision: None,
                executor_ref: None,
            };
            let _ = complete_job(&client, &cli.daemon_url, &agent_id, &assignment, result).await;
        }
    }
}

async fn run_job(
    client: &Client,
    daemon_url: &str,
    agent_id: &str,
    assignment: &JobAssignment,
    heartbeat_interval_seconds: u64,
) -> Result<()> {
    let bundle_root = PathBuf::from(&assignment.bundle_root);
    let workspace_root = PathBuf::from(&assignment.workspace_root);

    std::fs::create_dir_all(&bundle_root)?;
    std::fs::create_dir_all(&workspace_root)?;

    // Persist assignment for debugging.
    let _ = std::fs::write(
        bundle_root.join("assignment.json"),
        serde_json::to_vec_pretty(assignment).unwrap_or_default(),
    );

    // Safety checks
    validate_workdir(&workspace_root, &workspace_root)?;
    for cmd in &assignment.exec.commands {
        validate_command(cmd, &workspace_root, &workspace_root)?;
    }

    // ORCH_TMP per job bundle
    let tmp_dir = bundle_root.join("tmp");
    std::fs::create_dir_all(&tmp_dir)?;

    // Heartbeat task
    let (stop_tx, mut stop_rx) = tokio::sync::watch::channel(false);
    let hb_client = client.clone();
    let hb_daemon = daemon_url.to_string();
    let hb_agent = agent_id.to_string();
    let hb_job = assignment.job_id.clone();
    let hb_token = assignment.lease.token.clone();
    let hb_interval = Duration::from_secs(heartbeat_interval_seconds);

    let hb_handle = tokio::spawn(async move {
        loop {
            if *stop_rx.borrow() {
                break;
            }
            let req = HeartbeatRequest {
                agent_id: hb_agent.clone(),
                job_id: hb_job.clone(),
                lease_token: hb_token.clone(),
            };
            let res = hb_client
                .post(format!("{}/v1/agent/heartbeat", hb_daemon))
                .json(&req)
                .send()
                .await;

            match res {
                Ok(r) => {
                    if !r.status().is_success() {
                        warn!("heartbeat non-200: {}", r.status());
                    }
                }
                Err(e) => warn!("heartbeat error: {e:?}"),
            }

            sleep(hb_interval).await;
        }
    });

    let started_ms = now_ms();
    let (commands, executor_ref, status) = match assignment.exec.executor {
        ExecutorKind::Local => {
            let (cmds, st) = run_local(&assignment.exec, &workspace_root, &bundle_root, &tmp_dir).await?;
            (cmds, None, st)
        }
        ExecutorKind::Slurm => {
            let (cmds, job_ref, st) = slurm::run_slurm(&assignment.exec, &workspace_root, &bundle_root, &tmp_dir).await?;
            (cmds, Some(job_ref), st)
        }
    };
    let ended_ms = now_ms();

    // Collect output revision
    let output_revision = vcs::current_revision(&workspace_root).await.ok();

    let result = JobResult {
        status,
        started_ms,
        ended_ms,
        commands,
        output_revision,
        executor_ref,
    };

    // stop heartbeat
    let _ = stop_tx.send(true);
    let _ = hb_handle.await;

    // Persist result to bundle too
    let _ = std::fs::write(
        bundle_root.join("job_result.json"),
        serde_json::to_vec_pretty(&result).unwrap_or_default(),
    );

    complete_job(client, daemon_url, agent_id, assignment, result).await?;

    Ok(())
}

async fn run_local(
    exec: &ExecBlockSpec,
    workspace_root: &Path,
    bundle_root: &Path,
    tmp_dir: &Path,
) -> Result<(Vec<CommandResult>, JobResultStatus)> {
    let cmd_dir = bundle_root.join("cmd");
    std::fs::create_dir_all(&cmd_dir)?;

    let mut results = Vec::new();
    let mut overall_ok = true;

    for (i, cmd) in exec.commands.iter().enumerate() {
        let idx = i as u32;
        let stdout_path = cmd_dir.join(format!("{:03}.stdout.log", idx));
        let stderr_path = cmd_dir.join(format!("{:03}.stderr.log", idx));
        let meta_path = cmd_dir.join(format!("{:03}.meta.json", idx));

        let started = now_ms();

        let stdout_file = std::fs::File::create(&stdout_path)?;
        let stderr_file = std::fs::File::create(&stderr_path)?;

        let mut proc = tokio::process::Command::new(&cmd.program);
        proc.args(&cmd.args)
            .current_dir(workspace_root)
            .stdout(std::process::Stdio::from(stdout_file))
            .stderr(std::process::Stdio::from(stderr_file))
            .env("ORCH_TMP", tmp_dir);

        // exec-level env
        for (k, v) in &exec.env {
            proc.env(k, v);
        }
        // cmd-level env
        for (k, v) in &cmd.env {
            proc.env(k, v);
        }

        let mut child = proc.spawn().with_context(|| format!("spawning {}", cmd.program))?;

        let status_res = if let Some(secs) = cmd.timeout_secs {
            match timeout(Duration::from_secs(secs), child.wait()).await {
                Ok(r) => r.map(Some).context("wait")?,
                Err(_) => {
                    let _ = child.kill().await;
                    None
                }
            }
        } else {
            Some(child.wait().await.context("wait")?)
        };

        let ended = now_ms();

        let (status, exit_code) = match status_res {
            Some(st) => {
                let code = st.code();
                if st.success() {
                    (CommandStatus::Succeeded, code)
                } else {
                    (CommandStatus::Failed, code)
                }
            }
            None => (CommandStatus::TimedOut, None),
        };

        if status != CommandStatus::Succeeded && !cmd.allow_failure {
            overall_ok = false;
        }

        let r = CommandResult {
            index: idx,
            name: cmd.name.clone(),
            program: cmd.program.clone(),
            args: cmd.args.clone(),
            status,
            exit_code,
            started_ms: started,
            ended_ms: ended,
            stdout_path: stdout_path.to_string_lossy().to_string(),
            stderr_path: stderr_path.to_string_lossy().to_string(),
        };

        let _ = std::fs::write(&meta_path, serde_json::to_vec_pretty(&r).unwrap_or_default());
        results.push(r);

        if !overall_ok {
            break;
        }
    }

    let overall = if overall_ok {
        JobResultStatus::Succeeded
    } else {
        JobResultStatus::Failed
    };

    Ok((results, overall))
}

async fn complete_job(
    client: &Client,
    daemon_url: &str,
    agent_id: &str,
    assignment: &JobAssignment,
    result: JobResult,
) -> Result<()> {
    let req = CompleteRequest {
        agent_id: agent_id.to_string(),
        job_id: assignment.job_id.clone(),
        lease_token: assignment.lease.token.clone(),
        result,
    };
    let resp = client
        .post(format!("{}/v1/agent/complete", daemon_url))
        .json(&req)
        .send()
        .await
        .context("complete request")?;
    if !resp.status().is_success() {
        warn!("complete returned {}", resp.status());
    }
    Ok(())
}
