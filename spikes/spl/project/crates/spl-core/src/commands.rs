use crate::{ids::*, model::*};

#[derive(Clone, Debug)]
pub enum Command {
    AcquireLease { queue_id: QueueId },
    StartRun { queue_id: QueueId },
    EnsureWorkspace { task_id: TaskId, vcs: VcsType },
    BuildContextPack { revision_id: RevisionId },
    RunGate { gate: GateName },
    EnqueueLandLane { task_id: TaskId, revision_id: RevisionId },
    EmitAsk { task_id: TaskId, body_md: String },
    MarkBlockedFailure { task_id: TaskId, reason: String },
    MarkDone { task_id: TaskId },
    ReleaseLease { queue_id: QueueId },
}
