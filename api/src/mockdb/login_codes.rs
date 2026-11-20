use crate::db;

use super::Handler;

impl db::LoginCodeStore for Handler {
    async fn put_login_code(
        &self,
        _email: &str,
        _code_hash: &str,
        _expires_at: u64,
        _last_sent_at: u64,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn get_login_code(&self, _email: &str) -> db::Result<Option<db::LoginCode>> {
        Self::unsupported()
    }

    async fn delete_login_code(&self, _email: &str) -> db::Result<()> {
        Self::unsupported()
    }

    async fn increment_login_code_attempts(&self, _email: &str) -> db::Result<()> {
        Self::unsupported()
    }
}
