use spl_core::{TaskId};

/// Minimal ASK packet generator (markdown).
pub fn format_ask(task_id: &TaskId, decision_needed: &str, options: &[&str], recommendation: &str, next_if_chosen: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("# ASK: {}\n\n", task_id.as_str()));
    s.push_str(&format!("**Decision needed:** {}\n\n", decision_needed));
    s.push_str("## Options\n");
    for (i, opt) in options.iter().enumerate() {
        s.push_str(&format!("- [{}] {}\n", i + 1, opt));
    }
    s.push_str("\n");
    s.push_str(&format!("**Recommended:** {}\n\n", recommendation));
    s.push_str(&format!("**If chosen, SPL will do next:** {}\n", next_if_chosen));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use spl_core::TaskId;

    #[test]
    fn test_format_ask() {
        let task_id = TaskId::from_str("task1");
        let decision_needed = "Does this look right?";
        let options = ["Yes", "No"];
        let recommendation = "Yes";
        let next_if_chosen = "Proceed";
        let expected = "# ASK: task1\n\n**Decision needed:** Does this look right?\n\n## Options\n- [1] Yes\n- [2] No\n\n**Recommended:** Yes\n\n**If chosen, SPL will do next:** Proceed\n";
        let actual = format_ask(&task_id, decision_needed, &options, recommendation, next_if_chosen);
        assert_eq!(actual, expected);
    }
}
