//! Local implementation of [`crate::storage::Handler`]: objects live in a directory
//! (or in memory), so the API runs with no AWS account.
//!
//! Like [`crate::mockqueue`] and unlike [`crate::mockdb`], it *succeeds*.
//!
//! * [`Handler::from_env`] (what `poem-local` uses) stores objects as files under
//!   `MOCK_STORAGE_DIR`, default `local/storage/` at the repo root (git-ignored). A
//!   relative `MOCK_STORAGE_DIR` resolves from the server's working directory,
//!   `api/`, as `MOCK_MAIL_DIR` does; the default is anchored to this crate's
//!   directory instead, so it lands at the repo root wherever the server is started.
//!   An object `pending/abc` is the file `<dir>/pending/abc`. Content types are not
//!   kept.
//! * [`Handler::in_memory`] (tests) keeps everything in a map, and remembers content types.
//! * [`Handler::disabled`] is storage with no bucket: the "uploads are off" environment.
//!
//! A real presigned POST goes to S3; here [`Handler::presign_post`] returns the
//! relative URL [`UPLOAD_PATH`] and the fields `key`, `Content-Type` and `policy`,
//! which `poem-local` serves at `/dev/storage/upload` ([`Handler::accept_upload`]).
//! The `policy` is an opaque token that binds the key, content type, exact size and
//! expiry under an HMAC with a per-process key, so a form can't be edited, and a form
//! from before a restart no longer works (as with an expired one, the client just
//! asks for a new one).
//!
//! Clones share one store.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::storage::{self, ObjectInfo, PresignedPost};

/// Directory objects are written to (see the module docs for how relative paths resolve).
pub const DIR_VAR: &str = "MOCK_STORAGE_DIR";

/// Where the dev server accepts uploads, relative to the API origin.
pub const UPLOAD_PATH: &str = "/dev/storage/upload";

/// `local/storage/` at the repo root.
const DEFAULT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../local/storage");

enum Store {
    Dir(PathBuf),
    Memory(Mutex<HashMap<String, (Vec<u8>, String)>>),
}

struct Inner {
    /// `None`: no bucket.
    store: Option<Store>,
    /// Signs upload policies. Random per handler.
    policy_key: String,
}

#[derive(Clone)]
pub struct Handler {
    inner: Arc<Inner>,
}

/// What a `policy` token says the one permitted upload is.
#[derive(Serialize, Deserialize)]
struct Policy {
    key: String,
    content_type: String,
    byte_size: u64,
    expires_at: u64,
}

impl Handler {
    fn with(store: Option<Store>) -> Self {
        Self {
            inner: Arc::new(Inner {
                store,
                policy_key: crate::nonce::generate_nonce(32),
            }),
        }
    }

    /// Files under [`DIR_VAR`], else `local/storage/` at the repo root.
    pub fn from_env() -> Self {
        let dir = std::env::var_os(DIR_VAR)
            .map(PathBuf::from)
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or_else(|| PathBuf::from(DEFAULT_DIR));
        tracing::info!("mock storage: writing objects to {}", dir.display());
        Self::in_dir(dir)
    }

    pub fn in_dir(dir: impl Into<PathBuf>) -> Self {
        Self::with(Some(Store::Dir(dir.into())))
    }

    pub fn in_memory() -> Self {
        Self::with(Some(Store::Memory(Mutex::default())))
    }

    /// Storage as on an environment with no `ASSET_BUCKET`.
    pub fn disabled() -> Self {
        Self::with(None)
    }

    fn store(&self) -> storage::Result<&Store> {
        self.inner.store.as_ref().ok_or(storage::Error::Unavailable)
    }

    /// A content type recorded by [`storage::Handler::put`] (in-memory only; `None` for a directory).
    pub fn content_type(&self, key: &str) -> Option<String> {
        match self.inner.store.as_ref()? {
            Store::Memory(map) => map
                .lock()
                .expect("mockstorage lock")
                .get(key)
                .map(|(_, ct)| ct.clone()),
            Store::Dir(_) => None,
        }
    }

