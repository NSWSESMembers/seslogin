//! `GET /api/v1/contacts/sync`: the member directory as a cursor-based change feed,
//! for Tally's offline contact list.
//!
//! - No cursor: every live member, page by page (a full sync).
//! - With a cursor: only what changed since it — upserts, and `{id, deleted: true}`
//!   tombstones for soft-deleted members — page by page.
//! - Every response carries a `nextCursor`, the last page's included.
//!
//! Ordering comes from the person table's `contact_key` (see SCHEMA.md), restamped by
//! every write that changes a field returned here. The cursor is the last key sent,
//! HMAC-signed so a client can't forge one. A read never serves keys from the last
//! [`SETTLE_US`]: a write whose key was taken just before another's but which lands
//! just after could otherwise be skipped by a client that already moved past it.
//!
//! Soft-deleted rows are kept forever, so a cursor never outlives its tombstones and
//! `410 cursor_expired` only means the cursor is unusable: malformed, tampered with,
//! from another version of the format, or from another database.
//!
//! Open to any member signed in to Tally — the directory is statewide, so every caller
//! sees the same feed. Users on their own, kiosks, API tokens and edit links are refused
//! with 401; only a Tally sign-in grant is a member, so an MCP grant can't read it. No reply is a 403 or 404, which CloudFront would replace with
//! the web app's index.html. Framework-agnostic like `oauth_http.rs`: both `server.rs` and the Lambda
//! handler wrap [`sync`].

use std::collections::{HashMap, HashSet};
use std::io::Write;

use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tracing::{error, info};

use crate::app::{App, HasDb, HasQueues};
use crate::auth::{self, AuthError, AuthInfo};
use crate::client_info::ClientReport;
use crate::db::{self, Handler};

pub const SYNC_PATH: &str = "/api/v1/contacts/sync";

const DEFAULT_LIMIT: usize = 1000;
const MAX_LIMIT: usize = 2000;
const CURSOR_VERSION: u32 = 1;
/// How far behind "now" a read stays, in microseconds. A write takes its key from the
/// clock moments before DynamoDB applies it, and Lambda clocks agree to well under a
/// second; 30s covers both with room to spare.
const SETTLE_US: u64 = 30_000_000;

/// A response with a byte body (it may be gzip-compressed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Cursor {
    v: u32,
    /// The `contact_key` of the last row sent, or the upper bound of an empty read.
    k: String,
    /// `DB_PREFIX`: a cursor only means anything against the database it came from.
    e: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Contact<'a> {
    id: &'a str,
    first_name: &'a str,
    last_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    member_number: Option<&'a str>,
    location_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    unit: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<u64>,
}

