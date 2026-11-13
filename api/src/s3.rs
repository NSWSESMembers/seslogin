//! AWS S3 implementation of [`crate::storage::Handler`].
//!
//! Everything is the plain SDK call except the presigned POST: `aws-sdk-s3` can
//! presign GET and PUT but not POST, and only a POST policy lets S3 itself enforce
//! the exact key, content type and size of an upload. So [`sign_post`] builds the
//! policy and signs it by hand with SigV4 (`AWS4-HMAC-SHA256`, service `s3`).

use std::time::Duration;

use aws_sdk_s3::Client;
use aws_sdk_s3::config::http::HttpResponse;
use aws_sdk_s3::config::{ProvideCredentials, SharedCredentialsProvider};
use aws_sdk_s3::error::{ProvideErrorMetadata, SdkError};
use aws_sdk_s3::primitives::ByteStream;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use sha2::Sha256;

use crate::storage::{self, ObjectInfo, PresignedPost};

/// The region of both asset buckets.
pub const REGION: &str = "ap-southeast-2";

const ALGORITHM: &str = "AWS4-HMAC-SHA256";

/// An S3 `CopySource` is URL-encoded, keeping `/`.
const COPY_SOURCE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'/')
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

pub struct Handler {
    client: Client,
    /// `None` disables storage: see [`storage`]'s "Unavailable storage".
    bucket: Option<String>,
    credentials: Option<SharedCredentialsProvider>,
}

impl Handler {
    /// Build from `ASSET_BUCKET`. Unset or blank disables storage rather than failing startup.
    pub async fn from_env() -> Self {
        let bucket = std::env::var("ASSET_BUCKET")
            .ok()
            .map(|b| b.trim().to_string())
            .filter(|b| !b.is_empty());
        if bucket.is_none() {
            tracing::warn!("ASSET_BUCKET is not set: uploads are disabled on this server");
        }
        let config = crate::aws_config_loader()
            .region(aws_sdk_s3::config::Region::new(REGION))
            .load()
            .await;
        Self {
            client: Client::new(&config),
            bucket,
            credentials: config.credentials_provider(),
        }
    }

    fn bucket(&self) -> storage::Result<&str> {
        self.bucket.as_deref().ok_or(storage::Error::Unavailable)
    }
}

fn infra(e: impl std::fmt::Debug) -> storage::Error {
    storage::Error::Infrastructure(format!("{e:?}"))
}

/// The HTTP status of the response behind an SDK error, if there was one.
fn status<E>(e: &SdkError<E, HttpResponse>) -> Option<u16> {
    e.raw_response().map(|r| r.status().as_u16())
}

impl storage::Handler for Handler {
    fn is_enabled(&self) -> bool {
        self.bucket.is_some()
    }

    async fn presign_post(
        &self,
        key: &str,
        content_type: &str,
        byte_size: u64,
        expires_in: Duration,
    ) -> storage::Result<PresignedPost> {
        let bucket = self.bucket()?;
        let provider = self
            .credentials
            .as_ref()
            .ok_or_else(|| storage::Error::Infrastructure("no AWS credentials provider".into()))?;
        let creds = provider
            .provide_credentials()
            .await
            .map_err(|e| storage::Error::Infrastructure(format!("AWS credentials: {e}")))?;
        let creds = Credentials {
            access_key_id: creds.access_key_id().to_string(),
            secret_access_key: creds.secret_access_key().to_string(),
            session_token: creds.session_token().map(str::to_string),
        };
        let now = DateTime::<Utc>::from_timestamp(crate::clock::now_sec() as i64, 0)
            .expect("the clock is in range");
        Ok(sign_post(
            &creds,
            bucket,
            key,
            content_type,
            byte_size,
            now,
            expires_in,
        ))
    }

    async fn head(&self, key: &str) -> storage::Result<Option<ObjectInfo>> {
        let bucket = self.bucket()?;
        match self
            .client
            .head_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
        {
            Ok(out) => Ok(Some(ObjectInfo {
                size: out.content_length().unwrap_or(0).max(0) as u64,
            })),
            // S3 answers a missing key with 403, not 404, to a caller without ListBucket.
            Err(e) if matches!(status(&e), Some(403 | 404)) => Ok(None),
            Err(e) => Err(infra(e)),
        }
    }

