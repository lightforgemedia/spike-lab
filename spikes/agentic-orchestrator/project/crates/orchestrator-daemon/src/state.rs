use axum::response::IntoResponse;
use orchestrator_core::{
    now_ms, ArtifactUploadResponse, ExecAttemptRow, JobClaim, JobCompleteRequest, JobRow, JobStatus,
    RunRow, RunStatus, StageApprovalRequest, StageApprovalResponse, StageKind, StageRunRow,
    StageStatus,
};
use surrealdb::Surreal;
use thiserror::Error;
use tracing::{info, warn};
use ulid::Ulid;

use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Config {
    pub listen: String,
    pub db_url: String,
    pub ns: String,
    pub db: String,
    pub lease_ttl_secs: u64,
    pub scheduler_tick_ms: u64,

    /// Where the daemon stores uploaded artifact bundles.
    pub artifact_store_dir: PathBuf,

    /// Optional Google Drive service-account JSON key.
    ///
    /// If set together with `gdrive_folder_id`, the daemon will upload artifact bundles
    /// to Drive and return a `gdrive://<file_id>` URI.
    pub gdrive_service_account_json: Option<PathBuf>,

    /// Optional destination folder ID in Google Drive.
    pub gdrive_folder_id: Option<String>,

    /// Max request body bytes (applies to uploads).
    pub max_upload_bytes: usize,
}

impl Config {
    pub fn from_args() -> Self {
        use clap::Parser;

        #[derive(Parser, Debug)]
        #[command(name = "orchestrator-daemon")]
        struct Args {
            #[arg(long, default_value = "127.0.0.1:7777")]
            listen: String,
            #[arg(long, default_value = "surrealkv://.orchestrator/db")]
            db_url: String,
            #[arg(long, default_value = "orchestrator")]
            ns: String,
            #[arg(long, default_value = "orchestrator")]
            db: String,
            #[arg(long, default_value_t = 900)]
            lease_ttl_secs: u64,
            #[arg(long, default_value_t = 1000)]
            scheduler_tick_ms: u64,

            #[arg(long, default_value = ".orchestrator/daemon_artifacts")]
            artifact_store_dir: PathBuf,

            /// Path to a Google service-account JSON key used for Drive uploads.
            /// Requires --gdrive-folder-id.
            #[arg(long, requires = "gdrive_folder_id")]
            gdrive_service_account_json: Option<PathBuf>,

            /// Destination folder ID in Google Drive.
            /// Requires --gdrive-service-account-json.
            #[arg(long, requires = "gdrive_service_account_json")]
            gdrive_folder_id: Option<String>,

            #[arg(long, default_value_t = 100 * 1024 * 1024)]
            max_upload_bytes: usize,
        }

        let a = Args::parse();
        Self {
            listen: a.listen,
            db_url: a.db_url,
            ns: a.ns,
            db: a.db,
            lease_ttl_secs: a.lease_ttl_secs,
            scheduler_tick_ms: a.scheduler_tick_ms,
            artifact_store_dir: a.artifact_store_dir,
            gdrive_service_account_json: a.gdrive_service_account_json,
            gdrive_folder_id: a.gdrive_folder_id,
            max_upload_bytes: a.max_upload_bytes,
        }
    }
}

