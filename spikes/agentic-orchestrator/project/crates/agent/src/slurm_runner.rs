use std::path::{Path, PathBuf};
use std::process::Stdio;

use orchestrator_core::model::{
    CommandResult, ExecBlockResult, ExecBlockSpec, JobLease, JobStatus, SlurmSpec,
};
use orchestrator_core::now_ms;
use tokio::io::AsyncWriteExt;

/// Execute an exec block by submitting a Slurm batch job.
///
/// This is intentionally conservative:
/// - it assumes a shared filesystem so the compute node can write logs into the bundle
/// - it stops on first failed command when `spec.halt_on_error=true`
/// - it writes per-command stdout/stderr files the same way the local runner does
pub async fn run_slurm(
    spec: &ExecBlockSpec,
    slurm: &SlurmSpec,
    lease: &JobLease,
    bundle_root: &Path,
    started_at_ms: i64,
    mut extra_files: Vec<String>,
) -> ExecBlockResult {
    let workdir = PathBuf::from(&spec.workdir);

    // Write batch script and helper marker paths.
    let script_path = bundle_root.join("slurm-job.sh");
    let slurm_out = bundle_root.join("slurm.stdout.log");
    let slurm_err = bundle_root.join("slurm.stderr.log");
    let done_marker = bundle_root.join("slurm.done");
    let failed_marker = bundle_root.join("slurm.failed");
    let failed_idx_path = bundle_root.join("slurm.failed_idx");

    extra_files.extend([
        "slurm-job.sh".to_string(),
        "slurm.stdout.log".to_string(),
        "slurm.stderr.log".to_string(),
        "slurm.done".to_string(),
        "slurm.failed".to_string(),
        "slurm.failed_idx".to_string(),
    ]);

    if let Err(e) = write_batch_script(
        &script_path,
        &workdir,
        bundle_root,
        spec,
        &done_marker,
        &failed_marker,
        &failed_idx_path,
    )
    .await
    {
        return ExecBlockResult {
            run_id: lease.run_id.clone(),
            stage_id: lease.stage_id.clone(),
            bundle_root: bundle_root.display().to_string(),
            executor: spec.executor.clone(),
            slurm_job_id: None,
            extra_files,
            started_at_ms,
            finished_at_ms: now_ms(),
            status: JobStatus::Failed,
            commands: vec![],
            error: Some(format!("failed to write slurm script: {e}")),
        };
    }

    // Submit.
    let job_id = match sbatch_submit(&script_path, &slurm_out, &slurm_err, slurm).await {
        Ok(id) => id,
        Err(e) => {
            return ExecBlockResult {
                run_id: lease.run_id.clone(),
                stage_id: lease.stage_id.clone(),
                bundle_root: bundle_root.display().to_string(),
                executor: spec.executor.clone(),
                slurm_job_id: None,
                extra_files,
                started_at_ms,
                finished_at_ms: now_ms(),
                status: JobStatus::Failed,
                commands: vec![],
                error: Some(format!("sbatch failed: {e}")),
            };
        }
    };

    // Poll until completion.
    if let Err(e) = poll_squeue_until_done(&job_id, slurm.poll_ms).await {
        return ExecBlockResult {
            run_id: lease.run_id.clone(),
            stage_id: lease.stage_id.clone(),
            bundle_root: bundle_root.display().to_string(),
            executor: spec.executor.clone(),
            slurm_job_id: Some(job_id),
            extra_files,
            started_at_ms,
            finished_at_ms: now_ms(),
            status: JobStatus::Failed,
            commands: vec![],
            error: Some(format!("failed waiting for slurm job: {e}")),
        };
    }

    // Build results from per-command marker files.
    let mut commands = Vec::with_capacity(spec.commands.len());
    let mut overall_ok = true;
    let mut overall_error = None;

    // Find which command failed (if any).
    let failed_idx = tokio::fs::read_to_string(&failed_idx_path)
        .await
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok());

    for (index, cmd) in spec.commands.iter().enumerate() {
        // Command marker files are written by the batch script.
        let started_path = bundle_root.join(format!("cmd-{index:03}.started"));
        let finished_path = bundle_root.join(format!("cmd-{index:03}.finished"));
        let exit_path = bundle_root.join(format!("cmd-{index:03}.exit"));

        let stdout_name = format!("cmd-{index:03}.stdout.log");
        let stderr_name = format!("cmd-{index:03}.stderr.log");

        let started_at_ms = read_rfc3339_ms(&started_path).await.unwrap_or(started_at_ms);
        let finished_at_ms = read_rfc3339_ms(&finished_path).await.unwrap_or(now_ms());
        let exit_code = tokio::fs::read_to_string(&exit_path)
            .await
            .ok()
            .and_then(|s| s.trim().parse::<i32>().ok());

        let cmd_ok = exit_code.unwrap_or(1) == 0;
        if !cmd_ok {
            overall_ok = false;
            overall_error.get_or_insert_with(|| {
                format!("command {index} failed (exit={exit_code:?})")
            });
        }

        commands.push(CommandResult {
            index,
            program: cmd.program.clone(),
            args: cmd.args.clone(),
            cwd: cmd.cwd.clone(),
            started_at_ms,
            finished_at_ms,
            exit_code,
            status: if cmd_ok {
                JobStatus::Succeeded
            } else {
                JobStatus::Failed
            },
            stdout_path: stdout_name,
            stderr_path: stderr_name,
            error: None,
        });

        // If we stop on error, remaining commands may have no markers.
        if spec.halt_on_error {
            if let Some(fi) = failed_idx {
                if index >= fi {
                    break;
                }
            }
        }
    }

    // Interpret completion markers.
    if failed_marker.exists() {
        overall_ok = false;
        overall_error.get_or_insert_with(|| {
            format!("slurm job reported failure at cmd index {:?}", failed_idx)
        });
    }
    if !done_marker.exists() && overall_ok {
        overall_ok = false;
        overall_error.get_or_insert_with(|| "slurm job finished but done marker missing".into());
    }

    let finished_at_ms = now_ms();

    let result = ExecBlockResult {
        run_id: lease.run_id.clone(),
        stage_id: lease.stage_id.clone(),
        bundle_root: bundle_root.display().to_string(),
        executor: spec.executor.clone(),
        slurm_job_id: Some(job_id),
        extra_files,
        started_at_ms,
        finished_at_ms,
        status: if overall_ok {
            JobStatus::Succeeded
        } else {
            JobStatus::Failed
        },
        commands,
        error: overall_error,
    };

    // Write manifest.json.
    let manifest_path = bundle_root.join("manifest.json");
    if let Ok(json) = serde_json::to_vec_pretty(&result) {
        if let Ok(mut f) = tokio::fs::File::create(manifest_path).await {
            let _ = f.write_all(&json).await;
            let _ = f.flush().await;
        }
    }

    result
}

