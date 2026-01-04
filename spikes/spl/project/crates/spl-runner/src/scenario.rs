use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use spl_core::{GateName, GateStatus, TaskStatus, VcsType};

#[derive(Debug, Deserialize)]
pub struct ScenarioExpected {
    pub scenario_id: String,
    pub vcs_modes: Vec<String>,
    pub task: ScenarioExpectedTask,
    pub messages: ScenarioExpectedMessages,
}

#[derive(Debug, Deserialize)]
pub struct ScenarioExpectedTask {
    pub expect_final_status: String,
}

#[derive(Debug, Deserialize)]
pub struct ScenarioExpectedMessages {
    pub ask_required: bool,
}

#[derive(Debug)]
pub struct ScenarioResult {
    pub final_status: TaskStatus,
    pub ask_emitted: bool,
    pub executed_gates: Vec<(GateName, GateStatus)>,
    /// Number of attempts simulated (used for crash-recovery scenarios).
    pub attempts: u32,
    /// Number of crashed attempts simulated (used for crash-recovery scenarios).
    pub crashed_attempts: u32,
    /// Gate at which the first attempt crashed, if any.
    pub crash_at_gate: Option<GateName>,
}

pub fn load_expected(dir: &Path) -> Result<ScenarioExpected> {
    let p = dir.join("expected.yaml");
    let s = std::fs::read_to_string(&p).with_context(|| format!("read expected.yaml: {}", p.display()))?;
    let exp: ScenarioExpected = serde_yaml::from_str(&s).with_context(|| "parse expected.yaml")?;
    Ok(exp)
}

/// Minimal fixture-mode scenario simulation:
/// - loads spec pack required gate list (string names)
/// - reads gate fixture files under gates/
/// - stops on first FAIL
/// - if all execute gates pass, runs post_smoke (land) if required
///
/// This does NOT run VCS operations. VCS parity is enforced by adapter contract tests.
/// These scenario tests are about SPL semantics and fixture ingestion.
pub fn simulate(dir: &Path, _vcs: VcsType) -> Result<ScenarioResult> {
    let spec_path = dir.join("spec_pack.yaml");
    let pack = spl_spec::load_spec_pack(&spec_path)?;
    let draft = spl_spec::compile_revision_draft(&pack);

    // Gate list comes from spec pack required list when present; else profile default.
    // Use canonical spec names for the fallback (not Debug formatting).
    let required = pack
        .gates
        .as_ref()
        .map(|g| g.required.clone())
        .unwrap_or_else(|| draft.required_gates.iter().map(gate_to_spec_name).collect());

    // Split into execute lane gates and land lane gates.
    // Land lane may include `land` and/or `post_smoke`.
    let mut execute_gates: Vec<GateName> = Vec::new();
    let mut land_gates: Vec<GateName> = Vec::new();
    for name in required {
        let g = parse_gate_name(&name)?;
        match g {
            GateName::Land | GateName::PostSmoke => land_gates.push(g),
            _ => execute_gates.push(g),
        }
    }

    // Optional crash recovery simulation: if `crash_once.txt` exists, the first attempt
    // will "crash" at the specified gate name, and the second attempt will run normally.
    let crash_at_gate = read_crash_once(dir)?;
    let attempts: u32 = if crash_at_gate.is_some() { 2 } else { 1 };
    let crashed_attempts: u32 = if crash_at_gate.is_some() { 1 } else { 0 };

    let mut executed = Vec::new();

    // Simulate the final attempt only for gate execution (attempt 1 crash is tracked separately).
    // This keeps fixture assertions stable while still letting us test crash recovery.
    for g in execute_gates {
        let st = read_gate_status(dir, &g)?;
        executed.push((g.clone(), st.clone()));
        if st == GateStatus::Fail {
            return Ok(ScenarioResult {
                final_status: TaskStatus::BlockedFailure,
                ask_emitted: false,
                executed_gates: executed,
                attempts,
                crashed_attempts,
                crash_at_gate,
            });
        }
    }

    // Land lane simulation.
    // If `land.json` indicates a conflict, treat it as blocked_hitl + ASK.
    for g in land_gates {
        if g == GateName::Land {
            if let Some(outcome) = read_land_outcome(dir)? {
                match outcome.as_str() {
                    "OK" => {
                        executed.push((GateName::Land, GateStatus::Pass));
                    }
                    "CONFLICT" => {
                        executed.push((GateName::Land, GateStatus::Fail));
                        return Ok(ScenarioResult {
                            final_status: TaskStatus::BlockedHitl,
                            ask_emitted: true,
                            executed_gates: executed,
                            attempts,
                            crashed_attempts,
                            crash_at_gate,
                        });
                    }
                    other => {
                        return Err(anyhow!("unknown land outcome: {other}"));
                    }
                }
                continue;
            }
        }

        // Default handling for post_smoke and other gates.
        let st = read_gate_status(dir, &g)?;
        executed.push((g.clone(), st.clone()));
        if st == GateStatus::Fail {
            return Ok(ScenarioResult {
                final_status: TaskStatus::BlockedFailure,
                ask_emitted: false,
                executed_gates: executed,
                attempts,
                crashed_attempts,
                crash_at_gate,
            });
        }
    }

    Ok(ScenarioResult {
        final_status: TaskStatus::Done,
        ask_emitted: false,
        executed_gates: executed,
        attempts,
        crashed_attempts,
        crash_at_gate,
    })
}

