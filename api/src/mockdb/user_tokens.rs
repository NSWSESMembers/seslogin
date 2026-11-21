use crate::db;

use super::Handler;

impl db::UserTokenStore for Handler {
    async fn create_user_token(
        &self,
        _id: &str,
        _token_hash: &str,
        _user_id: &str,
        _expires_at: u64,
    ) -> db::Result<db::UserToken> {
        Self::unsupported()
    }

    async fn get_user_token(&self, _id: &str) -> db::Result<Option<db::UserToken>> {
        Self::unsupported()
    }

    async fn update_user_token(
        &self,
        _id: &str,
        _change: db::UserTokenUpdateShape,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn delete_user_token(&self, _id: &str) -> db::Result<()> {
        Self::unsupported()
    }
}
