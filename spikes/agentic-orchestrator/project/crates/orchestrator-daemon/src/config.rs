use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub default_project_root: PathBuf,
    pub db_dir: PathBuf,
    pub runs_root: PathBuf,
    pub workspaces_root: PathBuf,

    pub lease_seconds: u64,
    pub scheduler_interval_seconds: u64,

    pub gc_enabled: bool,
    pub gc_interval_seconds: u64,
    pub gc_max_run_age_days: u64,
    pub gc_keep_last_n: u64,
}
