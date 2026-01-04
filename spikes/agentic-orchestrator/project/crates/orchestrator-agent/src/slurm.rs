    use anyhow::{anyhow, Context, Result};
    use orchestrator_core::{now_ms, CommandResult, CommandStatus, ExecBlockSpec, JobResultStatus};
    use std::path::{Path, PathBuf};
    use std::time::Duration;
    use tokio::process::Command;
    use tokio::time::sleep;
    use tracing::warn;

    #[derive(serde::Deserialize)]
    struct SlurmCmdMeta {
        index: u32,
        started_ms: i64,
        ended_ms: i64,
        exit_code: i32,
    }

    /// Execute an exec block on Slurm by submitting a generated batch script.
    ///
    /// Returns:
    /// - per-command results
    /// - slurm job id as executor_ref
    /// - overall job status
    pub async fn run_slurm(
        exec: &ExecBlockSpec,
        workspace_root: &Path,
        bundle_root: &Path,
        tmp_dir: &Path,
    ) -> Result<(Vec<CommandResult>, String, JobResultStatus)> {
        if !has_cmd("sbatch").await {
            return Err(anyhow!("sbatch not found on PATH"));
        }

        let slurm_dir = bundle_root.join("slurm");
        std::fs::create_dir_all(&slurm_dir)?;

        let cmd_dir = bundle_root.join("cmd");
        std::fs::create_dir_all(&cmd_dir)?;

        let script_path = slurm_dir.join("batch.sh");
        write_batch_script(exec, &script_path, &cmd_dir, tmp_dir)?;

        // Submit
        let slurm_stdout = slurm_dir.join("slurm.stdout.log");
        let slurm_stderr = slurm_dir.join("slurm.stderr.log");

        let mut sbatch = Command::new("sbatch");
        sbatch
            .arg("--parsable")
            .arg("--chdir")
            .arg(workspace_root)
            .arg("--output")
            .arg(&slurm_stdout)
            .arg("--error")
            .arg(&slurm_stderr);

        if let Some(spec) = &exec.slurm {
            if let Some(part) = &spec.partition {
                sbatch.arg("--partition").arg(part);
            }
            if let Some(t) = &spec.time_limit {
                sbatch.arg("--time").arg(t);
            }
            if let Some(a) = &spec.account {
                sbatch.arg("--account").arg(a);
            }
            if let Some(q) = &spec.qos {
                sbatch.arg("--qos").arg(q);
            }
            if let Some(c) = spec.cpus_per_task {
                sbatch.arg("--cpus-per-task").arg(c.to_string());
            }
            if let Some(m) = &spec.mem {
                sbatch.arg("--mem").arg(m);
            }
            for a in &spec.extra_args {
                sbatch.arg(a);
            }
        }

        sbatch.arg(&script_path);

        let out = sbatch.output().await.context("running sbatch")?;
        if !out.status.success() {
            return Err(anyhow!(
                "sbatch failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }

        let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let job_id = raw.split(';').next().unwrap_or(&raw).to_string();

        // Wait for completion.
        wait_for_job(&job_id).await?;

        // Determine exit code (try sacct; fallback to marker in slurm stdout).
        let overall_rc = determine_exit_code(&job_id, &slurm_stdout).await.unwrap_or(1);

        // Read per-command meta if present; otherwise, best-effort from exit code.
        let mut results = Vec::new();
        let mut overall_ok = overall_rc == 0;

        for (i, cmd) in exec.commands.iter().enumerate() {
            let idx = i as u32;
            let stdout_path = cmd_dir.join(format!("{:03}.stdout.log", idx));
            let stderr_path = cmd_dir.join(format!("{:03}.stderr.log", idx));
            let meta_path = cmd_dir.join(format!("{:03}.slurm-meta.json", idx));

            let (started_ms, ended_ms, exit_code) = match std::fs::read(&meta_path) {
                Ok(bytes) => match serde_json::from_slice::<SlurmCmdMeta>(&bytes) {
                    Ok(m) => (m.started_ms, m.ended_ms, Some(m.exit_code)),
                    Err(_) => (now_ms(), now_ms(), None),
                },
                Err(_) => (now_ms(), now_ms(), None),
            };

            let status = match exit_code {
                Some(0) => CommandStatus::Succeeded,
                Some(_) => CommandStatus::Failed,
                None => CommandStatus::Failed,
            };

            if status != CommandStatus::Succeeded && !cmd.allow_failure {
                overall_ok = false;
            }

            results.push(CommandResult {
                index: idx,
                name: cmd.name.clone(),
                program: cmd.program.clone(),
                args: cmd.args.clone(),
                status,
                exit_code,
                started_ms,
                ended_ms,
                stdout_path: stdout_path.to_string_lossy().to_string(),
                stderr_path: stderr_path.to_string_lossy().to_string(),
            });

            if !overall_ok {
                break;
            }
        }

        let overall_status = if overall_ok {
            JobResultStatus::Succeeded
        } else {
            JobResultStatus::Failed
        };

        Ok((results, job_id, overall_status))
    }

    fn write_batch_script(
    exec: &ExecBlockSpec,
    script_path: &Path,
    cmd_dir: &Path,
    tmp_dir: &Path,
) -> Result<()> {
    // Use epoch millis if possible; fall back to seconds.
    let mut lines = Vec::new();
    lines.push("#!/usr/bin/env bash".to_string());
    lines.push("set -u".to_string());
    lines.push(format!("export ORCH_TMP="{}"", escape(tmp_dir)));
    lines.push("mkdir -p "$ORCH_TMP"".to_string());
    lines.push(format!("mkdir -p "{}"", escape(cmd_dir)));

    // exec-level env
    for (k, v) in &exec.env {
        if is_env_key_safe(k) {
            lines.push(format!("export {}={}", k, shell_escape(v)));
        }
    }

    lines.push("overall_rc=0".to_string());

    for (i, cmd) in exec.commands.iter().enumerate() {
        let idx = i as u32;
        let stdout_path = cmd_dir.join(format!("{:03}.stdout.log", idx));
        let stderr_path = cmd_dir.join(format!("{:03}.stderr.log", idx));
        let meta_path = cmd_dir.join(format!("{:03}.slurm-meta.json", idx));

        // cmd-level env (overrides)
        for (k, v) in &cmd.env {
            if is_env_key_safe(k) {
                lines.push(format!("export {}={}", k, shell_escape(v)));
            }
        }

        let allow_failure = if cmd.allow_failure { "1" } else { "0" };

        lines.push("cmd_start=$(date +%s%3N 2>/dev/null || (date +%s000))".to_string());
        lines.push(format!(
            ""{}" {} >"{}" 2>"{}"",
            escape(&PathBuf::from(&cmd.program)),
            cmd.args
                .iter()
                .map(|a| shell_escape(a))
                .collect::<Vec<_>>()
                .join(" "),
            escape(&stdout_path),
            escape(&stderr_path)
        ));
        lines.push("rc=$?".to_string());
        lines.push("cmd_end=$(date +%s%3N 2>/dev/null || (date +%s000))".to_string());
        lines.push(format!(
            "echo '{{"index":{},"started_ms":'"$cmd_start"',"ended_ms":'"$cmd_end"',"exit_code":'"$rc"'}}' >"{}"",
            idx,
            escape(&meta_path)
        ));
        lines.push(format!(
            "if [ "$rc" -ne 0 ] && [ "{}" -ne 1 ]; then overall_rc=$rc; break; fi",
            allow_failure
        ));
    }

    lines.push("echo "__ORCH_OVERALL_RC=$overall_rc"".to_string());
    lines.push("exit "$overall_rc"".to_string());

    let script = lines.join("\n") + "\n";
    std::fs::write(script_path, script)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(script_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(script_path, perms)?;
    }
    Ok(())
}

fn is_env_key_safe(k: &str) -> bool {
    !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

async fn wait_for_job(job_id: &str) -> Result<()> {
        // Prefer squeue; fall back to naive sleep if absent.
        if !has_cmd("squeue").await {
            sleep(Duration::from_secs(2)).await;
            return Ok(());
        }

        loop {
            let out = Command::new("squeue")
                .args(["-j", job_id, "-h"])
                .output()
                .await
                .context("running squeue")?;
            if !out.status.success() {
                // If squeue fails, don't spin forever.
                warn!("squeue failed: {}", String::from_utf8_lossy(&out.stderr));
                break;
            }
            let text = String::from_utf8_lossy(&out.stdout);
            if text.trim().is_empty() {
                break;
            }
            sleep(Duration::from_secs(2)).await;
        }
        Ok(())
    }

    async fn determine_exit_code(job_id: &str, slurm_stdout: &Path) -> Result<i32> {
        // Use sacct when available for more reliable exit info.
        if has_cmd("sacct").await {
            let out = Command::new("sacct")
                .args([
                    "-j",
                    job_id,
                    "--format=State,ExitCode",
                    "-n",
                    "-P",
                ])
                .output()
                .await
                .context("running sacct")?;
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                // Example: "COMPLETED|0:0"
                for line in text.lines() {
                    let parts: Vec<&str> = line.split('|').collect();
                    if parts.len() >= 2 {
                        if let Some(code) = parts[1].split(':').next() {
                            if let Ok(v) = code.parse::<i32>() {
                                return Ok(v);
                            }
                        }
                    }
                }
            }
        }

        // Fallback: parse marker from slurm stdout.
        if let Ok(text) = std::fs::read_to_string(slurm_stdout) {
            for line in text.lines().rev() {
                if let Some(rest) = line.strip_prefix("__ORCH_OVERALL_RC=") {
                    if let Ok(v) = rest.trim().parse::<i32>() {
                        return Ok(v);
                    }
                }
            }
        }

        Err(anyhow!("unable to determine slurm exit code"))
    }

    async fn has_cmd(name: &str) -> bool {
        Command::new(name).arg("--version").output().await.is_ok()
    }

    fn escape(p: &Path) -> String {
        p.to_string_lossy().replace('"', "\"")
    }

    fn shell_escape(s: &str) -> String {
        // Minimal POSIX shell escaping for args (single quotes).
        if s.is_empty() {
            return "''".to_string();
        }
        if s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:@".contains(c)) {
            return s.to_string();
        }
        format!("'{}'", s.replace(''', "'\''"))
    }
