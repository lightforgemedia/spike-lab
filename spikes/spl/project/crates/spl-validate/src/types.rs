use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ValidationCategory {
    Structural,
    SpecCoverage,
    MeaningChange,
    Policy,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity {
    Warn,
    Fail,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Finding {
    pub rule_id: String,
    pub category: ValidationCategory,
    pub severity: Severity,
    pub message: String,
    pub evidence_ref: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ValidateInput {
    // v0 placeholder: index diffs, spec, etc.
    pub exported_signature_changed: bool,
    pub details: Vec<String>,
}
