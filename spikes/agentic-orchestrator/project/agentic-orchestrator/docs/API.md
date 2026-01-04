# HTTP API (v0)

Base URL: `http://<daemon>/`

## Health

- `GET /healthz` -> 200 OK

## Demo

- `POST /v1/demo/enqueue`
  - Body: `{ "project_path": "...", "description": "..." }`
  - Creates:
    - a demo workflow (if missing)
    - an intent
    - a run
    - runnable jobs

## Agent

- `POST /v1/agent/claim`
  - Body: `{ "agent_id": "agent-1" }`
  - Response: `{ "lease": JobLease | null }`

- `POST /v1/agent/complete`
  - Body: `{ "agent_id": "...", "job_id": "...", "lease_token": "...", "result": ExecBlockResult }`
  - Response: `{ "ok": true }`