async fn write_batch_script(
    script_path: &Path,
    workdir: &Path,
    bundle_root: &Path,
    spec: &ExecBlockSpec,
    done_marker: &Path,
    failed_marker: &Path,
    failed_idx_path: &Path,
) -> anyhow::Result<()> {
    // We generate a POSIX-ish bash script. Slurm typically runs it via /bin/sh
    // unless configured otherwise; using env bash makes behavior predictable on
    // most clusters.
    let mut script = String::new();
    script.push_str("#!/usr/bin/env bash\n");
    script.push_str("set -u\n");
    script.push_str(&format!("cd {}\n", shell_quote(workdir.to_string_lossy().as_ref())));
    script.push_str("umask 077\n");
    script.push_str(&format!("BUNDLE={}\n", shell_quote(bundle_root.to_string_lossy().as_ref())));
    script.push_str(&format!("DONE={}\n", shell_quote(done_marker.to_string_lossy().as_ref())));
    script.push_str(&format!("FAILED={}\n", shell_quote(failed_marker.to_string_lossy().as_ref())));
    script.push_str(&format!("FAILED_IDX={}\n", shell_quote(failed_idx_path.to_string_lossy().as_ref())));
    script.push_str("mkdir -p \"$BUNDLE\"\n");
    script.push_str("rm -f \"$DONE\" \"$FAILED\" \"$FAILED_IDX\"\n");
    script.push_str("\n");
    script.push_str("run_cmd() {\n");
    script.push_str("  local IDX=\"$1\"; shift\n");
    script.push_str("  local STDOUT=\"$BUNDLE/cmd-${IDX}.stdout.log\"\n");
    script.push_str("  local STDERR=\"$BUNDLE/cmd-${IDX}.stderr.log\"\n");
    script.push_str("  local STARTED=\"$BUNDLE/cmd-${IDX}.started\"\n");
    script.push_str("  local FINISHED=\"$BUNDLE/cmd-${IDX}.finished\"\n");
    script.push_str("  local EXITF=\"$BUNDLE/cmd-${IDX}.exit\"\n");
    script.push_str("  date -Iseconds > \"$STARTED\"\n");
    script.push_str("  (\"$@\") > \"$STDOUT\" 2> \"$STDERR\"\n");
    script.push_str("  local EC=$?\n");
    script.push_str("  echo $EC > \"$EXITF\"\n");
    script.push_str("  date -Iseconds > \"$FINISHED\"\n");
    script.push_str("  return $EC\n");
    script.push_str("}\n\n");

    // Execute commands sequentially.
    for (index, cmd) in spec.commands.iter().enumerate() {
        let idx = format!("{index:03}");

        // Support per-command cwd relative to workdir.
        if let Some(rel) = &cmd.cwd {
            script.push_str(&format!(
                "pushd {} >/dev/null\n",
                shell_quote(rel.as_str())
            ));
        }

        let mut argv = Vec::with_capacity(1 + cmd.args.len());
        argv.push(cmd.program.clone());
        argv.extend(cmd.args.clone());

        script.push_str(&format!("run_cmd {idx} "));
        for a in argv {
            script.push_str(&shell_quote(&a));
            script.push(' ');
        }
        script.push_str("\n");
        script.push_str("EC=$?\n");
        script.push_str("if [ $EC -ne 0 ]; then\n");
        script.push_str(&format!("  echo {index} > \"$FAILED_IDX\"\n"));
        script.push_str("  touch \"$FAILED\"\n");
        if spec.halt_on_error {
            script.push_str("  exit $EC\n");
        }
        script.push_str("fi\n");

        if cmd.cwd.is_some() {
            script.push_str("popd >/dev/null\n");
        }
        script.push_str("\n");
    }

    script.push_str("touch \"$DONE\"\n");
    script.push_str("exit 0\n");

    tokio::fs::write(script_path, script).await?;

    // Make executable.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(script_path).await?.permissions();
        perms.set_mode(0o700);
        tokio::fs::set_permissions(script_path, perms).await?;
    }

    Ok(())
}

