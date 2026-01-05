//! Integration tests for the core crate.

use orchestrator_core::model::{
    CommandSpec, ExecBlockSpec, ExecutorSpec, RunStatus, SlurmSpec, StageConfig, StageDef,
    StageKind, StageStatus,
};
use orchestrator_core::validation::{validate_exec_block, Decision};

#[test]
fn test_stage_kind_serde() {
    let exec_block = StageKind::ExecBlock;
    let serialized = serde_json::to_string(&exec_block).unwrap();
    assert_eq!(serialized, r#""exec_block""#);
    let deserialized: StageKind = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, exec_block);
}

#[test]
fn test_executor_spec_serde() {
    let local = ExecutorSpec::Local;
    let serialized = serde_json::to_string(&local).unwrap();
    assert_eq!(serialized, r#"{"kind":"local"}"#);
    let deserialized: ExecutorSpec = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, local);

    let slurm = ExecutorSpec::Slurm(SlurmSpec {
        partition: Some("debug".into()),
        time_limit: Some("00:10:00".into()),
        cpus_per_task: Some(2),
        mem_mb: Some(1024),
        extra_args: vec!["--exclusive".into()],
        poll_ms: 2000,
    });
    let serialized = serde_json::to_string(&slurm).unwrap();
    let deserialized: ExecutorSpec = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, slurm);
}

#[test]
fn test_run_status_serde() {
    let running = RunStatus::Running;
    let serialized = serde_json::to_string(&running).unwrap();
    assert_eq!(serialized, r#""running""#);
    let deserialized: RunStatus = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, running);
}

#[test]
fn test_stage_status_serde() {
    let pending = StageStatus::Pending;
    let serialized = serde_json::to_string(&pending).unwrap();
    assert_eq!(serialized, r#""pending""#);
    let deserialized: StageStatus = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, pending);
}


#[test]
fn test_stage_def_serde() {
    let stage_def = StageDef {
        stage_id: "test-stage".into(),
        kind: StageKind::ExecBlock,
        config: StageConfig::ExecBlock(ExecBlockSpec {
            workdir: "/tmp".into(),
            executor: Default::default(),
            allow_shell: false,
            halt_on_error: true,
            env: Default::default(),
            commands: vec![],
        }),
    };

    let serialized = serde_json::to_string(&stage_def).unwrap();
    let deserialized: StageDef = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.stage_id, stage_def.stage_id);
}

