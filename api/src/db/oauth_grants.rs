//! OAuth grants: MCP and app access/refresh token grants. The row types, the write shapes and
//! [`OAuthGrantStore`], the slice of the database that stores them.

use super::{HasID, Result};

/// One authorized OAuth client for a user (MCP and, later, other audiences). Holds
/// both the current access and refresh token hashes — a grant is the unit of
/// revocation, so rotating either token rewrites the same row rather than creating a
/// new one. `expires_at` is the absolute cap (also the DynamoDB TTL attribute):
/// `refresh_expires_at` slides forward on every rotation but never past it.
#[derive(Clone, Debug, PartialEq)]
pub struct OAuthGrant {
    pub id: String,
    /// The user the grant acts for. Always set on a grant approved at the consent
    /// screen; on a Tally sign-in grant (`person_id` set) only when the email they
    /// signed in with is also a user's.
    pub user_id: Option<String>,
    /// Set only on a Tally sign-in grant: the member it acts for. Such a grant resolves
    /// to [`crate::auth::AuthInfo::Member`] rather than a user.
    pub person_id: Option<String>,
    pub client_id: String,
    pub client_name: String,
    pub redirect_uri: String,
    /// The audience (RFC 8707 `resource`) this grant's tokens are bound to, e.g.
    /// `<base>/mcp`. Checked on every use so a token minted for one audience can't
    /// authenticate against another.
    pub resource: String,
    pub scope: String,
    pub access_token_hash: String,
    pub access_expires_at: u64,
    pub refresh_token_hash: String,
    pub refresh_expires_at: u64,
    pub expires_at: u64,
    pub created_at: u64,
    pub last_used_at: Option<u64>,
}

impl HasID for OAuthGrant {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OAuthGrantUpdateShape {
    /// Refresh-token rotation: replace both secrets, sliding `refresh_expires_at`
    /// forward (capped by the grant's `expires_at` — enforced by the caller in
    /// `oauth.rs`, not here). Compare-and-swap on `expected_refresh_token_hash`, so
    /// of two concurrent refreshes with the same token only one wins; the loser gets
    /// [`super::Error::NotFound`], the same as a missing grant.
    Rotate {
        expected_refresh_token_hash: String,
        access_token_hash: String,
        access_expires_at: u64,
        refresh_token_hash: String,
        refresh_expires_at: u64,
    },
    TouchLastUsed,
}

/// The `oauth_grant` table. One of the domain traits [`super::Handler`] is made of.
pub trait OAuthGrantStore: Sync {
    /// Caller supplies the id (and every other field), as with `create_user_token`.
    fn create_oauth_grant(&self, grant: &OAuthGrant) -> impl Future<Output = Result<()>> + Send;

    fn get_oauth_grant(&self, id: &str) -> impl Future<Output = Result<Option<OAuthGrant>>> + Send;

    fn update_oauth_grant(
        &self,
        id: &str,
        change: OAuthGrantUpdateShape,
    ) -> impl Future<Output = Result<()>> + Send;

    fn delete_oauth_grant(&self, id: &str) -> impl Future<Output = Result<()>> + Send;

    /// Backs the "connected apps" list: every grant a user has authorized.
    fn list_oauth_grants_by_user(
        &self,
        user_id: &str,
    ) -> impl Future<Output = Result<Vec<OAuthGrant>>> + Send;

    /// Every grant naming this member (`person_id`): their Tally sign-ins, with or
    /// without a user alongside. Through `person_id-index`.
    fn list_oauth_grants_by_person(
        &self,
        person_id: &str,
    ) -> impl Future<Output = Result<Vec<OAuthGrant>>> + Send;
}
