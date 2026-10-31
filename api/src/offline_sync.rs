//! `GET /api/v1/sync`: Tally's offline sync, a cursor-based change feed of the data the
//! app keeps on the device: the statewide member directory (`contacts`) and the unit
//! directory (`units`), so the app has one sync to run and the API one to maintain.
//!
//! Contacts are a change feed:
//!
//! - No cursor: every live member, page by page (a full sync).
//! - With a cursor: only what changed since it — upserts, and `{id, deleted: true}`
//!   tombstones for soft-deleted members — page by page.
//! - Every response carries a `nextCursor`, the last page's included.
//!
//! An upsert is the whole contact: an optional field or list it leaves out is empty, so
//! the client replaces what it holds rather than merging. A contact carries the member's
//! directory details (preferred name, mobile, every phone number), their SES capability
//! tags and their unit assignments (`person_unit`, primary first, then by name).
//!
//! Units are small (one per enabled location, ~280) and change rarely, so they are not a
//! feed: a response carries the whole list, by name, whenever it differs from the one
//! the cursor last delivered, and leaves `units` out otherwise. The cursor records a
//! hash of the list it was issued with, so a full sync's first page always has them and
//! the pages after it don't. Each unit is what location sync stored for the location
//! (code, zone, coordinates, addresses, phone, email, accreditations), any of which may be
//! missing.
//!
//! Shared contacts (`sharedContacts`, see SCHEMA.md's `shared_contact`) work the same
//! way as units: the whole list, whenever it differs from the one the cursor last
//! delivered. It is the caller's own, not statewide: every statewide contact, plus the
//! contacts of each location the caller may use in Tally (their home unit and their
//! user's units, or every location for a super user), the same set GraphQL lets them
//! read. So the list, and the hash the cursor keeps of it, differ between members, and
//! a member who gains or loses a unit gets the new list on their next sync.
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

use async_graphql::futures_util::stream::{self, StreamExt};

use crate::app::{App, HasDb, HasQueues};
use crate::auth::{self, AuthError, AuthInfo};
use crate::db::{self, Handler};
use crate::graphql::ses_details;

pub const SYNC_PATH: &str = "/api/v1/sync";

const DEFAULT_LIMIT: usize = 1000;
const MAX_LIMIT: usize = 2000;
/// How many members' `person_unit` rows are read at once. The table has no batch read by
/// member, so each member on a page is one query.
const UNIT_READ_CONCURRENCY: usize = 32;
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
    /// The [`units_version`] of the units list already delivered, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    u: Option<String>,
    /// The [`list_version`] of the shared contacts already delivered, if any. Cursors
    /// from before shared contacts were synced have none, so get the list once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    s: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Contact<'a> {
    id: &'a str,
    first_name: &'a str,
    last_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    preferred_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    member_number: Option<&'a str>,
    location_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    unit: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mobile: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    phones: Vec<Phone>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    capability_tags: Vec<CapabilityTag>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    units: Vec<UnitAssignment<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<u64>,
}

/// A phone number from the SES directory, with its contact types (Primary Telephone,
/// Personal Mobile Phone, SMS, Work Mobile Phone, ...).
#[derive(Serialize)]
struct Phone {
    number: String,
    types: Vec<String>,
}

/// An SES capability tag (qualification), such as Job Ready, a flood rescue level,
/// driver or chainsaw.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CapabilityTag {
    id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

/// A directory entity the member is assigned to: a unit, cluster, Community Action
/// Team, zone capability unit or staff section.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UnitAssignment<'a> {
    entity_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<&'a str>,
    /// The location this entity is, when it is one of ours.
    #[serde(skip_serializing_if = "Option::is_none")]
    location_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    zone: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    membership_type: Option<&'a str>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    ranks: &'a [String],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    roles: &'a [String],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    positions: &'a [String],
    is_primary: bool,
}

