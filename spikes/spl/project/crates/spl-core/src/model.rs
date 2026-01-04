use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Draft,
    Ready,
    BlockedHitl,
    BlockedFailure,
    Done,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Lane {
    Execute,
    Land,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum RunResult {
    Pass,
    FailGate,
    BlockedHitl,
    Crash,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum GateName {
    SpecCompile,
    CtxPack,
    PreSmoke,
    Delegate,
    Audit,
    AdversarialReview,
    Validate,
    PostSmoke,
    Land,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum GateStatus {
    Pass,
    Fail,
    Warn,
    Skipped,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageType {
    Ask,
    Update,
    Review,
    Decision,
    Reset,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum VcsType {
    Git,
    Jj,
}