    fn sign(&self, payload: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.inner.policy_key.as_bytes())
            .expect("HMAC takes any key length");
        mac.update(payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    fn policy_token(&self, policy: &Policy) -> String {
        let payload =
            BASE64_URL_SAFE_NO_PAD.encode(serde_json::to_vec(policy).expect("a policy serialises"));
        format!("{payload}.{}", self.sign(&payload))
    }

    /// Check an upload form against the policy it was presigned with, then store it.
    ///
    /// `fields` are the form's text parts by name, `bytes` its file part. The error is a
    /// short message for the client; it is the same for a forged and an expired
    /// policy so that nothing can be probed.
    pub async fn accept_upload(
        &self,
        fields: &HashMap<String, String>,
        bytes: Vec<u8>,
        now: u64,
    ) -> Result<(), String> {
        use storage::Handler as _;
        let field = |name: &str| {
            fields
                .get(name)
                .map(String::as_str)
                .ok_or_else(|| format!("missing form field {name}"))
        };
        let (payload, signature) = field("policy")?.split_once('.').ok_or("invalid policy")?;
        if !constant_time_eq(&self.sign(payload), signature) {
            return Err("invalid policy".into());
        }
        let policy: Policy = BASE64_URL_SAFE_NO_PAD
            .decode(payload)
            .ok()
            .and_then(|json| serde_json::from_slice(&json).ok())
            .ok_or("invalid policy")?;
        if now >= policy.expires_at {
            return Err("the upload form has expired".into());
        }
        if field("key")? != policy.key {
            return Err("the key does not match the upload form".into());
        }
        if field("Content-Type")? != policy.content_type {
            return Err("the content type does not match the upload form".into());
        }
        if bytes.len() as u64 != policy.byte_size {
            return Err(format!(
                "the file must be exactly {} bytes",
                policy.byte_size
            ));
        }
        self.put(&policy.key, bytes, &policy.content_type)
            .await
            .map_err(|e| e.to_string())
    }
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    use subtle::ConstantTimeEq;
    a.as_bytes().ct_eq(b.as_bytes()).into()
}

fn infra(e: impl std::fmt::Display) -> storage::Error {
    storage::Error::Infrastructure(e.to_string())
}

/// The file for `key` under `dir`; refuses anything that could leave it.
fn path_for(dir: &Path, key: &str) -> storage::Result<PathBuf> {
    let plain = !key.contains('\\')
        && key
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
    if !plain {
        return Err(infra(format!("invalid storage key {key:?}")));
    }
    Ok(dir.join(key))
}

fn read_file(path: &Path, key: &str) -> storage::Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        // A key that is a "directory" (`pending` when `pending/x` exists) is not an object.
        Err(e) if path.is_dir() => {
            tracing::debug!("mock storage: {key} is a directory: {e}");
            Ok(None)
        }
        Err(e) => Err(infra(e)),
    }
}

impl storage::Handler for Handler {
    fn is_enabled(&self) -> bool {
        self.inner.store.is_some()
    }

    async fn presign_post(
        &self,
        key: &str,
        content_type: &str,
        byte_size: u64,
        expires_in: Duration,
    ) -> storage::Result<PresignedPost> {
        self.store()?;
        let expires_at = crate::clock::now_sec() + expires_in.as_secs();
        let policy = self.policy_token(&Policy {
            key: key.to_string(),
            content_type: content_type.to_string(),
            byte_size,
            expires_at,
        });
        Ok(PresignedPost {
            url: UPLOAD_PATH.to_string(),
            fields: vec![
                ("key".to_string(), key.to_string()),
                ("Content-Type".to_string(), content_type.to_string()),
                ("policy".to_string(), policy),
            ],
            expires_at,
        })
    }

    async fn head(&self, key: &str) -> storage::Result<Option<ObjectInfo>> {
        Ok(self.get(key).await?.map(|bytes| ObjectInfo {
            size: bytes.len() as u64,
        }))
    }

