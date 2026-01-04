use std::collections::VecDeque;
use std::path::PathBuf;

use orchestrator_core::{
    api::{CompleteResponse, DemoEnqueueRequest},
    model::{
        CommandSpec, Edge, ExecBlockResult, ExecBlockSpec, JobLease, JobStatus, StageConfig, StageDef,
        StageKind, WorkflowSpec,
    },
    new_ulid, now_ms,
    validation::{validate_exec_block, Decision},
};
use surrealdb::sql::Thing;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::db::Db;
use crate::vcs_jj;

/// Main service implementing orchestration logic.
pub struct OrchestratorService {
    db: Db,
    lease_ms: i64,
    claim_lock: Mutex<()>,
}

impl OrchestratorService {
    pub fn new(db: Db, lease_ms: i64) -> Self {
        Self {
            db,
            lease_ms,
            claim_lock: Mutex::new(()),
        }
    }

    /// Periodic reconcile: requeue jobs with expired leases.
    pub async fn reconcile(&self) -> anyhow::Result<()> {
        let now = now_ms();
        let q = r#"
            UPDATE job
            SET status = 'queued', lease_owner = NONE, lease_token = NONE, lease_expires_at_ms = NONE
            WHERE status = 'running' AND lease_expires_at_ms != NONE AND lease_expires_at_ms < $now;
        "#;
        self.db.inner().query(q).bind(("now", now)).await?;

        // Also clear any expired run owner leases.
        let q = r#"
            UPDATE run
            SET owner_agent = NONE, owner_lease_expires_at_ms = NONE
            WHERE owner_lease_expires_at_ms != NONE AND owner_lease_expires_at_ms < $now;
        "#;
        self.db.inner().query(q).bind(("now", now)).await?;
        Ok(())
    }

    /// Demo helper: creates a small 3-stage workflow and enqueues a run.
    pub async fn demo_enqueue(&self, req: DemoEnqueueRequest) -> anyhow::Result<(String, String)> {
        // Create workflow spec (A -> B -> C).
        let wf = demo_workflow(&req.project_path);

        // Create intent.
        let intent_ulid = new_ulid().to_string();
        let intent_id = format!("intent:{intent_ulid}");
        let created_at_ms = now_ms();

        let intent_rec: serde_json::Value = serde_json::json!({
            "project_path": req.project_path,
            "description": req.description,
            "created_at_ms": created_at_ms,
            "workflow_name": wf.name,
        });

        let _: Option<serde_json::Value> = self
            .db
            .inner()
            .create(("intent", intent_ulid))
            .content(intent_rec)
            .await?;

        // Create run.
        let run_ulid = new_ulid().to_string();
        let run_id = format!("run:{run_ulid}");
        let run_rec: serde_json::Value = serde_json::json!({
            "intent_id": intent_id,
            "workflow_name": wf.name,
            "status": "running",
            "created_at_ms": created_at_ms,
            "owner_agent": null,
            "owner_lease_expires_at_ms": null,
        });
        let _: Option<serde_json::Value> = self
            .db
            .inner()
            .create(("run", run_ulid))
            .content(run_rec)
            .await?;

        // Materialize stage runs and edges.
        self.materialize_run(&run_id, &wf).await?;

        // Enqueue initial runnable stages.
        self.enqueue_runnable_stages(&run_id).await?;

        Ok((intent_id, run_id))
    }

