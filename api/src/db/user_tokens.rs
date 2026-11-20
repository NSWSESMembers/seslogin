//! User tokens: opaque, hashed, sliding-expiry credentials issued to users. The row types, the write shapes and
//! [`UserTokenStore`], the slice of the database that stores them.

use super::Result;

#[derive(Clone, Debug, PartialEq)]
pub struct UserToken {
    pub id: String,
    pub token_hash: String,
    pub user_id: String,
    pub created_at: u64,
    pub expires_at: u64,
    pub last_used_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserTokenUpdateShape {
    TouchLastUsed,
}

/// The `user_token` table. One of the domain traits [`super::Handler`] is made of.
pub trait UserTokenStore: Sync {
    /// Caller supplies the id: it is embedded in the token (`slu_<id>.<secret>`), so
    /// it has to exist before the token can be minted and hashed.
    fn create_user_token(
        &self,
        id: &str,
        token_hash: &str,
        user_id: &str,
        expires_at: u64,
    ) -> impl Future<Output = Result<UserToken>> + Send;

    /// Strongly consistent, so a token is verifiable the moment it's issued.
    fn get_user_token(&self, id: &str) -> impl Future<Output = Result<Option<UserToken>>> + Send;

    fn update_user_token(
        &self,
        id: &str,
        change: UserTokenUpdateShape,
    ) -> impl Future<Output = Result<()>> + Send;

    fn delete_user_token(&self, id: &str) -> impl Future<Output = Result<()>> + Send;
}
