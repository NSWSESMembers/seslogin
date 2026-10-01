use crate::clock::now_sec;

pub const DEFAULT_USER_EXPIRE_S: u64 = 60 * 60 * 48;
pub const DEFAULT_SESSION_EXPIRE_S: u64 = 60 * 60 * 24 * 14;

pub enum ExpirePolicy {
    UserDefault,
    SessionDefault,
    UserTokenDefault,
    TimeSec(u64),
}

impl ExpirePolicy {
    /// Resolve to an absolute epoch-seconds timestamp. `user_s`/`session_s` override
    /// the baked-in defaults for `UserDefault`/`SessionDefault` (used by jwt::Key,
    /// which supports configurable durations). Ignored by the other variants.
    pub fn expires_at(&self, user_s: u64, session_s: u64) -> u64 {
        let now = now_sec();
        match self {
            Self::UserDefault => now + user_s,
            Self::SessionDefault => now + session_s,
            Self::UserTokenDefault => now + DEFAULT_USER_EXPIRE_S,
            Self::TimeSec(sec) => now + sec,
        }
    }

    /// Resolve using the module-level `DEFAULT_*` constants.
    pub fn from_now(self) -> u64 {
        self.expires_at(DEFAULT_USER_EXPIRE_S, DEFAULT_SESSION_EXPIRE_S)
    }
}
