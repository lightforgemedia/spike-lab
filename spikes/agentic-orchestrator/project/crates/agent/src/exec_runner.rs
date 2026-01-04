use std::path::{Path, PathBuf};
use std::process::Stdio;

use orchestrator_core::model::{
    CommandResult, ExecBlockResult, ExecutorSpec, JobLease, JobStatus, StageConfig,
};
use orchestrator_core::{new_ulid, now_ms};
use tokio::io::AsyncWriteExt;

use crate::slurm_runner;

/// Execute an exec block, write an execution bundle, return result.
///
/// Bundle layout:
/// `<runs_root>/<run_id>/<stage_id>/<exec_id>/...`
pub async fn run_exec_block(lease: &JobLease, runs_root: &Path, agent_id: &str) -> ExecBlockResult {
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
            executor: ExecutorSpec::Local,
            slurm_job_id: None,
            extra_files: vec![],
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
            executor: ExecutorSpec::Local,
            slurm_job_id: None,
            extra_files: vec![],
            started_at_ms,
            finished_at_ms: now_ms(),
            status: JobStatus::Failed,
            commands: vec![],
            error: Some("unexpected stage config".into()),
        };
    };

    // Write small, always-on metadata in the bundle. Best-effort.
    let mut extra_files = vec![];
    if let Ok(more) = write_bundle_meta(&bundle_root, spec, lease, agent_id).await {
        extra_files.extend(more);
    }

    match &spec.executor {
        ExecutorSpec::Local => {
            run_local(spec, lease, &bundle_root, started_at_ms, extra_files).await
        }
        ExecutorSpec::Slurm(slurm) => {
            slurm_runner::run_slurm(spec, slurm, lease, &bundle_root, started_at_ms, extra_files)
                .await
        }
    }
}

async fn run_local(
    spec: &orchestrator_core::model::ExecBlockSpec,
    lease: &JobLease,
    bundle_root: &Path,
    started_at_ms: i64,
    extra_files: Vec<String>,
) -> ExecBlockResult {
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
            status: if cmd_ok {
                JobStatus::Succeeded
            } else {
                JobStatus::Failed
            },
            stdout_path: stdout_name,
            stderr_path: stderr_name,
            error: err,
        });

        if !cmd_ok && spec.halt_on_error {
            break;
        }
    }

    let finished_at_ms = now_ms();
    let status = if overall_ok {
        JobStatus::Succeeded
    } else {
        JobStatus::Failed
    };

    let result = ExecBlockResult {
        run_id: lease.run_id.clone(),
        stage_id: lease.stage_id.clone(),
        bundle_root: bundle_root.display().to_string(),
        executor: spec.executor.clone(),
        slurm_job_id: None,
        extra_files,
        started_at_ms,
        finished_at_ms,
        status,
        commands: results,
        error: overall_error,
    };

    write_manifest(bundle_root, &result).await;
    result
}

async fn write_manifest(bundle_root: &Path, result: &ExecBlockResult) {
    let manifest_path = bundle_root.join("manifest.json");
    if let Ok(json) = serde_json::to_vec_pretty(result) {
        if let Ok(mut f) = tokio::fs::File::create(manifest_path).await {
            let _ = f.write_all(&json).await;
            let _ = f.flush().await;
        }
    }
}

async fn write_bundle_meta(
    bundle_root: &Path,
    spec: &orchestrator_core::model::ExecBlockSpec,
    lease: &JobLease,
    agent_id: &str,
) -> anyhow::Result<Vec<String>> {
    let meta_dir = bundle_root.join("meta");
    tokio::fs::create_dir_all(&meta_dir).await?;

    let env_path = meta_dir.join("env.json");
    let env_rel = "meta/env.json".to_string();
    let repo_path = meta_dir.join("repo.txt");
    let repo_rel = "meta/repo.txt".to_string();

    let meta = serde_json::json!({
        "agent_id": agent_id,
        "run_id": lease.run_id,
        "stage_id": lease.stage_id,
        "workdir": spec.workdir,
        "executor": spec.executor,
        "started_at_ms": now_ms(),
    });
    tokio::fs::write(&env_path, serde_json::to_vec_pretty(&meta)?).await?;

    // Best-effort VCS snapshot. Prefer jj, fall back to git.
    let workdir = PathBuf::from(&spec.workdir);
    let mut text = String::new();

    if let Ok(jj) = run_cmd_capture(&workdir, "jj", &["--version"]).await {
        text.push_str("# jj --version\n");
        text.push_str(&jj);
        text.push_str("\n\n");
        if let Ok(status) = run_cmd_capture(&workdir, "jj", &["status", "--no-pager"]).await {
            text.push_str("# jj status\n");
            text.push_str(&status);
            text.push_str("\n\n");
        }
        if let Ok(log) = run_cmd_capture(&workdir, "jj", &["log", "-r", "@", "-n", "1", "--no-pager"]).await {
            text.push_str("# jj log -r @ -n 1\n");
            text.push_str(&log);
            text.push_str("\n\n");
        }
    } else {
        text.push_str("# jj not available; falling back to git\n\n");
    }

    if let Ok(head) = run_cmd_capture(&workdir, "git", &["rev-parse", "HEAD"]).await {
        text.push_str("# git rev-parse HEAD\n");
        text.push_str(&head);
        text.push_str("\n\n");
    }
    if let Ok(status) = run_cmd_capture(&workdir, "git", &["status", "--porcelain"]).await {
        text.push_str("# git status --porcelain\n");
        text.push_str(&status);
        text.push_str("\n\n");
    }

    tokio::fs::write(&repo_path, text).await?;

    Ok(vec![env_rel, repo_rel])
}

async fn run_cmd_capture(cwd: &Path, program: &str, args: &[&str]) -> anyhow::Result<String> {
    let out = tokio::process::Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !out.status.success() {
        anyhow::bail!(
            "command failed: {} {:?}: {}",
            program,
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn sanitize_component(s: &str) -> String {
    // Directory-safe component:
    // - replace ':' used in Surreal record ids
    // - replace path separators
    s.replace(':', "_").replace('/', "_")
}
