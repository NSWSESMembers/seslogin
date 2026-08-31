use std::time::SystemTime;

/// Current Unix time in whole seconds (UTC). Used for all created/updated/expiry timestamps.
pub fn now_sec() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Current Unix time in milliseconds (UTC), signed so it can be differenced against a
/// client's own clock without underflowing when the client is behind.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
