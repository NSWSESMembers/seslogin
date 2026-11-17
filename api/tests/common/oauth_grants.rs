//! [`FakeDb`]'s OAuth grants.

use seslogin::db::{self, OAuthGrant, OAuthGrantUpdateShape};

use super::FakeDb;

impl db::OAuthGrantStore for FakeDb {
    async fn create_oauth_grant(&self, grant: &OAuthGrant) -> db::Result<()> {
        self.oauth_grants
            .lock()
            .unwrap()
            .insert(grant.id.clone(), grant.clone());
        Ok(())
    }

    async fn get_oauth_grant(&self, id: &str) -> db::Result<Option<OAuthGrant>> {
        Ok(self.oauth_grants.lock().unwrap().get(id).cloned())
    }

    async fn update_oauth_grant(&self, id: &str, change: OAuthGrantUpdateShape) -> db::Result<()> {
        let mut grants = self.oauth_grants.lock().unwrap();
        let grant = grants
            .get_mut(id)
            .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
        match change {
            OAuthGrantUpdateShape::Rotate {
                expected_refresh_token_hash,
                access_token_hash,
                access_expires_at,
                refresh_token_hash,
                refresh_expires_at,
            } => {
                if grant.refresh_token_hash != expected_refresh_token_hash {
                    return Err(db::Error::NotFound(id.to_string()));
                }
                grant.access_token_hash = access_token_hash;
                grant.access_expires_at = access_expires_at;
                grant.refresh_token_hash = refresh_token_hash;
                grant.refresh_expires_at = refresh_expires_at;
                Ok(())
            }
            OAuthGrantUpdateShape::TouchLastUsed => {
                grant.last_used_at = Some(seslogin::clock::now_sec());
                Ok(())
            }
        }
    }

    async fn delete_oauth_grant(&self, id: &str) -> db::Result<()> {
        self.oauth_grants.lock().unwrap().remove(id);
        Ok(())
    }

    async fn list_oauth_grants_by_user(&self, user_id: &str) -> db::Result<Vec<OAuthGrant>> {
        Ok(self
            .oauth_grants
            .lock()
            .unwrap()
            .values()
            .filter(|g| g.user_id.as_deref() == Some(user_id))
            .cloned()
            .collect())
    }

    async fn list_oauth_grants_by_person(&self, person_id: &str) -> db::Result<Vec<OAuthGrant>> {
        Ok(self
            .oauth_grants
            .lock()
            .unwrap()
            .values()
            .filter(|g| g.person_id.as_deref() == Some(person_id))
            .cloned()
            .collect())
    }
}
