use crate::{ids::*, model::*};

#[derive(Clone, Debug)]
pub struct GateOutcome {
    pub gate: GateName,
    pub status: GateStatus,
    pub remediation: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Outcome {
    LeaseAcquired { queue_id: QueueId },
    LeaseUnavailable { queue_id: QueueId },
    RunStarted { run_id: RunId },
    GateCompleted { run_id: RunId, outcome: GateOutcome },
    AskEmitted { task_id: TaskId },
    LandEnqueued { task_id: TaskId, queue_id: QueueId },
    DoneMarked { task_id: TaskId },
    BlockedFailureMarked { task_id: TaskId },
    CrashRecorded { run_id: RunId },
}
