use std::time::{SystemTime, UNIX_EPOCH};

use ulid::Ulid;

/// Returns current unix epoch milliseconds.
pub fn now_ms() -> i64 {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock set before UNIX_EPOCH");
    dur.as_millis() as i64
}

/// Generates a new ULID.
pub fn new_ulid() -> Ulid {
    Ulid::new()
}