#[derive(Serialize)]
struct Tombstone<'a> {
    id: &'a str,
    deleted: bool,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Entry<'a> {
    Contact(Contact<'a>),
    Tombstone(Tombstone<'a>),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncResponse<'a> {
    contacts: Vec<Entry<'a>>,
    next_cursor: String,
    has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    total: Option<u64>,
}

/// Handle `GET /api/v1/contacts/sync`. `query` is the raw query string (no `?`),
/// `accept_encoding` the request's `Accept-Encoding` header.
pub async fn sync<A: App + HasDb + HasQueues>(
    app: &A,
    host: Option<&str>,
    authorization: Option<&str>,
    accept_encoding: Option<&str>,
    query: &str,
) -> Reply {
    let Some(token) = authorization.and_then(|h| h.strip_prefix("Bearer ")) else {
        return unauthorized();
    };
    let person_id = match auth::verify_token(app, token, &ClientReport::default(), host).await {
        Ok(AuthInfo::Member { person_id, .. }) => person_id,
        // 401, not the 403 the Tally brief suggests: CloudFront rewrites every 403
        // on the site's distribution to the SPA's index.html (see web_*.tf). Tally
        // only ever presents its own members' tokens, so it never sees this.
        Ok(_) => return unauthorized(),
        Err(AuthError::Permanent(msg)) => {
            info!("contacts: auth rejected: {msg}");
            return unauthorized();
        }
        Err(AuthError::Transient(msg)) => {
            error!("contacts: transient auth error: {msg}");
            return error_reply(503, "unavailable");
        }
    };

    let params = parse_query(query);
    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1, MAX_LIMIT);
    let key = app.jwt().contacts_cursor_key();
    let env = db_prefix();
    let after = match params.get("cursor").filter(|c| !c.is_empty()) {
        None => None,
        Some(raw) => match decode_cursor(&key, raw) {
            Some(c) if c.v == CURSOR_VERSION && c.e == env => Some(c.k),
            _ => return error_reply(410, "cursor_expired"),
        },
    };
    let full_sync = after.is_none();
    let before = format!("{:020}", crate::clock::now_us().saturating_sub(SETTLE_US));

    let db = app.db();
    // A full sync skips tombstones: the client removes whatever it didn't receive.
    let page = match db
        .list_contact_changes(after.as_deref(), &before, !full_sync, limit)
        .await
    {
        Ok(page) => page,
        Err(e) => return db_failure("list contact changes", e),
    };
    // Only a full sync's first page has no cursor; later pages carry one.
    let total = if full_sync {
        match db.count_contacts(&before).await {
            Ok(n) => Some(n),
            Err(e) => return db_failure("count contacts", e),
        }
    } else {
        None
    };

    let location_ids: Vec<&str> = page
        .changes
        .iter()
        .filter(|c| c.person.deleted.is_none())
        .map(|c| c.person.location_id.as_str())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let locations: HashMap<String, db::Location> = match db.get_locations(&location_ids).await {
        Ok(rows) => rows
            .into_iter()
            .flatten()
            .map(|l| (l.id.clone(), l))
            .collect(),
        Err(e) => return db_failure("fetch locations", e),
    };

    let contacts = page
        .changes
        .iter()
        .map(|c| {
            let p = &c.person;
            if p.deleted.is_some() {
                Entry::Tombstone(Tombstone {
                    id: &p.id,
                    deleted: true,
                })
            } else {
                Entry::Contact(Contact {
                    id: &p.id,
                    first_name: &p.first_name,
                    last_name: &p.last_name,
                    member_number: p.registration_number.as_deref(),
                    location_id: &p.location_id,
                    unit: locations.get(&p.location_id).map(|l| l.name.as_str()),
                    email: p.email.as_deref(),
                    updated_at: p.updated_at,
                })
            }
        })
        .collect::<Vec<_>>();

    // Past the last row sent; with nothing sent, past everything this read covered
    // (a delta that found nothing keeps its place, an empty full sync starts at the
    // bound so its next call is a delta).
    // A cursor ahead of `before` (clock skew between Lambdas) is kept, never moved back.
    let next_key = match page.changes.last() {
        Some(last) => last.key.clone(),
        None => after.clone().unwrap_or(before),
    };
    let next_cursor = encode_cursor(
        &key,
        &Cursor {
            v: CURSOR_VERSION,
            k: next_key,
            e: env,
        },
    );

    if full_sync {
        info!(
            person_id,
            count = contacts.len(),
            has_more = page.has_more,
            "contacts: full sync page"
        );
    }

    let body = serde_json::to_vec(&SyncResponse {
        contacts,
        next_cursor,
        has_more: page.has_more,
        total,
    })
    .expect("contacts response is a plain serde struct");
    json_reply(200, body, accept_encoding)
}

fn db_prefix() -> String {
    std::env::var("DB_PREFIX").unwrap_or_default()
}

fn parse_query(query: &str) -> HashMap<String, String> {
    url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect()
}

fn encode_cursor(key: &[u8], cursor: &Cursor) -> String {
    let payload = serde_json::to_vec(cursor).expect("Cursor always serializes");
    let payload_b64 = BASE64_URL_SAFE_NO_PAD.encode(payload);
    let sig_b64 = BASE64_URL_SAFE_NO_PAD.encode(hmac_sha256(key, payload_b64.as_bytes()));
    format!("{payload_b64}.{sig_b64}")
}

/// `None` for anything malformed or not signed with `key`.
fn decode_cursor(key: &[u8], raw: &str) -> Option<Cursor> {
    let (payload_b64, sig_b64) = raw.split_once('.')?;
    let given = BASE64_URL_SAFE_NO_PAD.decode(sig_b64).ok()?;
    let expected = hmac_sha256(key, payload_b64.as_bytes());
    if !crate::oauth::constant_time_eq(&expected, &given) {
        return None;
    }
    serde_json::from_slice(&BASE64_URL_SAFE_NO_PAD.decode(payload_b64).ok()?).ok()
}

fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut mac =
        <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC accepts a key of any length");
    mac.update(msg);
    mac.finalize().into_bytes().into()
}