    /// Claim next queued job (lease-based).
    pub async fn claim_job(&self, agent_id: String) -> anyhow::Result<Option<JobLease>> {
        let _guard = self.claim_lock.lock().await;

        // Pick the oldest queued job which is compatible with the agent.
        //
        // To keep multi-agent behavior sane while we introduce per-run JJ workspaces,
        // we "pin" a run to a single agent for the duration of the run (with an
        // expiring owner lease, see `reconcile`).
        let q_select = r#"
            SELECT * FROM job
            WHERE status = 'queued'
            ORDER BY created_at_ms ASC
            LIMIT 10;
        "#;

        let mut resp = self.db.inner().query(q_select).await?;
        let jobs: Vec<serde_json::Value> = resp.take(0)?;

        for candidate in jobs {
            let candidate_job_id = job_id_str(&candidate)?;
            let run_id = candidate
                .get("run_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("job missing run_id"))?
                .to_string();

            // Check run owner lease.
            let q_run = r#"SELECT * FROM $run;"#;
            let mut resp = self
                .db
                .inner()
                .query(q_run)
                .bind(("run", thing_from_id(&run_id)))
                .await?;
            let runs: Vec<serde_json::Value> = resp.take(0)?;
            let Some(run) = runs.into_iter().next() else {
                continue;
            };

            let owner = run.get("owner_agent").and_then(|v| v.as_str());
            if let Some(o) = owner {
                if o != agent_id {
                    continue;
                }
            }

            let lease_token = Uuid::new_v4().to_string();
            let now = now_ms();
            let lease_expires_at_ms = now + self.lease_ms;

            // Claim the job.
            let q_update = r#"
                UPDATE $job
                SET status = 'running',
                    lease_owner = $agent_id,
                    lease_token = $lease_token,
                    lease_expires_at_ms = $lease_expires_at_ms,
                    started_at_ms = $now
                RETURN AFTER;
            "#;

            let mut resp = self
                .db
                .inner()
                .query(q_update)
                .bind(("job", thing_from_id(&candidate_job_id)))
                .bind(("agent_id", agent_id.clone()))
                .bind(("lease_token", lease_token.clone()))
                .bind(("lease_expires_at_ms", lease_expires_at_ms))
                .bind(("now", now))
                .await?;

            let updated: Vec<serde_json::Value> = resp.take(0)?;
            let job = updated
                .into_iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("job update returned no record"))?;

            // Set or extend run ownership.
            let owner_lease_expires_at_ms = now + (self.lease_ms * 3);
            let q_owner = r#"
                UPDATE $run
                SET owner_agent = $agent_id,
                    owner_lease_expires_at_ms = $expires
                RETURN NONE;
            "#;
            let _ = self
                .db
                .inner()
                .query(q_owner)
                .bind(("run", thing_from_id(&run_id)))
                .bind(("agent_id", agent_id.clone()))
                .bind(("expires", owner_lease_expires_at_ms))
                .await?;

            // Mark stage_run running.
            if let Some(stage_run_id) = job.get("stage_run_id").and_then(|v| v.as_str()) {
                let q_stage = r#"
                    UPDATE $sr
                    SET status = 'running'
                    RETURN NONE;
                "#;
                let _ = self
                    .db
                    .inner()
                    .query(q_stage)
                    .bind(("sr", thing_from_id(stage_run_id)))
                    .await?;
            }

            // Resolve job kind/config.
            let stage_id = job
                .get("stage_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("job missing stage_id"))?
                .to_string();
            let kind: StageKind = serde_json::from_value(
                job.get("kind")
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("job missing kind"))?,
            )?;
            let mut config: StageConfig = serde_json::from_value(
                job.get("config")
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("job missing config"))?,
            )?;

            // JJ workspace assignment (best-effort).
            if let StageConfig::ExecBlock(exec) = &config {
                let project_root = PathBuf::from(&exec.workdir);
                let ws_path = project_root
                    .join(".orchestrator")
                    .join("workspaces")
                    .join(sanitize_component(&run_id))
                    .join(sanitize_component(&agent_id));
                let ws_name = format!("{}-{}", sanitize_component(&agent_id), sanitize_component(&run_id));

                let mut effective_workdir = project_root.clone();
                if let Err(e) = vcs_jj::ensure_jj_initialized(&project_root).await {
                    tracing::warn!(error = %e, project = %project_root.display(), "jj not available; running in project root");
                } else if let Err(e) = vcs_jj::ensure_workspace(&project_root, &ws_path, &ws_name).await {
                    tracing::warn!(error = %e, workspace = %ws_path.display(), "failed to create jj workspace; running in project root");
                } else {
                    effective_workdir = ws_path.clone();
                }

                if effective_workdir != project_root {
                    // Patch job config.workdir so agents run inside the workspace.
                    let mut patched = exec.clone();
                    patched.workdir = effective_workdir.display().to_string();
                    config = StageConfig::ExecBlock(patched);

                    let q_patch = r#"
                        UPDATE $job
                        SET config = $config,
                            workspace_path = $workspace_path
                        RETURN NONE;
                    "#;
                    let _ = self
                        .db
                        .inner()
                        .query(q_patch)
                        .bind(("job", thing_from_id(&candidate_job_id)))
                        .bind(("config", serde_json::to_value(&config)?))
                        .bind(("workspace_path", effective_workdir.display().to_string()))
                        .await?;

                    // Also store on stage_run for easier debugging.
                    if let Some(stage_run_id) = job.get("stage_run_id").and_then(|v| v.as_str()) {
                        let q_sr = r#"UPDATE $sr SET workspace_path = $p RETURN NONE;"#;
                        let _ = self
                            .db
                            .inner()
                            .query(q_sr)
                            .bind(("sr", thing_from_id(stage_run_id)))
                            .bind(("p", effective_workdir.display().to_string()))
                            .await?;
                    }
                }
            }

            // Build lease response.
            return Ok(Some(JobLease {
                job_id: candidate_job_id,
                lease_token,
                run_id,
                stage_id,
                kind,
                config,
                lease_expires_at_ms,
            }));
        }

