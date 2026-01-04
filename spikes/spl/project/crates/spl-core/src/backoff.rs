/// Pure backoff policy used by the functional core.
///
/// v0 default:
/// Attempt 1: immediate (0s)
/// Attempt 2: 15m
/// Attempt 3: 1h
/// Attempt 4+: 6h (caller may dead-letter after max_attempt avoid hot-looping)
pub fn default_backoff_seconds(attempt_number: u32) -> u64 {
    match attempt_number {
        0 | 1 => 0,
        2 => 15 * 60,
        3 => 60 * 60,
        _ => 6 * 60 * 60,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_backoff_matches_spec() {
        assert_eq!(default_backoff_seconds(1), 0);
        assert_eq!(default_backoff_seconds(2), 900);
        assert_eq!(default_backoff_seconds(3), 3600);
        assert_eq!(default_backoff_seconds(4), 21600);
        assert_eq!(default_backoff_seconds(10), 21600);
    }
}
