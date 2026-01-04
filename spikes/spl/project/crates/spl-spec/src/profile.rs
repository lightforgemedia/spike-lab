use spl_core::GateName;

/// Built-in profile -> required gate mapping (v0 KISS).
pub fn required_gates_for_profile(profile: &str) -> Vec<GateName> {
    match profile {
        "docs" => vec![GateName::Audit, GateName::AdversarialReview],
        "hotfix" => vec![
            GateName::PreSmoke,
            GateName::Audit,
            GateName::Validate,
            GateName::PostSmoke,
        ],
        // standard default
        _ => vec![
            GateName::PreSmoke,
            GateName::Audit,
            GateName::AdversarialReview,
            GateName::Validate,
            GateName::PostSmoke,
        ],
    }
}
