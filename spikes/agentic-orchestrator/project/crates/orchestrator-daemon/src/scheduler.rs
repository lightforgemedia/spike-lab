use crate::{api::AppState, db};
use anyhow::Result;
use orchestrator_core::now_ms;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::interval;
use tracing::{info, warn};

pub fn spawn_scheduler(state: AppState) {
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(state.config.scheduler_interval_seconds));
        loop {
            tick.tick().await;
            if let Err(e) = scheduler_tick(&state).await {
                warn!("scheduler tick error: {e:?}");
            }
        }
    });
}

async fn scheduler_tick(state: &AppState) -> Result<()> {
    // Select runs that are not terminal.
    let mut res = state
        .db
        .inner()
        .query("SELECT * FROM run WHERE state != 'succeeded' AND state != 'failed';")
        .await?;
    let runs: Vec<db::Run> = res.take(0)?;
    for run in runs {
        schedule_run(state, &run).await?;
    }
    Ok(())
}

async fn schedule_run(state: &AppState, run: &db::Run) -> Result<()> {
    let stages = state.db.list_stages_for_run(&run.id).await?;
    if stages.is_empty() {
        return Ok(());
    }

    let map: HashMap<_, _> = stages.iter().map(|s| (s.id.clone(), s.clone())).collect();

    let mut any_enqueued = false;

    for stage in stages.iter().filter(|s| s.state == db::StageState::Pending && s.job_id.is_none()) {
        // If no exec spec, block.
        let exec = match &stage.exec {
            Some(e) => e.clone(),
            None => {
                warn!("stage {} has no exec spec; blocking", stage.id);
                state
                    .db
                    .update_stage_state(&stage.id, db::StageState::Blocked, stage.input_revision.clone(), None, None)
                    .await?;
                continue;
            }
        };

        // Check dependencies.
        let mut deps_ok = true;
        let mut deps_failed = false;
        let mut dep_revs: Vec<String> = Vec::new();

        for dep_id in &stage.deps {
            let dep = match map.get(dep_id) {
                Some(d) => d,
                None => {
                    deps_ok = false;
                    break;
                }
            };
            match dep.state {
                db::StageState::Succeeded => {
                    if let Some(r) = dep.output_revision.clone() {
                        dep_revs.push(r);
                    }
                }
                db::StageState::Failed | db::StageState::Blocked => {
                    deps_failed = true;
                    deps_ok = false;
                    break;
                }
                _ => {
                    deps_ok = false;
                    break;
                }
            }
        }

        if deps_failed {
            state
                .db
                .update_stage_state(&stage.id, db::StageState::Blocked, stage.input_revision.clone(), None, None)
                .await?;
            continue;
        }
        if !deps_ok {
            continue;
        }

        // Determine input revision.
        let input_rev = if let Some(inrev) = stage.input_revision.clone() {
            Some(inrev)
        } else if stage.deps.is_empty() {
            run.base_revision.clone()
        } else if stage.deps.len() == 1 {
            dep_revs.first().cloned()
        } else {
            // Multi-parent handoff not implemented; require identical revs for now.
            let all_same = dep_revs.iter().all(|r| r == &dep_revs[0]);
            if all_same {
                dep_revs.first().cloned()
            } else {
                warn!(
                    "stage {} has multiple dep revisions {:?}; blocking (merge not implemented)",
                    stage.id, dep_revs
                );
                state
                    .db
                    .update_stage_state(&stage.id, db::StageState::Blocked, None, None, None)
                    .await?;
                continue;
            }
        };

        // Update stage input revision if we computed one.
        if stage.input_revision.is_none() && input_rev.is_some() {
            state
                .db
                .update_stage_state(
                    &stage.id,
                    db::StageState::Pending,
                    input_rev.clone(),
                    stage.output_revision.clone(),
                    None,
                )
                .await?;
        }

        // Attempt number = count(existing jobs)+1
        let mut c = state
            .db
            .inner()
            .query("SELECT count() AS c FROM job WHERE stage_id = $sid GROUP ALL;")
            .bind(("sid", stage.id.clone()))
            .await?;
        #[derive(serde::Deserialize)]
        struct CountRow {
            c: i64,
        }
        let rows: Vec<CountRow> = c.take(0)?;
        let attempt = rows.first().map(|r| r.c as u32).unwrap_or(0) + 1;

        let bundle_root = state
            .config
            .runs_root
            .join(&run.id)
            .join("stages")
            .join(&stage.id)
            .join(format!("attempt-{}", attempt));
        let workspace_root = state
            .config
            .workspaces_root
            .join(&run.id)
            .join("stages")
            .join(&stage.id)
            .join(format!("attempt-{}", attempt));

        std::fs::create_dir_all(&bundle_root)?;
        std::fs::create_dir_all(&workspace_root)?;

        let job = state
            .db
            .create_job(
                &run.id,
                &stage.id,
                exec,
                attempt,
                bundle_root.to_str().unwrap(),
                workspace_root.to_str().unwrap(),
                input_rev.clone(),
            )
            .await?;

        state
            .db
            .update_stage_state(
                &stage.id,
                db::StageState::Pending,
                input_rev,
                stage.output_revision.clone(),
                Some(job.id.clone()),
            )
            .await?;

        any_enqueued = true;
    }

    if any_enqueued {
        // Move run to Running once there's work.
        let _ = state.db.update_run_state(&run.id, db::RunState::Running).await;
    }

    Ok(())
}
