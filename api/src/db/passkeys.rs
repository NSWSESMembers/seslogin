//! WebAuthn passkey credentials and their challenge state. The row types, the write shapes and
//! [`PasskeyStore`], the slice of the database that stores them.

use super::Result;

/// Whose passkey a [`WebauthnCredential`] is. A user's passkey signs in to the admin
/// site (and to Tally, as that user's member); a member's is for Tally sign-in only,
/// and is made only for a member who signs in with no user alongside them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PasskeyOwner {
    /// A user ID: stored as `user_id`.
    User(String),
    /// A person ID: stored as `person_id`.
    Member(String),
}

impl PasskeyOwner {
    pub fn user_id(&self) -> Option<&str> {
        match self {
            Self::User(id) => Some(id),
            Self::Member(_) => None,
        }
    }

    pub fn person_id(&self) -> Option<&str> {
        match self {
            Self::User(_) => None,
            Self::Member(id) => Some(id),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WebauthnCredential {
    /// Credential ID (base64url) — partition key in DynamoDB.
    pub id: String,
    pub owner: PasskeyOwner,
    /// User-supplied label (e.g. "MacBook Touch ID").
    pub name: String,
    /// JSON-serialized webauthn_rs::prelude::Passkey (contains counter, public key, etc.).
    pub passkey_json: String,
    pub created_at: u64,
    pub last_used_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebauthnCredentialUpdate {
    Rename(String),
    TouchLastUsed { passkey_json: String },
}

/// Short-lived challenge state stored between the two WebAuthn round-trips.
#[derive(Clone, Debug, PartialEq)]
pub struct WebauthnState {
    /// Opaque challenge ID returned to the client.
    pub id: String,
    pub kind: String,
    /// Set for registration states.
    pub user_id: Option<String>,
    /// JSON-serialized PasskeyRegistration or DiscoverableAuthentication.
    pub state_json: String,
    /// Unix timestamp; DynamoDB TTL auto-deletes after this.
    pub expires_at: u64,
}

/// The `webauthn_credential` table, and the challenge state kept in the ephemeral state table. One of the domain traits [`super::Handler`] is made of.
pub trait PasskeyStore: Sync {
    fn create_webauthn_credential(
        &self,
        id: &str,
        owner: &PasskeyOwner,
        name: &str,
        passkey_json: &str,
    ) -> impl Future<Output = Result<WebauthnCredential>> + Send;

    fn get_webauthn_credential(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<WebauthnCredential>>> + Send;

    /// The owner's passkeys: a user's through `user_id-index`, a member's through
    /// `person_id-index`.
    fn list_webauthn_credentials_by_owner(
        &self,
        owner: &PasskeyOwner,
    ) -> impl Future<Output = Result<Vec<WebauthnCredential>>> + Send;

    fn count_webauthn_credentials_by_owner(
        &self,
        owner: &PasskeyOwner,
    ) -> impl Future<Output = Result<usize>> + Send;

    fn update_webauthn_credential(
        &self,
        id: &str,
        change: WebauthnCredentialUpdate,
    ) -> impl Future<Output = Result<()>> + Send;

    fn delete_webauthn_credential(&self, id: &str) -> impl Future<Output = Result<()>> + Send;

    fn put_webauthn_state(
        &self,
        id: &str,
        kind: &str,
        user_id: Option<&str>,
        state_json: &str,
        expires_at: u64,
    ) -> impl Future<Output = Result<()>> + Send;

    fn get_webauthn_state(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<WebauthnState>>> + Send;

    fn delete_webauthn_state(&self, id: &str) -> impl Future<Output = Result<()>> + Send;
}