pub struct AppState {
    pub cfg: Config,
    pub db: Surreal<surrealdb::engine::any::Any>,
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl ApiError {
    pub fn bad_request<E: std::fmt::Display>(e: E) -> Self {
        Self::BadRequest(e.to_string())
    }
    pub fn not_found(msg: &str) -> Self {
        Self::NotFound(msg.to_string())
    }
    pub fn internal<E: std::fmt::Display>(e: E) -> Self {
        Self::Internal(e.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (code, msg) = match self {
            ApiError::BadRequest(m) => (axum::http::StatusCode::BAD_REQUEST, m),
            ApiError::NotFound(m) => (axum::http::StatusCode::NOT_FOUND, m),
            ApiError::Internal(m) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, m),
        };
        (code, msg).into_response()
    }
}

pub async fn claim_next_job(state: &Arc<AppState>, agent_id: &str) -> Result<Option<JobClaim>, ApiError> {
    let now = now_ms();
    let exp = now + (state.cfg.lease_ttl_secs as i64) * 1000;

    // Grab a small batch of candidates and attempt a conditional update.
    let candidates: Vec<JobRow> = state.db
        .query(r#"
            SELECT * FROM job
            WHERE status = "queued"
               OR (status = "running" AND lease_expires_at_ms < $now)
            ORDER BY created_at_ms
            LIMIT 10;
        "#)
        .bind(("now", now))
        .await
        .map_err(ApiError::internal)?
        .take(0)
        .map_err(ApiError::internal)?;

    for j in candidates {
        let token = Ulid::new().to_string();

        // Conditional update to claim.
        let mut resp = state.db
            .query(r#"
                UPDATE type::thing("job", $id)
                SET status = "running",
                    lease_owner = $agent,
                    lease_token = $token,
                    lease_expires_at_ms = $exp,
                    updated_at_ms = $now
                WHERE status = "queued"
                   OR (status = "running" AND lease_expires_at_ms < $now)
                RETURN AFTER;
            "#)
            .bind(("id", j.id.clone()))
            .bind(("agent", agent_id.to_string()))
            .bind(("token", token.clone()))
            .bind(("exp", exp))
            .bind(("now", now))
            .await
            .map_err(ApiError::internal)?;

        let updated: Option<JobRow> = resp.take(0).map_err(ApiError::internal)?;
        if updated.is_none() {
            continue; // raced; try next
        }

        let stage: Option<StageRunRow> = state.db
            .select(("stage_run", j.stage_id.as_str()))
            .await
            .map_err(ApiError::internal)?;
        let Some(stage) = stage else { return Err(ApiError::Internal("stage missing for claimed job".into())); };

        let run: Option<RunRow> = state.db
            .select(("run", stage.run_id.as_str()))
            .await
            .map_err(ApiError::internal)?;
        let Some(run) = run else { return Err(ApiError::Internal("run missing for claimed job".into())); };

        // Mark stage as running for visibility
        if stage.kind == StageKind::ExecBlock && stage.status != StageStatus::Running {
            let _ : Option<StageRunRow> = state.db
                .update(("stage_run", stage.id.as_str()))
                .merge(serde_json::json!({"status": StageStatus::Running, "updated_at_ms": now}))
                .await
                .map_err(ApiError::internal)?;
        }

        // Best-effort input revisions: upstream output_revision values
        let mut input_revs = Vec::new();
        for dep in &stage.deps {
            let dep_stage: Option<StageRunRow> = state.db
                .select(("stage_run", dep.as_str()))
                .await
                .map_err(ApiError::internal)?;
            if let Some(ds) = dep_stage {
                if let Some(r) = ds.output_revision {
                    input_revs.push(r);
                }
            }
        }

        let claim = JobClaim {
            job_id: j.id.clone(),
            stage_id: stage.id.clone(),
            run_id: stage.run_id.clone(),
            lease_token: token,
            lease_expires_at_ms: exp,
            project_path: run.project_path.clone(),
            kind: stage.kind,
            exec: stage.exec.clone(),
            input_revisions: input_revs,
        };
        return Ok(Some(claim));
    }

    Ok(None)
}

pub async fn complete_job(state: &Arc<AppState>, req: JobCompleteRequest) -> Result<(), ApiError> {
    let job: Option<JobRow> = state.db
        .select(("job", req.job_id.as_str()))
        .await
        .map_err(ApiError::internal)?;
    let Some(job) = job else { return Err(ApiError::not_found("job not found")); };

    // Idempotency: if already terminal, accept
    if matches!(job.status, JobStatus::Succeeded | JobStatus::Failed) {
        return Ok(());
    }

    // Validate lease token
    if job.lease_token.as_deref() != Some(req.lease_token.as_str()) {
        return Err(ApiError::BadRequest("lease_token mismatch".into()));
    }

    let now = now_ms();

    let stage: Option<StageRunRow> = state.db
        .select(("stage_run", job.stage_id.as_str()))
        .await
        .map_err(ApiError::internal)?;
    let Some(stage) = stage else { return Err(ApiError::Internal("stage missing for completed job".into())); };

    // Update job status
    let final_job_status = if req.succeeded { JobStatus::Succeeded } else { JobStatus::Failed };
    let _updated: Option<JobRow> = state.db
        .update(("job", req.job_id.as_str()))
        .merge(serde_json::json!({
            "status": final_job_status,
            "updated_at_ms": now,
        }))
        .await
        .map_err(ApiError::internal)?;

    // Persist exec_attempt
    let attempt_id = req.attempt.id.clone();
    let _created: Option<ExecAttemptRow> = state.db
        .create(("exec_attempt", attempt_id.as_str()))
        .content(req.attempt.clone())
        .await
        .map_err(ApiError::internal)?;

    // Determine stage outcome + retry policy
    if req.succeeded {
        let _updated: Option<StageRunRow> = state.db
            .update(("stage_run", stage.id.as_str()))
            .merge(serde_json::json!({
                "status": StageStatus::Succeeded,
                "updated_at_ms": now,
                "output_revision": req.output_revision,
            }))
            .await
            .map_err(ApiError::internal)?;
    } else {
        let next_attempts = stage.attempts_used.saturating_add(1);
        if next_attempts < stage.max_attempts {
            // Retry: return stage to pending; scheduler will create a new job attempt.
            let _updated: Option<StageRunRow> = state.db
                .update(("stage_run", stage.id.as_str()))
                .merge(serde_json::json!({
                    "status": StageStatus::Pending,
                    "attempts_used": next_attempts,
                    "updated_at_ms": now,
                }))
                .await
                .map_err(ApiError::internal)?;
        } else {
            // Terminal failure
            let _updated: Option<StageRunRow> = state.db
                .update(("stage_run", stage.id.as_str()))
                .merge(serde_json::json!({
                    "status": StageStatus::Failed,
                    "attempts_used": next_attempts,
                    "updated_at_ms": now,
                }))
                .await
                .map_err(ApiError::internal)?;

            // Mark run failed
            let _ = state.db
                .update(("run", stage.run_id.as_str()))
                .merge(serde_json::json!({"status": RunStatus::Failed, "updated_at_ms": now}))
                .await;
        }
    }

    info!(job_id=%req.job_id, succeeded=req.succeeded, "job completed");
    Ok(())
}

pub async fn approve_stage(
    state: &Arc<AppState>,
    stage_id: &str,
    req: StageApprovalRequest,
) -> Result<StageApprovalResponse, ApiError> {
    let stage: Option<StageRunRow> = state.db
        .select(("stage_run", stage_id))
        .await
        .map_err(ApiError::internal)?;
    let Some(stage) = stage else { return Err(ApiError::not_found("stage not found")); };

    if stage.kind != StageKind::Gate {
        return Err(ApiError::bad_request("stage is not a gate"));
    }
    if stage.status != StageStatus::NeedsHuman {
        return Err(ApiError::bad_request("stage is not awaiting approval"));
    }

    let now = now_ms();
    let approval = serde_json::json!({
        "status": "approved",
        "approver": req.approver,
        "note": req.note,
        "at_ms": now,
    });

    let _updated: Option<StageRunRow> = state.db
        .update(("stage_run", stage_id))
        .merge(serde_json::json!({
            "status": StageStatus::Succeeded,
            "approval": approval,
            "updated_at_ms": now,
        }))
        .await
        .map_err(ApiError::internal)?;

    Ok(StageApprovalResponse { stage_id: stage_id.to_string(), status: StageStatus::Succeeded })
}

pub async fn reject_stage(
    state: &Arc<AppState>,
    stage_id: &str,
    req: StageApprovalRequest,
) -> Result<StageApprovalResponse, ApiError> {
    let stage: Option<StageRunRow> = state.db
        .select(("stage_run", stage_id))
        .await
        .map_err(ApiError::internal)?;
    let Some(stage) = stage else { return Err(ApiError::not_found("stage not found")); };

    if stage.kind != StageKind::Gate {
        return Err(ApiError::bad_request("stage is not a gate"));
    }
    if stage.status != StageStatus::NeedsHuman {
        return Err(ApiError::bad_request("stage is not awaiting approval"));
    }

    let now = now_ms();
    let approval = serde_json::json!({
        "status": "rejected",
        "approver": req.approver,
        "note": req.note,
        "at_ms": now,
    });

    let _updated: Option<StageRunRow> = state.db
        .update(("stage_run", stage_id))
        .merge(serde_json::json!({
            "status": StageStatus::Failed,
            "approval": approval,
            "updated_at_ms": now,
        }))
        .await
        .map_err(ApiError::internal)?;

    // Mark run failed
    let _ = state.db
        .update(("run", stage.run_id.as_str()))
        .merge(serde_json::json!({"status": RunStatus::Failed, "updated_at_ms": now}))
        .await;

    Ok(StageApprovalResponse { stage_id: stage_id.to_string(), status: StageStatus::Failed })
}

pub async fn store_artifact_bundle(
    state: &Arc<AppState>,
    job_id: &str,
    bytes: axum::body::Bytes,
) -> Result<ArtifactUploadResponse, ApiError> {
    // Lookup job -> stage -> run for a good directory structure
    let job: Option<JobRow> = state.db
        .select(("job", job_id))
        .await
        .map_err(ApiError::internal)?;
    let Some(job) = job else { return Err(ApiError::not_found("job not found")); };

    let stage: Option<StageRunRow> = state.db
        .select(("stage_run", job.stage_id.as_str()))
        .await
        .map_err(ApiError::internal)?;
    let Some(stage) = stage else { return Err(ApiError::Internal("stage missing for job".into())); };

    let run: Option<RunRow> = state.db
        .select(("run", stage.run_id.as_str()))
        .await
        .map_err(ApiError::internal)?;
    let Some(run) = run else { return Err(ApiError::Internal("run missing for stage".into())); };

    let dir = state
        .cfg
        .artifact_store_dir
        .join(&run.id)
        .join(&stage.id)
        .join(&job.id);
    tokio::fs::create_dir_all(&dir).await.map_err(ApiError::internal)?;
    let path = dir.join("bundle.zip");
    let bytes_vec = bytes.to_vec();
    tokio::fs::write(&path, &bytes_vec)
        .await
        .map_err(ApiError::internal)?;

    // Default to local path.
    let mut artifact_uri = path.to_string_lossy().to_string();

    // Optional Drive upload.
    if let (Some(sa_json), Some(folder_id)) = (
        state.cfg.gdrive_service_account_json.as_ref(),
        state.cfg.gdrive_folder_id.as_deref(),
    ) {
        let filename = format!(
            "bundle-{}-{}-{}.zip",
            run.id,
            stage.id,
            job.id
        );

        match crate::gdrive::upload_zip_bytes(sa_json, folder_id, &filename, bytes_vec).await {
            Ok(outcome) => {
                artifact_uri = format!("gdrive://{}", outcome.file_id);
            }
            Err(e) => {
                warn!(
                    job_id = %job.id,
                    stage_id = %stage.id,
                    run_id = %run.id,
                    err = %e,
                    "google drive upload failed; falling back to local artifact path"
                );
            }
        }
    }

    Ok(ArtifactUploadResponse { artifact_uri })
}
