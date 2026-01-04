PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tasks (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  status TEXT NOT NULL,
  priority INTEGER NOT NULL,
  tags_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS spec_revisions (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL,
  spec_hash TEXT NOT NULL,
  spec_path TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  FOREIGN KEY(task_id) REFERENCES tasks(id)
);

CREATE TABLE IF NOT EXISTS revisions (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL,
  spec_rev_id TEXT NOT NULL,
  spec_hash TEXT NOT NULL,
  profile TEXT NOT NULL,
  required_gates_json TEXT NOT NULL,
  required_validations_json TEXT NOT NULL,
  anchors_json TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  FOREIGN KEY(task_id) REFERENCES tasks(id),
  FOREIGN KEY(spec_rev_id) REFERENCES spec_revisions(id)
);

CREATE TABLE IF NOT EXISTS queue_items (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL,
  revision_id TEXT NOT NULL,
  lane TEXT NOT NULL,
  idempotency_key TEXT NOT NULL,
  priority INTEGER NOT NULL,
  enqueued_at INTEGER NOT NULL,
  visible_at INTEGER NOT NULL,
  attempts INTEGER NOT NULL,
  max_attempts INTEGER NOT NULL,
  FOREIGN KEY(task_id) REFERENCES tasks(id),
  FOREIGN KEY(revision_id) REFERENCES revisions(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS queue_items_idem_uq ON queue_items(lane, idempotency_key);

CREATE TABLE IF NOT EXISTS leases (
  id TEXT PRIMARY KEY,
  queue_id TEXT NOT NULL UNIQUE,
  worker_id TEXT NOT NULL,
  acquired_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  FOREIGN KEY(queue_id) REFERENCES queue_items(id)
);

CREATE INDEX IF NOT EXISTS leases_expires_idx ON leases(expires_at);

CREATE TABLE IF NOT EXISTS runs (
  id TEXT PRIMARY KEY,
  queue_id TEXT NOT NULL,
  task_id TEXT NOT NULL,
  revision_id TEXT NOT NULL,
  lane TEXT NOT NULL,
  started_at INTEGER NOT NULL,
  result TEXT,
  current_gate TEXT,
  FOREIGN KEY(queue_id) REFERENCES queue_items(id)
);

CREATE TABLE IF NOT EXISTS messages (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL,
  ty TEXT NOT NULL,
  body_md TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  FOREIGN KEY(task_id) REFERENCES tasks(id)
);
