use crate::config::DaemonConfig;
use anyhow::{Context, Result};
use orchestrator_core::{now_ms, EpochMs, ExecBlockSpec, Id, JobState, Lease};
use serde::{Deserialize, Serialize};
use std::path::Path;

use surrealdb::engine::local::SurrealKv;
use surrealdb::Surreal;

pub type SurrealConn = surrealdb::engine::local::Db;
pub type SurrealDb = Surreal<SurrealConn>;

#[derive(Clone)]
pub struct Db {
    inner: SurrealDb,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Id,
    pub root_path: String,
    pub created_ms: EpochMs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Queued,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: Id,
    pub project_id: Id,
    pub created_ms: EpochMs,
    pub state: RunState,
    pub description: Option<String>,
    pub base_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageState {
    Pending,
    Running,
    Succeeded,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stage {
    pub id: Id,
    pub run_id: Id,
    pub name: String,
    #[serde(default)]
    pub deps: Vec<Id>, // stage ids
    pub state: StageState,
    pub input_revision: Option<String>,
    pub output_revision: Option<String>,
    pub job_id: Option<Id>,
    pub created_ms: EpochMs,
    pub updated_ms: EpochMs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: Id,
    pub run_id: Id,
    pub stage_id: Id,
    pub exec: ExecBlockSpec,
    pub state: JobState,

    pub attempt: u32,
    pub created_ms: EpochMs,
    pub started_ms: Option<EpochMs>,
    pub ended_ms: Option<EpochMs>,

    pub lease: Option<Lease>,

    pub bundle_root: String,
    pub workspace_root: String,

    pub input_revision: Option<String>,
    pub output_revision: Option<String>,

    pub executor_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: Id,
    pub last_seen_ms: EpochMs,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl Db {
    pub async fn connect(config: &DaemonConfig) -> Result<Self> {
        std::fs::create_dir_all(&config.db_dir)
            .with_context(|| format!("creating db_dir {}", config.db_dir.display()))?;

        let db_path = config
            .db_dir
            .to_str()
            .context("db_dir must be valid utf-8")?
            .to_string();

        // Embedded SurrealKV.
        // See SurrealDB Rust SDK: Surreal::new::<SurrealKv>(path).versioned().await
        let inner = Surreal::new::<SurrealKv>(db_path)
            .versioned()
            .await
            .context("connecting to embedded SurrealKV")?;

        inner
            .use_ns("orchestrator")
            .use_db("main")
            .await
            .context("selecting surreal namespace/db")?;

        Ok(Self { inner })
    }

    pub async fn bootstrap_schema(&self) -> Result<()> {
        let schema = include_str!("../schema.surql");
        let _ = self
            .inner
            .query(schema)
            .await
            .context("applying schema")?;
        Ok(())
    }

    pub fn inner(&self) -> &SurrealDb {
        &self.inner
    }

    pub async fn get_or_create_project(&self, root_path: &str) -> Result<Project> {
        let mut res = self
            .inner
            .query("SELECT * FROM project WHERE root_path = $root_path LIMIT 1;")
            .bind(("root_path", root_path))
            .await?;
        let existing: Option<Project> = res.take(0)?;
        if let Some(p) = existing {
            return Ok(p);
        }
        let p = Project {
            id: new_ulid(),
            root_path: root_path.to_string(),
            created_ms: now_ms(),
        };
        let _: Option<Project> = self
            .inner
            .create(("project", p.id.clone()))
            .content(&p)
            .await?;
        Ok(p)
    }

    pub async fn create_run(
        &self,
        project_id: &str,
        description: Option<String>,
        base_revision: Option<String>,
    ) -> Result<Run> {
        let run = Run {
            id: new_ulid(),
            project_id: project_id.to_string(),
            created_ms: now_ms(),
            state: RunState::Queued,
            description,
            base_revision,
        };
        let _: Option<Run> = self
            .inner
            .create(("run", run.id.clone()))
            .content(&run)
            .await?;
        Ok(run)
    }

    pub async fn create_stage(
        &self,
        run_id: &str,
        name: &str,
        deps: Vec<Id>,
    ) -> Result<Stage> {
        let now = now_ms();
        let stage = Stage {
            id: new_ulid(),
            run_id: run_id.to_string(),
            name: name.to_string(),
            deps: deps.clone(),
            state: StageState::Pending,
            input_revision: None,
            output_revision: None,
            job_id: None,
            created_ms: now,
            updated_ms: now,
        };
        let _: Option<Stage> = self
            .inner
            .create(("stage", stage.id.clone()))
            .content(&stage)
            .await?;

        // Also create graph edges for dependencies.
        for dep in deps {
            let _ = self
                .inner
                .query("RELATE stage:$from->depends_on->stage:$to SET created_ms = $now;")
                .bind(("from", stage.id.clone()))
                .bind(("to", dep))
                .bind(("now", now))
                .await?;
        }

        Ok(stage)
    }

    pub async fn list_stages_for_run(&self, run_id: &str) -> Result<Vec<Stage>> {
        let mut res = self
            .inner
            .query("SELECT * FROM stage WHERE run_id = $run_id ORDER BY created_ms ASC;")
            .bind(("run_id", run_id))
            .await?;
        let stages: Vec<Stage> = res.take(0)?;
        Ok(stages)
    }

    pub async fn get_run(&self, run_id: &str) -> Result<Option<Run>> {
        let mut res = self
            .inner
            .query("SELECT * FROM run WHERE id = $run_id LIMIT 1;")
            .bind(("run_id", run_id))
            .await?;
        let run: Option<Run> = res.take(0)?;
        Ok(run)
    }

    pub async fn update_run_state(&self, run_id: &str, state: RunState) -> Result<()> {
        let _ = self
            .inner
            .query("UPDATE run SET state = $state WHERE id = $id;")
            .bind(("id", run_id))
            .bind(("state", state))
            .await?;
        Ok(())
    }

    pub async fn update_stage_state(
        &self,
        stage_id: &str,
        state: StageState,
        input_revision: Option<String>,
        output_revision: Option<String>,
        job_id: Option<String>,
    ) -> Result<()> {
        let now = now_ms();
        let _ = self
            .inner
            .query(
                "UPDATE stage SET state=$state, input_revision=$inrev, output_revision=$outrev, job_id=$job, updated_ms=$now WHERE id=$id;",
            )
            .bind(("id", stage_id))
            .bind(("state", state))
            .bind(("inrev", input_revision))
            .bind(("outrev", output_revision))
            .bind(("job", job_id))
            .bind(("now", now))
            .await?;
        Ok(())
    }

    pub async fn create_job(
        &self,
        run_id: &str,
        stage_id: &str,
        exec: ExecBlockSpec,
        attempt: u32,
        bundle_root: &str,
        workspace_root: &str,
        input_revision: Option<String>,
    ) -> Result<Job> {
        let job = Job {
            id: new_ulid(),
            run_id: run_id.to_string(),
            stage_id: stage_id.to_string(),
            exec,
            state: JobState::Queued,
            attempt,
            created_ms: now_ms(),
            started_ms: None,
            ended_ms: None,
            lease: None,
            bundle_root: bundle_root.to_string(),
            workspace_root: workspace_root.to_string(),
            input_revision,
            output_revision: None,
            executor_ref: None,
        };
        let _: Option<Job> = self
            .inner
            .create(("job", job.id.clone()))
            .content(&job)
            .await?;
        Ok(job)
    }

    /// Attempt to atomically claim the next available job.
    ///
    /// This uses a select + conditional update loop. The conditional update prevents
    /// double-claims under contention.
    pub async fn claim_next_job(
        &self,
        agent_id: &str,
        capabilities: &[String],
        lease_seconds: u64,
    ) -> Result<Option<Job>> {
        // Record/refresh agent heartbeat.
        self.upsert_agent(agent_id, capabilities).await?;

        let now = now_ms();
        let mut res = self
            .inner
            .query(
                "SELECT * FROM job                      WHERE state = 'queued'                        AND (lease.expires_ms IS NONE OR lease.expires_ms < $now)                      ORDER BY created_ms ASC                      LIMIT 10;",
            )
            .bind(("now", now))
            .await?;
        let mut candidates: Vec<Job> = res.take(0)?;

        for mut job in candidates.drain(..) {
            let token = uuid::Uuid::new_v4().to_string();
            let lease = Lease {
                agent_id: agent_id.to_string(),
                token: token.clone(),
                expires_ms: now + (lease_seconds as i64) * 1000,
            };

            // Conditional update: only claim if still queued and lease expired.
            let mut upd = self
                .inner
                .query(
                    "UPDATE job SET state='running', lease=$lease, started_ms=$now                          WHERE id = $id AND state='queued' AND (lease.expires_ms IS NONE OR lease.expires_ms < $now)                          RETURN AFTER;",
                )
                .bind(("id", job.id.clone()))
                .bind(("lease", lease.clone()))
                .bind(("now", now))
                .await?;

            let claimed: Option<Job> = upd.take(0)?;
            if let Some(mut claimed) = claimed {
                claimed.lease = Some(lease);
                return Ok(Some(claimed));
            }
        }

        Ok(None)
    }

    pub async fn renew_lease(
        &self,
        job_id: &str,
        agent_id: &str,
        token: &str,
        lease_seconds: u64,
    ) -> Result<Option<Lease>> {
        let now = now_ms();
        let new_expires = now + (lease_seconds as i64) * 1000;

        let mut res = self
            .inner
            .query(
                "UPDATE job SET lease.expires_ms = $expires                      WHERE id = $id AND state='running' AND lease.agent_id = $agent AND lease.token = $token                      RETURN AFTER;",
            )
            .bind(("id", job_id))
            .bind(("agent", agent_id))
            .bind(("token", token))
            .bind(("expires", new_expires))
            .await?;

        let job: Option<Job> = res.take(0)?;
        Ok(job.and_then(|j| j.lease))
    }

    pub async fn complete_job(
        &self,
        job_id: &str,
        agent_id: &str,
        token: &str,
        state: JobState,
        ended_ms: EpochMs,
        output_revision: Option<String>,
        executor_ref: Option<String>,
    ) -> Result<bool> {
        // Only the leasing agent with correct token may complete.
        let mut res = self
            .inner
            .query(
                "UPDATE job SET state=$state, ended_ms=$ended, output_revision=$outrev, executor_ref=$eref                      WHERE id=$id AND lease.agent_id=$agent AND lease.token=$token                      RETURN BEFORE;",
            )
            .bind(("id", job_id))
            .bind(("agent", agent_id))
            .bind(("token", token))
            .bind(("state", state))
            .bind(("ended", ended_ms))
            .bind(("outrev", output_revision))
            .bind(("eref", executor_ref))
            .await?;
        let before: Option<Job> = res.take(0)?;
        Ok(before.is_some())
    }

    pub async fn upsert_agent(&self, agent_id: &str, capabilities: &[String]) -> Result<()> {
        let now = now_ms();
        let agent = Agent {
            id: agent_id.to_string(),
            last_seen_ms: now,
            capabilities: capabilities.to_vec(),
        };
        // UPSERT by id
        let _ = self
            .inner
            .query("UPSERT agent:$id CONTENT $agent;")
            .bind(("id", agent_id))
            .bind(("agent", agent))
            .await?;
        Ok(())
    }

    pub async fn get_job(&self, job_id: &str) -> Result<Option<Job>> {
        let mut res = self
            .inner
            .query("SELECT * FROM job WHERE id=$id LIMIT 1;")
            .bind(("id", job_id))
            .await?;
        let job: Option<Job> = res.take(0)?;
        Ok(job)
    }

    pub async fn get_stage(&self, stage_id: &str) -> Result<Option<Stage>> {
        let mut res = self
            .inner
            .query("SELECT * FROM stage WHERE id=$id LIMIT 1;")
            .bind(("id", stage_id))
            .await?;
        let stage: Option<Stage> = res.take(0)?;
        Ok(stage)
    }

    pub async fn list_jobs_for_run(&self, run_id: &str) -> Result<Vec<Job>> {
        let mut res = self
            .inner
            .query("SELECT * FROM job WHERE run_id=$run_id ORDER BY created_ms ASC;")
            .bind(("run_id", run_id))
            .await?;
        let jobs: Vec<Job> = res.take(0)?;
        Ok(jobs)
    }

    pub fn ensure_dir(path: &Path) -> Result<()> {
        std::fs::create_dir_all(path)
            .with_context(|| format!("creating dir {}", path.display()))?;
        Ok(())
    }
}

fn new_ulid() -> Id {
    ulid::Ulid::new().to_string()
}
