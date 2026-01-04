use crate::vcs;
use orchestrator_core::{now_ms, CommandResult, ExecAttemptRow, JobClaim, JobStatus};
use std::path::Path;
use std::process::Stdio;

use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// Executes the job and returns:
/// - Ok((attempt_row, output_revision)) on success
/// - Err((attempt_row, output_revision)) on failure
pub async fn run_job(
    args: &crate::Args,
    job: &JobClaim,
) -> Result<(ExecAttemptRow, Option<String>), (ExecAttemptRow, Option<String>)> {
    let exec = match &job.exec {
        None => {
            let attempt = ExecAttemptRow {
                id: ulid::Ulid::new().to_string(),
                job_id: job.job_id.clone(),
                stage_id: job.stage_id.clone(),
                status: JobStatus::Failed,
                artifact_dir: "".into(),
                artifact_bundle: None,
                started_at_ms: now_ms(),
                finished_at_ms: now_ms(),
                commands: vec![],
            };
            return Err((attempt, None));
        }
        Some(e) => e.clone(),
    };

    // Prepare workspace
    let workspace = match vcs::prepare_workspace(
        Path::new(&job.project_path),
        &args.state_dir,
        &job.run_id,
        &job.stage_id,
        &job.job_id,
        &job.input_revisions,
    )
    .await
    {
        Ok(ws) => ws,
        Err(_e) => {
            let attempt = ExecAttemptRow {
                id: ulid::Ulid::new().to_string(),
                job_id: job.job_id.clone(),
                stage_id: job.stage_id.clone(),
                status: JobStatus::Failed,
                artifact_dir: "".into(),
                artifact_bundle: None,
                started_at_ms: now_ms(),
                finished_at_ms: now_ms(),
                commands: vec![],
            };
            return Err((attempt, None));
        }
    };

    // Resolve workdir relative to workspace root
    let workdir = exec.workdir.clone().unwrap_or_else(|| ".".into());
    let workdir_path = workspace.root.join(&workdir);

    // Containment check (basic): workdir must not escape workspace root
    if !is_within(&workspace.root, &workdir_path).await {
        let attempt = ExecAttemptRow {
            id: ulid::Ulid::new().to_string(),
            job_id: job.job_id.clone(),
            stage_id: job.stage_id.clone(),
            status: JobStatus::Failed,
            artifact_dir: "".into(),
            artifact_bundle: None,
            started_at_ms: now_ms(),
            finished_at_ms: now_ms(),
            commands: vec![],
        };
        return Err((attempt, vcs::current_revision(&workspace.root).await));
    }

    // Prepare artifacts dir
    let artifact_dir = args
        .state_dir
        .join("artifacts")
        .join(&job.run_id)
        .join(&job.stage_id)
        .join(&job.job_id);
    let _ = tokio::fs::create_dir_all(&artifact_dir).await;

    let started = now_ms();
    let mut results: Vec<CommandResult> = Vec::new();
    let mut succeeded = true;

    for (i, cmd) in exec.commands.iter().enumerate() {
        let name = cmd.name.clone().unwrap_or_else(|| format!("cmd{i}"));
        let cmd_dir = artifact_dir.join(format!("{i:02}-{name}"));
        let _ = tokio::fs::create_dir_all(&cmd_dir).await;

        let stdout_path = cmd_dir.join("stdout.log");
        let stderr_path = cmd_dir.join("stderr.log");

        let c_started = now_ms();
        let mut command = tokio::process::Command::new(&cmd.argv[0]);
        command.args(&cmd.argv[1..]);
        command.current_dir(&workdir_path);
        command.envs(exec.env.iter().map(|(k, v)| (k, v)));
        command.envs(cmd.env.iter().map(|(k, v)| (k, v)));
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => {
                succeeded = false;
                let c_finished = now_ms();
                let _ = tokio::fs::write(&stderr_path, format!("spawn failed: {e:?}\n")).await;
                results.push(CommandResult {
                    index: i,
                    name,
                    argv: cmd.argv.clone(),
                    exit_code: 127,
                    started_at_ms: c_started,
                    finished_at_ms: c_finished,
                    stdout_path: stdout_path.to_string_lossy().to_string(),
                    stderr_path: stderr_path.to_string_lossy().to_string(),
                });
                break;
            }
        };

        let output = match child.wait_with_output().await {
            Ok(o) => o,
            Err(e) => {
                succeeded = false;
                let c_finished = now_ms();
                let _ = tokio::fs::write(&stderr_path, format!("wait failed: {e:?}\n")).await;
                results.push(CommandResult {
                    index: i,
                    name,
                    argv: cmd.argv.clone(),
                    exit_code: 1,
                    started_at_ms: c_started,
                    finished_at_ms: c_finished,
                    stdout_path: stdout_path.to_string_lossy().to_string(),
                    stderr_path: stderr_path.to_string_lossy().to_string(),
                });
                break;
            }
        };

        let c_finished = now_ms();
        let _ = tokio::fs::write(&stdout_path, &output.stdout).await;
        let _ = tokio::fs::write(&stderr_path, &output.stderr).await;

        let code = output.status.code().unwrap_or(1);
        if code != 0 && !cmd.allow_failure {
            succeeded = false;
        }

        results.push(CommandResult {
            index: i,
            name,
            argv: cmd.argv.clone(),
            exit_code: code,
            started_at_ms: c_started,
            finished_at_ms: c_finished,
            stdout_path: stdout_path.to_string_lossy().to_string(),
            stderr_path: stderr_path.to_string_lossy().to_string(),
        });

        if !succeeded {
            break;
        }
    }

    let finished = now_ms();
    let status = if succeeded {
        JobStatus::Succeeded
    } else {
        JobStatus::Failed
    };

    // Create manifest.json
    let manifest = serde_json::json!({
        "job_id": job.job_id,
        "stage_id": job.stage_id,
        "run_id": job.run_id,
        "started_at_ms": started,
        "finished_at_ms": finished,
        "succeeded": succeeded,
        "commands": results.clone(),
    });
    let _ = tokio::fs::write(
        artifact_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap_or_default(),
    )
    .await;

    // Upload artifact bundle (best-effort)
    let bundle_zip = artifact_dir.join("bundle.zip");
    let artifact_bundle_uri = match zip_dir(&artifact_dir, &bundle_zip) {
        Ok(()) => upload_bundle(args, job, &bundle_zip).await.ok(),
        Err(_) => None,
    };

    let attempt_row = ExecAttemptRow {
        id: ulid::Ulid::new().to_string(),
        job_id: job.job_id.clone(),
        stage_id: job.stage_id.clone(),
        status,
        artifact_dir: artifact_dir.to_string_lossy().to_string(),
        artifact_bundle: artifact_bundle_uri,
        started_at_ms: started,
        finished_at_ms: finished,
        commands: results,
    };

    let out_rev = vcs::current_revision(&workspace.root).await;

    if succeeded {
        Ok((attempt_row, out_rev))
    } else {
        Err((attempt_row, out_rev))
    }
}

