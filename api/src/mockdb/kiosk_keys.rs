use crate::db;

use super::Handler;

impl db::KioskKeyStore for Handler {
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
