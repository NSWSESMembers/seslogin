use crate::db;

use super::Handler;

impl db::KioskKeyStore for Handler {
    async fn get_kiosk_key(&self, _fingerprint: &str) -> db::Result<Option<db::KioskKey>> {
        Self::unsupported()
    }

    async fn list_kiosk_keys(&self) -> db::Result<Vec<db::Result<db::KioskKey>>> {
        Self::unsupported()
    }

    async fn repair_kiosk_key_binding(
        &self,
        _fingerprint: &str,
        _public_key: &str,
        _session_id: &str,
    ) -> db::Result<bool> {
        Self::unsupported()
    }

    async fn repair_kiosk_key_unbind(&self, _row: &db::KioskKey, _now: u64) -> db::Result<bool> {
        Self::unsupported()
    }

    async fn record_kiosk_key_request(
        &self,
        _fingerprint: &str,
        _public_key: &str,
        _now: u64,
        _request_expires_at: u64,
    ) -> db::Result<()> {
        Self::unsupported()
    }
}
