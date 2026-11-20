//! [`FakeDb`]'s user tokens.

use seslogin::db;

use super::{FakeDb, unsupported};

impl db::UserTokenStore for FakeDb {
    async fn create_user_token(
        &self,
        id: &str,
        token_hash: &str,
        user_id: &str,
        expires_at: u64,
    ) -> db::Result<db::UserToken> {
        let token = db::UserToken {
            id: id.to_string(),
            token_hash: token_hash.to_string(),
            user_id: user_id.to_string(),
            created_at: seslogin::clock::now_sec(),
            expires_at,
            last_used_at: None,
        };
        self.user_tokens
            .lock()
            .unwrap()
            .insert(id.to_string(), token.clone());
        Ok(token)
    }

    async fn get_user_token(&self, id: &str) -> db::Result<Option<db::UserToken>> {
        Ok(self.user_tokens.lock().unwrap().get(id).cloned())
    }

    async fn update_user_token(
        &self,
        id: &str,
        change: db::UserTokenUpdateShape,
    ) -> db::Result<()> {
        match change {
            db::UserTokenUpdateShape::TouchLastUsed => {
                let mut tokens = self.user_tokens.lock().unwrap();
                let token = tokens
                    .get_mut(id)
                    .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
                let now = seslogin::clock::now_sec();
                token.last_used_at = Some(now);
                token.expires_at = now + seslogin::expire::DEFAULT_USER_EXPIRE_S;
                Ok(())
            }
        }
    }

    async fn delete_user_token(&self, _id: &str) -> db::Result<()> {
        unsupported()
    }
}
