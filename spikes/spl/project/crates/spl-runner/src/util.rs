use anyhow::{anyhow, Context, Result};
use std::process::Command;

pub fn now_unix() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    dur.as_secs() as i64
}

pub fn run_cmd(dir: &std::path::Path, program: &str, args: &[&str]) -> Result<String> {
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(dir);
    let out = cmd.output().with_context(|| format!("run {} {:?}", program, args))?;
    if !out.status.success() {
        return Err(anyhow!(
            "command failed: {} {:?}\nstdout:{}\nstderr:{}",
            program,
            args,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
