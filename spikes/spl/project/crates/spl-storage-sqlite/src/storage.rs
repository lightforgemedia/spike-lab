use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde_json::json;
use spl_core::{GateOutcome, Lane, Lease, Message, MessageType, QueueId, QueueItem, Run, RunId, Snapshot, Task, TaskId, TaskStatus};
use spl_storage::Storage;

pub struct SqliteStorage {
    conn: Mutex<Connection>,
}

impl SqliteStorage {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(db_path).with_context(|| format!("open sqlite db {}", db_path.display()))?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        // init schema
        let init_sql = include_str!("../migrations/0001_init.sql");
        conn.execute_batch(init_sql)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn status_to_str(s: &TaskStatus) -> &'static str {
        match s {
            TaskStatus::Draft => "draft",
            TaskStatus::Ready => "ready",
            TaskStatus::BlockedHitl => "blocked_hitl",
            TaskStatus::BlockedFailure => "blocked_failure",
            TaskStatus::Done => "done",
        }
    }

    fn str_to_status(s: &str) -> TaskStatus {
        match s {
            "draft" => TaskStatus::Draft,
            "ready" => TaskStatus::Ready,
            "blocked_hitl" => TaskStatus::BlockedHitl,
            "blocked_failure" => TaskStatus::BlockedFailure,
            "done" => TaskStatus::Done,
            _ => TaskStatus::Draft,
        }
    }

    fn lane_to_str(l: &Lane) -> &'static str {
        match l {
            Lane::Execute => "execute",
            Lane::Land => "land",
        }
    }

    fn str_to_lane(s: &str) -> Lane {
        match s {
            "land" => Lane::Land,
            _ => Lane::Execute,
        }
    }

    pub fn insert_spec_revision(&self, spec_rev_id: &str, task_id: &str, spec_hash: &str, spec_path: &str, created_at: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO spec_revisions(id, task_id, spec_hash, spec_path, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![spec_rev_id, task_id, spec_hash, spec_path, created_at],
        )?;
        Ok(())
    }

    pub fn insert_revision_row(
        &self,
        revision_id: &str,
        task_id: &str,
        spec_rev_id: &str,
        spec_hash: &str,
        profile: &str,
        required_gates_json: &str,
        required_validations_json: &str,
        anchors_json: &str,
        created_at: i64,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO revisions(id, task_id, spec_rev_id, spec_hash, profile, required_gates_json, required_validations_json, anchors_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                revision_id,
                task_id,
                spec_rev_id,
                spec_hash,
                profile,
                required_gates_json,
                required_validations_json,
                anchors_json,
                created_at
            ],
        )?;
        Ok(())
    }
}

