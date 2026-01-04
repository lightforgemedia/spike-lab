use std::path::{Path, PathBuf};
use std::process::Stdio;

use orchestrator_core::model::{CommandResult, ExecBlockResult, JobLease, JobStatus, StageConfig};
use orchestrator_core::{new_ulid, now_ms};
use tokio::io::AsyncWriteExt;

/// Execute an exec block, write an execution bundle, return result.
///
/// Bundle layout:
/// `<runs_root>/<run_id>/<stage_id>/<exec_id>/...`
pub async fn run_exec_block(lease: &JobLease, runs_root: &Path) -> ExecBlockResult {
    let started_at_ms = now_ms();

    let exec_id = new_ulid().to_string();
    let bundle_root = runs_root
        .join(sanitize_component(&lease.run_id))
        .join(sanitize_component(&lease.stage_id))
        .join(exec_id);

    if let Err(e) = tokio::fs::create_dir_all(&bundle_root).await {
        return ExecBlockResult {
            run_id: lease.run_id.clone(),
            stage_id: lease.stage_id.clone(),
            bundle_root: bundle_root.display().to_string(),
            started_at_ms,
            finished_at_ms: now_ms(),
            status: JobStatus::Failed,
            commands: vec![],
            error: Some(format!("failed to create bundle dir: {e}")),
        };
    }

    let StageConfig::ExecBlock(spec) = &lease.config else {
        return ExecBlockResult {
            run_id: lease.run_id.clone(),
            stage_id: lease.stage_id.clone(),
            bundle_root: bundle_root.display().to_string(),
            started_at_ms,
            finished_at_ms: now_ms(),
            status: JobStatus::Failed,
            commands: vec![],
            error: Some("unexpected stage config".into()),
        };
    };

    let workdir = PathBuf::from(&spec.workdir);

    let mut results: Vec<CommandResult> = Vec::with_capacity(spec.commands.len());
    let mut overall_ok = true;
    let mut overall_error: Option<String> = None;

    for (index, cmd) in spec.commands.iter().enumerate() {
        let cmd_started = now_ms();

        let stdout_name = format!("cmd-{index:03}.stdout.log");
        let stderr_name = format!("cmd-{index:03}.stderr.log");
        let stdout_path = bundle_root.join(&stdout_name);
        let stderr_path = bundle_root.join(&stderr_name);

        let cwd = cmd
            .cwd
            .as_ref()
            .map(|rel| workdir.join(rel))
            .unwrap_or_else(|| workdir.clone());

        let mut child = match tokio::process::Command::new(&cmd.program)
            .args(&cmd.args)
            .current_dir(&cwd)
            .envs(spec.env.iter())
            .envs(cmd.env.iter())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(ch) => ch,
            Err(e) => {
                overall_ok = false;
                overall_error.get_or_insert_with(|| format!("spawn failed: {e}"));
                results.push(CommandResult {
                    index,
                    program: cmd.program.clone(),
                    args: cmd.args.clone(),
                    cwd: cmd.cwd.clone(),
                    started_at_ms: cmd_started,
                    finished_at_ms: now_ms(),
                    exit_code: None,
                    status: JobStatus::Failed,
                    stdout_path: stdout_name,
                    stderr_path: stderr_name,
                    error: Some(format!("spawn failed: {e}")),
                });
                if spec.halt_on_error {
                    break;
                }
                continue;
            }
        };

        let mut out_file = match tokio::fs::File::create(&stdout_path).await {
            Ok(f) => f,
            Err(e) => {
                overall_ok = false;
                overall_error.get_or_insert_with(|| format!("failed to create stdout file: {e}"));
                // Proceed without capturing output.
                tokio::fs::File::create(&stdout_path).await.ok();
                tokio::fs::File::create(&stderr_path).await.ok();
                results.push(CommandResult {
                    index,
                    program: cmd.program.clone(),
                    args: cmd.args.clone(),
                    cwd: cmd.cwd.clone(),
                    started_at_ms: cmd_started,
                    finished_at_ms: now_ms(),
                    exit_code: None,
                    status: JobStatus::Failed,
                    stdout_path: stdout_name,
                    stderr_path: stderr_name,
                    error: Some(format!("log file error: {e}")),
                });
                if spec.halt_on_error {
                    break;
                }
                continue;
            }
        };

        let mut err_file = match tokio::fs::File::create(&stderr_path).await {
            Ok(f) => f,
            Err(e) => {
                overall_ok = false;
                overall_error.get_or_insert_with(|| format!("failed to create stderr file: {e}"));
                results.push(CommandResult {
                    index,
                    program: cmd.program.clone(),
                    args: cmd.args.clone(),
                    cwd: cmd.cwd.clone(),
                    started_at_ms: cmd_started,
                    finished_at_ms: now_ms(),
                    exit_code: None,
                    status: JobStatus::Failed,
                    stdout_path: stdout_name,
                    stderr_path: stderr_name,
                    error: Some(format!("log file error: {e}")),
                });
                if spec.halt_on_error {
                    break;
                }
                continue;
            }
        };

        // Copy stdout/stderr concurrently while waiting for process exit.
        let mut stdout = child.stdout.take();
        let mut stderr = child.stderr.take();

        let out_fut = async {
            if let Some(ref mut s) = stdout {
                let _ = tokio::io::copy(s, &mut out_file).await;
                let _ = out_file.flush().await;
            }
        };

        let err_fut = async {
            if let Some(ref mut s) = stderr {
                let _ = tokio::io::copy(s, &mut err_file).await;
                let _ = err_file.flush().await;
            }
        };

        let status_fut = child.wait();

        let (_, _, status_res) = tokio::join!(out_fut, err_fut, status_fut);

        let cmd_finished = now_ms();

        let (exit_code, cmd_ok, err) = match status_res {
            Ok(s) => (s.code(), s.success(), None),
            Err(e) => (None, false, Some(format!("wait failed: {e}"))),
        };

        if !cmd_ok {
            overall_ok = false;
            overall_error.get_or_insert_with(|| {
                format!("command {index} failed (exit={:?})", exit_code)
            });
        }

        results.push(CommandResult {
            index,
            program: cmd.program.clone(),
            args: cmd.args.clone(),
            cwd: cmd.cwd.clone(),
            started_at_ms: cmd_started,
            finished_at_ms: cmd_finished,
            exit_code,
            status: if cmd_ok { JobStatus::Succeeded } else { JobStatus::Failed },
            stdout_path: stdout_name,
            stderr_path: stderr_name,
            error: err,
        });

        if !cmd_ok && spec.halt_on_error {
            break;
        }
    }

    let finished_at_ms = now_ms();
    let status = if overall_ok { JobStatus::Succeeded } else { JobStatus::Failed };

    let result = ExecBlockResult {
        run_id: lease.run_id.clone(),
        stage_id: lease.stage_id.clone(),
        bundle_root: bundle_root.display().to_string(),
        started_at_ms,
        finished_at_ms,
        status,
        commands: results,
        error: overall_error,
    };

    // Write manifest.json (same shape as result).
    let manifest_path = bundle_root.join("manifest.json");
    if let Ok(json) = serde_json::to_vec_pretty(&result) {
        if let Ok(mut f) = tokio::fs::File::create(manifest_path).await {
            let _ = f.write_all(&json).await;
            let _ = f.flush().await;
        }
    }

    result
}

fn sanitize_component(s: &str) -> String {
    // Directory-safe component:
    // - replace ':' used in Surreal record ids
    // - replace path separators
    s.replace(':', "_").replace('/', "_")
}
