use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use spl_runner::Runner;
use spl_storage::Storage;

#[derive(Parser)]
#[command(name = "spl", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize SPL in the current repo (creates .spl/, config, db)
    Init,

    /// Validate tooling and repo safety invariants
    Doctor,

    /// Show a basic status snapshot
    Status,

    /// Create a task (v0: minimal)
    TaskAdd {
        #[arg(long)]
        id: String,
        #[arg(long)]
        title: String,
        #[arg(long, default_value_t = true)]
        ready: bool,
    },

    /// Compile a spec pack YAML into a spec revision + runnable revision row (v0: minimal)
    SpecCompile {
        #[arg(long)]
        task: String,
        #[arg(long)]
        spec: String,
        #[arg(long, default_value = "standard")]
        profile: String,
    },

    /// Enqueue an execute-lane queue item for a task+revision (v0 helper)
    QueueEnqueue {
        #[arg(long)]
        task: String,
        #[arg(long)]
        revision: String,
    },

    /// Run one queue item (default dry-run)
    WorkerRun {
        #[arg(long, default_value_t = true)]
        dry_run: bool,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

    let cli = Cli::parse();
    let repo_root = std::env::current_dir()?;

    match cli.cmd {
        Command::Init => {
            Runner::init_repo(&repo_root)?;
            println!("Initialized SPL in {}", repo_root.display());
        }
        Command::Doctor => {
            let r = Runner::open(repo_root)?;
            r.doctor()?;
            println!("OK");
        }
        Command::Status => {
            let r = Runner::open(repo_root)?;
            let snap = r.storage.load_snapshot(spl_runner::now_unix())?;
            println!("Tasks: {}", snap.tasks.len());
            for t in snap.tasks {
                println!("- {} [{:?}] {}", t.id.as_str(), t.status, t.title);
            }
            println!("Visible queue items: {}", snap.queue.len());
            println!("Active leases: {}", snap.leases.len());
        }
        Command::TaskAdd { id, title, ready } => {
            let r = Runner::open(repo_root)?;
            let status = if ready { spl_core::TaskStatus::Ready } else { spl_core::TaskStatus::Draft };
            r.storage.insert_task(spl_core::Task {
                id: spl_core::TaskId::from_str(id.clone()),
                title,
                status,
                priority: 0,
                tags: vec![],
            })?;
            println!("Added task {}", id);
        }
        Command::SpecCompile { task, spec, profile } => {
            let r = Runner::open(repo_root)?;
            let pack = spl_spec::load_spec_pack(std::path::Path::new(&spec))?;
            let mut pack = pack;
            pack.profile = Some(profile);
            let draft = spl_spec::compile_revision_draft(&pack);

            // copy spec into repo-local area for durability
            let specs_dir = r.repo_root.join(".spl").join("specs").join(&task);
            std::fs::create_dir_all(&specs_dir)?;
            let dest = specs_dir.join("spec_pack.yaml");
            std::fs::copy(&spec, &dest)?;

            // Insert spec_rev + revision rows (v0 minimal; ids derived)
            let now = spl_runner::now_unix();
            let spec_rev_id = spl_core::SpecRevId::new().0;
            let revision_id = spl_core::RevisionId::new().0;

            r.storage.insert_spec_revision(&spec_rev_id, &task, &draft.spec_hash, dest.to_str().unwrap(), now)?;
            r.storage.insert_revision_row(
                &revision_id,
                &task,
                &spec_rev_id,
                &draft.spec_hash,
                &draft.profile,
                &serde_json::to_string(&draft.required_gates)?,
                "[]",
                &serde_json::to_string(&draft.anchors.iter().map(|a| a.as_str()).collect::<Vec<_>>())?,
                now,
            )?;

            println!("Compiled spec for {} -> revision {}", task, revision_id);
        }
        Command::QueueEnqueue { task, revision } => {
            let r = Runner::open(repo_root)?;
            r.enqueue_execute(&task, &revision)?;
            println!("Enqueued execute for {} {}", task, revision);
        }
        Command::WorkerRun { dry_run } => {
            let r = Runner::open(repo_root)?;
            r.run_once(dry_run)?;
            println!("worker run complete");
        }
    }

    Ok(())
}
