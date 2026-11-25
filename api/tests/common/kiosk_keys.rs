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

    async fn list_kiosk_keys(&self) -> db::Result<Vec<db::Result<KioskKey>>> {
        Ok(self
            .kiosk_keys
            .lock()
            .unwrap()
            .values()
            .cloned()
            .map(Ok)
            .collect())
    }

    async fn repair_kiosk_key_binding(
        &self,
        fingerprint: &str,
        public_key: &str,
        session_id: &str,
    ) -> db::Result<bool> {
        let holds = self
            .sessions
            .lock()
            .unwrap()
            .get(session_id)
            .is_some_and(|s| s.active && s.key_fingerprint.as_deref() == Some(fingerprint));
        if !holds {
            return Ok(false);
        }
        let mut keys = self.kiosk_keys.lock().unwrap();
        let row = keys
            .entry(fingerprint.to_string())
            .or_insert_with(|| KioskKey {
                fingerprint: fingerprint.to_string(),
                ..KioskKey::default()
            });
        row.public_key = Some(public_key.to_string());
        row.session_id = Some(session_id.to_string());
        row.version += 1;
        Ok(true)
    }

    async fn repair_kiosk_key_unbind(&self, row: &KioskKey, now: u64) -> db::Result<bool> {
        let Some(session_id) = row.session_id.as_deref() else {
            return Ok(false);
        };
        let holds = self
            .sessions
            .lock()
            .unwrap()
            .get(session_id)
            .is_some_and(|s| s.active && s.key_fingerprint == Some(row.fingerprint.clone()));
        let mut keys = self.kiosk_keys.lock().unwrap();
        let Some(current) = keys.get_mut(&row.fingerprint) else {
            return Ok(false);
        };
        if holds || current.session_id.as_deref() != Some(session_id) {
            return Ok(false);
        }
        if row.is_requested(now) {
            current.session_id = None;
            current.bound_at = None;
            current.version += 1;
        } else {
            keys.remove(&row.fingerprint);
        }
        Ok(true)
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
