use std::time::{SystemTime, UNIX_EPOCH};

/// Milliseconds since UNIX epoch.
pub type EpochMs = i64;

pub fn now_ms() -> EpochMs {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (dur.as_millis() as i64)
}
