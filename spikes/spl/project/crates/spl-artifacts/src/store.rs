use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use spl_core::RunId;

use crate::manifest::{EvidenceManifest, EvidenceRole};

pub trait ArtifactStore: Send + Sync {
    fn create_run_dir(&self, project_id: &str, run_id: &RunId) -> Result<PathBuf>;
    fn write_manifest(&self, run_dir: &Path, manifest: &EvidenceManifest) -> Result<()>;
    fn append_worklog(&self, run_dir: &Path, line: &str) -> Result<()>;
    fn write_role_bytes(&self, run_dir: &Path, role: EvidenceRole, name: &str, bytes: &[u8]) -> Result<PathBuf>;
}

#[derive(Clone)]
pub struct FsArtifactStore {
    pub root: PathBuf,
}

impl FsArtifactStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn role_dir(role: EvidenceRole) -> &'static str {
        match role {
            EvidenceRole::SpecPack => "spec_pack",
            EvidenceRole::ContextPack => "context_pack",
            EvidenceRole::Worklog => "worklog",
            EvidenceRole::PreSmoke => "pre_smoke",
            EvidenceRole::Audit => "audit",
            EvidenceRole::Review => "review",
            EvidenceRole::Validate => "validate",
            EvidenceRole::PostSmoke => "post_smoke",
            EvidenceRole::Diff => "diff",
            EvidenceRole::CtxExplain => "ctx_explain",
        }
    }
}

impl ArtifactStore for FsArtifactStore {
    fn create_run_dir(&self, project_id: &str, run_id: &RunId) -> Result<PathBuf> {
        let dir = self.root.join(project_id).join(run_id.as_str());
        std::fs::create_dir_all(&dir).with_context(|| format!("create run dir {}", dir.display()))?;
        Ok(dir)
    }

    fn write_manifest(&self, run_dir: &Path, manifest: &EvidenceManifest) -> Result<()> {
        let path = run_dir.join("evidence_manifest.json");
        let bytes = serde_json::to_vec_pretty(manifest)?;
        std::fs::write(&path, bytes).with_context(|| format!("write manifest {}", path.display()))?;
        Ok(())
    }

    fn append_worklog(&self, run_dir: &Path, line: &str) -> Result<()> {
        let path = run_dir.join("worklog.md");
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
        writeln!(f, "{}", line)?;
        Ok(())
    }

    fn write_role_bytes(&self, run_dir: &Path, role: EvidenceRole, name: &str, bytes: &[u8]) -> Result<PathBuf> {
        let dir = run_dir.join(Self::role_dir(role));
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(name);
        std::fs::write(&path, bytes)?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_manifest_and_worklog() {
        let dir = tempdir().unwrap();
        let store = FsArtifactStore::new(dir.path().to_path_buf());
        let run_dir = store.create_run_dir("p", &RunId::from_str("r")).unwrap();
        store.append_worklog(&run_dir, "hello").unwrap();
        store.write_manifest(&run_dir, &EvidenceManifest {
            run_id: "r".into(),
            task_id: "t".into(),
            revision_id: "rev".into(),
            lane: "execute".into(),
            vcs_type: "git".into(),
            spec_hash: "h".into(),
            base_rev: None,
            head_rev: None,
            gates: vec![],
        }).unwrap();
        assert!(run_dir.join("worklog.md").exists());
        assert!(run_dir.join("evidence_manifest.json").exists());
    }
}
