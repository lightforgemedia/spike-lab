use spl_core::{GateOutcome, MessageType, QueueId, QueueItem, RunId, Snapshot, Task, TaskId, TaskStatus};

pub trait Storage: Send + Sync {
    fn load_snapshot(&self, now_unix: i64) -> anyhow::Result<Snapshot>;

    fn insert_task(&self, task: Task) -> anyhow::Result<()>;
    fn set_task_status(&self, task_id: &TaskId, status: TaskStatus) -> anyhow::Result<()>;

    fn enqueue(&self, item: QueueItem) -> anyhow::Result<()>;

    /// Attempt to acquire a lease for a queue item. Returns Some(LeaseId) if acquired.
    fn try_acquire_lease(&self, queue_id: &QueueId, worker_id: &str, ttl_secs: i64) -> anyhow::Result<Option<String>>;
    fn release_lease(&self, queue_id: &QueueId, worker_id: &str) -> anyhow::Result<()>;

    fn create_run(&self, queue_id: &QueueId, run_id: RunId) -> anyhow::Result<()>;
    fn record_gate_outcome(&self, run_id: &RunId, outcome: &GateOutcome) -> anyhow::Result<()>;

    fn add_message(&self, task_id: &TaskId, ty: MessageType, body_md: &str, now_unix: i64) -> anyhow::Result<()>;
}
