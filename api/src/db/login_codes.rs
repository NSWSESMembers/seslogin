//! Email login codes: the one-time codes emailed to sign a user in. The row types, the write shapes and
//! [`LoginCodeStore`], the slice of the database that stores them.

use super::Result;

#[derive(Clone, Debug, PartialEq)]
pub struct LoginCode {
    pub email: String,
    pub code_hash: String,
    pub expires_at: u64,
    pub attempts: u64,
    pub last_sent_at: u64,
}

/// The `login_code` table. One of the domain traits [`super::Handler`] is made of.
pub trait LoginCodeStore: Sync {
    fn put_login_code(
        &self,
        email: &str,
        code_hash: &str,
        expires_at: u64,
        last_sent_at: u64,
    ) -> impl Future<Output = Result<()>> + Send;

    fn get_login_code(&self, email: &str)
    -> impl Future<Output = Result<Option<LoginCode>>> + Send;

    fn delete_login_code(&self, email: &str) -> impl Future<Output = Result<()>> + Send;

    fn increment_login_code_attempts(&self, email: &str)
    -> impl Future<Output = Result<()>> + Send;
}
