# Specs

## Data model

### WorkflowDef

Logical workflow (name + metadata).

Fields:
- `name: string` (unique)
- `description: string?`
- `created_at_ms: number`

### WorkflowVersion

Immutable version of a workflow definition.

Fields:
- `workflow_name: string`
- `hash: string` (sha256 of canonical JSON)
- `spec: WorkflowSpec` (stored as JSON)
- `created_at_ms: number`

### Run

Fields:
- `id: ULID`
- `project_path: string`
- `intent: IntentSpec`
- `workflow_name: string`
- `workflow_hash: string`
- `status: run_status`
- timestamps

### StageRun

Fields:
- `id: ULID`
- `run_id: ULID`
- `node_id: string` (from workflow)
- `kind: stage_kind`
- `deps: ULID[]` (stage IDs that must succeed)
- `status: stage_status`
- `exec: ExecBlockSpec?` (snapshot for exec stages)
- `output_revision: string?`
- retry fields:
  - `attempts_used: u32` (failed attempts so far)
  - `max_attempts: u32` (default 1)
- timestamps

### Job

Fields:
- `id: ULID`
- `stage_id: ULID`
- `status: job_status`
- lease fields:
  - `lease_owner: string?`
  - `lease_token: string?`
  - `lease_expires_at_ms: number?`
- `attempt: u32` (attempt number derived from stage attempts)
- timestamps

### ExecAttempt

Fields:
- `id: ULID`
- `job_id: ULID`
- `stage_id: ULID`
- `status: succeeded|failed`
- `artifact_dir: string` (agent-side path)
- `artifact_bundle: string?` (daemon-side path returned from upload)
- `commands: CommandResult[]`
- timestamps

## WorkflowSpec schema

```json
{
  "name": "rust-ci",
  "description": "Build & test",
  "nodes": [
    {
      "id": "build",
      "type": "exec_block",
      "exec": {
        "executor": "local",
        "workdir": ".",
        "env": {},
        "max_attempts": 2,
        "commands": [{"argv": ["cargo", "build", "--locked"]}]
      }
    }
  ],
  "edges": [{"from": "build", "to": "test"}]
}
```

## API

### POST /v1/runs/enqueue

Body: `RunEnqueueRequest`

Returns: `{ run_id, workflow_hash }`

### POST /v1/agent/claim

Body: `{ agent_id }`

Returns either:
- `204 No Content` if no job available
- `200 OK` with `JobClaim`

### POST /v1/agent/complete

Body: `JobCompleteRequest`

Must include correct `lease_token`.
Updates job/stage/run statuses.

### POST /v1/stages/{stage_id}/approve

Body: `{ approver, note? }`

Effect:
- only works when stage is `gate` and `needs_human`
- sets stage to `succeeded` and unblocks downstream

### POST /v1/stages/{stage_id}/reject

Body: `{ approver, note? }`

Effect:
- sets stage to `failed`
- marks run failed

### POST /v1/jobs/{job_id}/artifacts

Body: raw ZIP bytes (`Content-Type: application/zip`)

Returns: `{ artifact_uri }`

Agents upload the entire artifact directory as a single ZIP bundle.
The daemon stores the bundle under `--artifact-store-dir/<run_id>/<stage_id>/<job_id>/bundle.zip`.

## Status transitions

StageRun:
- `pending` → `queued` (scheduler creates job)
- `queued` → `running` (agent claims job)
- `running` → `succeeded` (agent complete OK)
- `running` → `pending` (agent complete FAILED but attempts remain)
- `running` → `failed` (agent complete FAILED and attempts exhausted)
- `pending` → `needs_human` (gate stage w/ approval)
- `needs_human` → `succeeded` (approval endpoint)

Job:
- `queued` → `running` (lease token issued)
- `running` → `succeeded|failed`
- `running` (lease expired) → eligible to be reclaimed

## Artifacts layout (agent)

`{state_dir}/artifacts/{run_id}/{stage_id}/{job_id}/`

Contains:
- `manifest.json`
- `{cmd_index}-{cmd_name}/stdout.log`
- `{cmd_index}-{cmd_name}/stderr.log`
- `bundle.zip` (uploaded to daemon)
