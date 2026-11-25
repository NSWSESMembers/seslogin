//! [`FakeDb`]'s kiosk keys.

use seslogin::db::{self, KioskKey};

use super::FakeDb;

impl FakeDb {
    /// What the DynamoDB session writes do to the shadow when a session gives up `fingerprint`.
    pub(crate) fn unbind_kiosk_key_shadow(&self, fingerprint: &str, session_id: &str) {
        let mut keys = self.kiosk_keys.lock().unwrap();
        if keys
            .get(fingerprint)
            .is_some_and(|k| k.session_id.as_deref() == Some(session_id))
        {
            keys.remove(fingerprint);
        }
    }
}

impl db::KioskKeyStore for FakeDb {
    async fn get_kiosk_key(&self, fingerprint: &str) -> db::Result<Option<KioskKey>> {
        Ok(self.kiosk_keys.lock().unwrap().get(fingerprint).cloned())
    }

    async fn record_kiosk_key_request(
        &self,
        fingerprint: &str,
        public_key: &str,
        now: u64,
        request_expires_at: u64,
    ) -> db::Result<()> {
        let mut keys = self.kiosk_keys.lock().unwrap();
        let row = keys
            .entry(fingerprint.to_string())
            .or_insert_with(|| KioskKey {
                fingerprint: fingerprint.to_string(),
                ..KioskKey::default()
            });
        if row.session_id.is_none() {
            row.public_key = Some(public_key.to_string());
        }
        row.requested_at = Some(now);
        row.request_expires_at = Some(request_expires_at);
        Ok(())
    }
}
