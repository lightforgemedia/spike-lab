use crate::{Command, GateName, Lane, Snapshot, TaskStatus};

/// Very small v0 planner:
/// - pick the highest priority visible queue item in Execute lane
/// - ask shell to acquire a lease
///
/// The imperative shell will translate this into storage calls and then call back
/// with outcomes (future work). For now, `plan_next` is used by the runner as
/// a convenience for dry-run printing.
pub fn plan_next(snapshot: &Snapshot) -> Vec<Command> {
    // Derive visible items: visible_at <= now and no lease present.
    // KISS: assume snapshot.queue is already filtered by storage for v0.
    let mut queue = snapshot.queue.clone();
    queue.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.visible_at_unix.cmp(&b.visible_at_unix)));

    if let Some(item) = queue.into_iter().find(|qi| qi.lane == Lane::Execute) {
        return vec![Command::AcquireLease { queue_id: item.id }];
    }

    vec![]
}

/// Pure helper: compute whether a task can be enqueued.
/// v0 rule: task must be Ready and not Blocked* or Done.
pub fn task_is_enqueueable(status: &TaskStatus) -> bool {
    matches!(status, TaskStatus::Ready)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueueable_only_when_ready() {
        assert!(task_is_enqueueable(&TaskStatus::Ready));
        assert!(!task_is_enqueueable(&TaskStatus::Draft));
        assert!(!task_is_enqueueable(&TaskStatus::BlockedFailure));
        assert!(!task_is_enqueueable(&TaskStatus::BlockedHitl));
        assert!(!task_is_enqueueable(&TaskStatus::Done));
    }
}
