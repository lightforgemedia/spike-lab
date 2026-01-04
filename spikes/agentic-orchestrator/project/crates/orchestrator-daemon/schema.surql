-- Minimal schema / indexes for orchestrator.
-- SurrealDB is schemaless by default; these indexes help scheduling queries.

DEFINE TABLE project SCHEMALESS;
DEFINE TABLE run SCHEMALESS;
DEFINE TABLE stage SCHEMALESS;
DEFINE TABLE job SCHEMALESS;
DEFINE TABLE agent SCHEMALESS;

DEFINE TABLE depends_on SCHEMALESS;

-- Helpful indexes
DEFINE INDEX project_root_path_unique ON project FIELDS root_path UNIQUE;

DEFINE INDEX run_by_project ON run FIELDS project_id;
DEFINE INDEX stage_by_run ON stage FIELDS run_id;
DEFINE INDEX job_by_run ON job FIELDS run_id;
DEFINE INDEX job_by_state ON job FIELDS state;
DEFINE INDEX stage_by_state ON stage FIELDS state;