    async fn get_range(&self, key: &str, start: u64, len: u64) -> storage::Result<Vec<u8>> {
        let bucket = self.bucket()?;
        if len == 0 {
            return Ok(Vec::new());
        }
        let res = self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .range(format!("bytes={start}-{}", start + len - 1))
            .send()
            .await;
        match res {
            Ok(out) => Ok(out.body.collect().await.map_err(infra)?.to_vec()),
            Err(e) if e.code() == Some("NoSuchKey") => {
                Err(storage::Error::NotFound(key.to_string()))
            }
            // A range starting at or past the end of the object (including any range of an empty one).
            Err(e) if e.code() == Some("InvalidRange") => Ok(Vec::new()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn get(&self, key: &str) -> storage::Result<Option<Vec<u8>>> {
        let bucket = self.bucket()?;
        match self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
        {
            Ok(out) => Ok(Some(out.body.collect().await.map_err(infra)?.to_vec())),
            Err(e) if e.code() == Some("NoSuchKey") => Ok(None),
            Err(e) => Err(infra(e)),
        }
    }

    async fn put(&self, key: &str, bytes: Vec<u8>, content_type: &str) -> storage::Result<()> {
        let bucket = self.bucket()?;
        self.client
            .put_object()
            .bucket(bucket)
            .key(key)
            .content_type(content_type)
            .body(ByteStream::from(bytes))
            .send()
            .await
            .map_err(infra)?;
        Ok(())
    }

    async fn copy(&self, from: &str, to: &str) -> storage::Result<()> {
        let bucket = self.bucket()?;
        let source = format!("{bucket}/{}", utf8_percent_encode(from, COPY_SOURCE));
        match self
            .client
            .copy_object()
            .bucket(bucket)
            .key(to)
            .copy_source(source)
            .send()
            .await
        {
            Ok(_) => Ok(()),
            Err(e) if e.code() == Some("NoSuchKey") => {
                Err(storage::Error::NotFound(from.to_string()))
            }
            Err(e) => Err(infra(e)),
        }
    }

    async fn delete(&self, key: &str) -> storage::Result<()> {
        let bucket = self.bucket()?;
        self.client
            .delete_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(infra)?;
        Ok(())
    }
}

/// The parts of AWS credentials the signature needs.
#[derive(Debug, Clone)]
pub struct Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    /// Present on Lambda and for any temporary credentials; S3 then also requires it as a form field and policy condition.
    pub session_token: Option<String>,
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC takes any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// The SigV4 signing key: HMAC chain over date, region, service and `aws4_request`.
/// `date` is `YYYYMMDD`.
pub fn signing_key(secret_access_key: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac_sha256(
        format!("AWS4{secret_access_key}").as_bytes(),
        date.as_bytes(),
    );
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

/// Build the upload form for exactly one object at `key` in `bucket`, signed at `now`.
///
/// The policy lets S3 enforce: the bucket, the key exactly, the content type
/// exactly, a size of exactly `byte_size` bytes, and the signing fields themselves.
/// Fields come back in the order a client must send them (the file part goes last).
pub fn sign_post(
    creds: &Credentials,
    bucket: &str,
    key: &str,
    content_type: &str,
    byte_size: u64,
    now: DateTime<Utc>,
    expires_in: Duration,
) -> PresignedPost {
    let date = now.format("%Y%m%d").to_string();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let credential = format!("{}/{date}/{REGION}/s3/aws4_request", creds.access_key_id);
    let expires = now + chrono::Duration::from_std(expires_in).expect("a sane expiry");

    let mut conditions = vec![
        serde_json::json!({"bucket": bucket}),
        serde_json::json!({"key": key}),
        serde_json::json!({"Content-Type": content_type}),
        serde_json::json!(["content-length-range", byte_size, byte_size]),
        serde_json::json!({"x-amz-algorithm": ALGORITHM}),
        serde_json::json!({"x-amz-credential": credential}),
        serde_json::json!({"x-amz-date": amz_date}),
    ];
    if let Some(token) = &creds.session_token {
        conditions.push(serde_json::json!({"x-amz-security-token": token}));
    }
    // A struct, not a `json!` map, so the bytes that get signed don't depend on key ordering.
    #[derive(serde::Serialize)]
    struct Policy {
        expiration: String,
        conditions: Vec<serde_json::Value>,
    }
    let policy = Policy {
        expiration: expires.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        conditions,
    };
    let policy = STANDARD.encode(serde_json::to_string(&policy).expect("a policy serialises"));
    let signature = hex::encode(hmac_sha256(
        &signing_key(&creds.secret_access_key, &date, REGION, "s3"),
        policy.as_bytes(),
    ));

    let mut fields = vec![
        ("key".to_string(), key.to_string()),
        ("Content-Type".to_string(), content_type.to_string()),
        ("x-amz-algorithm".to_string(), ALGORITHM.to_string()),
        ("x-amz-credential".to_string(), credential),
        ("x-amz-date".to_string(), amz_date),
    ];
    if let Some(token) = &creds.session_token {
        fields.push(("x-amz-security-token".to_string(), token.clone()));
    }
    fields.push(("policy".to_string(), policy));
    fields.push(("x-amz-signature".to_string(), signature));

    PresignedPost {
        url: format!("https://{bucket}.s3.{REGION}.amazonaws.com/"),
        fields,
        expires_at: expires.timestamp() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Handler as _;

    fn creds(token: Option<&str>) -> Credentials {
        Credentials {
            access_key_id: "AKIDEXAMPLE".into(),
            secret_access_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".into(),
            session_token: token.map(str::to_string),
        }
    }

    fn at() -> DateTime<Utc> {
        "2026-11-04T01:02:03Z".parse().unwrap()
    }

    fn field<'a>(post: &'a PresignedPost, name: &str) -> &'a str {
        &post
            .fields
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no field {name}"))
            .1
    }

    /// AWS's published SigV4 example (docs: "Examples of how to derive a signing key").
    #[test]
    fn signing_key_matches_the_published_vector() {
        let key = signing_key(
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            "20120215",
            "us-east-1",
            "iam",
        );
        assert_eq!(
            hex::encode(key),
            "f4780e2d9f65fa895f9c67b32ce1baf0b0d8a43505a000a1a9e090d414db404d"
        );
    }

    #[test]
    fn policy_has_exactly_the_expected_conditions() {
        let post = sign_post(
            &creds(None),
            "my-bucket",
            "pending/abc",
            "image/jpeg",
            1234,
            at(),
            Duration::from_secs(900),
        );
        let policy: serde_json::Value =
            serde_json::from_slice(&STANDARD.decode(field(&post, "policy")).unwrap()).unwrap();
        assert_eq!(
            policy,
            serde_json::json!({
                "expiration": "2026-11-04T01:17:03Z",
                "conditions": [
                    {"bucket": "my-bucket"},
                    {"key": "pending/abc"},
                    {"Content-Type": "image/jpeg"},
                    ["content-length-range", 1234, 1234],
                    {"x-amz-algorithm": "AWS4-HMAC-SHA256"},
                    {"x-amz-credential": "AKIDEXAMPLE/20261104/ap-southeast-2/s3/aws4_request"},
                    {"x-amz-date": "20261104T010203Z"},
                ],
            })
        );
        assert_eq!(post.expires_at, at().timestamp() as u64 + 900);
        assert_eq!(
            post.url,
            "https://my-bucket.s3.ap-southeast-2.amazonaws.com/"
        );
    }

    #[test]
    fn fields_come_back_in_order_with_the_signature_pinned() {
        let post = sign_post(
            &creds(None),
            "my-bucket",
            "pending/abc",
            "image/jpeg",
            1234,
            at(),
            Duration::from_secs(900),
        );
        let names: Vec<_> = post.fields.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "key",
                "Content-Type",
                "x-amz-algorithm",
                "x-amz-credential",
                "x-amz-date",
                "policy",
                "x-amz-signature"
            ]
        );
        // Recomputed by an independent implementation (python hmac/hashlib).
        assert_eq!(
            field(&post, "x-amz-signature"),
            "ecea17f27ffd4e6fa277046b893c7659d8d09d821bcd49d0699ac48b9d4f8b0b"
        );
    }

    #[test]
    fn a_session_token_is_a_field_and_a_condition() {
        let post = sign_post(
            &creds(Some("tok/en+1")),
            "my-bucket",
            "pending/abc",
            "image/webp",
            9,
            at(),
            Duration::from_secs(900),
        );
        let names: Vec<_> = post.fields.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "key",
                "Content-Type",
                "x-amz-algorithm",
                "x-amz-credential",
                "x-amz-date",
                "x-amz-security-token",
                "policy",
                "x-amz-signature"
            ]
        );
        assert_eq!(field(&post, "x-amz-security-token"), "tok/en+1");
        let policy: serde_json::Value =
            serde_json::from_slice(&STANDARD.decode(field(&post, "policy")).unwrap()).unwrap();
        assert_eq!(
            policy["conditions"].as_array().unwrap().last().unwrap(),
            &serde_json::json!({"x-amz-security-token": "tok/en+1"})
        );
        assert_eq!(
            field(&post, "x-amz-signature"),
            "22fe1e9c525d943a36dd92b814663288721202b4241d34db11780a28d19940e0"
        );
    }

    #[tokio::test]
    async fn a_handler_with_no_bucket_is_unavailable_without_calling_aws() {
        let handler = Handler {
            client: Client::from_conf(
                aws_sdk_s3::Config::builder()
                    .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
                    .region(aws_sdk_s3::config::Region::new(REGION))
                    .build(),
            ),
            bucket: None,
            credentials: None,
        };
        assert!(!handler.is_enabled());
        assert!(matches!(
            handler.head("k").await,
            Err(storage::Error::Unavailable)
        ));
        assert!(matches!(
            handler
                .presign_post("k", "image/jpeg", 1, Duration::from_secs(1))
                .await,
            Err(storage::Error::Unavailable)
        ));
        assert!(matches!(
            handler.delete("k").await,
            Err(storage::Error::Unavailable)
        ));
    }
}
