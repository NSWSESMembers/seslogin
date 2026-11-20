use crate::db;

use super::Handler;

impl db::PasskeyStore for Handler {
    async fn create_webauthn_credential(
        &self,
        _id: &str,
        _owner: &db::PasskeyOwner,
        _name: &str,
        _passkey_json: &str,
    ) -> db::Result<db::WebauthnCredential> {
        Self::unsupported()
    }

    async fn get_webauthn_credential(
        &self,
        _id: &str,
    ) -> db::Result<Option<db::WebauthnCredential>> {
        Self::unsupported()
    }

    async fn list_webauthn_credentials_by_owner(
        &self,
        _owner: &db::PasskeyOwner,
    ) -> db::Result<Vec<db::WebauthnCredential>> {
        Self::unsupported()
    }

    async fn count_webauthn_credentials_by_owner(
        &self,
        _owner: &db::PasskeyOwner,
    ) -> db::Result<usize> {
        Self::unsupported()
    }

    async fn update_webauthn_credential(
        &self,
        _id: &str,
        _change: db::WebauthnCredentialUpdate,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn delete_webauthn_credential(&self, _id: &str) -> db::Result<()> {
        Self::unsupported()
    }

    async fn put_webauthn_state(
        &self,
        _id: &str,
        _kind: &str,
        _user_id: Option<&str>,
        _state_json: &str,
        _expires_at: u64,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn get_webauthn_state(&self, _id: &str) -> db::Result<Option<db::WebauthnState>> {
        Self::unsupported()
    }

    async fn delete_webauthn_state(&self, _id: &str) -> db::Result<()> {
        Self::unsupported()
    }
}
