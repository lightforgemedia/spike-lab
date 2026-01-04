pub mod config;
pub mod doctor;
pub mod runner;
pub mod util;
pub mod scenario;

pub use config::*;
pub use doctor::*;
pub use runner::*;
pub use util::*;


#[cfg(test)]
mod fixture_tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn loads_and_compiles_sc01_spec_pack() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scenarios/SC-01-happy-path/spec_pack.yaml");
        let pack = spl_spec::load_spec_pack(&path).unwrap();
        let draft = spl_spec::compile_revision_draft(&pack);
        assert_eq!(draft.profile, "standard");
        assert!(!draft.spec_hash.is_empty());
    }
}


#[cfg(test)]
mod scenario_tests {
    use super::scenario::*;
    use spl_core::VcsType;
    use std::path::Path;

    fn run(dir: &str) -> ScenarioResult {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios").join(dir);
        // simulate under both vcs types; semantic harness should be identical
        let _ = simulate(&p, VcsType::Git).unwrap();
        simulate(&p, VcsType::Jj).unwrap()
    }

    #[test]
    fn scenario_sc01_happy_path_done() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/scenarios/SC-01-happy-path");
        let exp = load_expected(&p).unwrap();
        for mode in &exp.vcs_modes {
            let vcs = if mode == "jj" { VcsType::Jj } else { VcsType::Git };
            let res = simulate(&p, vcs).unwrap();
            assert_eq!(exp.task.expect_final_status, "done");
            assert_eq!(res.final_status, spl_core::TaskStatus::Done);
            assert_eq!(exp.messages.ask_required, false);
        }
    }

    #[test]
    fn scenario_sc03_pre_smoke_fail_blocks() {
        let res = run("SC-03-pre-smoke-fail");
        assert_eq!(res.final_status, spl_core::TaskStatus::BlockedFailure);
        // should fail at pre_smoke
        assert!(res.executed_gates.iter().any(|(g,s)| *g==spl_core::GateName::PreSmoke && *s==spl_core::GateStatus::Fail));
    }

    #[test]
    fn scenario_sc05_audit_violation_blocks() {
        let res = run("SC-05-audit-violation");
        assert_eq!(res.final_status, spl_core::TaskStatus::BlockedFailure);
        // pre_smoke passes and audit fails
        let mut seen_pre=false;
        let mut seen_audit_fail=false;
        for (g,s) in res.executed_gates {
            if g==spl_core::GateName::PreSmoke && s==spl_core::GateStatus::Pass { seen_pre=true; }
            if g==spl_core::GateName::Audit && s==spl_core::GateStatus::Fail { seen_audit_fail=true; }
        }
        assert!(seen_pre);
        assert!(seen_audit_fail);
    }

    #[test]
    fn scenario_sc09_landing_conflict_blocks_hitl_and_emits_ask() {
        let res = run("SC-09-landing-conflict");
        assert_eq!(res.final_status, spl_core::TaskStatus::BlockedHitl);
        assert!(res.ask_emitted);
        assert!(res.executed_gates.iter().any(|(g, s)| {
            *g == spl_core::GateName::Land && *s == spl_core::GateStatus::Fail
        }));
    }

    #[test]
    fn scenario_sc11_crash_recovery_retries_and_completes() {
        let res = run("SC-11-crash-recovery");
        assert_eq!(res.final_status, spl_core::TaskStatus::Done);
        assert_eq!(res.attempts, 2);
        assert_eq!(res.crashed_attempts, 1);
        assert_eq!(res.crash_at_gate, Some(spl_core::GateName::Delegate));
    }
}
