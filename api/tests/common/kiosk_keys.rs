//! [`FakeDb`]'s kiosk keys.

use seslogin::db::{self, KioskKey};

use super::FakeDb;

/// What the DynamoDB writes do to `fingerprint`'s row when `session_id` gives the key
/// up: a row the device is still asking with is kept as an unbound request, any other is
/// deleted. Only a row bound to that session is touched.
pub(crate) fn unbind_row(
    keys: &mut std::collections::HashMap<String, KioskKey>,
    fingerprint: &str,
    session_id: &str,
    now: u64,
) {
    let Some(row) = keys
        .get_mut(fingerprint)
        .filter(|k| k.session_id.as_deref() == Some(session_id))
    else {
        return;
    };
    if row.is_requested(now) {
        row.session_id = None;
        row.bound_at = None;
        row.version += 1;
    } else {
        keys.remove(fingerprint);
    }
}

#[allow(dead_code)] // each test binary uses some of these
impl FakeDb {
    /// A device on its QR screen asking to be set up with `fingerprint`, as
    /// `submitEnrollmentKey` leaves it: a request on the key's `kiosk_key` row (a bound
    /// row keeps its binding), and the `ephemeral_state` record older API versions read.
    pub(crate) fn device_asking(&self, fingerprint: &str, public_key: &str) {
        self.ephemeral_state.lock().unwrap().insert(
            seslogin::session_key::enroll_state_id(fingerprint),
            db::EphemeralState {
                id: seslogin::session_key::enroll_state_id(fingerprint),
                kind: seslogin::session_key::ENROLL_STATE_KIND.to_string(),
                payload: format!(r#"{{"public_key":"{public_key}","submitted_at":0}}"#),
                expires_at: u64::MAX,
            },
        );
        let mut keys = self.kiosk_keys.lock().unwrap();
        let row = keys
            .entry(fingerprint.to_string())
            .or_insert_with(|| KioskKey {
                fingerprint: fingerprint.to_string(),
                public_key: Some(public_key.to_string()),
                ..KioskKey::default()
            });
        row.requested_at = Some(0);
        row.request_expires_at = Some(u64::MAX);
    }

    /// `fingerprint`'s `kiosk_key` row bound to `session_id`, as a bind leaves it.
    pub(crate) fn bind_key_row(&self, fingerprint: &str, session_id: &str) {
        self.kiosk_keys.lock().unwrap().insert(
            fingerprint.to_string(),
            KioskKey {
                fingerprint: fingerprint.to_string(),
                public_key: Some(format!("pk-{fingerprint}")),
                session_id: Some(session_id.to_string()),
                bound_at: Some(0),
                version: 1,
                ..KioskKey::default()
            },
        );
    }

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
