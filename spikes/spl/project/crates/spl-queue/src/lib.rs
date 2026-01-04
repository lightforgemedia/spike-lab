use spl_core::GateName;

/// Classification of a failure to decide retry/backoff vs blocking.
/// This stays pure and testable; the shell applies it to storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailureClass {
    Crash,
    Flake,
    Deterministic,
    Policy,
    SpecDrift,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RetryDecision {
    RetryAfterSecs(u64),
    BlockFailure { reason: String },
    BlockHitl { ask_md: String },
}

/// v0 retry decision policy (KISS):
/// - Crash: retry with backoff
/// - Flake: retry once quickly, then backoff
/// - Deterministic gate failures: block_failure
pub fn decide_retry(gate: GateName, class: FailureClass, attempt_number: u32) -> RetryDecision {
    match class {
        FailureClass::Crash => RetryDecision::RetryAfterSecs(spl_core::default_backoff_seconds(attempt_number)),
        FailureClass::Flake => {
            if attempt_number <= 1 {
                RetryDecision::RetryAfterSecs(5)
            } else {
                RetryDecision::RetryAfterSecs(spl_core::default_backoff_seconds(attempt_number))
            }
        }
        FailureClass::SpecDrift | FailureClass::Policy => RetryDecision::BlockHitl { ask_md: format!("ASK: {:?} requires decision", gate) },
        FailureClass::Deterministic => RetryDecision::BlockFailure { reason: format!("{:?} failed (deterministic)", gate) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_retries() {
        let d = decide_retry(GateName::PreSmoke, FailureClass::Crash, 2);
        assert!(matches!(d, RetryDecision::RetryAfterSecs(_)));
    }
}
