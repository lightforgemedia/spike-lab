use crate::{types::*, Lease, QueueItem, Revision, Run, Task};

/// Read-only view of state used by the functional core to plan next actions.
/// The imperative shell is responsible for producing this snapshot from storage.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub now_unix: i64,
    pub tasks: Vec<Task>,
    pub revisions: Vec<Revision>,
    pub queue: Vec<QueueItem>,
    pub leases: Vec<Lease>,
    pub runs: Vec<Run>,
    pub messages: Vec<Message>,
}
