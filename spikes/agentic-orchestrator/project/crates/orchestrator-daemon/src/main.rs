use axum::{routing::post, Router};
use clap::Parser;
use orchestrator_daemon::{api, config::DaemonConfig, db::Db, gc, scheduler};
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::signal;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{info, Level};
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Debug, Parser)]
#[command(name = "orchestrator-daemon", version, about = "Agentic CI orchestrator daemon")]
struct Cli {
    /// Where the HTTP API will listen, e.g. 127.0.0.1:8080
    #[arg(long, default_value = "127.0.0.1:8080")]
    listen: String,

    /// Project root (repo) directory. Used as the default project for demo endpoints.
    #[arg(long, default_value = ".")]
    project_root: PathBuf,

    /// SurrealKV directory for embedded SurrealDB.
    #[arg(long, default_value = ".orchestrator/db")]
    db_dir: PathBuf,

    /// Root directory for run bundles (logs, artifacts).
    #[arg(long, default_value = ".orchestrator/runs")]
    runs_root: PathBuf,

    /// Root directory for workspaces (jj workspaces / git worktrees).
    #[arg(long, default_value = ".orchestrator/workspaces")]
    workspaces_root: PathBuf,

    /// Lease duration in seconds for claimed jobs.
    #[arg(long, default_value_t = 60)]
    lease_seconds: u64,

    /// Scheduler tick interval in seconds.
    #[arg(long, default_value_t = 2)]
    scheduler_interval_seconds: u64,

    /// Enable garbage-collection of old run bundles / workspaces.
    #[arg(long, default_value_t = false)]
    gc_enabled: bool,

    /// GC tick interval in seconds.
    #[arg(long, default_value_t = 3600)]
    gc_interval_seconds: u64,

    /// GC max run age (days). Runs older than this may be deleted (unless kept by keep-last-n).
    #[arg(long, default_value_t = 30)]
    gc_max_run_age_days: u64,

    /// Keep the last N runs per project even if older than gc-max-run-age-days.
    #[arg(long, default_value_t = 20)]
    gc_keep_last_n: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fmt()
        .with_target(false)
                .with_max_level(Level::INFO)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

let project_root = std::fs::canonicalize(&cli.project_root)
    .unwrap_or_else(|_| cli.project_root.clone());

let db_dir = make_abs(&project_root, &cli.db_dir);
let runs_root = make_abs(&project_root, &cli.runs_root);
let workspaces_root = make_abs(&project_root, &cli.workspaces_root);


    let config = DaemonConfig {
        default_project_root: project_root,
        db_dir,
        runs_root,
        workspaces_root,
        lease_seconds: cli.lease_seconds,
        scheduler_interval_seconds: cli.scheduler_interval_seconds,
        gc_enabled: cli.gc_enabled,
        gc_interval_seconds: cli.gc_interval_seconds,
        gc_max_run_age_days: cli.gc_max_run_age_days,
        gc_keep_last_n: cli.gc_keep_last_n,
    };

    info!("starting daemon with config: {:?}", config);

    let db = Db::connect(&config).await?;
    db.bootstrap_schema().await?;

    let state = api::AppState::new(db.clone(), config.clone());

    // Background tasks
    scheduler::spawn_scheduler(state.clone());
    if config.gc_enabled {
        gc::spawn_gc(state.clone());
    }

    let app = Router::new()
        .route("/v1/demo/enqueue", post(api::enqueue_demo))
        .route("/v1/agent/claim", post(api::agent_claim))
        .route("/v1/agent/heartbeat", post(api::agent_heartbeat))
        .route("/v1/agent/complete", post(api::agent_complete))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr: SocketAddr = cli.listen.parse()?;
    info!("listening on http://{}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await?, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}


fn make_abs(base: &std::path::Path, p: &std::path::Path) -> std::path::PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    }
}

async fn shutdown_signal() {
    let _ = signal::ctrl_c().await;
    info!("shutdown requested");
}