impl Storage for SqliteStorage {
    fn load_snapshot(&self, now_unix: i64) -> Result<Snapshot> {
        let conn = self.conn.lock().unwrap();

        let mut tasks = vec![];
        {
            let mut stmt = conn.prepare("SELECT id, title, status, priority, tags_json FROM tasks")?;
            let rows = stmt.query_map([], |r| {
                let tags_json: String = r.get(4)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                Ok(Task {
                    id: TaskId::from_str(r.get::<_, String>(0)?),
                    title: r.get(1)?,
                    status: Self::str_to_status(&r.get::<_, String>(2)?),
                    priority: r.get(3)?,
                    tags,
                })
            })?;
            for row in rows {
                tasks.push(row?);
            }
        }

        let mut queue = vec![];
        {
            let mut stmt = conn.prepare(
                "SELECT id, task_id, revision_id, lane, visible_at, attempts, max_attempts, priority, idempotency_key
                 FROM queue_items WHERE visible_at <= ?1"
            )?;
            let rows = stmt.query_map([now_unix], |r| {
                Ok(QueueItem {
                    id: QueueId::from_str(r.get::<_, String>(0)?),
                    task_id: TaskId::from_str(r.get::<_, String>(1)?),
                    revision_id: spl_core::RevisionId::from_str(r.get::<_, String>(2)?),
                    lane: Self::str_to_lane(&r.get::<_, String>(3)?),
                    visible_at_unix: r.get(4)?,
                    attempts: r.get::<_, i64>(5)? as u32,
                    max_attempts: r.get::<_, i64>(6)? as u32,
                    priority: r.get(7)?,
                    idempotency_key: r.get(8)?,
                })
            })?;
            for row in rows {
                queue.push(row?);
            }
        }

        let mut leases = vec![];
        {
            let mut stmt = conn.prepare("SELECT id, queue_id, worker_id, acquired_at, expires_at FROM leases")?;
            let rows = stmt.query_map([], |r| {
                Ok(Lease {
                    id: spl_core::LeaseId::from_str(r.get::<_, String>(0)?),
                    queue_id: QueueId::from_str(r.get::<_, String>(1)?),
                    worker_id: r.get(2)?,
                    acquired_at_unix: r.get(3)?,
                    expires_at_unix: r.get(4)?,
                })
            })?;
            for row in rows {
                leases.push(row?);
            }
        }

        let mut runs = vec![];
        {
            let mut stmt = conn.prepare("SELECT id, task_id, revision_id, lane, result, current_gate FROM runs")?;
            let rows = stmt.query_map([], |r| {
                let result: Option<String> = r.get(4)?;
                Ok(Run {
                    id: RunId::from_str(r.get::<_, String>(0)?),
                    task_id: TaskId::from_str(r.get::<_, String>(1)?),
                    revision_id: spl_core::RevisionId::from_str(r.get::<_, String>(2)?),
                    lane: Self::str_to_lane(&r.get::<_, String>(3)?),
                    result: result.map(|_| spl_core::RunResult::Pass), // placeholder mapping
                    current_gate: None,
                })
            })?;
            for row in rows {
                runs.push(row?);
            }
        }

        let mut messages = vec![];
        {
            let mut stmt = conn.prepare("SELECT id, task_id, ty, body_md, created_at FROM messages")?;
            let rows = stmt.query_map([], |r| {
                Ok(Message {
                    id: r.get(0)?,
                    task_id: TaskId::from_str(r.get::<_, String>(1)?),
                    ty: match r.get::<_, String>(2)?.as_str() {
                        "ask" => MessageType::Ask,
                        "decision" => MessageType::Decision,
                        "reset" => MessageType::Reset,
                        "review" => MessageType::Review,
                        _ => MessageType::Update,
                    },
                    body_md: r.get(3)?,
                    created_at_unix: r.get(4)?,
                })
            })?;
            for row in rows {
                messages.push(row?);
            }
        }

        Ok(Snapshot {
            now_unix,
            tasks,
            revisions: vec![],
            queue,
            leases,
            runs,
            messages,
        })
    }