        Ok(None)
    }

    /// Complete a job and schedule downstream.
    pub async fn complete_job(
        &self,
        agent_id: String,
        job_id: String,
        lease_token: String,
        result: ExecBlockResult,
    ) -> anyhow::Result<CompleteResponse> {
        // Fetch job.
        let q = r#"SELECT * FROM $job;"#;
        let mut resp = self
            .db
            .inner()
            .query(q)
            .bind(("job", thing_from_id(&job_id)))
            .await?;
        let jobs: Vec<serde_json::Value> = resp.take(0)?;
        let Some(job) = jobs.into_iter().next() else {
            return Ok(CompleteResponse {
                ok: false,
                message: Some("job not found".into()),
            });
        };

        // Idempotency: if already finished, accept.
        if let Some(status) = job.get("status").and_then(|v| v.as_str()) {
            if status == "succeeded" || status == "failed" {
                return Ok(CompleteResponse {
                    ok: true,
                    message: Some("already completed".into()),
                });
            }
        }

        // Lease check.
        let owner_ok = job
            .get("lease_owner")
            .and_then(|v| v.as_str())
            .map(|s| s == agent_id)
            .unwrap_or(false);
        let token_ok = job
            .get("lease_token")
            .and_then(|v| v.as_str())
            .map(|s| s == lease_token)
            .unwrap_or(false);

        if !owner_ok || !token_ok {
            return Ok(CompleteResponse {
                ok: false,
                message: Some("lease mismatch".into()),
            });
        }

        // Persist artifact pointer.
        let artifact_ulid = new_ulid().to_string();
        let artifact_id = format!("artifact:{artifact_ulid}");
        let artifact_rec = serde_json::json!({
            "run_id": result.run_id,
            "stage_id": result.stage_id,
            "bundle_root": result.bundle_root,
            "created_at_ms": now_ms(),
        });
        let _: Option<serde_json::Value> = self
            .db
            .inner()
            .create(("artifact", artifact_ulid))
            .content(artifact_rec)
            .await?;

        // Update job.
        let finished_at_ms = result.finished_at_ms;
        let job_status_str = match result.status {
            JobStatus::Succeeded => "succeeded",
            JobStatus::Failed => "failed",
            JobStatus::Queued | JobStatus::Running => "failed",
        };

        let q_update = r#"
            UPDATE $job
            SET status = $status,
                finished_at_ms = $finished_at_ms,
                artifact_id = $artifact_id,
                result = $result,
                lease_owner = NONE,
                lease_token = NONE,
                lease_expires_at_ms = NONE
            RETURN AFTER;
        "#;

        let _ = self
            .db
            .inner()
            .query(q_update)
            .bind(("job", thing_from_id(&job_id)))
            .bind(("status", job_status_str))
            .bind(("finished_at_ms", finished_at_ms))
            .bind(("artifact_id", artifact_id.clone()))
            .bind(("result", serde_json::to_value(&result)?))
            .await?;

        // Update stage_run.
        if let Some(stage_run_id) = job.get("stage_run_id").and_then(|v| v.as_str()) {
            let stage_status = if job_status_str == "succeeded" {
                "succeeded"
            } else {
                "failed"
            };
            let q_stage = r#"
                UPDATE $sr
                SET status = $status,
                    finished_at_ms = $finished_at_ms,
                    artifact_id = $artifact_id
                RETURN NONE;
            "#;
            self.db
                .inner()
                .query(q_stage)
                .bind(("sr", thing_from_id(stage_run_id)))
                .bind(("status", stage_status))
                .bind(("finished_at_ms", finished_at_ms))
                .bind(("artifact_id", artifact_id))
                .await?;

            // Schedule downstream if stage succeeded; otherwise skip downstream.
            if stage_status == "succeeded" {
                self.enqueue_downstream(stage_run_id).await?;
            } else {
                self.skip_downstream(stage_run_id).await?;
            }
        }

        // Update run status.
        self.update_run_status(&result.run_id).await?;

        Ok(CompleteResponse {
            ok: true,
            message: None,
        })
    }

    async fn materialize_run(&self, run_id: &str, wf: &WorkflowSpec) -> anyhow::Result<()> {
        // stage_id -> stage_run record id string ("stage_run:...").
        let mut stage_map = std::collections::HashMap::<String, String>::new();

        for stage in &wf.stages {
            let sr_ulid = new_ulid().to_string();
            let sr_id = format!("stage_run:{sr_ulid}");
            let rec = serde_json::json!({
                "run_id": run_id,
                "stage_id": stage.stage_id,
                "kind": stage.kind,
                "config": stage.config,
                "status": "pending",
                "created_at_ms": now_ms(),
            });
            let _: Option<serde_json::Value> = self
                .db
                .inner()
                .create(("stage_run", sr_ulid))
                .content(rec)
                .await?;
            stage_map.insert(stage.stage_id.clone(), sr_id);
        }

        // Create requires edges: (to depends on from) => to_sr ->requires-> from_sr
        for Edge { from, to } in &wf.edges {
            let Some(from_sr) = stage_map.get(from) else { continue; };
            let Some(to_sr) = stage_map.get(to) else { continue; };
            let q = r#"RELATE $to->requires->$from;"#;
            self.db
                .inner()
                .query(q)
                .bind(("to", thing_from_id(to_sr)))
                .bind(("from", thing_from_id(from_sr)))
                .await?;
        }

        Ok(())
    }

    async fn enqueue_runnable_stages(&self, run_id: &str) -> anyhow::Result<()> {
        // Find pending stage_runs for this run and enqueue those with zero unmet deps.
        let q = r#"
            SELECT * FROM stage_run WHERE run_id = $run_id AND status = 'pending';
        "#;
        let mut resp = self.db.inner().query(q).bind(("run_id", run_id.to_string())).await?;
        let stage_runs: Vec<serde_json::Value> = resp.take(0)?;

        for sr in stage_runs {
            let sr_id = sr
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("stage_run missing id"))?
                .to_string();

            if self.unmet_deps(&sr_id).await? == 0 {
                self.enqueue_job_for_stage_run(&sr_id, &sr).await?;
            }
        }
        Ok(())
    }

    async fn enqueue_downstream(&self, succeeded_stage_run_id: &str) -> anyhow::Result<()> {
        // Dependents are edges where `in = succeeded`, so select `out`.
        let q = r#"
            SELECT * FROM stage_run
            WHERE id IN (SELECT VALUE out FROM requires WHERE in = $sr);
        "#;
        let mut resp = self
            .db
            .inner()
            .query(q)
            .bind(("sr", thing_from_id(succeeded_stage_run_id)))
            .await?;
        let dependents: Vec<serde_json::Value> = resp.take(0)?;

        for dep in dependents {
            let dep_id = dep.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let status = dep.get("status").and_then(|v| v.as_str()).unwrap_or("");
            if status != "pending" {
                continue;
            }
            if self.unmet_deps(dep_id).await? == 0 {
                self.enqueue_job_for_stage_run(dep_id, &dep).await?;
            }
        }
        Ok(())
    }

    async fn skip_downstream(&self, failed_stage_run_id: &str) -> anyhow::Result<()> {
        // BFS over dependents via requires table.
        let mut queue = VecDeque::new();
        queue.push_back(failed_stage_run_id.to_string());

        let mut seen = std::collections::HashSet::new();
        seen.insert(failed_stage_run_id.to_string());

        while let Some(cur) = queue.pop_front() {
            let q = r#"SELECT VALUE out FROM requires WHERE in = $sr;"#;
            let mut resp = self.db.inner().query(q).bind(("sr", thing_from_id(&cur))).await?;
            let outs: Vec<Thing> = resp.take(0)?;

            for dep_thing in outs {
                let dep_id = dep_thing.to_string();
                if seen.contains(&dep_id) {
                    continue;
                }
                seen.insert(dep_id.clone());
                queue.push_back(dep_id.clone());

                let q_update = r#"
                    UPDATE $sr
                    SET status = 'skipped'
                    WHERE status = 'pending'
                    RETURN NONE;
                "#;
                let _ = self
                    .db
                    .inner()
                    .query(q_update)
                    .bind(("sr", dep_thing))
                    .await?;
            }
        }

        Ok(())
    }

    async fn unmet_deps(&self, stage_run_id: &str) -> anyhow::Result<i64> {
        // Count dependencies that are not succeeded.
        // Dependencies are edges where `out = stage`, so select `in`.
        let q = r#"
            SELECT count() AS c FROM stage_run
            WHERE id IN (SELECT VALUE in FROM requires WHERE out = $sr)
              AND status != 'succeeded';
        "#;
        let mut resp = self
            .db
            .inner()
            .query(q)
            .bind(("sr", thing_from_id(stage_run_id)))
            .await?;
        let rows: Vec<serde_json::Value> = resp.take(0)?;
        let c = rows
            .get(0)
            .and_then(|r| r.get("c"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        Ok(c)
    }

    async fn enqueue_job_for_stage_run(
        &self,
        stage_run_id: &str,
        stage_run_rec: &serde_json::Value,
    ) -> anyhow::Result<()> {
        // Avoid double-enqueue: only if no queued/running job exists for this stage_run.
        let q_check = r#"
            SELECT count() AS c FROM job WHERE stage_run_id = $sr AND (status = 'queued' OR status = 'running');
        "#;
        let mut resp = self
            .db
            .inner()
            .query(q_check)
            .bind(("sr", stage_run_id.to_string()))
            .await?;
        let rows: Vec<serde_json::Value> = resp.take(0)?;
        let c = rows
            .get(0)
            .and_then(|r| r.get("c"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if c > 0 {
            return Ok(());
        }

        // Extract stage info.
        let run_id = stage_run_rec
            .get("run_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("stage_run missing run_id"))?
            .to_string();
        let stage_id = stage_run_rec
            .get("stage_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("stage_run missing stage_id"))?
            .to_string();
        let kind: StageKind = serde_json::from_value(
            stage_run_rec
                .get("kind")
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("stage_run missing kind"))?,
        )?;
        let config: StageConfig = serde_json::from_value(
            stage_run_rec
                .get("config")
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("stage_run missing config"))?,
        )?;

        // Pre-exec validation.
        if let StageConfig::ExecBlock(exec) = &config {
            let outcome = validate_exec_block(exec);
            match outcome.decision {
                Decision::Block => {
                    let q = r#"
                        UPDATE $sr
                        SET status = 'needs_human',
                            validation = $validation
                        RETURN NONE;
                    "#;
                    self.db
                        .inner()
                        .query(q)
                        .bind(("sr", thing_from_id(stage_run_id)))
                        .bind(("validation", serde_json::to_value(outcome)?))
                        .await?;
                    tracing::warn!(stage_run_id = %stage_run_id, "stage blocked by validation");
                    return Ok(());
                }
                Decision::Warn => {
                    let q = r#"
                        UPDATE $sr
                        SET validation = $validation
                        RETURN NONE;
                    "#;
                    let _ = self
                        .db
                        .inner()
                        .query(q)
                        .bind(("sr", thing_from_id(stage_run_id)))
                        .bind(("validation", serde_json::to_value(outcome)?))
                        .await?;
                }
                Decision::Allow => {}
            }
        }

        // Create job record.
        let job_ulid = new_ulid().to_string();
        let job_rec = serde_json::json!({
            "run_id": run_id,
            "stage_run_id": stage_run_id,
            "stage_id": stage_id,
            "kind": kind,
            "config": config,
            "status": "queued",
            "created_at_ms": now_ms(),
        });
        let _: Option<serde_json::Value> = self
            .db
            .inner()
            .create(("job", job_ulid))
            .content(job_rec)
            .await?;

        Ok(())
    }

    async fn update_run_status(&self, run_id: &str) -> anyhow::Result<()> {
        // If any stage failed/needs_human -> run failed.
        let q_any_fail = r#"
            SELECT count() AS c FROM stage_run
            WHERE run_id = $run_id AND (status = 'failed' OR status = 'needs_human');
        "#;
        let mut resp = self
            .db
            .inner()
            .query(q_any_fail)
            .bind(("run_id", run_id.to_string()))
            .await?;
        let rows: Vec<serde_json::Value> = resp.take(0)?;
        let fail_count = rows
            .get(0)
            .and_then(|r| r.get("c"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        if fail_count > 0 {
            let q = r#"UPDATE $run SET status = 'failed', owner_agent = NONE, owner_lease_expires_at_ms = NONE RETURN NONE;"#;
            self.db
                .inner()
                .query(q)
                .bind(("run", thing_from_id(run_id)))
                .await?;
            return Ok(());
        }

        // If no pending/running left -> succeeded.
        let q_remaining = r#"
            SELECT count() AS c FROM stage_run
            WHERE run_id = $run_id AND (status = 'pending' OR status = 'running');
        "#;
        let mut resp = self
            .db
            .inner()
            .query(q_remaining)
            .bind(("run_id", run_id.to_string()))
            .await?;
        let rows: Vec<serde_json::Value> = resp.take(0)?;
        let remaining = rows
            .get(0)
            .and_then(|r| r.get("c"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        if remaining == 0 {
            let q = r#"UPDATE $run SET status = 'succeeded', owner_agent = NONE, owner_lease_expires_at_ms = NONE RETURN NONE;"#;
            self.db
                .inner()
                .query(q)
                .bind(("run", thing_from_id(run_id)))
                .await?;
        }

        Ok(())
    }
}

fn job_id_str(job_rec: &serde_json::Value) -> anyhow::Result<String> {
    job_rec
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("job record missing id"))
}

fn sanitize_component(s: &str) -> String {
    s.replace(':', "_").replace('/', "_")
}

/// Parse a record id string like "job:01H..." into a SurrealDB Thing.
fn thing_from_id(id: &str) -> Thing {
    let (tb, rest) = id.split_once(':').unwrap_or((id, ""));
    Thing::from((tb, rest))
}

/// A small workflow that demonstrates dependencies and exec blocks.
fn demo_workflow(project_path: &str) -> WorkflowSpec {
    let stage_a = StageDef {
        stage_id: "prep".into(),
        kind: StageKind::ExecBlock,
        config: StageConfig::ExecBlock(ExecBlockSpec {
            workdir: project_path.into(),
            executor: Default::default(),
            allow_shell: false,
            halt_on_error: true,
            env: Default::default(),
            commands: vec![CommandSpec {
                program: "echo".into(),
                args: vec!["prep: starting".into()],
                cwd: None,
                env: Default::default(),
                timeout_sec: None,
            }],
        }),
    };

    let stage_b = StageDef {
        stage_id: "build".into(),
        kind: StageKind::ExecBlock,
        config: StageConfig::ExecBlock(ExecBlockSpec {
            workdir: project_path.into(),
            executor: Default::default(),
            allow_shell: false,
            halt_on_error: true,
            env: Default::default(),
            commands: vec![
                CommandSpec {
                    program: "echo".into(),
                    args: vec!["build: compiling".into()],
                    cwd: None,
                    env: Default::default(),
                    timeout_sec: None,
                },
                CommandSpec {
                    program: "echo".into(),
                    args: vec!["build: done".into()],
                    cwd: None,
                    env: Default::default(),
                    timeout_sec: None,
                },
            ],
        }),
    };

    let stage_c = StageDef {
        stage_id: "test".into(),
        kind: StageKind::ExecBlock,
        config: StageConfig::ExecBlock(ExecBlockSpec {
            workdir: project_path.into(),
            executor: Default::default(),
            allow_shell: false,
            halt_on_error: true,
            env: Default::default(),
            commands: vec![CommandSpec {
                program: "echo".into(),
                args: vec!["test: running".into()],
                cwd: None,
                env: Default::default(),
                timeout_sec: None,
            }],
        }),
    };

    WorkflowSpec {
        name: "demo".into(),
        stages: vec![stage_a, stage_b, stage_c],
        edges: vec![
            Edge {
                from: "prep".into(),
                to: "build".into(),
            },
            Edge {
                from: "build".into(),
                to: "test".into(),
            },
        ],
    }
}
