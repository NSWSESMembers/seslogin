//! The server's HMAC secret, read from `JWT_SECRET` (named for the kiosk session JWTs it
//! once signed; those are gone, and the variable keeps its name so no deployment has to
//! change).
//!
//! Its one remaining use is deriving the key that signs stateless OAuth DCR client ids
//! (see [`crate::oauth::encode_client_id`]). Changing the secret, or the derivation below,
//! invalidates every registered client id, so MCP clients would have to register again.

use hmac::{Hmac, Mac};

/// Domain-separation label for the client-id HMAC. Part of every issued client id: do not
/// change it.
const OAUTH_CLIENT_ID_LABEL: &[u8] = b"seslogin-oauth-client-id-v1";

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
        let mut mac = <Hmac<sha2::Sha256> as Mac>::new_from_slice(&self.secret)
            .expect("HMAC accepts a key of any length");
        mac.update(OAUTH_CLIENT_ID_LABEL);
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
}