async fn upload_bundle(
    args: &crate::Args,
    job: &JobClaim,
    zip_path: &Path,
) -> anyhow::Result<String> {
    let bytes = tokio::fs::read(zip_path).await?;
    let url = format!(
        "{}/v1/jobs/{}/artifacts",
        args.daemon.trim_end_matches('/'),
        job.job_id
    );
    let client = reqwest::Client::new();
    let resp: orchestrator_core::ArtifactUploadResponse = client
        .post(url)
        .header("content-type", "application/zip")
        .body(bytes)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(resp.artifact_uri)
}

fn zip_dir(src_dir: &Path, dest_zip: &Path) -> anyhow::Result<()> {
    let file = std::fs::File::create(dest_zip)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    for entry in WalkDir::new(src_dir).follow_links(false) {
        let entry = entry?;
        let path = entry.path();
        if path == dest_zip {
            continue;
        }
        if path.is_dir() {
            continue;
        }
        let rel = path.strip_prefix(src_dir).unwrap_or(path);
        let name = rel.to_string_lossy().replace("\\", "/");

        zip.start_file(name, options)?;
        let mut f = std::fs::File::open(path)?;
        std::io::copy(&mut f, &mut zip)?;
    }

    let _ = zip.finish()?;
    Ok(())
}

async fn is_within(root: &Path, path: &Path) -> bool {
    let root = tokio::fs::canonicalize(root)
        .await
        .unwrap_or_else(|_| root.to_path_buf());
    let path = tokio::fs::canonicalize(path)
        .await
        .unwrap_or_else(|_| path.to_path_buf());
    path.starts_with(root)
}
