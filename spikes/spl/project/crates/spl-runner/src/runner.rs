use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use spl_artifacts::{EvidenceManifest, EvidenceRole, FsArtifactStore, GateRecord};
use spl_core::{GateName, GateOutcome, GateStatus, Lane, QueueId, QueueItem, RunId, TaskId, TaskStatus, VcsType};
use spl_storage::Storage;
use spl_storage_sqlite::SqliteStorage;
use spl_vcs::{Patch, VcsAdapter};
use spl_vcs_git::GitAdapter;
use spl_vcs_jj::JjAdapter;

use crate::{doctor::doctor, util::now_unix, Config};

pub struct Runner {
    pub repo_root: PathBuf,
    pub cfg: Config,
    pub storage: SqliteStorage,
    pub artifacts: FsArtifactStore,
    pub vcs: Box<dyn VcsAdapter>,
    pub worker_id: String,
}

impl Runner {
    pub fn open(repo_root: PathBuf) -> Result<Self> {
        let cfg_path = Config::config_path(&repo_root);
        let cfg = if cfg_path.exists() {
            Config::load_from(&cfg_path)?
        } else {
            let project_id = repo_root.file_name().and_then(|s| s.to_str()).unwrap_or("repo");
            let cfg = Config::default_for_repo(project_id);
            cfg.save_to(&cfg_path)?;
            cfg
        };

        let db_path = Config::db_path(&repo_root);
        let storage = SqliteStorage::open(&db_path)?;
        let artifacts_root = shellexpand::tilde(&cfg.project.artifact_root).to_string();
        let artifacts = FsArtifactStore::new(PathBuf::from(artifacts_root));

        let vcs: Box<dyn VcsAdapter> = match cfg.vcs_type() {
            VcsType::Git => {
                let main = cfg.vcs.git_main_branch.clone().unwrap_or_else(|| cfg.project.main_ref.clone());
                Box::new(GitAdapter::new(main))
            }
            VcsType::Jj => {
                let jj_main = cfg.vcs.jj_main_bookmark.clone().unwrap_or_else(|| "main".to_string());
                let git_main = cfg.vcs.git_main_branch.clone().unwrap_or_else(|| cfg.project.main_ref.clone());
                let require = cfg.vcs.jj_require_colocated.unwrap_or(true);
                Box::new(JjAdapter::new(jj_main, git_main, require))
            }
        };

        Ok(Self {
            repo_root,
            cfg,
            storage,
            artifacts,
            vcs,
            worker_id: format!("worker-{}", std::process::id()),
        })
    }

    pub fn init_repo(repo_root: &Path) -> Result<()> {
        std::fs::create_dir_all(repo_root.join(".spl").join("reviews")).ok();
        let cfg_path = Config::config_path(repo_root);
        if !cfg_path.exists() {
            let project_id = repo_root.file_name().and_then(|s| s.to_str()).unwrap_or("repo");
            Config::default_for_repo(project_id).save_to(&cfg_path)?;
        }
        // create db
        let _ = SqliteStorage::open(&Config::db_path(repo_root))?;
        Ok(())
    }

    pub fn doctor(&self) -> Result<()> {
        doctor(&self.repo_root, &self.cfg)
    }

    /// Enqueue a dummy execute item (v0 helper).
    pub fn enqueue_execute(&self, task_id: &str, revision_id: &str) -> Result<()> {
        let qi = QueueItem {
            id: QueueId::new(),
            task_id: TaskId::from_str(task_id),
            revision_id: spl_core::RevisionId::from_str(revision_id),
            lane: Lane::Execute,
            visible_at_unix: now_unix(),
            attempts: 0,
            max_attempts: 3,
            priority: 0,
            idempotency_key: format!("{}:{}:execute", task_id, revision_id),
        };
        self.storage.enqueue(qi)?;
        Ok(())
    }

