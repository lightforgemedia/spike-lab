use crate::model::WorkflowSpec;
use anyhow::Context;
use sha2::{Digest, Sha256};

/// Stable content hash for a workflow specification.
///
/// Notes:
/// - We serialize using serde_json, relying on deterministic struct field order.
/// - Maps in specs use `BTreeMap` to keep key order stable.
pub fn workflow_hash(spec: &WorkflowSpec) -> anyhow::Result<String> {
    let bytes = serde_json::to_vec(spec).context("serialize workflow spec")?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}