#[derive(Serialize)]
struct Tombstone<'a> {
    id: &'a str,
    deleted: bool,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Entry<'a> {
    Contact(Box<Contact<'a>>),
    Tombstone(Tombstone<'a>),
}

/// One enabled location, as location sync stored it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Unit<'a> {
    id: &'a str,
    name: &'a str,
    /// The SES code, e.g. `HOR`.
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<&'a str>,
    /// The `id` of the unit's zone, itself in the list. Units only.
    #[serde(skip_serializing_if = "Option::is_none")]
    zone_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<Address<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    postal_address: Option<Address<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    longitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    accreditations: Vec<Accreditation>,
}

/// A directory address; `formatted` is the whole address on one line.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Address<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    line1: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line2: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suburb: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    postcode: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    formatted: Option<&'a str>,
}

/// One of a unit's rescue accreditations. "Not Applicable" rows are kept, so "not
/// applicable" and "no record" stay distinct.
#[derive(Serialize)]
struct Accreditation {
    #[serde(rename = "type")]
    accreditation_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

/// A contact added by hand: a location's (`locationId`) or statewide (no `locationId`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SharedContact<'a> {
    id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    location_id: Option<&'a str>,
    name: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    description: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<&'a str>,
    phone_can_sms: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<&'a str>,
    updated_at: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncResponse<'a> {
    contacts: Vec<Entry<'a>>,
    /// Every unit, when the list differs from the one the cursor last delivered.
    #[serde(skip_serializing_if = "Option::is_none")]
    units: Option<&'a [Unit<'a>]>,
    /// The caller's shared contacts, when they differ from those the cursor last
    /// delivered.
    #[serde(skip_serializing_if = "Option::is_none")]
    shared_contacts: Option<&'a [SharedContact<'a>]>,
    next_cursor: String,
    has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    total: Option<u64>,
}