    async fn get_range(&self, key: &str, start: u64, len: u64) -> storage::Result<Vec<u8>> {
        let bytes = self
            .get(key)
            .await?
            .ok_or_else(|| storage::Error::NotFound(key.to_string()))?;
        let start = (start as usize).min(bytes.len());
        let end = start.saturating_add(len as usize).min(bytes.len());
        Ok(bytes[start..end].to_vec())
    }

    async fn get(&self, key: &str) -> storage::Result<Option<Vec<u8>>> {
        match self.store()? {
            Store::Dir(dir) => read_file(&path_for(dir, key)?, key),
            Store::Memory(map) => Ok(map
                .lock()
                .expect("mockstorage lock")
                .get(key)
                .map(|(bytes, _)| bytes.clone())),
        }
    }

    async fn put(&self, key: &str, bytes: Vec<u8>, content_type: &str) -> storage::Result<()> {
        match self.store()? {
            Store::Dir(dir) => {
                let path = path_for(dir, key)?;
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(infra)?;
                }
                std::fs::write(&path, bytes).map_err(infra)
            }
            Store::Memory(map) => {
                map.lock()
                    .expect("mockstorage lock")
                    .insert(key.to_string(), (bytes, content_type.to_string()));
                Ok(())
            }
        }
    }

    async fn copy(&self, from: &str, to: &str) -> storage::Result<()> {
        let bytes = self
            .get(from)
            .await?
            .ok_or_else(|| storage::Error::NotFound(from.to_string()))?;
        let content_type = self.content_type(from).unwrap_or_default();
        self.put(to, bytes, &content_type).await
    }

    async fn delete(&self, key: &str) -> storage::Result<()> {
        match self.store()? {
            Store::Dir(dir) => match std::fs::remove_file(path_for(dir, key)?) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(infra(e)),
            },
            Store::Memory(map) => {
                map.lock().expect("mockstorage lock").remove(key);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Handler as _;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "seslogin-mockstorage-{name}-{}",
            crate::nonce::generate_nonce(6)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    async fn round_trip(h: Handler) {
        assert!(h.is_enabled());
        assert_eq!(h.head("pending/a").await.unwrap(), None);
        assert_eq!(h.get("pending/a").await.unwrap(), None);
        assert!(matches!(
            h.get_range("pending/a", 0, 4).await,
            Err(storage::Error::NotFound(_))
        ));
        assert!(matches!(
            h.copy("pending/a", "originals/a").await,
            Err(storage::Error::NotFound(_))
        ));

        h.put("pending/a", b"\xff\xd8\xff\xe0hello".to_vec(), "image/jpeg")
            .await
            .unwrap();
        assert_eq!(
            h.head("pending/a").await.unwrap(),
            Some(ObjectInfo { size: 9 })
        );
        assert_eq!(
            h.get_range("pending/a", 0, 3).await.unwrap(),
            b"\xff\xd8\xff"
        );
        assert_eq!(h.get_range("pending/a", 7, 100).await.unwrap(), b"lo");
        assert_eq!(h.get_range("pending/a", 50, 4).await.unwrap(), b"");

        h.copy("pending/a", "originals/a").await.unwrap();
        h.delete("pending/a").await.unwrap();
        h.delete("pending/a").await.unwrap();
        assert_eq!(h.head("pending/a").await.unwrap(), None);
        assert_eq!(
            h.get("originals/a").await.unwrap().unwrap(),
            b"\xff\xd8\xff\xe0hello"
        );
        // `pending` is a directory in the directory store, and absent in the other: never an object.
        assert_eq!(h.head("pending").await.unwrap(), None);
    }

    #[tokio::test]
    async fn in_memory_round_trip() {
        let h = Handler::in_memory();
        round_trip(h.clone()).await;
        assert_eq!(h.content_type("originals/a").as_deref(), Some("image/jpeg"));
    }

    #[tokio::test]
    async fn directory_round_trip() {
        let dir = temp_dir("rt");
        round_trip(Handler::in_dir(&dir)).await;
        assert!(dir.join("originals/a").is_file());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn keys_cannot_leave_the_directory() {
        let dir = temp_dir("escape");
        let h = Handler::in_dir(&dir);
        for key in ["../x", "a/../../x", "/etc/passwd", "", "a//b", "a\\b"] {
            assert!(
                matches!(
                    h.put(key, vec![1], "image/jpeg").await,
                    Err(storage::Error::Infrastructure(_))
                ),
                "{key:?}"
            );
        }
        assert!(!dir.exists(), "nothing was created");
    }

    #[tokio::test]
    async fn disabled_storage_is_unavailable() {
        let h = Handler::disabled();
        assert!(!h.is_enabled());
        assert!(matches!(
            h.head("k").await,
            Err(storage::Error::Unavailable)
        ));
        assert!(matches!(
            h.put("k", vec![], "image/jpeg").await,
            Err(storage::Error::Unavailable)
        ));
        assert!(matches!(
            h.presign_post("k", "image/jpeg", 1, Duration::from_secs(60))
                .await,
            Err(storage::Error::Unavailable)
        ));
    }

    fn form(post: &PresignedPost) -> HashMap<String, String> {
        post.fields.iter().cloned().collect()
    }

    #[tokio::test]
    async fn presigned_upload_is_accepted_once_it_matches() {
        let h = Handler::in_memory();
        let post = h
            .presign_post("pending/a", "image/jpeg", 4, Duration::from_secs(900))
            .await
            .unwrap();
        assert_eq!(post.url, "/dev/storage/upload");
        let names: Vec<_> = post.fields.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["key", "Content-Type", "policy"]);

        h.accept_upload(&form(&post), vec![1, 2, 3, 4], crate::clock::now_sec())
            .await
            .unwrap();
        assert_eq!(h.get("pending/a").await.unwrap().unwrap(), [1, 2, 3, 4]);
        assert_eq!(h.content_type("pending/a").as_deref(), Some("image/jpeg"));
    }

    #[tokio::test]
    async fn presigned_upload_refuses_anything_that_differs() {
        let h = Handler::in_memory();
        let now = crate::clock::now_sec();
        let post = h
            .presign_post("pending/a", "image/jpeg", 4, Duration::from_secs(900))
            .await
            .unwrap();
        let ok = form(&post);
        let with = |name: &str, value: &str| {
            let mut f = ok.clone();
            f.insert(name.to_string(), value.to_string());
            f
        };
        let without = |name: &str| {
            let mut f = ok.clone();
            f.remove(name);
            f
        };
        let attempts = [
            ("wrong size (short)", ok.clone(), vec![1, 2, 3], now),
            ("wrong size (long)", ok.clone(), vec![1, 2, 3, 4, 5], now),
            ("wrong key", with("key", "pending/b"), vec![1, 2, 3, 4], now),
            (
                "wrong type",
                with("Content-Type", "image/webp"),
                vec![1, 2, 3, 4],
                now,
            ),
            ("expired", ok.clone(), vec![1, 2, 3, 4], now + 901),
            (
                "forged policy",
                with("policy", "e30.deadbeef"),
                vec![1, 2, 3, 4],
                now,
            ),
            ("no policy", without("policy"), vec![1, 2, 3, 4], now),
            ("no key", without("key"), vec![1, 2, 3, 4], now),
        ];
        for (what, fields, bytes, at) in attempts {
            assert!(
                h.accept_upload(&fields, bytes, at).await.is_err(),
                "{what} should be refused"
            );
        }
        // A policy from another handler (another process) is no good either.
        assert!(
            Handler::in_memory()
                .accept_upload(&ok, vec![1, 2, 3, 4], now)
                .await
                .is_err()
        );
        assert_eq!(
            h.head("pending/a").await.unwrap(),
            None,
            "nothing was stored"
        );
    }
}
