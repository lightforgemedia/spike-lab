#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Orchestrator daemon: owns embedded DB and schedules jobs.

use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod db;
mod http;
mod service;
mod vcs_jj;

use crate::service::OrchestratorService;

#[derive(Parser, Debug)]
#[command(name = "orchestrator-daemon")]
struct Args {
    /// Listen address, e.g. 127.0.0.1:3000
    #[arg(long, default_value = "127.0.0.1:3000")]
    listen: SocketAddr,

    /// Directory for embedded SurrealDB storage.
    #[arg(long, default_value = ".orchestrator/db")]
    db_dir: PathBuf,

    /// Lease duration in milliseconds.
    #[arg(long, default_value_t = 30_000)]
    lease_ms: i64,

    /// Reconcile interval in milliseconds.
    #[arg(long, default_value_t = 5_000)]
    reconcile_ms: u64,

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

    tokio::fs::create_dir_all(&args.db_dir).await?;

    let db = db::Db::connect(&args.db_dir).await?;
    db.apply_schema().await?;

    let svc = Arc::new(OrchestratorService::new(db, args.lease_ms));

    // Reconciler: periodically requeue expired leases.
    {
        let svc = Arc::clone(&svc);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(args.reconcile_ms));
            loop {
                interval.tick().await;
                if let Err(e) = svc.reconcile().await {
                    tracing::warn!(error = %e, "reconcile failed");
                }
            }
        });
    }

    let app = http::router(svc);

    tracing::info!(listen = %args.listen, "daemon starting");
    axum::serve(tokio::net::TcpListener::bind(args.listen).await?, app).await?;
    Ok(())
}
