use std::collections::HashMap;
use std::sync::Mutex;

use spl_core::{GateOutcome, Lease, Message, MessageType, QueueId, QueueItem, Run, RunId, Snapshot, Task, TaskId, TaskStatus};
use crate::traits::Storage;

/// In-memory storage for tests. Not durable, but good for unit/small scenario tests.
#[derive(Default)]
pub struct InMemoryStorage {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    tasks: HashMap<String, Task>,
    queue: HashMap<String, QueueItem>,
    leases: HashMap<String, Lease>,
    runs: HashMap<String, Run>,
    messages: Vec<Message>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Storage for InMemoryStorage {
    fn load_snapshot(&self, now_unix: i64) -> anyhow::Result<Snapshot> {
        let inner = self.inner.lock().unwrap();
        Ok(Snapshot {
            now_unix,
            tasks: inner.tasks.values().cloned().collect(),
            revisions: vec![],
            queue: inner.queue.values().cloned().collect(),
            leases: inner.leases.values().cloned().collect(),
            runs: inner.runs.values().cloned().collect(),
            messages: inner.messages.clone(),
        })
    }

    fn insert_task(&self, task: Task) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        inner.tasks.insert(task.id.0.clone(), task);
        Ok(())
    }

    fn set_task_status(&self, task_id: &TaskId, status: TaskStatus) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(t) = inner.tasks.get_mut(&task_id.0) {
            t.status = status;
        }
        Ok(())
    }

    fn enqueue(&self, item: QueueItem) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        inner.queue.insert(item.id.0.clone(), item);
        Ok(())
    }

    fn try_acquire_lease(&self, queue_id: &QueueId, worker_id: &str, ttl_secs: i64) -> anyhow::Result<Option<String>> {
        let mut inner = self.inner.lock().unwrap();
        if inner.leases.contains_key(&queue_id.0) {
            return Ok(None);
        }
        let now = chrono_like_now();
        let lease_id = spl_core::LeaseId::new();
        inner.leases.insert(queue_id.0.clone(), Lease {
            id: lease_id.clone(),
            queue_id: queue_id.clone(),
            worker_id: worker_id.to_string(),
            acquired_at_unix: now,
            expires_at_unix: now + ttl_secs,
        });
        Ok(Some(lease_id.0))
    }

    fn release_lease(&self, queue_id: &QueueId, worker_id: &str) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(l) = inner.leases.get(&queue_id.0) {
            if l.worker_id == worker_id {
                inner.leases.remove(&queue_id.0);
            }
        }
        Ok(())
    }

    fn create_run(&self, queue_id: &QueueId, run_id: RunId) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        let qi = inner.queue.get(&queue_id.0).cloned();
        if let Some(qi) = qi {
            inner.runs.insert(run_id.0.clone(), Run {
                id: run_id,
                task_id: qi.task_id,
                revision_id: qi.revision_id,
                lane: qi.lane,
                result: None,
                current_gate: None,
            });
        }
        Ok(())
    }

    fn record_gate_outcome(&self, _run_id: &RunId, _outcome: &GateOutcome) -> anyhow::Result<()> {
        Ok(())
    }

    fn add_message(&self, task_id: &TaskId, ty: MessageType, body_md: &str, now_unix: i64) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        inner.messages.push(Message {
            id: spl_core::TaskId::new().0, // cheap unique id
            task_id: task_id.clone(),
            ty,
            body_md: body_md.to_string(),
            created_at_unix: now_unix,
        });
        Ok(())
    }
}

/// Keep chrono out of the workspace deps for now (KISS).
fn chrono_like_now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    dur.as_secs() as i64
}
