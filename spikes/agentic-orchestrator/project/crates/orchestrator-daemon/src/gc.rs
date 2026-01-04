use crate::{api::AppState, db};
use anyhow::Result;
use orchestrator_core::now_ms;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::interval;
use tracing::{info, warn};

pub fn spawn_gc(state: AppState) {
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(state.config.gc_interval_seconds));
        loop {
            tick.tick().await;
            if let Err(e) = gc_tick(&state).await {
                warn!("gc tick error: {e:?}");
            }
        }
    });
}

async fn gc_tick(state: &AppState) -> Result<()> {
    let now = now_ms();
    let max_age_ms = (state.config.gc_max_run_age_days as i64) * 24 * 60 * 60 * 1000;

    let mut res = state
        .db
        .inner()
        .query("SELECT * FROM run ORDER BY created_ms DESC;")
        .await?;
    let runs: Vec<db::Run> = res.take(0)?;

    let mut by_project: HashMap<String, Vec<db::Run>> = HashMap::new();
    for run in runs {
        by_project.entry(run.project_id.clone()).or_default().push(run);
    }

    for (_project, mut runs) in by_project {
        runs.sort_by_key(|r| -(r.created_ms as i64));

        for (idx, run) in runs.into_iter().enumerate() {
            if idx < state.config.gc_keep_last_n as usize {
                continue;
            }
            let age = now - run.created_ms;
            if age < max_age_ms {
                continue;
            }
            let run_dir = state.config.runs_root.join(&run.id);
            let ws_dir = state.config.workspaces_root.join(&run.id);
            warn!(
                "GC deleting run bundle '{}' and workspaces '{}' (age_ms={})",
                run_dir.display(),
                ws_dir.display(),
                age
            );
            let _ = std::fs::remove_dir_all(run_dir);
            let _ = std::fs::remove_dir_all(ws_dir);
        }
    }

    Ok(())
}
