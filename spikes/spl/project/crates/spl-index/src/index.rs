use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use spl_core::AnchorId;

/// Minimal v0 index: file-based map of AnchorId -> signature hash.
/// Real implementations can replace this with language-aware indexing.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SimpleIndex {
    pub anchors: HashMap<String, String>,
}

impl SimpleIndex {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = std::fs::read(path)?;
        let idx: Self = serde_json::from_slice(&bytes)?;
        Ok(idx)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let bytes = serde_json::to_vec_pretty(self)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    pub fn resolve_anchor(&self, anchor: &AnchorId) -> Option<String> {
        self.anchors.get(anchor.as_str()).cloned()
    }
}

/// v0 helper: where to store the index under repo root.
pub fn default_index_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".spl").join("index.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn index_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("index.json");
        let mut idx = SimpleIndex::default();
        idx.anchors.insert("a".into(), "h".into());
        idx.save(&path).unwrap();
        let idx2 = SimpleIndex::load(&path).unwrap();
        assert_eq!(idx2.anchors.get("a").unwrap(), "h");
    }
}
