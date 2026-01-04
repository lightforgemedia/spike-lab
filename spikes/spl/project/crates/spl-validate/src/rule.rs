use crate::types::{Finding, ValidateInput, ValidationCategory};

pub trait Rule: Send + Sync {
    fn id(&self) -> &str;
    fn category(&self) -> ValidationCategory;
    fn eval(&self, input: &ValidateInput) -> Vec<Finding>;
}

/// Example rule (v0): blocks if exported signature changed.
pub struct MeaningChangeExportedRule;

impl Rule for MeaningChangeExportedRule {
    fn id(&self) -> &str {
        "meaning_change_exported"
    }

    fn category(&self) -> ValidationCategory {
        ValidationCategory::MeaningChange
    }

    fn eval(&self, input: &ValidateInput) -> Vec<Finding> {
        if input.exported_signature_changed {
            return vec![Finding {
                rule_id: self.id().to_string(),
                category: self.category(),
                severity: crate::types::Severity::Fail,
                message: "exported signature changed; update spec or DECISION".to_string(),
                evidence_ref: None,
            }];
        }
        vec![]
    }
}
