use crate::{config::DaemonConfig, db, vcs};
use anyhow::{Context, Result};
use axum::{extract::State, Json};
use orchestrator_core::{
    now_ms, ClaimRequest, ClaimResponse, CompleteRequest, CompleteResponse, ExecBlockSpec,
    ExecutorKind, HeartbeatRequest, HeartbeatResponse, JobAssignment, JobResultStatus, JobState,
    Lease,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Clone)]
pub struct AppState {
    pub db: db::Db,
    pub config: DaemonConfig,
}

impl AppState {
    pub fn new(db: db::Db, config: DaemonConfig) -> Self {
        Self { db, config }
    }
}

/// Demo endpoint: enqueue a 2-stage run (build -> test) in the configured project_root.
pub async fn enqueue_demo(
    State(state): State<AppState>,
) -> Result<Json<EnqueueDemoResponse>, axum::http::StatusCode> {
    let project_root = canonical(&state.config.default_project_root)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let project = state
        .db
        .get_or_create_project(project_root.to_str().unwrap())
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    // Capture base revision (jj commit id if available). This is the input for the first stage.
    let base_rev = vcs::current_revision(&project_root).await.ok();

    let run = state
        .db
        .create_run(&project.id, Some("demo run".into()), base_rev.clone())
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    // Two stages: build and test (test depends on build)
    let build_exec = ExecBlockSpec {
        label: "build".into(),
        executor: ExecutorKind::Local,
        env: Default::default(),
        slurm: None,
        commands: vec![orchestrator_core::CommandSpec {
            name: Some("echo-build".into()),
            program: "bash".into(),
            args: vec!["-lc".into(), "echo building && ls -la".into()],
            env: Default::default(),
            allow_failure: false,
            timeout_secs: Some(120),
            allow_shell: true,
        }],
    };

    let test_exec = ExecBlockSpec {
        label: "test".into(),
        executor: ExecutorKind::Local,
        env: Default::default(),
        slurm: None,
        commands: vec![orchestrator_core::CommandSpec {
            name: Some("echo-test".into()),
            program: "bash".into(),
            args: vec!["-lc".into(), "echo testing && pwd".into()],
            env: Default::default(),
            allow_failure: false,
            timeout_secs: Some(120),
            allow_shell: true,
        }],
    };

    let build_stage = state
        .db
        .create_stage(&run.id, "build", vec![], Some(build_exec))
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    // For downstream stages, `input_revision` will be filled in by the daemon after deps complete.
    let _test_stage = state
        .db
        .create_stage(&run.id, "test", vec![build_stage.id.clone()], Some(test_exec))
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    // Set build stage input revision (the run base revision).
    let _ = state
        .db
        .update_stage_state(
            &build_stage.id,
            db::StageState::Pending,
            base_rev.clone(),
            None,
            None,
        )
        .await;

    Ok(Json(EnqueueDemoResponse { run_id: run.id }))
}

