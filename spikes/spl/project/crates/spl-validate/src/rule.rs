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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ValidateInput, ValidationCategory};

    #[test]
    fn test_meaning_change_exported_rule_no_change() {
        let rule = MeaningChangeExportedRule;
        let input = ValidateInput {
            exported_signature_changed: false,
            details: vec![],
        };
        let findings = rule.eval(&input);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_meaning_change_exported_rule_with_change() {
        let rule = MeaningChangeExportedRule;
        let input = ValidateInput {
            exported_signature_changed: true,
            details: vec![],
        };
        let findings = rule.eval(&input);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "meaning_change_exported");
        assert_eq!(findings[0].severity, crate::types::Severity::Fail);
    }
}
