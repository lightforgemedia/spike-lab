use anyhow::Context;
use clap::{Parser, Subcommand};
use orchestrator_core::{
    RunEnqueueRequest, RunEnqueueResponse, RunRow, StageApprovalRequest, StageApprovalResponse,
    StageRunRow, WorkflowDefRow, WorkflowVersionRow,
};

#[derive(Parser, Debug)]
#[command(name = "orchestratorctl")]
struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    Enqueue {
        #[arg(long)]
        daemon: String,
        #[arg(long)]
        file: String,
    },
    Run {
        #[command(subcommand)]
        run: RunCmd,
    },
    Workflows {
        #[command(subcommand)]
        wf: WorkflowCmd,
    },
    Stage {
        #[command(subcommand)]
        stage: StageCmd,
    },
}

#[derive(Subcommand, Debug)]
enum RunCmd {
    Get {
        #[arg(long)]
        daemon: String,
        #[arg(long)]
        run_id: String,
    },
    Stages {
        #[arg(long)]
        daemon: String,
        #[arg(long)]
        run_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum WorkflowCmd {
    List {
        #[arg(long)]
        daemon: String,
    },
    Versions {
        #[arg(long)]
        daemon: String,
        #[arg(long)]
        name: String,
    },
}

#[derive(Subcommand, Debug)]
enum StageCmd {
    Approve {
        #[arg(long)]
        daemon: String,
        #[arg(long)]
        stage_id: String,
        #[arg(long)]
        approver: String,
        #[arg(long)]
        note: Option<String>,
    },
    Reject {
        #[arg(long)]
        daemon: String,
        #[arg(long)]
        stage_id: String,
        #[arg(long)]
        approver: String,
        #[arg(long)]
        note: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let client = reqwest::Client::new();

    match args.cmd {
        Cmd::Enqueue { daemon, file } => {
            let bytes = tokio::fs::read(&file).await.context("read file")?;
            let req: RunEnqueueRequest = serde_json::from_slice(&bytes).context("parse json")?;
            let url = format!("{}/v1/runs/enqueue", daemon.trim_end_matches('/'));
            let resp: RunEnqueueResponse = client
                .post(url)
                .json(&req)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        Cmd::Run { run } => match run {
            RunCmd::Get { daemon, run_id } => {
                let url = format!("{}/v1/runs/{}", daemon.trim_end_matches('/'), run_id);
                let resp: RunRow = client
                    .get(url)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                println!("{}", serde_json::to_string_pretty(&resp)?);
            }
            RunCmd::Stages { daemon, run_id } => {
                let url = format!(
                    "{}/v1/runs/{}/stages",
                    daemon.trim_end_matches('/'),
                    run_id
                );
                let resp: Vec<StageRunRow> = client
                    .get(url)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                println!("{}", serde_json::to_string_pretty(&resp)?);
            }
        },
        Cmd::Workflows { wf } => match wf {
            WorkflowCmd::List { daemon } => {
                let url = format!("{}/v1/workflows", daemon.trim_end_matches('/'));
                let resp: Vec<WorkflowDefRow> = client
                    .get(url)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                println!("{}", serde_json::to_string_pretty(&resp)?);
            }
            WorkflowCmd::Versions { daemon, name } => {
                let url = format!(
                    "{}/v1/workflows/{}/versions",
                    daemon.trim_end_matches('/'),
                    name
                );
                let resp: Vec<WorkflowVersionRow> = client
                    .get(url)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                println!("{}", serde_json::to_string_pretty(&resp)?);
            }
        },
        Cmd::Stage { stage } => match stage {
            StageCmd::Approve {
                daemon,
                stage_id,
                approver,
                note,
            } => {
                let url = format!(
                    "{}/v1/stages/{}/approve",
                    daemon.trim_end_matches('/'),
                    stage_id
                );
                let req = StageApprovalRequest { approver, note };
                let resp: StageApprovalResponse = client
                    .post(url)
                    .json(&req)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                println!("{}", serde_json::to_string_pretty(&resp)?);
            }
            StageCmd::Reject {
                daemon,
                stage_id,
                approver,
                note,
            } => {
                let url = format!(
                    "{}/v1/stages/{}/reject",
                    daemon.trim_end_matches('/'),
                    stage_id
                );
                let req = StageApprovalRequest { approver, note };
                let resp: StageApprovalResponse = client
                    .post(url)
                    .json(&req)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                println!("{}", serde_json::to_string_pretty(&resp)?);
            }
        },
    }

    Ok(())
}
