use crate::db::{self, OAuthGrant};

use super::Handler;

impl db::OAuthGrantStore for Handler {
    async fn create_oauth_grant(&self, _grant: &OAuthGrant) -> db::Result<()> {
        Self::unsupported()
    }

    async fn get_oauth_grant(&self, _id: &str) -> db::Result<Option<OAuthGrant>> {
        Self::unsupported()
    }

    async fn update_oauth_grant(
        &self,
        _id: &str,
        _change: db::OAuthGrantUpdateShape,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn delete_oauth_grant(&self, _id: &str) -> db::Result<()> {
        Self::unsupported()
    }

    async fn list_oauth_grants_by_user(&self, _user_id: &str) -> db::Result<Vec<OAuthGrant>> {
        Self::unsupported()
    }

    async fn list_oauth_grants_by_person(&self, _person_id: &str) -> db::Result<Vec<OAuthGrant>> {
        Self::unsupported()
    }
}
