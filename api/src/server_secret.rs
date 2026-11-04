//! The server's HMAC secret, read from `JWT_SECRET` (named for the kiosk session JWTs it
//! once signed; those are gone, and the variable keeps its name so no deployment has to
//! change).
//!
//! It derives the key that signs stateless OAuth DCR client ids (see
//! [`crate::oauth::encode_client_id`]) and the key that signs offline sync cursors (see
//! `crate::offline_sync`). Changing the secret, or the derivation below, invalidates every
//! registered client id, so MCP clients would have to register again.

use hmac::{Hmac, Mac};

/// Domain-separation label for the client-id HMAC. Part of every issued client id: do not
/// change it.
const OAUTH_CLIENT_ID_LABEL: &[u8] = b"seslogin-oauth-client-id-v1";
/// Domain-separation label for the offline sync cursor HMAC.
const SYNC_CURSOR_LABEL: &[u8] = b"seslogin-offline-sync-cursor-v1";
/// Domain-separation label for the media URL HMAC (`crate::media`). Part of every
/// signed media URL a client may have cached: do not change it.
const MEDIA_URL_LABEL: &[u8] = b"seslogin-media-url-v1";

pub struct ServerSecret {
    secret: Box<[u8]>,
}

impl std::fmt::Debug for ServerSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ServerSecret([redacted])")
    }
}

impl ServerSecret {
    pub fn new(secret: &str) -> Self {
        ServerSecret {
            secret: secret.as_bytes().into(),
        }
    }

    /// A key for HMAC-signing the stateless OAuth DCR client id, derived from the secret
    /// via a domain-separated HMAC rather than used raw.
    pub fn oauth_client_id_key(&self) -> [u8; 32] {
        self.derived_key(OAUTH_CLIENT_ID_LABEL)
    }

    /// A key for HMAC-signing offline sync cursors (`crate::offline_sync`), derived the
    /// same way under its own label.
    pub fn sync_cursor_key(&self) -> [u8; 32] {
        self.derived_key(SYNC_CURSOR_LABEL)
    }

    /// A key for HMAC-signing media URLs (`crate::media`), derived the same way under
    /// its own label.
    pub fn media_url_key(&self) -> [u8; 32] {
        self.derived_key(MEDIA_URL_LABEL)
    }

    fn derived_key(&self, label: &[u8]) -> [u8; 32] {
        let mut mac = <Hmac<sha2::Sha256> as Mac>::new_from_slice(&self.secret)
            .expect("HMAC accepts a key of any length");
        mac.update(label);
        mac.finalize().into_bytes().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the derivation: HMAC-SHA256(secret, label). If this changes, every OAuth
    /// client id already issued stops decoding.
    #[test]
    fn oauth_client_id_key_is_stable() {
        assert_eq!(
            hex::encode(ServerSecret::new("test-secret").oauth_client_id_key()),
            "49d0a8684508de8448a552960ac14d3353a6dd986aaa7c9673b2de19e7f36917"
        );
    }

    /// Pins the derivation, as above: a change invalidates every media URL in circulation.
    #[test]
    fn media_url_key_is_stable() {
        assert_eq!(
            hex::encode(ServerSecret::new("test-secret").media_url_key()),
            "594213d3a368056ae466e5a8bdcd45e91b53237ca2c42881cf5bb09995fcf502"
        );
    }
}
