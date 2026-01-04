# HTTP API (v1)

All endpoints are JSON.

## POST /v1/demo/enqueue

Enqueues a small demo run with two stages: build -> test.

Response:
```json
{ "run_id": "01J..." }
```

## POST /v1/agent/claim

Request:
```json
{ "agent_id": "agent-1", "capabilities": ["linux","slurm"] }
```

Response:
```json
{
  "assignment": {
    "job_id": "01J...",
    "run_id": "01J...",
    "stage_id": "01J...",
    "lease": { "agent_id": "agent-1", "token": "...", "expires_ms": 1730000000000 },
    "bundle_root": "/abs/path/.orchestrator/runs/<run>/stages/<stage>/attempt-1",
    "workspace_root": "/abs/path/.orchestrator/workspaces/<run>/stages/<stage>/attempt-1",
    "input_revision": "wzvkvn...",
    "exec": { "...": "..." }
  },
  "server_now_ms": 1730000000000
}
```

If no job is available, `assignment` is `null`.

## POST /v1/agent/heartbeat

Request:
```json
{ "agent_id": "agent-1", "job_id": "01J...", "lease_token": "..." }
```

Response:
```json
{ "ok": true, "new_expires_ms": 1730000000000, "server_now_ms": 1730000000000 }
```

## POST /v1/agent/complete

Request:
```json
{
  "agent_id": "agent-1",
  "job_id": "01J...",
  "lease_token": "...",
  "result": {
    "status": "succeeded",
    "started_ms": 1730000000000,
    "ended_ms": 1730000000100,
    "commands": [ { "...": "..." } ],
    "output_revision": "wzvkvn...",
    "executor_ref": null
  }
}
```

Response:
```json
{ "ok": true, "server_now_ms": 1730000000000 }
```
