#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Orchestrator agent: claims jobs and executes exec blocks.

use std::{path::PathBuf, time::Duration};

use clap::Parser;
use orchestrator_core::api::{ClaimRequest, ClaimResponse, CompleteRequest};
use orchestrator_core::model::{ExecBlockResult, JobStatus, StageConfig, StageKind};
use orchestrator_core::validation::{validate_exec_block, Decision};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod exec_runner;

#[derive(Parser, Debug)]
#[command(name = "orchestrator-agent")]
struct Args {
    /// Daemon base URL, e.g. http://127.0.0.1:3000
    #[arg(long, default_value = "http://127.0.0.1:3000")]
    daemon: String,

    /// Agent identifier.
    #[arg(long, default_value = "agent-1")]
    agent_id: String,

    /// Poll interval in milliseconds.
    #[arg(long, default_value_t = 1_000)]
    poll_ms: u64,

    /// Root directory for execution bundles.
    #[arg(long, default_value = ".orchestrator/runs")]
    runs_root: PathBuf,

    /// If set, claim and run at most one job then exit (useful for scripts).
    #[arg(long)]
    once: bool,

    /// Log level (env-filter syntax).
    #[arg(long, default_value = "info")]
    log: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(args.log))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tokio::fs::create_dir_all(&args.runs_root).await?;

    let client = reqwest::Client::new();

    loop {
        let claim_url = format!("{}/v1/agent/claim", args.daemon);
        let resp = client
            .post(&claim_url)
            .json(&ClaimRequest {
                agent_id: args.agent_id.clone(),
            })
            .send()
            .await?;

        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "claim failed");
            tokio::time::sleep(Duration::from_millis(args.poll_ms)).await;
            continue;
        }

        let ClaimResponse { lease } = resp.json::<ClaimResponse>().await?;
        let Some(lease) = lease else {
            if args.once {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(args.poll_ms)).await;
            continue;
        };

        tracing::info!(
            job_id = %lease.job_id,
            run_id = %lease.run_id,
            stage_id = %lease.stage_id,
            "claimed job"
        );

        // Defense-in-depth validation.
        let mut result = match (&lease.kind, &lease.config) {
            (StageKind::ExecBlock, StageConfig::ExecBlock(spec)) => {
                let outcome = validate_exec_block(spec);
                if outcome.decision == Decision::Block {
                    ExecBlockResult {
                        run_id: lease.run_id.clone(),
                        stage_id: lease.stage_id.clone(),
                        bundle_root: String::new(),
                        started_at_ms: orchestrator_core::now_ms(),
                        finished_at_ms: orchestrator_core::now_ms(),
                        status: JobStatus::Failed,
                        commands: vec![],
                        error: Some(format!("blocked by validation: {:?}", outcome.violations)),
                    }
                } else {
                    exec_runner::run_exec_block(&lease, &args.runs_root).await
                }
            }
            _ => ExecBlockResult {
                run_id: lease.run_id.clone(),
                stage_id: lease.stage_id.clone(),
                bundle_root: String::new(),
                started_at_ms: orchestrator_core::now_ms(),
                finished_at_ms: orchestrator_core::now_ms(),
                status: JobStatus::Failed,
                commands: vec![],
                error: Some("unsupported stage kind in v0".into()),
            },
        };

        // If runner succeeded but didn't set bundle_root (shouldn't happen), set error.
        if result.bundle_root.is_empty() {
            result.status = JobStatus::Failed;
        }

        let complete_url = format!("{}/v1/agent/complete", args.daemon);
        let resp = client
            .post(&complete_url)
            .json(&CompleteRequest {
                agent_id: args.agent_id.clone(),
                job_id: lease.job_id.clone(),
                lease_token: lease.lease_token.clone(),
                result,
            })
            .send()
            .await?;

        if resp.status().is_success() {
            tracing::info!(job_id = %lease.job_id, "completed job");
        } else {
            tracing::warn!(status = %resp.status(), job_id = %lease.job_id, "completion rejected");
        }

        if args.once {
            return Ok(());
        }
    }
}