async fn sbatch_submit(
    script_path: &Path,
    stdout_path: &Path,
    stderr_path: &Path,
    slurm: &SlurmSpec,
) -> anyhow::Result<String> {
    let mut cmd = tokio::process::Command::new("sbatch");
    cmd.arg("--parsable")
        .arg("--output")
        .arg(stdout_path)
        .arg("--error")
        .arg(stderr_path);

    if let Some(p) = &slurm.partition {
        cmd.arg("--partition").arg(p);
    }
    if let Some(t) = &slurm.time_limit {
        cmd.arg("--time").arg(t);
    }
    if let Some(c) = slurm.cpus_per_task {
        cmd.arg("--cpus-per-task").arg(c.to_string());
    }
    if let Some(m) = slurm.mem_mb {
        cmd.arg("--mem").arg(m.to_string());
    }
    for a in &slurm.extra_args {
        cmd.arg(a);
    }

    cmd.arg(script_path);

    let out = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output().await?;
    if !out.status.success() {
        anyhow::bail!(
            "sbatch failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    // --parsable prints jobid[;cluster]. We only care about the first segment.
    let s = String::from_utf8_lossy(&out.stdout);
    let job_id = s.trim().split(';').next().unwrap_or("").to_string();
    if job_id.is_empty() {
        anyhow::bail!("sbatch returned empty job id")
    }
    Ok(job_id)
}

async fn poll_squeue_until_done(job_id: &str, poll_ms: u64) -> anyhow::Result<()> {
    loop {
        let out = tokio::process::Command::new("squeue")
            .args(["-h", "-j", job_id])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;
        if !out.status.success() {
            // Some clusters restrict squeue; treat failure as "unknown" and
            // attempt a short wait.
            tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;
            continue;
        }
        let body = String::from_utf8_lossy(&out.stdout);
        if body.trim().is_empty() {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;
    }
}

async fn read_rfc3339_ms(path: &Path) -> Option<i64> {
    let s = tokio::fs::read_to_string(path).await.ok()?;
    // RFC3339 parse without extra dependency: rely on the agent clock, using
    // the file's mtime as a fallback.
    //
    // If you want exact parsing, add `time` or `chrono` and parse properly.
    let _ = s;
    let meta = tokio::fs::metadata(path).await.ok()?;
    let mtime = meta.modified().ok()?;
    let dur = mtime.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(dur.as_millis() as i64)
}

fn shell_quote(s: &str) -> String {
    // Conservative single-quote quoting for bash.
    // e.g. abc'd -> 'abc'"'"'d'
    let mut out = String::new();
    out.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\"'\"'");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}
