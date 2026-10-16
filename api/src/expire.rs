/// How long a user (`slu_`) token lives without use (48 hours, sliding).
pub const DEFAULT_USER_EXPIRE_S: u64 = 60 * 60 * 48;
/// How long an enrolled kiosk key lives without a verified request (14 days, sliding).
pub const DEFAULT_SESSION_EXPIRE_S: u64 = 60 * 60 * 24 * 14;
