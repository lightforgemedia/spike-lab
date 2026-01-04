use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum EvidenceRole {
    SpecPack,
    ContextPack,
    Worklog,
    PreSmoke,
    Audit,
    Review,
    Validate,
    PostSmoke,
    Diff,
    CtxExplain,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GateRecord {
    pub gate: String,
    pub status: String,
    pub artifacts: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvidenceManifest {
    pub run_id: String,
    pub task_id: String,
    pub revision_id: String,
    pub lane: String,
    pub vcs_type: String,
    pub spec_hash: String,

    pub base_rev: Option<String>,
    pub head_rev: Option<String>,

    pub gates: Vec<GateRecord>,
}
