use orchestrator_core::model::{CommandSpec, ExecBlockSpec};
use orchestrator_core::validation::{validate_exec_block, Decision};

#[test]
fn blocks_shell_by_default() {
    let spec = ExecBlockSpec {
        workdir: "/tmp".into(),
        executor: Default::default(),
        allow_shell: false,
        halt_on_error: true,
        env: Default::default(),
        commands: vec![CommandSpec {
            program: "bash".into(),
            args: vec!["-c".into(), "echo hi".into()],
            cwd: None,
            env: Default::default(),
            timeout_sec: None,
        }],
    };

    let out = validate_exec_block(&spec);
    assert_eq!(out.decision, Decision::Block);
}

#[test]
fn warns_on_audited_commands() {
    let commands = vec![
        ("mv", vec!["a", "b"]),
        ("cp", vec!["a", "b"]),
        ("ln", vec!["-s", "a", "b"]),
    ];

    for (program, args) in commands {
        let spec = ExecBlockSpec {
            workdir: "/tmp".into(),
            executor: Default::default(),
            allow_shell: false,
            halt_on_error: true,
            env: Default::default(),
            commands: vec![CommandSpec {
                program: program.into(),
                args: args.into_iter().map(String::from).collect(),
                cwd: None,
                env: Default::default(),
                timeout_sec: None,
            }],
        };

        let out = validate_exec_block(&spec);
        assert_eq!(out.decision, Decision::Warn, "command '{}' should be warned", program);
    }
}

#[test]
fn allows_shell_with_opt_in() {
    let spec = ExecBlockSpec {
        workdir: "/tmp".into(),
        executor: Default::default(),
        allow_shell: true,
        halt_on_error: true,
        env: Default::default(),
        commands: vec![CommandSpec {
            program: "bash".into(),
            args: vec!["-c".into(), "echo hi".into()],
            cwd: None,
            env: Default::default(),
            timeout_sec: None,
        }],
    };

    let out = validate_exec_block(&spec);
    assert_ne!(out.decision, Decision::Block);
}

#[test]
fn blocks_rm_absolute_path() {
    let spec = ExecBlockSpec {
        workdir: "/tmp".into(),
        executor: Default::default(),
        allow_shell: false,
        halt_on_error: true,
        env: Default::default(),
        commands: vec![CommandSpec {
            program: "rm".into(),
            args: vec!["-rf".into(), "/".into()],
            cwd: None,
            env: Default::default(),
            timeout_sec: None,
        }],
    };

    let out = validate_exec_block(&spec);
    assert_eq!(out.decision, Decision::Block);
}

#[test]
fn allows_rm_relative_path_with_warning() {
    let spec = ExecBlockSpec {
        workdir: "/tmp".into(),
        executor: Default::default(),
        allow_shell: false,
        halt_on_error: true,
        env: Default::default(),
        commands: vec![CommandSpec {
            program: "rm".into(),
            args: vec!["-rf".into(), "target".into()],
            cwd: None,
            env: Default::default(),
            timeout_sec: None,
        }],
    };

    let out = validate_exec_block(&spec);
    assert!(matches!(out.decision, Decision::Warn | Decision::Allow));
}

#[test]
fn blocks_empty_commands_list() {
    let spec = ExecBlockSpec {
        workdir: "/tmp".into(),
        executor: Default::default(),
        allow_shell: false,
        halt_on_error: true,
        env: Default::default(),
        commands: vec![],
    };

    let out = validate_exec_block(&spec);
    assert_eq!(out.decision, Decision::Block);
}

