use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spl_core::{AnchorId, GateName};

use crate::profile::required_gates_for_profile;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpecPack {
    pub task: String,
    pub intent: String,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub scope: Option<Scope>,
    #[serde(default)]
    pub use_cases: Vec<UseCase>,
    #[serde(default)]
    pub behavior_contracts: Vec<BehaviorContract>,
    pub acceptance: Acceptance,
    #[serde(default)]
    pub policy: Option<Policy>,
    #[serde(default)]
    pub gates: Option<Gates>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scope {
    #[serde(default)]
    pub r#in: Vec<String>,
    #[serde(default)]
    pub out: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UseCase {
    pub id: String,
    pub actor: String,
    #[serde(default)]
    pub preconditions: Vec<String>,
    #[serde(default)]
    pub steps: Vec<String>,
    #[serde(default)]
    pub postconditions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BehaviorContract {
    pub id: String,
    pub anchor: String,
    #[serde(default)]
    pub examples: Vec<serde_json::Value>,
    #[serde(default)]
    pub invariants: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Acceptance {
    pub tests: Vec<String>,
    #[serde(default)]
    pub manual: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Policy {
    #[serde(default)]
    pub network: Option<String>, // deny | allow_readonly | allow
    #[serde(default)]
    pub allow_domains: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gates {
    #[serde(default)]
    pub required: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct RevisionDraft {
    pub spec_hash: String,
    pub profile: String,
    pub required_gates: Vec<GateName>,
    pub anchors: Vec<AnchorId>,
}

pub fn load_spec_pack(path: &std::path::Path) -> Result<SpecPack> {
    let s = std::fs::read_to_string(path).with_context(|| format!("read spec pack: {}", path.display()))?;
    let pack: SpecPack = serde_yaml::from_str(&s).with_context(|| "parse spec pack yaml")?;
    validate_spec_pack(&pack)?;
    Ok(pack)
}

pub fn validate_spec_pack(pack: &SpecPack) -> Result<()> {
    if pack.task.trim().is_empty() {
        return Err(anyhow!("spec pack missing task"));
    }
    if pack.intent.trim().is_empty() {
        return Err(anyhow!("spec pack missing intent"));
    }
    if pack.acceptance.tests.is_empty() {
        return Err(anyhow!("spec pack must include acceptance.tests"));
    }
    // v0: require at least one use case for standard/docs/hotfix (backfill_spec may relax later)
    let profile = pack.profile.as_deref().unwrap_or("standard");
    if profile != "backfill_spec" && pack.use_cases.is_empty() {
        return Err(anyhow!("spec pack must include at least one use_case (profile={})", profile));
    }
    Ok(())
}

pub fn canonical_json(pack: &SpecPack) -> serde_json::Value {
    let v = serde_json::to_value(pack).expect("SpecPack serializable");
    sort_json(v)
}

/// Recursively sort object keys for stable hashing.
fn sort_json(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();
            let mut new_map = serde_json::Map::new();
            for k in keys {
                let child = map.get(&k).cloned().unwrap_or(serde_json::Value::Null);
                new_map.insert(k, sort_json(child));
            }
            serde_json::Value::Object(new_map)
        }
        serde_json::Value::Array(arr) => serde_json::Value::Array(arr.into_iter().map(sort_json).collect()),
        other => other,
    }
}

pub fn spec_hash(pack: &SpecPack) -> String {
    let v = canonical_json(pack);
    let bytes = serde_json::to_vec(&v).expect("json bytes");
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    hex::encode(digest)
}

pub fn compile_revision_draft(pack: &SpecPack) -> RevisionDraft {
    let profile = pack.profile.clone().unwrap_or_else(|| "standard".to_string());
    let required_gates = required_gates_for_profile(&profile);

    let anchors = pack
        .behavior_contracts
        .iter()
        .map(|bc| AnchorId::from_str(bc.anchor.clone()))
        .collect::<Vec<_>>();

    RevisionDraft {
        spec_hash: spec_hash(pack),
        profile,
        required_gates,
        anchors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable() {
        let pack = SpecPack {
            task: "pt-1".into(),
            intent: "do thing".into(),
            profile: Some("standard".into()),
            scope: None,
            use_cases: vec![UseCase {
                id: "UC-1".into(),
                actor: "Owner".into(),
                preconditions: vec![],
                steps: vec!["step".into()],
                postconditions: vec!["post".into()],
            }],
            behavior_contracts: vec![],
            acceptance: Acceptance { tests: vec!["true".into()], manual: vec![] },
            policy: None,
            gates: None,
        };

        let h1 = spec_hash(&pack);
        let h2 = spec_hash(&pack);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }
}
