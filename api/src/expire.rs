use crate::clock::now_sec;

pub const DEFAULT_USER_EXPIRE_S: u64 = 60 * 60 * 48;
pub const DEFAULT_SESSION_EXPIRE_S: u64 = 60 * 60 * 24 * 14;

pub enum ExpirePolicy {
    SessionDefault,
    UserTokenDefault,
    TimeSec(u64),
}

impl ExpirePolicy {
    /// Resolve to an absolute epoch-seconds timestamp. `session_s` overrides the
    /// baked-in default for `SessionDefault` (used by jwt::Key, which supports a
    /// configurable duration). Ignored by the other variants.
    pub fn expires_at(&self, session_s: u64) -> u64 {
        let now = now_sec();
        match self {
            Self::SessionDefault => now + session_s,
            Self::UserTokenDefault => now + DEFAULT_USER_EXPIRE_S,
            Self::TimeSec(sec) => now + sec,
        }
    }

    /// Resolve using the module-level `DEFAULT_*` constants.
    pub fn from_now(self) -> u64 {
        self.expires_at(DEFAULT_SESSION_EXPIRE_S)
    }
}