fn db_failure(what: &str, e: db::Error) -> Reply {
    error!("contacts: {what}: {e}");
    error_reply(500, "server_error")
}

fn unauthorized() -> Reply {
    let mut reply = error_reply(401, "unauthorized");
    reply
        .headers
        .push(("WWW-Authenticate".to_string(), "Bearer".to_string()));
    reply
}

fn error_reply(status: u16, error: &str) -> Reply {
    json_reply(
        status,
        serde_json::to_vec(&serde_json::json!({ "error": error }))
            .expect("error body always serializes"),
        None,
    )
}

/// gzip the body when the client accepts it. A full sync is ~3MB of JSON per ~12k
/// members, which compresses roughly tenfold.
fn json_reply(status: u16, body: Vec<u8>, accept_encoding: Option<&str>) -> Reply {
    let mut headers = vec![
        ("Content-Type".to_string(), "application/json".to_string()),
        ("Cache-Control".to_string(), "no-store".to_string()),
    ];
    let accepts_gzip = accept_encoding.is_some_and(|h| {
        h.split(',')
            .any(|enc| enc.split(';').next().unwrap_or("").trim() == "gzip")
    });
    let body = if accepts_gzip {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        match encoder.write_all(&body).and_then(|_| encoder.finish()) {
            Ok(gz) => {
                headers.push(("Content-Encoding".to_string(), "gzip".to_string()));
                gz
            }
            // Writing to a Vec can't fail; if it somehow did, send it uncompressed.
            Err(_) => body,
        }
    } else {
        body
    };
    headers.push(("Vary".to_string(), "Accept-Encoding".to_string()));
    Reply {
        status,
        headers,
        body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursors_round_trip_and_reject_tampering() {
        let key = [7u8; 32];
        let cursor = Cursor {
            v: 1,
            k: "00000000000000000001#abc".into(),
            e: "seslogin_test".into(),
        };
        let raw = encode_cursor(&key, &cursor);
        assert_eq!(decode_cursor(&key, &raw), Some(cursor));
        assert_eq!(decode_cursor(&[8u8; 32], &raw), None);
        let (payload, sig) = raw.split_once('.').unwrap();
        let forged = BASE64_URL_SAFE_NO_PAD.encode(br#"{"v":1,"k":"0","e":"seslogin_test"}"#);
        assert_eq!(decode_cursor(&key, &format!("{forged}.{sig}")), None);
        assert_eq!(decode_cursor(&key, payload), None);
        assert_eq!(decode_cursor(&key, "garbage"), None);
    }

    #[test]
    fn gzip_only_when_accepted() {
        let body = br#"{"contacts":[]}"#.to_vec();
        let plain = json_reply(200, body.clone(), Some("br, deflate"));
        assert_eq!(plain.body, body);
        let gz = json_reply(200, body.clone(), Some("deflate, gzip;q=0.9"));
        assert!(
            gz.headers
                .contains(&("Content-Encoding".into(), "gzip".into()))
        );
        let mut out = Vec::new();
        std::io::Read::read_to_end(&mut flate2::read::GzDecoder::new(&gz.body[..]), &mut out)
            .unwrap();
        assert_eq!(out, body);
    }
}
