use spl_core::{Task, TaskStatus, TaskId, QueueItem, QueueId, Lease, LeaseId, Run, RunId, RunResult, Lane, GateName, RevisionId};

#[test]
fn test_task_creation() {
    let task = Task {
        id: TaskId::new(),
        title: "Test Task".to_string(),
        status: TaskStatus::Draft,
        priority: 1,
        tags: vec!["test".to_string()],
    };
    assert_eq!(task.title, "Test Task");
    assert_eq!(task.status, TaskStatus::Draft);
}

#[test]
fn test_task_status_enum() {
    assert_eq!(TaskStatus::Draft, TaskStatus::Draft);
    assert_ne!(TaskStatus::Draft, TaskStatus::Ready);
}

#[test]
fn test_task_id_new() {
    let task_id1 = TaskId::new();
    let task_id2 = TaskId::new();
    assert_ne!(task_id1, task_id2);
}

#[test]
fn test_queue_item_creation() {
    let queue_item = QueueItem {
        id: QueueId::new(),
        task_id: TaskId::new(),
        revision_id: RevisionId::new(),
        lane: Lane::Execute,
        visible_at_unix: 0,
        attempts: 0,
        max_attempts: 3,
        priority: 1,
        idempotency_key: "key1".to_string(),
    };
    assert_eq!(queue_item.attempts, 0);
    assert_eq!(queue_item.max_attempts, 3);
}

#[test]
fn test_lease_creation() {
    let lease = Lease {
        id: LeaseId::new(),
        queue_id: QueueId::new(),
        worker_id: "worker1".to_string(),
        acquired_at_unix: 12345,
        expires_at_unix: 67890,
    };
    assert_eq!(lease.worker_id, "worker1");
}

#[test]
fn test_run_creation() {
    let run = Run {
        id: RunId::new(),
        task_id: TaskId::new(),
        revision_id: RevisionId::new(),
        lane: Lane::Execute,
        result: None,
        current_gate: Some(GateName::SpecCompile),
    };
    assert_eq!(run.current_gate, Some(GateName::SpecCompile));
}

#[test]
fn test_run_result_enum() {
    assert_eq!(RunResult::Pass, RunResult::Pass);
    assert_ne!(RunResult::Pass, RunResult::FailGate);
}
