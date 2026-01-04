use crate::{ids::*, model::*};

#[derive(Clone, Debug)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub status: TaskStatus,
    pub priority: i32,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Revision {
    pub id: RevisionId,
    pub task_id: TaskId,
    pub spec_rev_id: SpecRevId,
    pub spec_hash: String,
    pub profile: String,
    pub required_gates: Vec<GateName>,
    pub required_validations: Vec<String>,
    pub anchors: Vec<AnchorId>,
}

#[derive(Clone, Debug)]
pub struct QueueItem {
    pub id: QueueId,
    pub task_id: TaskId,
    pub revision_id: RevisionId,
    pub lane: Lane,
    pub visible_at_unix: i64,
    pub attempts: u32,
    pub max_attempts: u32,
    pub priority: i32,
    pub idempotency_key: String,
}

#[derive(Clone, Debug)]
pub struct Lease {
    pub id: LeaseId,
    pub queue_id: QueueId,
    pub worker_id: String,
    pub acquired_at_unix: i64,
    pub expires_at_unix: i64,
}

#[derive(Clone, Debug)]
pub struct Run {
    pub id: RunId,
    pub task_id: TaskId,
    pub revision_id: RevisionId,
    pub lane: Lane,
    pub result: Option<RunResult>,
    pub current_gate: Option<GateName>,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub id: String,
    pub task_id: TaskId,
    pub ty: MessageType,
    pub body_md: String,
    pub created_at_unix: i64,
}
