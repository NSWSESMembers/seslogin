//! [`FakeDb`]'s email login codes.

use seslogin::db;

use super::FakeDb;

impl db::LoginCodeStore for FakeDb {
    async fn put_login_code(
        &self,
        email: &str,
        code_hash: &str,
        expires_at: u64,
        last_sent_at: u64,
    ) -> db::Result<()> {
        self.login_codes.lock().unwrap().insert(
            email.to_string(),
            db::LoginCode {
                email: email.to_string(),
                code_hash: code_hash.to_string(),
                expires_at,
                attempts: 0,
                last_sent_at,
            },
        );
        Ok(())
    }

    async fn get_login_code(&self, email: &str) -> db::Result<Option<db::LoginCode>> {
        Ok(self.login_codes.lock().unwrap().get(email).cloned())
    }

    async fn delete_login_code(&self, email: &str) -> db::Result<()> {
        self.login_codes.lock().unwrap().remove(email);
        Ok(())
    }

    async fn increment_login_code_attempts(&self, email: &str) -> db::Result<()> {
        if let Some(code) = self.login_codes.lock().unwrap().get_mut(email) {
            code.attempts += 1;
        }
        Ok(())
    }
}