/// Handle `GET /api/v1/sync`. `query` is the raw query string (no `?`),
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
    let (person_id, scope) = match auth::verify_token(app, token, host).await {
        Ok(AuthInfo::Member {
            person_id,
            location_id,
            is_super,
            user_location_ids,
            ..
        }) => {
            let scope = ContactScope {
                is_super,
                locations: std::iter::once(location_id)
                    .chain(user_location_ids)
                    .collect(),
            };
            (person_id, scope)
        }
        // 401, not the 403 the Tally brief suggests: CloudFront rewrites every 403
        // on the site's distribution to the SPA's index.html (see web_*.tf). Tally
        // only ever presents its own members' tokens, so it never sees this.
        Ok(_) => return unauthorized(),
        Err(AuthError::Permanent(msg)) => {
            info!("sync: auth rejected: {msg}");
            return unauthorized();
        }
        Err(AuthError::Transient(msg)) => {
            error!("sync: transient auth error: {msg}");
            return error_reply(503, "unavailable");
        }
    };

    let params = parse_query(query);
    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(DEFAULT_LIMIT)
        .clamp(1, MAX_LIMIT);
    let key = app.server_secret().sync_cursor_key();
    let env = db_prefix();
    let after = match params.get("cursor").filter(|c| !c.is_empty()) {
        None => None,
        Some(raw) => match decode_cursor(&key, raw) {
            Some(c) if c.v == CURSOR_VERSION && c.e == env => Some(c),
            _ => return error_reply(410, "cursor_expired"),
        },
    };
    let (after, units_delivered, contacts_delivered) = match after {
        Some(c) => (Some(c.k), c.u, c.s),
        None => (None, None, None),
    };
    let full_sync = after.is_none();
    let before = format!("{:020}", crate::clock::now_us().saturating_sub(SETTLE_US));

    let db = app.db();
    let mut directory = match db
        .list_locations(db::ListLocationsFilter::EnabledOnly)
        .await
    {
        Ok(rows) => rows,
        Err(e) => return db_failure("list locations", e),
    };
    directory.retain(|l| l.enabled);
    directory.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    let directory_units: Vec<Unit> = directory.iter().map(unit).collect();
    let version = units_version(&directory_units);
    let send_units = units_delivered.as_deref() != Some(version.as_str());

    // Shared contacts are a newer, smaller part of the feed: failing to read them (the
    // table not created yet, say) must not stop the member directory syncing. The
    // response then leaves them out and the cursor keeps the hash it came with, so the
    // next sync tries again.
    let shared = match db.list_all_shared_contacts().await {
        Ok(mut rows) => {
            rows.retain(|c| scope.allows(c.location_id.as_deref()));
            rows.sort_by(|a, b| {
                a.name
                    .to_lowercase()
                    .cmp(&b.name.to_lowercase())
                    .then_with(|| a.id.cmp(&b.id))
            });
            Some(rows)
        }
        Err(e) => {
            error!("sync: list shared contacts failed, leaving them out: {e}");
            None
        }
    };
    let shared_contacts: Option<Vec<SharedContact>> = shared
        .as_ref()
        .map(|rows| rows.iter().map(shared_contact).collect());
    let contacts_version = match &shared_contacts {
        Some(list) => Some(list_version(list)),
        None => contacts_delivered.clone(),
    };
    let send_contacts = shared_contacts.is_some() && contacts_delivered != contacts_version;

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
    let units = match person_units(db, &page.changes).await {
        Ok(units) => units,
        Err(e) => return db_failure("list person units", e),
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
                Entry::Contact(Box::new(Contact {
                    id: &p.id,
                    first_name: &p.first_name,
                    last_name: &p.last_name,
                    preferred_name: p.preferred_name.as_deref(),
                    member_number: p.registration_number.as_deref(),
                    location_id: &p.location_id,
                    unit: locations.get(&p.location_id).map(|l| l.name.as_str()),
                    email: p.email.as_deref(),
                    mobile: p.mobile.as_deref(),
                    phones: ses_details::read_phones(&p.id, p.phones.as_deref())
                        .into_iter()
                        .map(|ph| Phone {
                            number: ph.number,
                            types: ph.types,
                        })
                        .collect(),
                    capability_tags: ses_details::read_capability_tags(
                        &p.id,
                        p.capability_tags.as_deref(),
                    )
                    .into_iter()
                    .map(|t| CapabilityTag {
                        id: t.id,
                        name: t.name,
                        description: t.description,
                    })
                    .collect(),
                    units: units
                        .get(&p.id)
                        .map(|rows| rows.iter().map(unit_assignment).collect())
                        .unwrap_or_default(),
                    updated_at: p.updated_at,
                }))
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
            u: Some(version),
            s: contacts_version,
        },
    );

    if full_sync {
        info!(
            person_id,
            count = contacts.len(),
            has_more = page.has_more,
            "sync: full sync page"
        );
    }

    let body = serde_json::to_vec(&SyncResponse {
        contacts,
        units: send_units.then_some(&directory_units[..]),
        shared_contacts: shared_contacts.as_deref().filter(|_| send_contacts),
        next_cursor,
        has_more: page.has_more,
        total,
    })
    .expect("sync response is a plain serde struct");
    json_reply(200, body, accept_encoding)
}

/// The `person_unit` rows of every live member on the page, each member's primary
/// first and then by name.
async fn person_units<D: Handler>(
    db: &D,
    changes: &[db::ContactChange],
) -> db::Result<HashMap<String, Vec<db::PersonUnit>>> {
    // Owned ids: a stream over borrowed ones trips the compiler's higher-ranked
    // lifetime check inside the poem handler's future.
    let live: Vec<String> = changes
        .iter()
        .filter(|c| c.person.deleted.is_none())
        .map(|c| c.person.id.clone())
        .collect();
    let mut reads = stream::iter(live)
        .map(|id: String| async move {
            let units = db.list_person_units(&id).await;
            (id, units)
        })
        .buffer_unordered(UNIT_READ_CONCURRENCY);
    let mut by_person = HashMap::new();
    while let Some((id, units)) = reads.next().await {
        let mut units = units?;
        if units.is_empty() {
            continue;
        }
        units.sort_by(|a, b| {
            b.is_primary
                .cmp(&a.is_primary)
                .then_with(|| {
                    a.entity_name
                        .as_deref()
                        .unwrap_or_default()
                        .to_lowercase()
                        .cmp(&b.entity_name.as_deref().unwrap_or_default().to_lowercase())
                })
                .then_with(|| a.entity_id.cmp(&b.entity_id))
        });
        by_person.insert(id, units);
    }
    Ok(by_person)
}