fn gate_to_spec_name(g: &GateName) -> String {
    match g {
        GateName::SpecCompile => "spec_compile",
        GateName::CtxPack => "ctx_pack",
        GateName::PreSmoke => "pre_smoke",
        GateName::Delegate => "delegate",
        GateName::Audit => "audit",
        GateName::AdversarialReview => "adversarial_review",
        GateName::Validate => "validate",
        GateName::PostSmoke => "post_smoke",
        GateName::Land => "land",
    }
    .to_string()
}

fn parse_gate_name(s: &str) -> Result<GateName> {
    let n = s.trim();
    match n {
        "spec_compile" => Ok(GateName::SpecCompile),
        "ctx_pack" => Ok(GateName::CtxPack),
        "pre_smoke" => Ok(GateName::PreSmoke),
        "delegate" => Ok(GateName::Delegate),
        "audit" => Ok(GateName::Audit),
        "adversarial_review" => Ok(GateName::AdversarialReview),
        "validate" => Ok(GateName::Validate),
        "post_smoke" => Ok(GateName::PostSmoke),
        "land" => Ok(GateName::Land),
        _ => Err(anyhow!("unknown gate name in spec_pack: {n}")),
    }
}

fn read_gate_status(dir: &Path, gate: &GateName) -> Result<GateStatus> {
    let gates_dir = dir.join("gates");
    match gate {
        GateName::PreSmoke => parse_txt_status(&gates_dir.join("pre_smoke.txt")),
        GateName::PostSmoke => parse_txt_status(&gates_dir.join("post_smoke.txt")),
        GateName::Audit => parse_json_result(&gates_dir.join("audit.json")),
        GateName::AdversarialReview => parse_json_result(&gates_dir.join("review.json")),
        GateName::Validate => parse_json_result(&gates_dir.join("validate.json")),
        // For now, fixture scenarios don't simulate these; treat as PASS.
        GateName::SpecCompile | GateName::CtxPack | GateName::Delegate | GateName::Land => Ok(GateStatus::Pass),
    }
}

fn parse_txt_status(path: &PathBuf) -> Result<GateStatus> {
    let s = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    if s.to_uppercase().contains("FAIL") {
        Ok(GateStatus::Fail)
    } else {
        Ok(GateStatus::Pass)
    }
}

#[derive(Deserialize)]
struct ResultJson {
    result: String,
}

fn parse_json_result(path: &PathBuf) -> Result<GateStatus> {
    let s = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let r: ResultJson = serde_json::from_str(&s).with_context(|| format!("parse {}", path.display()))?;
    if r.result.to_uppercase() == "FAIL" {
        Ok(GateStatus::Fail)
    } else {
        Ok(GateStatus::Pass)
    }
}

#[derive(Deserialize)]
struct LandJson {
    result: String,
}

fn read_land_outcome(dir: &Path) -> Result<Option<String>> {
    let p = dir.join("gates").join("land.json");
    if !p.exists() {
        return Ok(None);
    }
    let s = std::fs::read_to_string(&p).with_context(|| format!("read {}", p.display()))?;
    let v: LandJson = serde_json::from_str(&s).with_context(|| format!("parse {}", p.display()))?;
    Ok(Some(v.result.trim().to_uppercase()))
}

fn read_crash_once(dir: &Path) -> Result<Option<GateName>> {
    let p = dir.join("crash_once.txt");
    if !p.exists() {
        return Ok(None);
    }
    let s = std::fs::read_to_string(&p).with_context(|| format!("read {}", p.display()))?;
    let gate = parse_gate_name(s.trim())?;
    Ok(Some(gate))
}
