use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use spl_core::VcsType;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub project: ProjectConfig,
    pub vcs: VcsConfig,
    pub workspace: WorkspaceConfig,
    pub commands: CommandsConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub id: String,
    pub main_ref: String,
    pub artifact_root: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VcsConfig {
    #[serde(rename = "type")]
    pub vcs_type: String, // "git" | "jj"
    #[serde(default)]
    pub git_main_branch: Option<String>,
    #[serde(default)]
    pub jj_main_bookmark: Option<String>,
    #[serde(default)]
    pub jj_require_colocated: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub root: String,
    #[serde(default)]
    pub cleanup_on_success: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandsConfig {
    pub pre_smoke: Vec<String>,
    pub post_smoke: Vec<String>,
}

impl Config {
    pub fn default_for_repo(project_id: &str) -> Self {
        Self {
            project: ProjectConfig {
                id: project_id.to_string(),
                main_ref: "main".to_string(),
                artifact_root: "~/.spl/artifacts".to_string(),
            },
            vcs: VcsConfig {
                vcs_type: "git".to_string(),
                git_main_branch: Some("main".to_string()),
                jj_main_bookmark: Some("main".to_string()),
                jj_require_colocated: Some(true),
            },
            workspace: WorkspaceConfig {
                root: ".spl/workspaces".to_string(),
                cleanup_on_success: Some(true),
            },
            commands: CommandsConfig {
                pre_smoke: vec!["true".to_string()],
                post_smoke: vec!["true".to_string()],
            },
        }
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        let s = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let cfg: Config = toml::from_str(&s).with_context(|| "parse spl.toml")?;
        Ok(cfg)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let s = toml::to_string_pretty(self).with_context(|| "serialize toml")?;
        std::fs::write(path, s).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn vcs_type(&self) -> VcsType {
        match self.vcs.vcs_type.as_str() {
            "jj" => VcsType::Jj,
            _ => VcsType::Git,
        }
    }

    pub fn workspace_root(&self, repo_root: &Path) -> PathBuf {
        repo_root.join(&self.workspace.root)
    }

    pub fn config_path(repo_root: &Path) -> PathBuf {
        repo_root.join(".spl").join("spl.toml")
    }

    pub fn db_path(repo_root: &Path) -> PathBuf {
        repo_root.join(".spl").join("spl.db")
    }
}