fn unit_assignment(u: &db::PersonUnit) -> UnitAssignment<'_> {
    UnitAssignment {
        entity_id: &u.entity_id,
        name: u.entity_name.as_deref(),
        code: u.entity_code.as_deref(),
        location_id: u.location_id.as_deref(),
        zone: u.zone.as_deref(),
        membership_type: u.membership_type.as_deref(),
        ranks: &u.ranks,
        roles: &u.roles,
        positions: &u.positions,
        is_primary: u.is_primary,
    }
}

fn address(a: &db::LocationAddress) -> Address<'_> {
    Address {
        line1: a.line1.as_deref(),
        line2: a.line2.as_deref(),
        suburb: a.suburb.as_deref(),
        state: a.state.as_deref(),
        postcode: a.postcode.as_deref(),
        formatted: a.formatted.as_deref(),
    }
}

fn unit(l: &db::Location) -> Unit<'_> {
    Unit {
        id: &l.id,
        name: &l.name,
        code: l.ses_details.code.as_deref(),
        zone_id: l.ses_details.zone_location_id.as_deref(),
        address: l.directory_details.address.as_ref().map(address),
        postal_address: l.directory_details.postal_address.as_ref().map(address),
        latitude: l.ses_details.latitude,
        longitude: l.ses_details.longitude,
        phone: l.directory_details.phone.as_deref(),
        email: l.directory_details.email.as_deref(),
        accreditations: ses_details::read_accreditations(&l.id, l.accreditations.as_deref())
            .into_iter()
            .map(|a| Accreditation {
                accreditation_type: a.accreditation_type,
                status: a.status,
                number: a.number,
                note: a.note,
            })
            .collect(),
    }
}

/// Whose shared contacts a member is sent: the same as `require_shared_contact_access`
/// lets them read.
struct ContactScope {
    is_super: bool,
    /// Their home unit and their user's units, either role.
    locations: HashSet<String>,
}

impl ContactScope {
    fn allows(&self, location_id: Option<&str>) -> bool {
        match location_id {
            None => true,
            Some(id) => self.is_super || self.locations.contains(id),
        }
    }
}

fn shared_contact(c: &db::SharedContact) -> SharedContact<'_> {
    SharedContact {
        id: &c.id,
        location_id: c.location_id.as_deref(),
        name: &c.name,
        description: &c.description,
        phone: c.phone.as_deref(),
        phone_can_sms: c.phone_can_sms,
        email: c.email.as_deref(),
        updated_at: c.updated_at,
    }
}

/// A short hash of the units list as it is serialized: equal exactly when the list is.
fn units_version(units: &[Unit]) -> String {
    list_version(units)
}

/// A short hash of a list as it is serialized: equal exactly when the list is.
fn list_version<T: Serialize>(items: &[T]) -> String {
    use sha2::Digest;
    let json = serde_json::to_vec(items).expect("sync lists are plain serde structs");
    BASE64_URL_SAFE_NO_PAD.encode(&Sha256::digest(&json)[..12])
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
    error!("sync: {what}: {e}");
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

/// gzip the body when the client accepts it. A full sync of ~12k members, with their
/// phones, tags and units, is several MB of JSON, which compresses roughly tenfold.
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
            u: Some("abc".into()),
            s: Some("def".into()),
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