    /// Run a single visible execute-lane queue item and, if successful, run land lane.
    ///
    /// v0 implementation is intentionally small: gates are stubbed, but evidence is real.
    pub fn run_once(&self, dry_run: bool) -> Result<()> {
        self.doctor()?;

        let now = now_unix();
        let snap = self.storage.load_snapshot(now)?;
        let mut items = snap.queue.clone();
        items.sort_by(|a,b| b.priority.cmp(&a.priority).then(a.visible_at_unix.cmp(&b.visible_at_unix)));
        let item = items.into_iter().find(|q| q.lane == Lane::Execute).ok_or_else(|| anyhow!("no visible execute items"))?;

        if dry_run {
            println!("DRY RUN: would attempt lease for queue_id={}", item.id.as_str());
            return Ok(());
        }

        let lease = self.storage.try_acquire_lease(&item.id, &self.worker_id, 300)?;
        if lease.is_none() {
            return Err(anyhow!("could not acquire lease (another worker?)"));
        }

        let run_id = RunId::new();
        self.storage.create_run(&item.id, run_id.clone())?;

        let run_dir = self.artifacts.create_run_dir(&self.cfg.project.id, &run_id)?;
        self.artifacts.append_worklog(&run_dir, &format!("## Run {} for task {}", run_id.as_str(), item.task_id.as_str()))?;

        // Create workspace and run stub gates
        let ws_root = self.cfg.workspace_root(&self.repo_root);
        let ws = self.vcs.create_workspace(&self.repo_root, &ws_root, &item.task_id)?;

        let mut manifest = EvidenceManifest {
            run_id: run_id.as_str().to_string(),
            task_id: item.task_id.as_str().to_string(),
            revision_id: item.revision_id.as_str().to_string(),
            lane: "execute".into(),
            vcs_type: format!("{:?}", self.vcs.vcs_type()),
            spec_hash: "stub".into(),
            base_rev: None,
            head_rev: None,
            gates: vec![],
        };

        // pre_smoke stub
        self.write_gate(&run_id, &run_dir, &mut manifest, GateName::PreSmoke, GateStatus::Pass, EvidenceRole::PreSmoke, b"pre_smoke: PASS (stub)")?;
        // audit stub
        self.write_gate(&run_id, &run_dir, &mut manifest, GateName::Audit, GateStatus::Pass, EvidenceRole::Audit, b"{"result":"PASS"}")?;
        // review stub
        self.write_gate(&run_id, &run_dir, &mut manifest, GateName::AdversarialReview, GateStatus::Pass, EvidenceRole::Review, b"{"result":"PASS"}")?;
        // validate stub
        self.write_gate(&run_id, &run_dir, &mut manifest, GateName::Validate, GateStatus::Pass, EvidenceRole::Validate, b"{"result":"PASS"}")?;

        // Snapshot + patch (may be empty if no changes)
        let base = self.vcs.get_base_rev(&self.repo_root)?;
        let head = self.vcs.snapshot(&ws, "spl snapshot")?;
        let patch = self.vcs.export_patch(&ws, &base, &head)?;
        manifest.base_rev = Some(base.clone());
        manifest.head_rev = Some(head.clone());
        let _ = self.artifacts.write_role_bytes(&run_dir, EvidenceRole::Diff, "diff.patch", &patch.bytes)?;

        self.artifacts.write_manifest(&run_dir, &manifest)?;

        // cleanup workspace
        self.vcs.cleanup_workspace(&self.repo_root, ws)?;

        // enqueue land lane item (idempotency protected in db)
        let land_item = QueueItem {
            id: QueueId::new(),
            task_id: item.task_id.clone(),
            revision_id: item.revision_id.clone(),
            lane: Lane::Land,
            visible_at_unix: now_unix(),
            attempts: 0,
            max_attempts: 3,
            priority: item.priority,
            idempotency_key: format!("{}:{}:land", item.task_id.as_str(), item.revision_id.as_str()),
        };
        let _ = self.storage.enqueue(land_item);

        // run land lane immediately (v0 convenience)
        self.run_land(&item.task_id, &patch)?;

        self.storage.set_task_status(&item.task_id, TaskStatus::Done)?;
        self.storage.release_lease(&item.id, &self.worker_id)?;
        Ok(())
    }

    fn run_land(&self, task_id: &TaskId, patch: &Patch) -> Result<()> {
        let run_id = RunId::new();
        let run_dir = self.artifacts.create_run_dir(&self.cfg.project.id, &run_id)?;
        self.artifacts.append_worklog(&run_dir, &format!("## Land run {} for task {}", run_id.as_str(), task_id.as_str()))?;

        let mut manifest = EvidenceManifest {
            run_id: run_id.as_str().to_string(),
            task_id: task_id.as_str().to_string(),
            revision_id: "stub".into(),
            lane: "land".into(),
            vcs_type: format!("{:?}", self.vcs.vcs_type()),
            spec_hash: "stub".into(),
            base_rev: None,
            head_rev: None,
            gates: vec![],
        };

        // Apply patch (may be empty)
        let landed = self.vcs.apply_patch_to_repo_root(&self.repo_root, patch, "spl land")?;
        manifest.head_rev = Some(landed);

        // post_smoke stub
        self.write_gate(&run_id, &run_dir, &mut manifest, GateName::PostSmoke, GateStatus::Pass, EvidenceRole::PostSmoke, b"post_smoke: PASS (stub)")?;
        self.artifacts.write_manifest(&run_dir, &manifest)?;
        Ok(())
    }

    fn write_gate(
        &self,
        run_id: &RunId,
        run_dir: &Path,
        manifest: &mut EvidenceManifest,
        gate: GateName,
        status: GateStatus,
        role: EvidenceRole,
        bytes: &[u8],
    ) -> Result<()> {
        let name = format!("{:?}.txt", gate);
        let _path = self.artifacts.write_role_bytes(run_dir, role, &name, bytes)?;
        self.storage.record_gate_outcome(run_id, &GateOutcome { gate, status, remediation: None })?;
        manifest.gates.push(GateRecord {
            gate: format!("{:?}", gate),
            status: format!("{:?}", status),
            artifacts: vec![name],
        });
        Ok(())
    }
}
