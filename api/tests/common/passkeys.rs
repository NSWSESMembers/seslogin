//! [`FakeDb`]'s passkeys.

use seslogin::db;

use super::FakeDb;

impl db::PasskeyStore for FakeDb {
    async fn create_webauthn_credential(
        &self,
        id: &str,
        owner: &db::PasskeyOwner,
        name: &str,
        passkey_json: &str,
    ) -> db::Result<db::WebauthnCredential> {
        let credential = db::WebauthnCredential {
            id: id.to_string(),
            owner: owner.clone(),
            name: name.to_string(),
            passkey_json: passkey_json.to_string(),
            created_at: seslogin::clock::now_sec(),
            last_used_at: None,
        };
        self.webauthn_credentials
            .lock()
            .unwrap()
            .insert(id.to_string(), credential.clone());
        Ok(credential)
    }

    async fn get_webauthn_credential(
        &self,
        id: &str,
    ) -> db::Result<Option<db::WebauthnCredential>> {
        Ok(self.webauthn_credentials.lock().unwrap().get(id).cloned())
    }

    async fn list_webauthn_credentials_by_owner(
        &self,
        owner: &db::PasskeyOwner,
    ) -> db::Result<Vec<db::WebauthnCredential>> {
        let mut creds: Vec<_> = self
            .webauthn_credentials
            .lock()
            .unwrap()
            .values()
            .filter(|c| &c.owner == owner)
            .cloned()
            .collect();
        creds.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(creds)
    }

    async fn count_webauthn_credentials_by_owner(
        &self,
        owner: &db::PasskeyOwner,
    ) -> db::Result<usize> {
        Ok(self
            .webauthn_credentials
            .lock()
            .unwrap()
            .values()
            .filter(|c| &c.owner == owner)
            .count())
    }

    async fn update_webauthn_credential(
        &self,
        id: &str,
        change: db::WebauthnCredentialUpdate,
    ) -> db::Result<()> {
        match change {
            db::WebauthnCredentialUpdate::Rename(name) => {
                let mut creds = self.webauthn_credentials.lock().unwrap();
                let cred = creds
                    .get_mut(id)
                    .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
                cred.name = name;
                Ok(())
            }
            db::WebauthnCredentialUpdate::TouchLastUsed { passkey_json } => {
                let mut creds = self.webauthn_credentials.lock().unwrap();
                let cred = creds
                    .get_mut(id)
                    .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
                cred.passkey_json = passkey_json;
                cred.last_used_at = Some(seslogin::clock::now_sec());
                Ok(())
            }
        }
    }

    async fn delete_webauthn_credential(&self, id: &str) -> db::Result<()> {
        self.webauthn_credentials.lock().unwrap().remove(id);
        Ok(())
    }

    async fn put_webauthn_state(
        &self,
        id: &str,
        kind: &str,
        user_id: Option<&str>,
        state_json: &str,
        expires_at: u64,
    ) -> db::Result<()> {
        self.webauthn_states.lock().unwrap().insert(
            id.to_string(),
            db::WebauthnState {
                id: id.to_string(),
                kind: kind.to_string(),
                user_id: user_id.map(str::to_string),
                state_json: state_json.to_string(),
                expires_at,
            },
        );
        Ok(())
    }

    async fn get_webauthn_state(&self, id: &str) -> db::Result<Option<db::WebauthnState>> {
        Ok(self.webauthn_states.lock().unwrap().get(id).cloned())
    }

    async fn delete_webauthn_state(&self, id: &str) -> db::Result<()> {
        self.webauthn_states.lock().unwrap().remove(id);
        Ok(())
    }
}