pub async fn agent_claim(
    State(state): State<AppState>,
    Json(req): Json<ClaimRequest>,
) -> Result<Json<ClaimResponse>, axum::http::StatusCode> {
    let server_now = now_ms();

    let job = state
        .db
        .claim_next_job(&req.agent_id, &req.capabilities, state.config.lease_seconds)
        .await
        .map_err(|e| {
            warn!("claim_next_job error: {e:?}");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if job.is_none() {
        return Ok(Json(ClaimResponse {
            assignment: None,
            server_now_ms: server_now,
        }));
    }

    let job = job.unwrap();
    let lease: Lease = job.lease.clone().expect("lease set on claim");

    // Ensure run/stage exist and set stage running state.
    let stage = state
        .db
        .get_stage(&job.stage_id)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(axum::http::StatusCode::NOT_FOUND)?;

    // Resolve the project root for this run
    let run = state
        .db
        .get_run(&job.run_id)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(axum::http::StatusCode::NOT_FOUND)?;

    let project_root = canonical(&state.config.default_project_root)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    // Build/test pipelines usually want workspace + bundle dirs ready before execution.
    let bundle_root = PathBuf::from(&job.bundle_root);
    let workspace_root = PathBuf::from(&job.workspace_root);
    db::Db::ensure_dir(&bundle_root).map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    db::Db::ensure_dir(&workspace_root).map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    // Prepare VCS workspace at the input revision.
    if let Err(e) = vcs::prepare_workspace(&project_root, &workspace_root, job.input_revision.clone()).await {
        warn!("prepare_workspace failed (continuing): {e:?}");
    }

    let _ = state
        .db
        .update_stage_state(
            &stage.id,
            db::StageState::Running,
            stage.input_revision.clone(),
            stage.output_revision.clone(),
            stage.job_id.clone(),
        )
        .await;

    Ok(Json(ClaimResponse {
        assignment: Some(JobAssignment {
            job_id: job.id.clone(),
            run_id: job.run_id.clone(),
            stage_id: job.stage_id.clone(),
            lease,
            bundle_root: job.bundle_root.clone(),
            workspace_root: job.workspace_root.clone(),
            input_revision: job.input_revision.clone(),
            exec: job.exec.clone(),
        }),
        server_now_ms: server_now,
    }))
}

pub async fn agent_heartbeat(
    State(state): State<AppState>,
    Json(req): Json<HeartbeatRequest>,
) -> Result<Json<HeartbeatResponse>, axum::http::StatusCode> {
    let server_now = now_ms();
    let lease = state
        .db
        .renew_lease(
            &req.job_id,
            &req.agent_id,
            &req.lease_token,
            state.config.lease_seconds,
        )
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(HeartbeatResponse {
        ok: lease.is_some(),
        new_expires_ms: lease.map(|l| l.expires_ms),
        server_now_ms: server_now,
    }))
}

pub async fn agent_complete(
    State(state): State<AppState>,
    Json(req): Json<CompleteRequest>,
) -> Result<Json<CompleteResponse>, axum::http::StatusCode> {
    let server_now = now_ms();

    // Persist a copy of the result to the job bundle (best-effort).
    if let Ok(Some(job)) = state.db.get_job(&req.job_id).await {
        let bundle_root = PathBuf::from(job.bundle_root);
        let _ = std::fs::create_dir_all(&bundle_root);
        let path = bundle_root.join("result.json");
        if let Ok(bytes) = serde_json::to_vec_pretty(&req.result) {
            let _ = std::fs::write(path, bytes);
        }
    }

    let (job_state, stage_state) = match req.result.status {
        JobResultStatus::Succeeded => (JobState::Succeeded, db::StageState::Succeeded),
        JobResultStatus::Cancelled => (JobState::Cancelled, db::StageState::Failed),
        JobResultStatus::Failed => (JobState::Failed, db::StageState::Failed),
    };

    let ok = state
        .db
        .complete_job(
            &req.job_id,
            &req.agent_id,
            &req.lease_token,
            job_state.clone(),
            req.result.ended_ms,
            req.result.output_revision.clone(),
            req.result.executor_ref.clone(),
        )
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    if ok {
        // Update stage
        if let Ok(Some(job)) = state.db.get_job(&req.job_id).await {
            let _ = state
                .db
                .update_stage_state(
                    &job.stage_id,
                    stage_state,
                    job.input_revision.clone(),
                    req.result.output_revision.clone(),
                    Some(job.id.clone()),
                )
                .await;
        }

        // Wire downstream stages that have exactly one dep.
        if let Ok(Some(job)) = state.db.get_job(&req.job_id).await {
            if let Ok(stages) = state.db.list_stages_for_run(&job.run_id).await {
                for s in stages.iter().filter(|s| s.state == db::StageState::Pending) {
                    if s.deps.len() == 1 && s.deps[0] == job.stage_id {
                        let _ = state.db.update_stage_state(
                            &s.id,
                            db::StageState::Pending,
                            req.result.output_revision.clone(),
                            None,
                            s.job_id.clone(),
                        ).await;

                        // Also update the job input_revision so the agent will check out the right rev.
                        let _ = state.db.inner().query(
                            "UPDATE job SET input_revision=$inrev WHERE stage_id=$sid AND state='queued';"
                        )
                        .bind(("inrev", req.result.output_revision.clone()))
                        .bind(("sid", s.id.clone()))
                        .await;
                    }
                }
            }
        }

        // Update run state if all stages finished.
        if let Ok(Some(job)) = state.db.get_job(&req.job_id).await {
            if let Ok(stages) = state.db.list_stages_for_run(&job.run_id).await {
                let all_done = stages.iter().all(|s| matches!(s.state, db::StageState::Succeeded | db::StageState::Failed));
                if all_done {
                    let any_failed = stages.iter().any(|s| s.state == db::StageState::Failed);
                    let new_state = if any_failed { db::RunState::Failed } else { db::RunState::Succeeded };
                    let _ = state.db.update_run_state(&job.run_id, new_state).await;
                } else {
                    let _ = state.db.update_run_state(&job.run_id, db::RunState::Running).await;
                }
            }
        }
    }

    Ok(Json(CompleteResponse { ok, server_now_ms: server_now }))
}

fn default_paths(config: &DaemonConfig, run_id: &str, stage_id: &str, attempt: u32) -> (PathBuf, PathBuf) {
    let bundle_root = config
        .runs_root
        .join(run_id)
        .join("stages")
        .join(stage_id)
        .join(format!("attempt-{}", attempt));
    let workspace_root = config
        .workspaces_root
        .join(run_id)
        .join("stages")
        .join(stage_id)
        .join(format!("attempt-{}", attempt));
    (bundle_root, workspace_root)
}

fn canonical(p: &Path) -> Result<PathBuf> {
    std::fs::canonicalize(p).with_context(|| format!("canonicalize {}", p.display()))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnqueueDemoResponse {
    pub run_id: String,
}
