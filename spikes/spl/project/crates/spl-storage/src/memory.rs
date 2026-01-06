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

#[cfg(test)]
mod tests {
    use super::*;
    use spl_core::{LeaseId, QueueId, RevisionId, Task, TaskId};

    #[test]
    fn test_new_creates_empty_storage() {
        let storage = InMemoryStorage::new();
        let snapshot = storage.load_snapshot(0).unwrap();
        assert!(snapshot.tasks.is_empty());
        assert!(snapshot.queue.is_empty());
        assert!(snapshot.leases.is_empty());
        assert!(snapshot.runs.is_empty());
        assert!(snapshot.messages.is_empty());
    }

    #[test]
    fn test_insert_and_get_task() {
        let storage = InMemoryStorage::new();
        let task = Task {
            id: TaskId("task1".to_string()),
            title: "Test Task".to_string(),
            status: TaskStatus::Ready,
            priority: 0,
            tags: vec![],
        };
        storage.insert_task(task.clone()).unwrap();
        let snapshot = storage.load_snapshot(0).unwrap();
        assert_eq!(snapshot.tasks.len(), 1);
        assert_eq!(snapshot.tasks[0].id, task.id);
    }

    #[test]
    fn test_get_missing_task() {
        let storage = InMemoryStorage::new();
        let snapshot = storage.load_snapshot(0).unwrap();
        assert!(snapshot.tasks.is_empty());
    }

    #[test]
    fn test_delete_task() {
        let storage = InMemoryStorage::new();
        let task = Task {
            id: TaskId("task1".to_string()),
            title: "Test Task".to_string(),
            status: TaskStatus::Ready,
            priority: 0,
            tags: vec![],
        };
        storage.insert_task(task).unwrap();
        let mut inner = storage.inner.lock().unwrap();
        inner.tasks.remove("task1");
        drop(inner);
        let snapshot = storage.load_snapshot(0).unwrap();
        assert!(snapshot.tasks.is_empty());
    }

    #[test]
    fn test_set_task_status() {
        let storage = InMemoryStorage::new();
        let task_id = TaskId("task1".to_string());
        let task = Task {
            id: task_id.clone(),
            title: "Test Task".to_string(),
            status: TaskStatus::Ready,
            priority: 0,
            tags: vec![],
        };
        storage.insert_task(task).unwrap();
        storage.set_task_status(&task_id, TaskStatus::Done).unwrap();
        let snapshot = storage.load_snapshot(0).unwrap();
        assert_eq!(snapshot.tasks[0].status, TaskStatus::Done);
    }

    #[test]
    fn test_enqueue() {
        let storage = InMemoryStorage::new();
        let item = QueueItem {
            id: QueueId("queue1".to_string()),
            task_id: TaskId("task1".to_string()),
            revision_id: RevisionId("rev1".to_string()),
            lane: spl_core::Lane::Execute,
            visible_at_unix: 0,
            attempts: 0,
            max_attempts: 0,
            priority: 0,
            idempotency_key: "".to_string(),
        };
        storage.enqueue(item.clone()).unwrap();
        let snapshot = storage.load_snapshot(0).unwrap();
        assert_eq!(snapshot.queue.len(), 1);
        assert_eq!(snapshot.queue[0].id, item.id);
    }

    #[test]
    fn test_try_acquire_and_release_lease() {
        let storage = InMemoryStorage::new();
        let queue_id = QueueId("queue1".to_string());
        let worker_id = "worker1";
        let lease_id = storage.try_acquire_lease(&queue_id, worker_id, 60).unwrap();
        assert!(lease_id.is_some());

        let snapshot = storage.load_snapshot(0).unwrap();
        assert_eq!(snapshot.leases.len(), 1);

        storage.release_lease(&queue_id, worker_id).unwrap();
        let snapshot = storage.load_snapshot(0).unwrap();
        assert!(snapshot.leases.is_empty());
    }

    #[test]
    fn test_create_run() {
        let storage = InMemoryStorage::new();
        let task_id = TaskId("task1".to_string());
        let revision_id = RevisionId("rev1".to_string());
        let queue_id = QueueId("queue1".to_string());

        storage.enqueue(QueueItem {
            id: queue_id.clone(),
            task_id: task_id.clone(),
            revision_id: revision_id.clone(),
            lane: spl_core::Lane::Execute,
            visible_at_unix: 0,
            attempts: 0,
            max_attempts: 0,
            priority: 0,
            idempotency_key: "".to_string(),
        }).unwrap();

        let run_id = RunId("run1".to_string());
        storage.create_run(&queue_id, run_id.clone()).unwrap();

        let snapshot = storage.load_snapshot(0).unwrap();
        assert_eq!(snapshot.runs.len(), 1);
        assert_eq!(snapshot.runs[0].id, run_id);
    }

    #[test]
    fn test_add_message() {
        let storage = InMemoryStorage::new();
        let task_id = TaskId("task1".to_string());
        storage.add_message(&task_id, MessageType::Update, "test message", 0).unwrap();
        let snapshot = storage.load_snapshot(0).unwrap();
        assert_eq!(snapshot.messages.len(), 1);
        assert_eq!(snapshot.messages[0].body_md, "test message");
    }
}