    fn insert_task(&self, task: Task) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tags_json = serde_json::to_string(&task.tags).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "INSERT INTO tasks(id, title, status, priority, tags_json) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![task.id.0, task.title, Self::status_to_str(&task.status), task.priority, tags_json],
        )?;
        Ok(())
    }

    fn set_task_status(&self, task_id: &TaskId, status: TaskStatus) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE tasks SET status=?1 WHERE id=?2", params![Self::status_to_str(&status), task_id.0])?;
        Ok(())
    }

    fn enqueue(&self, item: QueueItem) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = crate::now_unix();
        conn.execute(
            "INSERT INTO queue_items(id, task_id, revision_id, lane, idempotency_key, priority, enqueued_at, visible_at, attempts, max_attempts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                item.id.0,
                item.task_id.0,
                item.revision_id.0,
                Self::lane_to_str(&item.lane),
                item.idempotency_key,
                item.priority,
                now,
                item.visible_at_unix,
                item.attempts as i64,
                item.max_attempts as i64
            ],
        )?;
        Ok(())
    }

    fn try_acquire_lease(&self, queue_id: &QueueId, worker_id: &str, ttl_secs: i64) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let now = crate::now_unix();
        let expires = now + ttl_secs;

        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM leases WHERE expires_at <= ?1", params![now])?;

        // Ensure item exists and visible (cheap check)
        let exists: i64 = tx.query_row(
            "SELECT COUNT(1) FROM queue_items WHERE id=?1 AND visible_at <= ?2",
            params![queue_id.0, now],
            |r| r.get(0),
        )?;
        if exists == 0 {
            tx.commit()?;
            return Ok(None);
        }

        // Insert lease (unique constraint on queue_id ensures exclusivity)
        let lease_id = spl_core::LeaseId::new().0;
        let res = tx.execute(
            "INSERT INTO leases(id, queue_id, worker_id, acquired_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![lease_id, queue_id.0, worker_id, now, expires],
        );

        match res {
            Ok(_) => {
                tx.commit()?;
                Ok(Some(lease_id))
            }
            Err(_) => {
                tx.commit()?;
                Ok(None)
            }
        }
    }

    fn release_lease(&self, queue_id: &QueueId, worker_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM leases WHERE queue_id=?1 AND worker_id=?2", params![queue_id.0, worker_id])?;
        Ok(())
    }

    fn create_run(&self, queue_id: &QueueId, run_id: RunId) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = crate::now_unix();

        let (task_id, revision_id, lane): (String, String, String) = conn.query_row(
            "SELECT task_id, revision_id, lane FROM queue_items WHERE id=?1",
            params![queue_id.0],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;

        conn.execute(
            "INSERT INTO runs(id, queue_id, task_id, revision_id, lane, started_at, result, current_gate)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL)",
            params![run_id.0, queue_id.0, task_id, revision_id, lane, now],
        )?;
        Ok(())
    }

    fn record_gate_outcome(&self, run_id: &RunId, outcome: &GateOutcome) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // v0: store only current gate string; full gate history lives in evidence artifacts.
        conn.execute(
            "UPDATE runs SET current_gate=?1 WHERE id=?2",
            params![format!("{:?}", outcome.gate), run_id.0],
        )?;
        Ok(())
    }

    fn add_message(&self, task_id: &TaskId, ty: MessageType, body_md: &str, now_unix: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let id = spl_core::TaskId::new().0;
        let ty_s = match ty {
            MessageType::Ask => "ask",
            MessageType::Update => "update",
            MessageType::Review => "review",
            MessageType::Decision => "decision",
            MessageType::Reset => "reset",
        };
        conn.execute(
            "INSERT INTO messages(id, task_id, ty, body_md, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, task_id.0, ty_s, body_md, now_unix],
        )?;
        Ok(())
    }
}

pub fn now_unix() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    dur.as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn sqlite_open_and_migrate() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("spl.db");
        let _ = SqliteStorage::open(&db_path).unwrap();
    }

    #[test]
    fn lease_is_exclusive() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("spl.db");
        let store = SqliteStorage::open(&db_path).unwrap();

        let task = Task {
            id: TaskId::from_str("pt-1"),
            title: "t".into(),
            status: TaskStatus::Ready,
            priority: 0,
            tags: vec![],
        };
        store.insert_task(task).unwrap();

        // insert a dummy spec_rev and revision so FK passes (minimal)
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT INTO spec_revisions(id, task_id, spec_hash, spec_path, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params!["s1","pt-1","h","/tmp/spec", now_unix()]
            ).unwrap();
            conn.execute("INSERT INTO revisions(id, task_id, spec_rev_id, spec_hash, profile, required_gates_json, required_validations_json, anchors_json, created_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params!["r1","pt-1","s1","h","standard","[]","[]","[]", now_unix()]
            ).unwrap();
        }

        let qi = QueueItem {
            id: QueueId::from_str("q1"),
            task_id: TaskId::from_str("pt-1"),
            revision_id: spl_core::RevisionId::from_str("r1"),
            lane: Lane::Execute,
            visible_at_unix: now_unix(),
            attempts: 0,
            max_attempts: 3,
            priority: 0,
            idempotency_key: "idem".into(),
        };
        store.enqueue(qi).unwrap();

        let a = store.try_acquire_lease(&QueueId::from_str("q1"), "w1", 60).unwrap();
        assert!(a.is_some());
        let b = store.try_acquire_lease(&QueueId::from_str("q1"), "w2", 60).unwrap();
        assert!(b.is_none());
    }
}
