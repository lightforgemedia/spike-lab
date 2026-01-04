# Slurm executor

The `slurm` executor allows an `exec_block` to run on a Slurm cluster.

## How it works

When `ExecBlockSpec.executor.kind = "slurm"`, the agent:

1. Writes a batch script into the stage bundle: `slurm-job.sh`.
2. Submits it with `sbatch --parsable`, directing Slurm stdout/stderr into `slurm.stdout.log` and `slurm.stderr.log`.
3. Polls `squeue -h -j <jobid>` until the job disappears.
4. Reads per-command marker files produced by the script (`cmd-000.exit`, `cmd-000.started`, etc.) and builds `manifest.json`.

## Bundle additions

The Slurm runner adds these files (also listed in `ExecBlockResult.extra_files`):

- `slurm-job.sh`
- `slurm.stdout.log`
- `slurm.stderr.log`
- `slurm.done` / `slurm.failed` / `slurm.failed_idx`

Command stdout/stderr follow the same naming as the local runner:

- `cmd-000.stdout.log`
- `cmd-000.stderr.log`

## Configuration

Example JSON payload fragment for an exec block:

```json
{
  "workdir": "/abs/path/to/workspace",
  "executor": {
    "kind": "slurm",
    "partition": "short",
    "time_limit": "00:10:00",
    "cpus_per_task": 4,
    "mem_mb": 4096,
    "extra_args": ["--qos=normal"],
    "poll_ms": 2000
  },
  "commands": [
    {"program": "cargo", "args": ["test", "--all"]}
  ]
}
```

## Limitations (v0)

- Assumes a **shared filesystem** between agent host and compute nodes.
- Uses `squeue` polling; clusters with restricted visibility may require `sacct` or a controller-side integration.
- Timestamp precision is based on marker file mtimes (not full RFC3339 parsing) to keep dependencies minimal.

These are all straightforward to improve in later phases.