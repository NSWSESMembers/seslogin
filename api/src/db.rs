use chrono::NaiveDate;
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use std::future::Future;
use thiserror::Error;

const NANOID_ALPHABET: [char; 62] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I',
    'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b',
    'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u',
    'v', 'w', 'x', 'y', 'z',
];

/// Generate a new unique ID for DB entities. Public so callers that need IDs
/// before the record exists (e.g. `cli id`, for pre-allocating an ID to reuse
/// verbatim across databases) use the exact same scheme rather than a
/// hand-rolled one that could silently drift from it.
///
/// Lives here rather than in `dynamodb.rs` so code that is independent of the AWS
/// implementation (the audit wrapper) can mint IDs too; `dynamodb::new_id` re-exports it.
/// How a person's email is compared when looking them up by it: trimmed and
/// lowercased, since SES and the person typing it may not agree on case. Stored as the
/// person's `email_key`, which backs the `email_key-index`.
pub fn email_key(email: &str) -> String {
    email.trim().to_lowercase()
}

pub fn new_id() -> String {
    // https://alex7kom.github.io/nano-nanoid-cc/?alphabet=0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz&size=12&speed=1000&speedUnit=hour
    nanoid!(12, &NANOID_ALPHABET)
}

/// these errors are separated into groups because we want to handle them differently
#[derive(Error, Debug)]
pub enum Error {
    /// returned when a queried record does not exist
    #[error("Record not found: {0}")]
    NotFound(String),
    /// returned when a DB row cannot be deserialized into the expected type
    #[error("Hydration error: {0}")]
    Hydration(String),
    /// this is an unexpected error, probably fine to ignore and retry
    #[error("Infrastructure error: {0}")]
    Infrastructure(String),
    /// returned when a row violates an expected data-integrity invariant
    #[error("Data integrity error: {0}")]
    Integrity(String),
    /// type conversion error ie convert string ID to integer
    #[error("Data type conversion error: {0}")]
    TypeConversion(String),
    /// returned when a conditional write finds the row changed since the caller read it
    /// (a stale expected version, a period already ended, or a member's badge state)
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Mutation disabled")]
    MutationDisabled,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Collapse the results of a lookup on an attribute that is *expected* to be unique
/// (but not enforced as unique by the data model) down to at most one row. Returns an
/// `Integrity` error if more than one row shares the attribute, so callers that assume
/// uniqueness fail loudly rather than silently picking an arbitrary match. The CLI
/// deliberately bypasses this to print every matching row when debugging duplicates.
pub fn at_most_one<T>(mut matches: Vec<T>, describe: impl FnOnce() -> String) -> Result<Option<T>> {
    if matches.len() > 1 {
        return Err(Error::Integrity(describe()));
    }
    Ok(matches.pop())
}

pub enum ListSessionsQuery {
    ByLocation(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeriodCursor {
    pub start_time: u64,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListPeriodsPage {
    pub after: Option<PeriodCursor>,
    pub before: Option<PeriodCursor>,
    pub limit: i32,
    pub descending: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TestPaginationRow {
    pub id: String,
    pub group_id: i64,
    pub number: i64,
    pub name: String,
    pub odd: Option<i64>,
    pub even: Option<String>,
    pub mod5: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TestPaginationFilter {
    OddOnly,
    EvenOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestPaginationCursor {
    pub number: i64,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListTestPaginationPage {
    pub after: Option<TestPaginationCursor>,
    pub before: Option<TestPaginationCursor>,
    pub limit: i32,
    /// true → scan descending (highest number first); false → ascending
    pub descending: bool,
    pub filter: Option<TestPaginationFilter>,
}

pub trait HasID {
    fn id(&self) -> &str;
}

#[derive(Clone, Debug, PartialEq)]
pub struct User {
    pub id: String,
    pub email: String,
    pub is_super: bool,
    pub is_dev: bool,
    pub enabled: bool,
    /// Locations where the user is an Admin (read and write).
    pub location_grants: Vec<String>,
    /// Locations where the user is Read only: they can view but not change.
    /// Disjoint from `location_grants`.
    pub location_read_only_grants: Vec<String>,
    /// The member record this user acts as when signing in or out from the mobile
    /// app. Set by a super user; any number of users may share one member.
    pub person_id: Option<String>,
    pub access_time: Option<u64>,
    pub email_config: serde_json::Map<String, serde_json::Value>,
    pub disaggregate_virtual_periods: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for User {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum UserUpdateShape<'a> {
    Fields {
        email: &'a str,
        is_super: bool,
        is_dev: bool,
        enabled: bool,
        location_grants: Vec<String>,
        /// `None` leaves the stored read-only grants unchanged.
        location_read_only_grants: Option<Vec<String>>,
    },
    AccessTime,
    EmailConfig {
        email_config: serde_json::Map<String, serde_json::Value>,
    },
    DisaggregateVirtualPeriods {
        value: bool,
    },
    /// Set (`Some`) or clear (`None`) the member the user acts as.
    Person {
        person_id: Option<&'a str>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Person {
    pub id: String,
    pub location_id: String,
    pub first_name: String,
    pub last_name: String,
    pub registration_number: Option<String>,
    pub ses_api_person_id: Option<String>,
    pub email: Option<String>,
    /// The member's first Australian mobile number from the SES directory, E.164.
    pub mobile: Option<String>,
    /// Every distinct phone number the SES directory lists for the member, exactly as
    /// stored: JSON text, `[{"number", "types": [..]}]`, the mobile first.
    pub phones: Option<String>,
    /// The SES directory's preferred name, when set. Stored, not yet displayed.
    pub preferred_name: Option<String>,
    /// Badge counters and awards, exactly as stored: the JSON text `badges::read_state`
    /// parses. Kept unparsed so that loading a person can never fail on it, and so a
    /// badge write can be made conditional on it being unchanged since it was read.
    /// `None` for a member who has never been evaluated, and for every member of a
    /// location that has not opted into gamification.
    pub badge_state: Option<String>,
    /// The member's SES capability tags (qualifications), exactly as stored: JSON text,
    /// `[{"id", "name", "description"}]` sorted by id. Written only by member sync;
    /// `None` when SES lists none.
    pub capability_tags: Option<String>,
    pub deleted: Option<u64>,
    /// Unix seconds at which member sync first observed this person missing from their
    /// location's SES payload. Cleared by any sync that sees them again, and on
    /// soft-delete — so a marker only ever exists on a live row. Once it is older than
    /// the configured grace window the person is soft-deleted.
    pub missing_since: Option<u64>,
    /// The other units the member is a volunteer at, by location id, sorted: those of
    /// their `person_unit` rows that are one of our locations and carry a Volunteer
    /// assignment (`membership_type`), less `location_id` as it stood when written.
    /// Written only by directory sync (`PersonUpdateShape::Directory`); empty when there
    /// are none. Read it through [`Person::member_location_ids`] or
    /// [`Person::is_member_of`], which add the home unit: member sync can move
    /// `location_id` between directory syncs, so this set may name the home unit too.
    pub secondary_location_ids: Vec<String>,
    pub created_at: Option<u64>,
    pub updated_at: Option<u64>,
}

impl HasID for Person {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Person {
    /// Every unit the member is a member of: their home unit (`location_id`) first,
    /// then their [secondary units](Person::secondary_location_ids), without repeats.
    pub fn member_location_ids(&self) -> Vec<&str> {
        let mut ids = vec![self.location_id.as_str()];
        for id in &self.secondary_location_ids {
            if !ids.contains(&id.as_str()) {
                ids.push(id);
            }
        }
        ids
    }

    /// Whether the member is a member of `location_id`: it is their home unit or one of
    /// their secondary units.
    pub fn is_member_of(&self, location_id: &str) -> bool {
        self.location_id == location_id
            || self.secondary_location_ids.iter().any(|l| l == location_id)
    }
}

/// One row of the contacts feed: a person and the `contact_key` that orders them.
/// See [`Handler::list_contact_changes`].
#[derive(Clone, Debug, PartialEq)]
pub struct ContactChange {
    pub key: String,
    pub person: Person,
}

/// A page of [`ContactChange`]s, oldest change first.
#[derive(Clone, Debug, PartialEq)]
pub struct ContactPage {
    pub changes: Vec<ContactChange>,
    /// Whether more rows lie between the last one returned and `before`.
    pub has_more: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PersonUpdateShape<'a> {
    Fields {
        first_name: &'a str,
        last_name: &'a str,
        registration_number: &'a str,
    },
    Location {
        location_id: &'a str,
    },
    SesApiPersonId {
        ses_api_person_id: Option<&'a str>,
    },
    /// Written only by directory sync: replaces all five directory fields, removing
    /// each one that is `None` (or, for `secondary_location_ids`, empty).
    Directory {
        email: Option<&'a str>,
        mobile: Option<&'a str>,
        phones: Option<&'a str>,
        preferred_name: Option<&'a str>,
        secondary_location_ids: &'a [String],
    },
    /// Replaces the capability tags JSON text; `None` removes it.
    CapabilityTags {
        capability_tags: Option<&'a str>,
    },
    /// `Some` stamps the missing marker, `None` removes it.
    MissingSince {
        missing_since: Option<u64>,
    },
    /// Replaces the whole badge state, but only if the stored state is still exactly
    /// `expected` (`None`: there is none), failing with [`Error::Conflict`] otherwise, so
    /// two writers racing on one member cannot overwrite each other's awards.
    /// `badge_state: None` removes the attribute, so a member who has earned nothing
    /// carries no badge data at all.
    BadgeState {
        badge_state: Option<&'a str>,
        expected: Option<&'a str>,
    },
    Undelete,
    Delete,
}

/// One SES directory entity (a unit, cluster, team, section, ...) a member is assigned
/// to, with everything they hold there. Written only by directory sync.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PersonUnit {
    pub person_id: String,
    /// The directory `EntityId`.
    pub entity_id: String,
    /// The location this entity is, when it is one of ours.
    pub location_id: Option<String>,
    pub entity_name: Option<String>,
    pub entity_code: Option<String>,
    /// The zone the directory files the assignment under.
    pub zone: Option<String>,
    /// The Volunteer assignment's detail (Member, Probationary, Life, ...). `None` for a
    /// staff-only row.
    pub membership_type: Option<String>,
    pub ranks: Vec<String>,
    pub roles: Vec<String>,
    pub positions: Vec<String>,
    /// Whether `location_id` is the member's primary (`Person.location_id`) location.
    pub is_primary: bool,
    pub updated_at: Option<u64>,
}

impl PersonUnit {
    /// Equal in everything directory sync writes, ignoring `updated_at`.
    pub fn same_content(&self, other: &PersonUnit) -> bool {
        PersonUnit {
            updated_at: None,
            ..self.clone()
        } == PersonUnit {
            updated_at: None,
            ..other.clone()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub location_id: String,
    /// False once the session has been soft-deleted (the `active` marker removed).
    pub active: bool,
    pub last_contact: Option<u64>,
    pub client_version: Option<String>,
    /// What the client last told us about itself (build channel, origin, device). `None`
    /// for a kiosk that has never reported — every field within is independently optional.
    pub client_info: Option<crate::client_info::ClientInfo>,
    /// When `client_info` was last written, as Unix seconds. Distinct from `last_contact`:
    /// the two move together today, but a snapshot that stops being refreshed while the
    /// kiosk keeps checking in is exactly the anomaly worth being able to see.
    pub client_info_updated_at: Option<u64>,
    pub config: serde_json::Map<String, serde_json::Value>,
    pub healthcheck_url: Option<String>,
    /// Base64 (standard) SPKI DER of the kiosk's enrolled ECDSA P-256 public key.
    /// Every session is created with one (the QR/public-key flow); it is dropped only
    /// when the key is released or the session deleted.
    pub public_key: Option<String>,
    /// Hex SHA-256 of the SPKI DER above. Backs the `key_fingerprint-index` GSI, so it
    /// must be omitted (never written as Null) when absent.
    pub key_fingerprint: Option<String>,
    /// Unix seconds after which the enrolled key no longer authenticates; extended on
    /// each verified request. A kiosk offline past this must re-enroll.
    pub key_expires_at: Option<u64>,
    /// Unix seconds at which this session's key was released so its device could enroll
    /// into a new session (see [`SessionUpdateShape::ReleaseKey`]). Set on such a
    /// leftover, which can never come back online.
    ///
    /// Deliberately not a soft delete: the record keeps its `active` marker so it stays
    /// listed at its own location, showing the admins there that the kiosk was replaced
    /// rather than silently vanishing. It authenticates nothing —
    /// [`is_live`](Self::is_live) is the test every caller that means "a working kiosk"
    /// must use.
    pub key_released_at: Option<u64>,
    /// Unix seconds at which the daily retirement job hid this session, having found it
    /// unusable for longer than its grace period (see [`crate::session_retire`] and
    /// [`SessionUpdateShape::Retire`]). Such a row is soft-deleted like an admin delete —
    /// no `active`, no key, no code — and this stamp is what tells the two apart. The row
    /// itself stays, so its id still resolves to its name and location.
    pub retired_at: Option<u64>,
    /// Bumped by every [`update_session`](Handler::update_session) that changes what
    /// the kiosk shows or whether it can authenticate (`Fields`, `ReleaseKey`,
    /// `Delete`, `Retire`), so a kiosk can tell a stale realtime message or refetch from a fresh
    /// one. Heartbeats (`Info`) and reactivation (`ExtendKey`) leave it alone. A row
    /// written before this existed reads as 0, so its first bump (to 1) still counts
    /// as newer.
    pub version: u64,
    pub created_at: Option<u64>,
    pub updated_at: Option<u64>,
}

impl Session {
    /// Whether this record is a working kiosk: not soft-deleted, and not left over from a
    /// re-enrollment of its device. An expired kiosk still counts — it can be reactivated
    /// or re-enrolled, so it is dormant rather than finished.
    pub fn is_live(&self) -> bool {
        self.active && self.key_released_at.is_none()
    }
}

/// Parameters for enrolling a public key onto a new session (the QR/public-key flow).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionKeyParams<'a> {
    /// Base64 (standard) SPKI DER of the ECDSA P-256 public key.
    pub public_key: &'a str,
    /// Hex SHA-256 of the SPKI DER.
    pub fingerprint: &'a str,
    /// Unix seconds at which the key initially expires.
    pub expires_at: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApiToken {
    pub id: String,
    pub name: String,
    pub token_hash: String,
    pub location_grants: Vec<String>,
    pub read_only: bool,
    pub created_at: u64,
    pub created_by_user_id: String,
    pub expires_at: Option<u64>,
    pub revoked_at: Option<u64>,
    pub last_used_at: Option<u64>,
}

impl HasID for ApiToken {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApiTokenUpdateShape<'a> {
    Fields {
        name: &'a str,
        location_grants: Vec<String>,
        read_only: bool,
        expires_at: Option<u64>,
    },
    TouchLastUsed,
    Revoke,
}

impl HasID for Session {
    fn id(&self) -> &str {
        &self.id
    }
}

pub enum LocationUpdateShape<'a> {
    Fields {
        name: &'a str,
        enabled: bool,
        nitc_enabled: Option<u64>,
        /// `None` leaves the current preference untouched.
        nitc_complete_on_export: Option<bool>,
        /// `None` leaves the current preference untouched.
        gamification_enabled: Option<bool>,
    },
    LastSyncTime {
        time: u64,
    },
    Name {
        name: &'a str,
    },
    /// Written only by location sync. Replaces the SES API details, and the directory
    /// details too when `directory` is `Some`; `None` leaves the stored directory details
    /// alone, for a run whose directory fetch failed. Absent values are removed.
    SesDetails {
        details: &'a LocationSesDetails,
        directory: Option<&'a LocationDirectoryDetails>,
    },
    /// Written only by location sync. The JSON text of the unit's accreditation rows;
    /// `None` (no rows) removes the attribute.
    Accreditations {
        accreditations: Option<&'a str>,
    },
}

/// Where a table scan left off. Scanning the base table returns a `LastEvaluatedKey`
/// of just the primary key, and every scannable table here is hash-keyed on `id`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanCursor {
    pub last_id: String,
}

/// One page of a table scan.
///
/// Rows are hydrated independently: an `Err` — always [`Error::Hydration`], naming the
/// offending row — is one bad record, not a failed page, so a scan can survey a table
/// end to end and report every problem it finds.
///
/// **`rows` being empty does not mean the scan is finished.** DynamoDB's `Limit` counts
/// items *examined*, and a page can come back empty while more remain. Only
/// `next == None` ends the walk.
#[derive(Debug)]
pub struct ScanPage<T> {
    pub rows: Vec<Result<T>>,
    pub next: Option<ScanCursor>,
}

/// Which API tokens a listing should include. Revoked tokens have no `active`
/// attribute, so they are absent from `active-index` and only a scan finds them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListApiTokensFilter {
    ActiveOnly,
    All,
}

pub enum ListLocationsFilter {
    EnabledOnly,
    All,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SessionUpdateShape<'a> {
    Fields {
        name: &'a str,
        config: &'a serde_json::Map<String, serde_json::Value>,
        healthcheck_url: Option<&'a str>,
    },
    Info {
        client_version: Option<&'a str>,
        /// The client's latest self-description. `Some` is an authoritative snapshot —
        /// fields it leaves empty are removed from the record, so a kiosk that stops
        /// reporting one doesn't leave a stale value behind. `None` means the client
        /// reported nothing at all, and the stored snapshot is left exactly as it was.
        client_info: Option<&'a crate::client_info::ClientInfo>,
        /// When set, also bump `key_expires_at` to this value (the sliding 14-day
        /// window for key-enrolled sessions). Written in the same update as the
        /// throttled `last_contact` refresh, so it costs no extra write.
        extend_key_expires_at: Option<u64>,
    },
    /// Set `key_expires_at` outright (kiosk reactivation). Deliberately leaves
    /// `last_contact` alone, so the admin status dot keeps telling the truth and the
    /// throttle in `auth::touch_session` stays open for the kiosk's redeeming request.
    ExtendKey {
        expires_at: u64,
    },
    /// Drop the key fields from a session that is *not* being deleted, freeing its
    /// `key_fingerprint-index` slot so the same device can enroll into a fresh session,
    /// and stamp `key_released_at` to mark the leftover. Deliberately leaves `active`
    /// alone — that is what holds the record in `active-location_id-index` — so it stays
    /// visible to its own location's admins, who can tidy it up.
    ///
    /// Fails with [`Error::NotFound`] if the row no longer holds `fingerprint`, meaning
    /// the enrollment it was read for has been overtaken.
    ReleaseKey {
        fingerprint: &'a str,
    },
    Delete,
    /// Hide a session that has been unusable for longer than its grace period (see
    /// [`crate::session_retire`]): remove `active`, so it leaves `active-location_id-index`,
    /// clear any leftover `code` and key fields, and stamp `retired_at`.
    ///
    /// Conditional on the session still being in the state it was planned from, with
    /// the timestamp that `reason` names still before `unusable_before`. A kiosk that
    /// came back, or was reactivated, in the meantime has moved that timestamp on, so
    /// the write fails with [`Error::NotFound`] rather than retiring a working kiosk.
    Retire {
        reason: RetireReason,
        unusable_before: u64,
    },
}

/// Why a session is being retired, which also names the attribute that dates it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RetireReason {
    /// Its key was handed to a new session enrolled from the same device. Dated by
    /// `key_released_at`.
    KeyReleased,
    /// Its key lapsed after two weeks without contact and nobody reactivated it. Dated
    /// by `key_expires_at`.
    KeyExpired,
    /// A legacy kiosk with no key, whose session JWT has lapsed. Dated by
    /// `last_contact`, which no JWT outlives by more than its 14-day lifetime.
    LegacyIdle,
}

impl RetireReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::KeyReleased => "key released",
            Self::KeyExpired => "key expired",
            Self::LegacyIdle => "legacy idle",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Period {
    pub id: String,
    pub person_id: Option<String>,
    pub guest_name: Option<String>,
    /// Free-text comment on the period. Populated from the guest sign-in "reason"
    /// today, but intended for general use on any period.
    pub comment: Option<String>,
    pub location_id: String,
    pub category_id: Option<String>,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub signed_in_session_id: Option<String>,
    pub signed_out_session_id: Option<String>,
    pub version: u64,
    pub nitc_event_id: Option<String>,
    pub nitc_participant_id: Option<i64>,
    pub nitc_exported_version: Option<u64>,
    pub deleted: Option<u64>,
    pub created_at: Option<u64>,
    pub updated_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NitcEvent {
    pub id: String,
    pub location_id: String,
    pub nitc_group_id: String,
    pub event_date: NaiveDate,
    pub ses_api_nitc_id: Option<i64>,
    pub version: u64,
    pub synced_version: Option<u64>,
    pub created_at: Option<u64>,
    pub updated_at: Option<u64>,
}

impl HasID for NitcEvent {
    fn id(&self) -> &str {
        &self.id
    }
}

/// NITC topic group configuration: type, tags. Location fields are fetched separately.
#[derive(Clone, Debug)]
pub struct NitcGroup {
    pub id: String,
    pub nitc_type: String,
    pub nitc_tag_ids: Vec<i32>,
    pub created_at: Option<u64>,
    pub updated_at: Option<u64>,
}

impl HasID for NitcGroup {
    fn id(&self) -> &str {
        &self.id
    }
}

/// How often a recurring [`Task`] comes back once it is completed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskRecurrenceUnit {
    Days,
    Weeks,
}

impl TaskRecurrenceUnit {
    /// Stable string stored in the `recurrence_unit` attribute.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Days => "days",
            Self::Weeks => "weeks",
        }
    }

    /// Inverse of [`TaskRecurrenceUnit::as_str`].
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "days" => Some(Self::Days),
            "weeks" => Some(Self::Weeks),
            _ => None,
        }
    }
}

/// A recurring task comes back `every` days or weeks after it is completed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskRecurrence {
    pub every: u32,
    pub unit: TaskRecurrenceUnit,
}

/// One item on a location's shared to-do list ("Tasks"). See SCHEMA.md's `task` section.
///
/// Dates are Sydney-local calendar dates. `show_date` is the day the task starts
/// appearing: its `due_date` when it has one, otherwise the day it was created. A task
/// is `open` until it is completed (non-recurring) or deleted; a recurring task is
/// never completed for good, it moves its dates forward instead.
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub id: String,
    pub location_id: String,
    pub list_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub show_date: NaiveDate,
    pub recurrence: Option<TaskRecurrence>,
    /// Whether the task is still to be done, i.e. it carries `open_location_id` and so
    /// is in `open_location_id-show_date-index`.
    pub open: bool,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    /// When a non-recurring task was completed, and by which user.
    pub completed_at: Option<u64>,
    pub completed_by: Option<String>,
    /// When a recurring task was last completed, and by which user.
    pub last_completed_at: Option<u64>,
    pub last_completed_by: Option<String>,
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for Task {
    fn id(&self) -> &str {
        &self.id
    }
}

/// The fields of a task being created. See [`Task`].
#[derive(Clone, Debug, PartialEq)]
pub struct NewTask<'a> {
    pub location_id: &'a str,
    pub list_id: Option<&'a str>,
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub due_date: Option<NaiveDate>,
    pub show_date: NaiveDate,
    pub recurrence: Option<TaskRecurrence>,
    pub created_by: Option<&'a str>,
}

/// Which open tasks of a location to list, by `show_date`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskShowDates {
    /// Showing on or before this day: what is due now.
    OnOrBefore(NaiveDate),
    /// Showing only after this day: what is scheduled.
    After(NaiveDate),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListTasksQuery {
    /// Open tasks of a location, in `show_date` order.
    Open {
        location_id: String,
        show_dates: TaskShowDates,
    },
}

/// Every write to a task. Each one fails with [`Error::NotFound`] when the task does not
/// exist, is deleted or — for all but `Delete` — is no longer open or no longer at
/// `expected_version`, so two people completing the same task cannot both succeed.
/// Each returns the task as it is after the write.
#[derive(Clone, Debug, PartialEq)]
pub enum TaskUpdateShape<'a> {
    /// Replace the editable fields. Absent optionals are removed.
    Fields {
        list_id: Option<&'a str>,
        title: &'a str,
        description: Option<&'a str>,
        due_date: Option<NaiveDate>,
        show_date: NaiveDate,
        recurrence: Option<TaskRecurrence>,
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Complete one occurrence of a recurring task: it stays open and is due again on
    /// `next_date`.
    CompleteOccurrence {
        next_date: NaiveDate,
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Complete a non-recurring task for good: it stops being open.
    Complete {
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the task. It stops being open.
    Delete { by: Option<&'a str> },
}

/// A named list a location's tasks can be put on. Every task belongs to its location;
/// a list is an optional way to group some of them.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskList {
    pub id: String,
    pub location_id: String,
    pub name: String,
    pub created_by: Option<String>,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for TaskList {
    fn id(&self) -> &str {
        &self.id
    }
}

/// Every write to a task list. Each fails with [`Error::NotFound`] when the list does
/// not exist or is deleted, and returns the list as it is after the write. Deleting a
/// list leaves its tasks alone: they keep `list_id` and stay on their location.
#[derive(Clone, Debug, PartialEq)]
pub enum TaskListUpdateShape<'a> {
    Name { name: &'a str },
    Delete,
}

/// A note in a location's shared notebook ("Notes"). See SCHEMA.md's `note` section.
///
/// Every note belongs to its location; `folder_id` optionally files it in one of the
/// location's [`NoteFolder`]s. `content` is plain text for now (empty when absent).
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub id: String,
    pub location_id: String,
    pub folder_id: Option<String>,
    pub title: String,
    pub content: String,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for Note {
    fn id(&self) -> &str {
        &self.id
    }
}

/// The fields of a note being created. See [`Note`].
#[derive(Clone, Debug, PartialEq)]
pub struct NewNote<'a> {
    pub location_id: &'a str,
    pub folder_id: Option<&'a str>,
    pub title: &'a str,
    pub content: &'a str,
    pub created_by: Option<&'a str>,
}

/// Every write to a note. Each fails with [`Error::NotFound`] when the note does not
/// exist, is deleted or — for `Fields` — is no longer at `expected_version`, so two
/// people saving the same note cannot silently overwrite each other. Each returns the
/// note as it is after the write.
#[derive(Clone, Debug, PartialEq)]
pub enum NoteUpdateShape<'a> {
    /// Replace the editable fields. An absent folder, or empty content, is removed.
    Fields {
        folder_id: Option<&'a str>,
        title: &'a str,
        content: &'a str,
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the note.
    Delete { by: Option<&'a str> },
}

/// A named folder a location's notes can be filed in.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteFolder {
    pub id: String,
    pub location_id: String,
    pub name: String,
    pub created_by: Option<String>,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for NoteFolder {
    fn id(&self) -> &str {
        &self.id
    }
}

/// Every write to a note folder. Each fails with [`Error::NotFound`] when the folder
/// does not exist or is deleted, and returns the folder as it is after the write.
/// Deleting a folder leaves its notes alone: they keep `folder_id` and stay on their
/// location.
#[derive(Clone, Debug, PartialEq)]
pub enum NoteFolderUpdateShape<'a> {
    Name { name: &'a str },
    Delete,
}

/// A piece of equipment in a location's register ("Equipment"). See SCHEMA.md's
/// `equipment` section and [`crate::equipment`] for the rules.
///
/// `equipment_type` and `status` hold [`crate::equipment::EquipmentType::as_str`] and
/// [`crate::equipment::EquipmentStatus::as_str`] values. The status is only changed by
/// [`Handler::add_equipment_log_entry`], so every change is in the log.
#[derive(Clone, Debug, PartialEq)]
pub struct Equipment {
    pub id: String,
    pub location_id: String,
    pub equipment_type: String,
    pub name: String,
    pub description: String,
    pub status: String,
    /// When the status last changed (or the equipment was added).
    pub status_changed_at: u64,
    /// When someone last logged that they checked it.
    pub last_checked_at: Option<u64>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    /// Bumped by every [`EquipmentUpdateShape`] write, not by log entries.
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for Equipment {
    fn id(&self) -> &str {
        &self.id
    }
}

/// A piece of equipment being added. Its first log entry, a status change from nothing
/// to `status` with `comment`, is written with it.
#[derive(Clone, Debug, PartialEq)]
pub struct NewEquipment<'a> {
    pub location_id: &'a str,
    pub equipment_type: &'a str,
    pub name: &'a str,
    pub description: &'a str,
    pub status: &'a str,
    pub comment: &'a str,
    pub created_by: Option<&'a str>,
}

/// Every write to a piece of equipment other than a log entry. Each fails with
/// [`Error::NotFound`] when it does not exist, is deleted or — for `Fields` — is no
/// longer at `expected_version`. Each returns the equipment as it is after the write.
#[derive(Clone, Debug, PartialEq)]
pub enum EquipmentUpdateShape<'a> {
    /// Replace the editable fields. Empty description is removed.
    Fields {
        equipment_type: &'a str,
        name: &'a str,
        description: &'a str,
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the equipment. Its log is kept.
    Delete { by: Option<&'a str> },
}

/// One line of a piece of equipment's log.
#[derive(Clone, Debug, PartialEq)]
pub struct EquipmentLogEntry {
    pub id: String,
    pub equipment_id: String,
    pub location_id: String,
    pub kind: EquipmentLogKind,
    /// The status before a status change; `None` for the entry written when the
    /// equipment was added, and for checks.
    pub from_status: Option<String>,
    /// The status after a status change, or the status it was checked in.
    pub status: String,
    /// Why, in the words of whoever made the entry; may be empty for a check.
    pub comment: String,
    pub created_by: Option<String>,
    pub created_at: u64,
    /// When it was written, in Unix microseconds: orders entries made within the same
    /// second.
    pub created_us: u64,
}

impl HasID for EquipmentLogEntry {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquipmentLogKind {
    StatusChange,
    Check,
}

impl EquipmentLogKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StatusChange => "status_change",
            Self::Check => "check",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "status_change" => Some(Self::StatusChange),
            "check" => Some(Self::Check),
            _ => None,
        }
    }
}

/// What a new log entry records.
#[derive(Clone, Debug, PartialEq)]
pub enum EquipmentLogChange<'a> {
    /// The status changed from `from` to `to`.
    Status { from: &'a str, to: &'a str },
    /// Someone checked it, in `status`; the status is unchanged.
    Check { status: &'a str },
}

/// A log entry being added to a piece of equipment.
#[derive(Clone, Debug, PartialEq)]
pub struct NewEquipmentLogEntry<'a> {
    pub equipment_id: &'a str,
    pub location_id: &'a str,
    pub change: EquipmentLogChange<'a>,
    pub comment: &'a str,
    pub by: Option<&'a str>,
}

/// A booking of one or more pieces of a location's equipment for a time range
/// ("Reservations"). See SCHEMA.md's `equipment_reservation` section and
/// [`crate::reservations`] for the rules.
///
/// It is for the members in `reserved_for_person_ids` (the member who made it, unless
/// changed) or, when `reserved_for_text` is set, for whoever that names; never both.
/// Overlapping reservations are allowed; the API reports them as a warning.
#[derive(Clone, Debug, PartialEq)]
pub struct EquipmentReservation {
    pub id: String,
    pub location_id: String,
    /// At least one, with no repeats. Stored as a set, so in no particular order.
    pub equipment_ids: Vec<String>,
    pub start_time: u64,
    pub end_time: u64,
    /// Person IDs, with no repeats; empty when `reserved_for_text` is set. Stored as a
    /// set, so in no particular order.
    pub reserved_for_person_ids: Vec<String>,
    pub reserved_for_text: Option<String>,
    pub description: String,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    /// Bumped by every [`EquipmentReservationUpdateShape`] write.
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for EquipmentReservation {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewEquipmentReservation<'a> {
    pub location_id: &'a str,
    pub equipment_ids: &'a [String],
    pub start_time: u64,
    pub end_time: u64,
    pub reserved_for_person_ids: &'a [String],
    pub reserved_for_text: Option<&'a str>,
    pub description: &'a str,
    pub created_by: Option<&'a str>,
}

/// Every write to a reservation. Each fails with [`Error::NotFound`] when it does not
/// exist, is deleted or — for `Fields` — is no longer at `expected_version`. Each
/// returns the reservation as it is after the write.
#[derive(Clone, Debug, PartialEq)]
pub enum EquipmentReservationUpdateShape<'a> {
    /// Replace the editable fields. Empty `reserved_for_person_ids`, an absent
    /// `reserved_for_text` or an empty description is removed.
    Fields {
        equipment_ids: &'a [String],
        start_time: u64,
        end_time: u64,
        reserved_for_person_ids: &'a [String],
        reserved_for_text: Option<&'a str>,
        description: &'a str,
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the reservation.
    Delete { by: Option<&'a str> },
}

/// A pin on a location's shared map ("Map"). See SCHEMA.md's `map_pin` section.
///
/// Every pin belongs to its location; `layer_id` optionally puts it on one of the
/// location's [`MapLayer`]s. `latitude` / `longitude` are WGS 84 degrees. `address` and
/// `note` are free text, empty when absent. `pin_type` is a
/// [`MapPinType::as_str`](crate::map::MapPinType::as_str), absent for a plain pin.
#[derive(Clone, Debug, PartialEq)]
pub struct MapPin {
    pub id: String,
    pub location_id: String,
    pub layer_id: Option<String>,
    pub pin_type: Option<String>,
    pub title: String,
    pub latitude: f64,
    pub longitude: f64,
    pub address: String,
    pub note: String,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for MapPin {
    fn id(&self) -> &str {
        &self.id
    }
}

/// The fields of a map pin being created. See [`MapPin`].
#[derive(Clone, Debug, PartialEq)]
pub struct NewMapPin<'a> {
    pub location_id: &'a str,
    pub layer_id: Option<&'a str>,
    pub pin_type: Option<&'a str>,
    pub title: &'a str,
    pub latitude: f64,
    pub longitude: f64,
    pub address: &'a str,
    pub note: &'a str,
    pub created_by: Option<&'a str>,
}

/// Every write to a map pin. Each fails with [`Error::NotFound`] when the pin does not
/// exist, is deleted or — for `Fields` — is no longer at `expected_version`, so two
/// people saving the same pin cannot silently overwrite each other. Each returns the
/// pin as it is after the write.
#[derive(Clone, Debug, PartialEq)]
pub enum MapPinUpdateShape<'a> {
    /// Replace the editable fields. An absent layer or type, or an empty address or
    /// note, is removed.
    Fields {
        layer_id: Option<&'a str>,
        pin_type: Option<&'a str>,
        title: &'a str,
        latitude: f64,
        longitude: f64,
        address: &'a str,
        note: &'a str,
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the pin.
    Delete { by: Option<&'a str> },
}

/// A named layer a location's map pins can be put on.
#[derive(Clone, Debug, PartialEq)]
pub struct MapLayer {
    pub id: String,
    pub location_id: String,
    pub name: String,
    pub created_by: Option<String>,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for MapLayer {
    fn id(&self) -> &str {
        &self.id
    }
}

/// Every write to a map layer. Each fails with [`Error::NotFound`] when the layer does
/// not exist or is deleted, and returns the layer as it is after the write. Deleting a
/// layer leaves its pins alone: they keep `layer_id` and stay on their location's map.
#[derive(Clone, Debug, PartialEq)]
pub enum MapLayerUpdateShape<'a> {
    Name { name: &'a str },
    Delete,
}

/// A named, coloured calendar a location's events can be put in ("Calendar"). See
/// SCHEMA.md's `calendar` section.
///
/// `color` holds a [`crate::calendar::CalendarColor::as_str`] value.
#[derive(Clone, Debug, PartialEq)]
pub struct Calendar {
    pub id: String,
    pub location_id: String,
    pub name: String,
    pub color: String,
    pub created_by: Option<String>,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for Calendar {
    fn id(&self) -> &str {
        &self.id
    }
}

/// Every write to a calendar. Each fails with [`Error::NotFound`] when the calendar does
/// not exist or is deleted, and returns it as it is after the write. Deleting a calendar
/// leaves its events alone: they keep `calendar_id` and show as in no calendar.
#[derive(Clone, Debug, PartialEq)]
pub enum CalendarUpdateShape<'a> {
    Fields { name: &'a str, color: &'a str },
    Delete,
}

/// An event in a location's shared calendar ("Calendar"). See SCHEMA.md's
/// `calendar_event` section and [`crate::calendar`] for the rules.
///
/// `calendar_id` optionally puts it in one of the location's [`Calendar`]s. `last_end`
/// is [`crate::calendar::last_end`] of its timing and recurrence, stored for the index
/// that finds the events reaching a window.
#[derive(Clone, Debug, PartialEq)]
pub struct CalendarEvent {
    pub id: String,
    pub location_id: String,
    pub calendar_id: Option<String>,
    pub title: String,
    pub description: String,
    pub timing: crate::calendar::Timing,
    pub recurrence: Option<crate::calendar::Recurrence>,
    pub last_end: u64,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    /// Bumped by every [`CalendarEventUpdateShape`] write.
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for CalendarEvent {
    fn id(&self) -> &str {
        &self.id
    }
}

/// The fields of an event being created. See [`CalendarEvent`].
#[derive(Clone, Debug, PartialEq)]
pub struct NewCalendarEvent<'a> {
    pub location_id: &'a str,
    pub calendar_id: Option<&'a str>,
    pub title: &'a str,
    pub description: &'a str,
    pub timing: crate::calendar::Timing,
    pub recurrence: Option<crate::calendar::Recurrence>,
    pub created_by: Option<&'a str>,
}

/// Every write to an event. Each fails with [`Error::NotFound`] when it does not exist,
/// is deleted or — for `Fields` — is no longer at `expected_version`. Each returns the
/// event as it is after the write.
#[derive(Clone, Debug, PartialEq)]
pub enum CalendarEventUpdateShape<'a> {
    /// Replace the editable fields. An absent calendar or recurrence, or an empty
    /// description, is removed.
    Fields {
        calendar_id: Option<&'a str>,
        title: &'a str,
        description: &'a str,
        timing: crate::calendar::Timing,
        recurrence: Option<crate::calendar::Recurrence>,
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the event.
    Delete { by: Option<&'a str> },
}

/// An on-call role at a location ("On Call"), such as Duty Officer, and who holds it
/// now. See SCHEMA.md's `on_call_role` section.
///
/// A role with anyone in it has exactly one primary; `secondary_person_ids` are the
/// others sharing it and is empty when the primary is absent.
#[derive(Clone, Debug, PartialEq)]
pub struct OnCallRole {
    pub id: String,
    pub location_id: String,
    pub name: String,
    /// Plain text; empty when absent.
    pub description: String,
    /// A number that reaches whoever holds the role (e.g. a diverted duty phone).
    pub phone: Option<String>,
    pub primary_person_id: Option<String>,
    pub secondary_person_ids: Vec<String>,
    /// When the people in the role last changed, or when it was created with people in
    /// it. Absent while nobody has been put in it.
    pub members_changed_at: Option<u64>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for OnCallRole {
    fn id(&self) -> &str {
        &self.id
    }
}

/// The fields of an on-call role being created. See [`OnCallRole`].
#[derive(Clone, Debug, PartialEq)]
pub struct NewOnCallRole<'a> {
    pub location_id: &'a str,
    pub name: &'a str,
    pub description: &'a str,
    pub phone: Option<&'a str>,
    pub primary_person_id: Option<&'a str>,
    pub secondary_person_ids: &'a [String],
    pub created_by: Option<&'a str>,
}

/// Every write to an on-call role. Each fails with [`Error::NotFound`] when the role
/// does not exist, is deleted or — for `Fields` — is no longer at `expected_version`.
/// Each returns the role as it is after the write.
#[derive(Clone, Debug, PartialEq)]
pub enum OnCallRoleUpdateShape<'a> {
    /// Replace the editable fields. Absent values, an empty description and an empty
    /// list of secondaries are removed.
    Fields {
        name: &'a str,
        description: &'a str,
        phone: Option<&'a str>,
        primary_person_id: Option<&'a str>,
        secondary_person_ids: &'a [String],
        members_changed_at: Option<u64>,
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the role.
    Delete { by: Option<&'a str> },
}

/// A contact added by hand to Tally's Contacts ("shared contacts"): a location's own,
/// such as its Duty Officer phone, or a statewide one, such as the SOC, the EAP or peer
/// support. See SCHEMA.md's `shared_contact` section.
#[derive(Clone, Debug, PartialEq)]
pub struct SharedContact {
    pub id: String,
    /// The location the contact belongs to; `None` for a statewide one.
    pub location_id: Option<String>,
    pub name: String,
    /// Plain text; empty when absent.
    pub description: String,
    pub phone: Option<String>,
    /// Whether `phone` takes text messages. Always false without a phone.
    pub phone_can_sms: bool,
    pub email: Option<String>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for SharedContact {
    fn id(&self) -> &str {
        &self.id
    }
}

/// Which list a shared contact is on: one location's, or the statewide one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharedContactScope<'a> {
    Location(&'a str),
    State,
}

impl<'a> SharedContactScope<'a> {
    pub fn of(location_id: Option<&'a str>) -> Self {
        match location_id {
            Some(id) => Self::Location(id),
            None => Self::State,
        }
    }

    pub fn location_id(self) -> Option<&'a str> {
        match self {
            Self::Location(id) => Some(id),
            Self::State => None,
        }
    }
}

/// The fields of a shared contact being created. See [`SharedContact`].
#[derive(Clone, Debug, PartialEq)]
pub struct NewSharedContact<'a> {
    pub scope: SharedContactScope<'a>,
    pub name: &'a str,
    pub description: &'a str,
    pub phone: Option<&'a str>,
    pub phone_can_sms: bool,
    pub email: Option<&'a str>,
    pub created_by: Option<&'a str>,
}

/// Every write to a shared contact. Each fails with [`Error::NotFound`] when the contact
/// does not exist, is deleted or — for `Fields` — is no longer at `expected_version`.
/// Each returns the contact as it is after the write. A contact never moves between
/// scopes.
#[derive(Clone, Debug, PartialEq)]
pub enum SharedContactUpdateShape<'a> {
    /// Replace the editable fields. Absent values and an empty description are removed.
    Fields {
        name: &'a str,
        description: &'a str,
        phone: Option<&'a str>,
        phone_can_sms: bool,
        email: Option<&'a str>,
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the contact.
    Delete { by: Option<&'a str> },
}

impl HasID for Period {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Clone, Debug)]
pub struct NitcTag {
    pub id: i32,
    pub name: String,
    pub primary_activity_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PeriodUpdateShape<'a> {
    Fields {
        person_id: &'a str,
        location_id: &'a str,
        category_id: &'a str,
        start_time: i64,
        end_time: i64,
        /// Three-state update for the free-text comment: `None` leaves the
        /// existing comment untouched, `Some(None)` clears it, and
        /// `Some(Some(_))` sets it.
        comment: Option<Option<&'a str>>,
    },
    TimeCategory {
        start_time: i64,
        end_time: i64,
        category_id: &'a str,
        signed_out_session_id: Option<&'a str>,
        /// Three-state update for the free-text comment: `None` leaves the
        /// existing comment untouched, `Some(None)` clears it, and
        /// `Some(Some(_))` sets it.
        comment: Option<Option<&'a str>>,
        /// When `Some`, the write only lands if the period is still at this version
        /// and still open; otherwise it fails with [`Error::Conflict`] and changes
        /// nothing. `None` (the admin edit pages, the kiosk) only requires the row to
        /// exist.
        expected_version: Option<u64>,
    },
    /// Admin edit of a guest period: name, times, and comment only. A guest
    /// period has no person and no category, so those are never touched here.
    Guest {
        guest_name: &'a str,
        start_time: i64,
        end_time: i64,
        /// Three-state update for the free-text comment: `None` leaves the
        /// existing comment untouched, `Some(None)` clears it, and
        /// `Some(Some(_))` sets it.
        comment: Option<Option<&'a str>>,
    },
    Delete,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Location {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub nitc_enabled: Option<u64>,
    /// Whether exported NITC events are marked completed in SES. Defaults to true; locations
    /// that want to finish their NITCs themselves opt out.
    pub nitc_complete_on_export: bool,
    /// Whether members of this location earn badges. Defaults to false; a location opts in
    /// once it wants gamification, so the feature can be rolled out unit by unit.
    pub gamification_enabled: bool,
    pub ses_api_headquarters_id: Option<String>,
    pub last_successful_member_sync: Option<u64>,
    pub ses_details: LocationSesDetails,
    pub directory_details: LocationDirectoryDetails,
    /// The unit's rescue accreditations from the SES API, exactly as stored: JSON text,
    /// `[{"type", "status", "number", "note"}]` sorted by type. `None` for a location
    /// with no rows (every zone, and State Headquarters).
    pub accreditations: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

/// What location sync copies from the SES API's headquarters record.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocationSesDetails {
    pub code: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    /// Our location for the unit's zone, the headquarters' `zone` resolved through its
    /// `ses_api_headquarters_id`. Units only; absent while the zone has no location.
    pub zone_location_id: Option<String>,
}

/// What location sync copies from the location's entity in the SES intranet directory.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocationDirectoryDetails {
    /// The directory `Entity` id the location resolved to. Absent when its code matched
    /// no entity, or more than one that its name could not tell apart.
    pub entity_id: Option<String>,
    pub address: Option<LocationAddress>,
    pub postal_address: Option<LocationAddress>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

/// A directory address, split into parts. Every part is optional; `formatted` is the
/// whole address on one line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocationAddress {
    pub line1: Option<String>,
    pub line2: Option<String>,
    pub suburb: Option<String>,
    pub state: Option<String>,
    pub postcode: Option<String>,
    pub formatted: Option<String>,
}

impl HasID for Location {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub is_virtual: bool,
    pub nitc_participant_type: Option<String>,
    pub nitc_group_id: Option<String>,
    /// The kiosk group the category sits under ("Training"). Display metadata
    /// duplicated from the kiosk's hard-coded tree for the mobile app.
    pub group_name: Option<String>,
    /// The short name the kiosk shows ("AIIMS").
    pub short_name: Option<String>,
    /// The kiosk icon's file stem ("aiims").
    pub icon: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl Category {
    pub fn display(&self) -> CategoryDisplay {
        CategoryDisplay {
            group_name: self.group_name.clone(),
            short_name: self.short_name.clone(),
            icon: self.icon.clone(),
        }
    }
}

/// A category's optional display metadata. Writes replace all three: `None` removes
/// the attribute.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CategoryDisplay {
    pub group_name: Option<String>,
    pub short_name: Option<String>,
    pub icon: Option<String>,
}

impl HasID for Category {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoginCode {
    pub email: String,
    pub code_hash: String,
    pub expires_at: u64,
    pub attempts: u64,
    pub last_sent_at: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UserToken {
    pub id: String,
    pub token_hash: String,
    pub user_id: String,
    pub created_at: u64,
    pub expires_at: u64,
    pub last_used_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserTokenUpdateShape {
    TouchLastUsed,
}

/// One authorized OAuth client for a user (MCP and, later, other audiences). Holds
/// both the current access and refresh token hashes — a grant is the unit of
/// revocation, so rotating either token rewrites the same row rather than creating a
/// new one. `expires_at` is the absolute cap (also the DynamoDB TTL attribute):
/// `refresh_expires_at` slides forward on every rotation but never past it.
#[derive(Clone, Debug, PartialEq)]
pub struct OAuthGrant {
    pub id: String,
    /// The user the grant acts for. Always set on a grant approved at the consent
    /// screen; on a Tally sign-in grant (`person_id` set) only when the email they
    /// signed in with is also a user's.
    pub user_id: Option<String>,
    /// Set only on a Tally sign-in grant: the member it acts for. Such a grant resolves
    /// to [`crate::auth::AuthInfo::Member`] rather than a user.
    pub person_id: Option<String>,
    pub client_id: String,
    pub client_name: String,
    pub redirect_uri: String,
    /// The audience (RFC 8707 `resource`) this grant's tokens are bound to, e.g.
    /// `<base>/mcp`. Checked on every use so a token minted for one audience can't
    /// authenticate against another.
    pub resource: String,
    pub scope: String,
    pub access_token_hash: String,
    pub access_expires_at: u64,
    pub refresh_token_hash: String,
    pub refresh_expires_at: u64,
    pub expires_at: u64,
    pub created_at: u64,
    pub last_used_at: Option<u64>,
}

impl HasID for OAuthGrant {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OAuthGrantUpdateShape {
    /// Refresh-token rotation: replace both secrets, sliding `refresh_expires_at`
    /// forward (capped by the grant's `expires_at` — enforced by the caller in
    /// `oauth.rs`, not here). Compare-and-swap on `expected_refresh_token_hash`, so
    /// of two concurrent refreshes with the same token only one wins; the loser gets
    /// [`Error::NotFound`], the same as a missing grant.
    Rotate {
        expected_refresh_token_hash: String,
        access_token_hash: String,
        access_expires_at: u64,
        refresh_token_hash: String,
        refresh_expires_at: u64,
    },
    TouchLastUsed,
}

/// Whose passkey a [`WebauthnCredential`] is. A user's passkey signs in to the admin
/// site (and to Tally, as that user's member); a member's is for Tally sign-in only,
/// and is made only for a member who signs in with no user alongside them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PasskeyOwner {
    /// A user ID: stored as `user_id`.
    User(String),
    /// A person ID: stored as `person_id`.
    Member(String),
}

impl PasskeyOwner {
    pub fn user_id(&self) -> Option<&str> {
        match self {
            Self::User(id) => Some(id),
            Self::Member(_) => None,
        }
    }

    pub fn person_id(&self) -> Option<&str> {
        match self {
            Self::User(_) => None,
            Self::Member(id) => Some(id),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WebauthnCredential {
    /// Credential ID (base64url) — partition key in DynamoDB.
    pub id: String,
    pub owner: PasskeyOwner,
    /// User-supplied label (e.g. "MacBook Touch ID").
    pub name: String,
    /// JSON-serialized webauthn_rs::prelude::Passkey (contains counter, public key, etc.).
    pub passkey_json: String,
    pub created_at: u64,
    pub last_used_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebauthnCredentialUpdate {
    Rename(String),
    TouchLastUsed { passkey_json: String },
}

/// Short-lived challenge state stored between the two WebAuthn round-trips.
#[derive(Clone, Debug, PartialEq)]
pub struct WebauthnState {
    /// Opaque challenge ID returned to the client.
    pub id: String,
    pub kind: String,
    /// Set for registration states.
    pub user_id: Option<String>,
    /// JSON-serialized PasskeyRegistration or DiscoverableAuthentication.
    pub state_json: String,
    /// Unix timestamp; DynamoDB TTL auto-deletes after this.
    pub expires_at: u64,
}

/// Generic short-lived key/value record in the `ephemeral_state` table. Namespaced
/// by `kind`, carries an opaque JSON `payload`, and is auto-deleted by DynamoDB TTL
/// after `expires_at`. Currently backs kiosk public-key enrollment; `webauthn_state`
/// is intended to migrate onto this shape later.
#[derive(Clone, Debug, PartialEq)]
pub struct EphemeralState {
    pub id: String,
    pub kind: String,
    pub payload: String,
    /// Unix timestamp; DynamoDB TTL auto-deletes after this.
    pub expires_at: u64,
}

/// Where a device was when it reported in for a [`LocationShare`]: WGS 84 degrees,
/// metres and metres per second, as the phone reported them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SharedPosition {
    pub latitude: f64,
    pub longitude: f64,
    /// Radius of uncertainty of the position, in metres.
    pub accuracy: Option<f64>,
    /// Metres above the WGS 84 ellipsoid.
    pub altitude: Option<f64>,
    pub altitude_accuracy: Option<f64>,
    /// Direction of travel, degrees clockwise from true north (0 ≤ heading < 360).
    pub heading: Option<f64>,
    pub speed: Option<f64>,
    /// Unix seconds when the device took the fix, never later than when it was stored.
    pub recorded_at: u64,
}

/// A user sharing their position with a location's members ("Locate"), for a fixed
/// time. One per user: starting another replaces it. Only the latest position is kept.
/// See SCHEMA.md's `location_share` section.
#[derive(Clone, Debug, PartialEq)]
pub struct LocationShare {
    /// The sharing member's person ID; also the table's key (`id`).
    pub person_id: String,
    pub location_id: String,
    pub position: SharedPosition,
    /// How often the device said it would report in, in seconds.
    pub interval_secs: u64,
    pub started_at: u64,
    /// When sharing stops: later reports are refused.
    pub ends_at: u64,
    /// When the latest position arrived.
    pub updated_at: u64,
    /// When the share stops showing, a while after the latest position (the table's TTL).
    pub expires_at: u64,
}

impl HasID for LocationShare {
    fn id(&self) -> &str {
        &self.person_id
    }
}

// ── Notifications ────────────────────────────────────────────────────────────

/// One notification in one user's inbox. An event notifying several users is stored as
/// a full copy per recipient, sharing `notification_id`, so reading an inbox is a
/// single query. See SCHEMA.md's `notification` section.
#[derive(Clone, Debug, PartialEq)]
pub struct NotificationRecord {
    /// The recipient: a member, by person ID. The table's hash key.
    pub person_id: String,
    /// See [`notification_sk`]. The table's range key, and the paging cursor.
    pub sk: String,
    /// Shared by every recipient's copy of one event.
    pub notification_id: String,
    /// The producing app, e.g. `reservations`.
    pub source: String,
    /// What happened, e.g. `reservation.for_you`.
    pub kind: String,
    pub location_id: Option<String>,
    /// The member whose action caused it, if a member did.
    pub actor_person_id: Option<String>,
    pub title: String,
    pub body: Option<String>,
    /// What tapping the notification should open, e.g. `reservation` and its ID.
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    /// Unix seconds.
    pub created_at: u64,
    /// Unix seconds; None while unread.
    pub read_at: Option<u64>,
    /// Unix seconds; the table's TTL.
    pub expires_at: u64,
}

impl NotificationRecord {
    pub fn is_read(&self) -> bool {
        self.read_at.is_some()
    }
}

/// The `sk` of a `notification` item: the creation time in milliseconds zero-padded to
/// 13 digits, `#`, then the event's `notification_id`. Lexical order is time order
/// (until the year 2286), so newest first is a descending query, and the ID keeps it
/// unique when two events share a millisecond.
pub fn notification_sk(created_ms: u64, notification_id: &str) -> String {
    format!("{created_ms:013}#{notification_id}")
}

// ── Audit log ────────────────────────────────────────────────────────────────

/// What an audited write did to its entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditAction {
    Create,
    Update,
    Delete,
    Restore,
}

impl AuditAction {
    /// Stable string stored in the `action` attribute.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Restore => "restore",
        }
    }

    /// Inverse of [`AuditAction::as_str`].
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "create" => Self::Create,
            "update" => Self::Update,
            "delete" => Self::Delete,
            "restore" => Self::Restore,
            _ => return None,
        })
    }
}

/// Which kind of record an audit entry is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditEntityType {
    User,
    Person,
    Period,
    Session,
    ApiToken,
    Location,
    Category,
    NitcGroup,
    NitcTag,
    UserToken,
    OAuthGrant,
    WebauthnCredential,
    Task,
    TaskList,
    Note,
    NoteFolder,
    Equipment,
    EquipmentLogEntry,
    EquipmentReservation,
    OnCallRole,
    SharedContact,
    MapPin,
    MapLayer,
    Calendar,
    CalendarEvent,
    LocationShare,
}

impl AuditEntityType {
    /// Inverse of [`AuditEntityType::as_str`].
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "user" => Self::User,
            "person" => Self::Person,
            "period" => Self::Period,
            "session" => Self::Session,
            "api_token" => Self::ApiToken,
            "location" => Self::Location,
            "category" => Self::Category,
            "nitc_group" => Self::NitcGroup,
            "nitc_tag" => Self::NitcTag,
            "user_token" => Self::UserToken,
            "oauth_grant" => Self::OAuthGrant,
            "webauthn_credential" => Self::WebauthnCredential,
            "task" => Self::Task,
            "task_list" => Self::TaskList,
            "on_call_role" => Self::OnCallRole,
            "shared_contact" => Self::SharedContact,
            "note" => Self::Note,
            "note_folder" => Self::NoteFolder,
            "equipment" => Self::Equipment,
            "equipment_log_entry" => Self::EquipmentLogEntry,
            "equipment_reservation" => Self::EquipmentReservation,
            "map_pin" => Self::MapPin,
            "map_layer" => Self::MapLayer,
            "calendar" => Self::Calendar,
            "calendar_event" => Self::CalendarEvent,
            "location_share" => Self::LocationShare,
            _ => return None,
        })
    }

    /// Stable string stored in the `entity_type` attribute.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Person => "person",
            Self::Period => "period",
            Self::Session => "session",
            Self::ApiToken => "api_token",
            Self::Location => "location",
            Self::Category => "category",
            Self::NitcGroup => "nitc_group",
            Self::NitcTag => "nitc_tag",
            Self::UserToken => "user_token",
            Self::OAuthGrant => "oauth_grant",
            Self::WebauthnCredential => "webauthn_credential",
            Self::OnCallRole => "on_call_role",
            Self::SharedContact => "shared_contact",
            Self::Task => "task",
            Self::TaskList => "task_list",
            Self::Note => "note",
            Self::NoteFolder => "note_folder",
            Self::Equipment => "equipment",
            Self::EquipmentLogEntry => "equipment_log_entry",
            Self::EquipmentReservation => "equipment_reservation",
            Self::MapPin => "map_pin",
            Self::MapLayer => "map_layer",
            Self::Calendar => "calendar",
            Self::CalendarEvent => "calendar_event",
            Self::LocationShare => "location_share",
        }
    }
}

/// One field of an entity that an audited write changed. `None` means the field was
/// absent on that side (so a create has `before: None` throughout). Secrets are never
/// carried here — the audit wrapper substitutes the string `"[redacted]"`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuditFieldChange {
    pub field: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<serde_json::Value>,
}

/// One recorded business write. A write that touches several locations is stored as one
/// table item per location, all sharing `event_id`; see SCHEMA.md's `audit_log` section.
#[derive(Clone, Debug, PartialEq)]
pub struct AuditEntry {
    /// Unique per stored item. The DB implementation mints one per item it writes, so
    /// this is only the id of the entry as built; `event_id` is what groups items.
    pub id: String,
    pub event_id: String,
    /// Unix seconds when the write happened.
    pub ts: u64,
    pub action: AuditAction,
    pub entity_type: AuditEntityType,
    pub entity_id: String,
    /// Human-readable name of the entity at the time (e.g. a person's full name).
    pub entity_label: Option<String>,
    /// Every location the write affected. Empty for global entities (users, categories…).
    pub location_ids: Vec<String>,
    /// See [`crate::audit::Actor::kind`].
    pub actor_kind: String,
    pub actor_id: Option<String>,
    pub actor_via: Option<String>,
    pub ip: Option<String>,
    pub changes: Vec<AuditFieldChange>,
}

impl AuditEntry {
    /// The entry's sort key in the audit indexes; see [`audit_sk`].
    pub fn sk(&self) -> String {
        audit_sk(self.ts, &self.id)
    }
}

/// The `sk` of an `audit_log` item: `ts` (Unix seconds) zero-padded to 10 digits, `#`,
/// then the item `id`. Lexical order is time order, and the `id` makes it unique. It is
/// also the paging cursor — the `id` it carries is the table's hash key, so a cursor holds
/// everything DynamoDB needs for an `ExclusiveStartKey`.
pub fn audit_sk(ts: u64, id: &str) -> String {
    format!("{ts:010}#{id}")
}

/// Check that `cursor` has the shape of an [`audit_sk`] and return the item `id` inside
/// it, or `None` for anything else. Callers treat `None` as a bad request: a cursor comes
/// from the client, and a malformed one must not reach the database.
pub fn parse_audit_cursor(cursor: &str) -> Option<&str> {
    let (ts, id) = cursor.split_once('#')?;
    let id_ok = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    (!ts.is_empty() && ts.len() <= 20 && ts.chars().all(|c| c.is_ascii_digit()) && id_ok)
        .then_some(id)
}

/// Which slice of the audit log to read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuditScope {
    /// Items for one location (`location_id-sk-index`). A write that touched several
    /// locations has an item in each, so it shows up in every one of their views.
    Location(String),
    /// Every event once, including those of global entities (`scope-sk-index`).
    All,
}

/// A page of the audit log, in the order `descending` names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListAuditEntriesPage {
    /// Only entries strictly after this cursor (an [`audit_sk`]) in the page's order.
    /// Reading backwards is the same query with `descending` flipped: the caller asks
    /// for the entries after the cursor in the opposite order, nearest first.
    pub after: Option<String>,
    /// How many entries to return at most; callers fetch `page_size + 1` to learn
    /// whether there is another page.
    pub limit: i32,
    /// Newest first when true.
    pub descending: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListAuditEntriesQuery {
    pub scope: AuditScope,
    /// Only entries about one of these kinds of record. Empty means every kind.
    pub entity_types: Vec<AuditEntityType>,
    pub page: ListAuditEntriesPage,
}

/// `Sync` is required so a `&impl Handler` (including the erased handle returned by
/// [`crate::app::HasDb::db`]) can be held across `.await` inside the `Send` futures the
/// GraphQL/Poem stack builds. Both implementors (`dynamodb::Handler`, `mockdb::Handler`)
/// are already `Sync`.
pub trait Handler: Sync {
    fn get_users<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<User>>>> + Send;
    /// Returns the IDs of every user with this email. Expected to be at most one;
    /// returns multiple only when the uniqueness invariant has been violated.
    fn get_user_id_by_email(&self, email: &str)
    -> impl Future<Output = Result<Vec<String>>> + Send;
    fn list_users(&self) -> impl Future<Output = Result<Vec<User>>> + Send;
    fn create_user(
        &self,
        email: &str,
        is_super: bool,
        location_grants: Vec<String>,
        location_read_only_grants: Vec<String>,
    ) -> impl Future<Output = Result<User>> + Send;
    fn update_user(
        &self,
        id: &str,
        change: UserUpdateShape<'_>,
    ) -> impl Future<Output = Result<()>> + Send;
    fn get_persons<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Person>>>> + Send;
    /// Returns the IDs of every person with this registration number. Expected to be at
    /// most one; returns multiple only when the uniqueness invariant has been violated.
    fn get_person_id_by_registration_number(
        &self,
        registration_number: &str,
    ) -> impl Future<Output = Result<Vec<String>>> + Send;
    /// Returns the IDs of every person with this SES API person ID. Expected to be at
    /// most one; returns multiple only when the uniqueness invariant has been violated.
    fn get_person_id_by_ses_api_person_id(
        &self,
        ses_api_person_id: &str,
    ) -> impl Future<Output = Result<Vec<String>>> + Send;
    /// Returns the IDs of every person whose email matches, compared by [`email_key`].
    /// Several people may share an address (a family's, say), and soft-deleted people
    /// are included: the caller decides what either means.
    fn get_person_ids_by_email(
        &self,
        email: &str,
    ) -> impl Future<Output = Result<Vec<String>>> + Send;
    fn get_sessions<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Session>>>> + Send;
    fn list_sessions(
        &self,
        query: ListSessionsQuery,
    ) -> impl Future<Output = Result<Vec<Session>>> + Send;
    fn list_people_for_location(
        &self,
        location_id: &str,
        skip_deleted: bool,
    ) -> impl Future<Output = Result<Vec<Person>>> + Send;
    fn list_periods_for_location(
        &self,
        location_id: &str,
        only_active: bool,
        timestamp_range: Option<(u64, u64)>,
        category_ids: Option<&[String]>,
        page: ListPeriodsPage,
    ) -> impl Future<Output = Result<Vec<Period>>> + Send;
    fn list_test_pagination(
        &self,
        page: ListTestPaginationPage,
    ) -> impl Future<Output = Result<Vec<TestPaginationRow>>> + Send;
    fn list_periods_for_person(
        &self,
        person_id: &str,
        location_id: Option<&str>,
        only_unfinished: Option<bool>,
        category_ids: Option<&[String]>,
        page: ListPeriodsPage,
    ) -> impl Future<Output = Result<Vec<Period>>> + Send;
    fn get_periods<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Period>>>> + Send;
    /// Ends the given period by setting its end_time to the current time
    fn end_period(
        &self,
        period: &Period,
        signed_out_session_id: Option<&str>,
    ) -> impl Future<Output = Result<Period>> + Send;
    /// Create a new open (not yet signed-out) period for the given person at the
    /// given location.
    ///
    /// `signed_in_session_id` is the originating kiosk; the scan flow always
    /// supplies it, but a record created by hand (the `cli period create-signin`
    /// test helper) may have none. `start_time` defaults to now — pass `Some` to
    /// backdate it, e.g. to exercise the kiosk "forgot to sign out" interstitial.
    fn start_period_for_person_location(
        &self,
        person_id: &str,
        location_id: &str,
        signed_in_session_id: Option<&str>,
        start_time: Option<u64>,
    ) -> impl Future<Output = Result<Period>> + Send;
    /// Create a new open guest period starting now at the given location.
    /// Writes no person_id attribute (sparse person GSI) and no category.
    fn start_guest_period(
        &self,
        location_id: &str,
        guest_name: &str,
        comment: Option<&str>,
        signed_in_session_id: &str,
    ) -> impl Future<Output = Result<Period>> + Send;
    fn create_person(
        &self,
        location_id: &str,
        first_name: &str,
        last_name: &str,
        registration_number: &str,
    ) -> impl Future<Output = Result<Person>> + Send;
    fn update_person(
        &self,
        id: &str,
        change: PersonUpdateShape<'_>,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Give the person a fresh `contact_key`, putting them back at the head of the
    /// contacts feed without changing anything else. Used when something the feed
    /// returns changed outside the person row (their location's name, their
    /// `person_unit` rows) and to backfill rows written before the feed existed. The
    /// writes that change a returned field (`create_person`, and `update_person` other
    /// than `SesApiPersonId`, `MissingSince` and `BadgeState`) stamp the key themselves.
    fn stamp_person_contact(&self, id: &str) -> impl Future<Output = Result<()>> + Send;

    /// Write the person's `email_key` (see [`email_key`]) from `email`, provided their
    /// email is still exactly `email`; [`Error::NotFound`] otherwise. Changes nothing
    /// else. `update_person`'s `Email` shape writes the key itself; this backfills rows
    /// written before the key existed.
    fn stamp_person_email_key(
        &self,
        id: &str,
        email: &str,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Up to `limit` people from the contacts feed with `after < contact_key < before`,
    /// in key order. `after: None` starts from the beginning. Soft-deleted people are
    /// included (as tombstones) only when `include_deleted`.
    fn list_contact_changes(
        &self,
        after: Option<&str>,
        before: &str,
        include_deleted: bool,
        limit: usize,
    ) -> impl Future<Output = Result<ContactPage>> + Send;

    /// How many live (not soft-deleted) people the contacts feed holds below `before`.
    /// Reads the whole index, so only for the first page of a full sync.
    fn count_contacts(&self, before: &str) -> impl Future<Output = Result<u64>> + Send;
    fn create_period(
        &self,
        person_id: &str,
        location_id: &str,
        category_id: &str,
        start_time: u64,
        end_time: u64,
        comment: Option<&str>,
    ) -> impl Future<Output = Result<Period>> + Send;
    /// Returns the period's new `version` (post-increment), so callers that
    /// publish a realtime event for the mutation don't need a second read to
    /// learn it.
    fn update_period(
        &self,
        id: &str,
        change: PeriodUpdateShape<'_>,
    ) -> impl Future<Output = Result<u64>> + Send;
    /// Returns the IDs of every session whose enrolled key fingerprint matches (from
    /// the `key_fingerprint-index` GSI). Expected to be at most one active session;
    /// callers should fetch each ID with [`get_sessions`](Self::get_sessions) to confirm
    /// it exists and is active.
    fn get_session_id_by_key_fingerprint(
        &self,
        fingerprint: &str,
    ) -> impl Future<Output = Result<Vec<String>>> + Send;
    fn create_session(
        &self,
        location_id: &str,
        name: &str,
        config: &serde_json::Map<String, serde_json::Value>,
        healthcheck_url: Option<&str>,
        key: SessionKeyParams<'_>,
    ) -> impl Future<Output = Result<Session>> + Send;
    /// Returns the session's new [`version`](Session::version) for the shapes that bump
    /// it (`Fields`, `ReleaseKey`, `Delete`), `None` for those that don't.
    fn update_session(
        &self,
        id: &str,
        change: SessionUpdateShape<'_>,
    ) -> impl Future<Output = Result<Option<u64>>> + Send;

    /// Strongly consistent, so a token is verifiable the moment it's created.
    fn get_api_token(&self, id: &str) -> impl Future<Output = Result<Option<ApiToken>>> + Send;
    /// List API tokens. [`ListApiTokensFilter::ActiveOnly`] queries `active-index` and
    /// so cannot see revoked tokens; `All` scans, which is the only way to reach them.
    fn list_api_tokens(
        &self,
        filter: ListApiTokensFilter,
    ) -> impl Future<Output = Result<Vec<ApiToken>>> + Send;
    /// Caller supplies the id: it is embedded in the token (`slgn_<id>.<secret>`).
    #[allow(clippy::too_many_arguments)]
    fn create_api_token(
        &self,
        id: &str,
        name: &str,
        token_hash: &str,
        location_grants: Vec<String>,
        read_only: bool,
        expires_at: Option<u64>,
        created_by_user_id: &str,
    ) -> impl Future<Output = Result<ApiToken>> + Send;
    fn update_api_token(
        &self,
        id: &str,
        change: ApiTokenUpdateShape<'_>,
    ) -> impl Future<Output = Result<()>> + Send;

    fn create_location(
        &self,
        name: &str,
        nitc_enabled: Option<u64>,
        ses_api_headquarters_id: Option<&str>,
    ) -> impl Future<Output = Result<Location>> + Send;
    fn get_locations<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Location>>>> + Send;
    fn list_locations(
        &self,
        filter: ListLocationsFilter,
    ) -> impl Future<Output = Result<Vec<Location>>> + Send;
    fn update_location(
        &self,
        id: &str,
        change: LocationUpdateShape<'_>,
    ) -> impl Future<Output = Result<()>> + Send;
    fn list_categories(&self) -> impl Future<Output = Result<Vec<Category>>> + Send;
    fn get_categories<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Category>>>> + Send;
    fn create_category(
        &self,
        id: Option<&str>,
        name: &str,
        is_virtual: bool,
        nitc_group_id: Option<&str>,
        nitc_participant_type: Option<&str>,
        display: &CategoryDisplay,
    ) -> impl Future<Output = Result<Category>> + Send;
    #[allow(clippy::too_many_arguments)]
    fn update_category(
        &self,
        id: &str,
        name: &str,
        active: bool,
        is_virtual: bool,
        nitc_group_id: Option<&str>,
        nitc_participant_type: Option<&str>,
        display: &CategoryDisplay,
    ) -> impl Future<Output = Result<()>> + Send;

    // ── NITC export ──────────────────────────────────────────────────────────

    /// Read-only lookup of the nitc_events rows for (location, nitc_group, date).
    ///
    /// There should be at most one such row; callers that need a single value can
    /// use [`at_most_one`] to collapse the result and surface an integrity error
    /// when duplicates exist. The CLI lists all rows so the duplicate situation
    /// can be inspected when it arises.
    fn list_nitc_events_for_day(
        &self,
        location_id: &str,
        nitc_group_id: &str,
        date: NaiveDate,
    ) -> impl Future<Output = Result<Vec<NitcEvent>>> + Send;

    /// Get or create the nitc_events row for (location, nitc_group, date).
    /// Creates with ses_api_nitc_id=NULL; the SES API call happens later in Phase 2.
    fn get_or_create_nitc_event_for_day(
        &self,
        location_id: &str,
        nitc_group_id: &str,
        date: NaiveDate,
    ) -> impl Future<Output = Result<NitcEvent>> + Send;

    fn get_nitc_event_by_id(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<NitcEvent>>> + Send;

    /// Batch-fetch NITC events. Results are positionally aligned with `ids`, so a
    /// `None` names exactly which requested event is missing.
    fn get_nitc_events_by_ids<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<NitcEvent>>>> + Send;

    /// Every NITC event at a location, across all groups and dates.
    ///
    /// `location_id-topic_date-index` is hash-keyed on `location_id`, so this is the
    /// same index [`Handler::list_nitc_events_for_day`] uses, queried without the sort
    /// key. It is the only way to enumerate the `nitc_event` table — otherwise events
    /// are reachable only by following `Period.nitc_event_id`.
    fn list_nitc_events_for_location(
        &self,
        location_id: &str,
    ) -> impl Future<Output = Result<Vec<NitcEvent>>> + Send;

    // ── Table scans ──────────────────────────────────────────────────────────
    //
    // Every other read here goes through an index, which means it can only return rows
    // whose index key attribute is present and well-formed. That is exactly the wrong
    // property for auditing data integrity: a person with a malformed `location_id` is
    // absent from `location_id-index`, and a soft-deleted period has neither
    // `location_open` nor `location_live`, so no query reaches either. These scans read
    // the base table, so they see every row regardless of what its index keys look like.
    //
    // Scans are expensive and eventually consistent. `person` and `period` in
    // particular are the largest tables in the system, so callers should treat walking
    // them as a deliberate, occasional operation rather than a routine one.

    fn scan_persons(
        &self,
        cursor: Option<ScanCursor>,
        limit: i32,
    ) -> impl Future<Output = Result<ScanPage<Person>>> + Send;

    fn scan_periods(
        &self,
        cursor: Option<ScanCursor>,
        limit: i32,
    ) -> impl Future<Output = Result<ScanPage<Period>>> + Send;

    /// Every `person_unit` row (about 12k). Directory sync reads the table once a run.
    fn scan_person_units(&self) -> impl Future<Output = Result<Vec<PersonUnit>>> + Send;

    /// A member's `person_unit` rows, by entity id.
    fn list_person_units(
        &self,
        person_id: &str,
    ) -> impl Future<Output = Result<Vec<PersonUnit>>> + Send;

    /// Make a member's `person_unit` rows exactly `units` (every one of which must carry
    /// this `person_id`): rows not in it are deleted, and rows that are new or differ
    /// are written. An empty `units` removes them all. If anything changed, the person
    /// gets a fresh `contact_key`, since the contacts feed serves their units.
    fn replace_person_units(
        &self,
        person_id: &str,
        units: &[PersonUnit],
    ) -> impl Future<Output = Result<()>> + Send;

    /// Unlike `list_sessions`, this reaches soft-deleted sessions — they have no
    /// `active` attribute, so they are absent from `active-location_id-index`.
    fn scan_sessions(
        &self,
        cursor: Option<ScanCursor>,
        limit: i32,
    ) -> impl Future<Output = Result<ScanPage<Session>>> + Send;

    /// The only way to enumerate user tokens: `user_token` has no secondary
    /// index, so there is no lookup by `user_id` or anything else.
    fn scan_user_tokens(
        &self,
        cursor: Option<ScanCursor>,
        limit: i32,
    ) -> impl Future<Output = Result<ScanPage<UserToken>>> + Send;

    /// Reaches events whose `location_id` is missing or malformed, which
    /// `list_nitc_events_for_location` cannot.
    fn scan_nitc_events(
        &self,
        cursor: Option<ScanCursor>,
        limit: i32,
    ) -> impl Future<Output = Result<ScanPage<NitcEvent>>> + Send;

    /// Fetch NITC group configuration (type, tags) by ID.
    fn get_nitc_group(&self, id: &str) -> impl Future<Output = Result<Option<NitcGroup>>> + Send;

    fn list_nitc_groups(&self) -> impl Future<Output = Result<Vec<NitcGroup>>> + Send;
    fn create_nitc_group(
        &self,
        id: Option<&str>,
        nitc_type: &str,
        nitc_tag_ids: &[i32],
    ) -> impl Future<Output = Result<NitcGroup>> + Send;
    fn update_nitc_group(
        &self,
        id: &str,
        nitc_type: &str,
        nitc_tag_ids: &[i32],
    ) -> impl Future<Output = Result<()>> + Send;
    fn delete_nitc_group(&self, id: &str) -> impl Future<Output = Result<()>> + Send;

    fn list_nitc_tags(&self) -> impl Future<Output = Result<Vec<NitcTag>>> + Send;
    fn put_nitc_tag(&self, tag: &NitcTag) -> impl Future<Output = Result<()>> + Send;

    /// Atomically increment period.version and return the new value, to trigger NITC re-export.
    fn bump_period_version(&self, period_id: &str) -> impl Future<Output = Result<u64>> + Send;

    /// Atomically increment nitc_event.version and return the new value.
    fn bump_nitc_event_version(&self, event_id: &str) -> impl Future<Output = Result<u64>> + Send;

    /// Set period.nitc_event_id and clear nitc_participant_id.
    fn set_period_nitc_event(
        &self,
        period_id: &str,
        event_id: &str,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Return IDs of all periods (including deleted ones with a participant) assigned to an event.
    fn list_period_ids_for_nitc_event(
        &self,
        event_id: &str,
    ) -> impl Future<Output = Result<Vec<String>>> + Send;

    /// Store the SES API ID after creating the event in SES.
    fn set_nitc_event_ses_id(
        &self,
        event_id: &str,
        ses_api_nitc_id: i64,
    ) -> impl Future<Output = Result<()>> + Send;

    fn set_period_nitc_exported_version(
        &self,
        period_id: &str,
        synced_version: u64,
    ) -> impl Future<Output = Result<()>> + Send;

    fn update_period_nitc_exported(
        &self,
        period_id: &str,
        nitc_event_id: &str,
        nitc_participant_id: i64,
        synced_version: u64,
    ) -> impl Future<Output = Result<()>> + Send;

    fn clear_period_nitc_participant(
        &self,
        period_id: &str,
        synced_version: u64,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Mark the event as exported at the given version.
    fn mark_nitc_event_synced(
        &self,
        event_id: &str,
        synced_version: u64,
    ) -> impl Future<Output = Result<()>> + Send;

    // ── Email login codes ────────────────────────────────────────────────────

    fn put_login_code(
        &self,
        email: &str,
        code_hash: &str,
        expires_at: u64,
        last_sent_at: u64,
    ) -> impl Future<Output = Result<()>> + Send;

    fn get_login_code(&self, email: &str)
    -> impl Future<Output = Result<Option<LoginCode>>> + Send;

    fn delete_login_code(&self, email: &str) -> impl Future<Output = Result<()>> + Send;

    fn increment_login_code_attempts(&self, email: &str)
    -> impl Future<Output = Result<()>> + Send;

    // ── User tokens (opaque, hashed, sliding expiry) ─────────────────────────

    /// Caller supplies the id: it is embedded in the token (`slu_<id>.<secret>`), so
    /// it has to exist before the token can be minted and hashed.
    fn create_user_token(
        &self,
        id: &str,
        token_hash: &str,
        user_id: &str,
        expires_at: u64,
    ) -> impl Future<Output = Result<UserToken>> + Send;

    /// Strongly consistent, so a token is verifiable the moment it's issued.
    fn get_user_token(&self, id: &str) -> impl Future<Output = Result<Option<UserToken>>> + Send;

    fn update_user_token(
        &self,
        id: &str,
        change: UserTokenUpdateShape,
    ) -> impl Future<Output = Result<()>> + Send;

    fn delete_user_token(&self, id: &str) -> impl Future<Output = Result<()>> + Send;

    // ── OAuth grants (MCP access/refresh tokens) ─────────────────────────────

    /// Caller supplies the id (and every other field), as with `create_user_token`.
    fn create_oauth_grant(&self, grant: &OAuthGrant) -> impl Future<Output = Result<()>> + Send;

    fn get_oauth_grant(&self, id: &str) -> impl Future<Output = Result<Option<OAuthGrant>>> + Send;

    fn update_oauth_grant(
        &self,
        id: &str,
        change: OAuthGrantUpdateShape,
    ) -> impl Future<Output = Result<()>> + Send;

    fn delete_oauth_grant(&self, id: &str) -> impl Future<Output = Result<()>> + Send;

    /// Backs the "connected apps" list: every grant a user has authorized.
    fn list_oauth_grants_by_user(
        &self,
        user_id: &str,
    ) -> impl Future<Output = Result<Vec<OAuthGrant>>> + Send;

    // ── WebAuthn passkey credentials ─────────────────────────────────────────

    fn create_webauthn_credential(
        &self,
        id: &str,
        owner: &PasskeyOwner,
        name: &str,
        passkey_json: &str,
    ) -> impl Future<Output = Result<WebauthnCredential>> + Send;

    fn get_webauthn_credential(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<WebauthnCredential>>> + Send;

    /// The owner's passkeys: a user's through `user_id-index`, a member's through
    /// `person_id-index`.
    fn list_webauthn_credentials_by_owner(
        &self,
        owner: &PasskeyOwner,
    ) -> impl Future<Output = Result<Vec<WebauthnCredential>>> + Send;

    fn count_webauthn_credentials_by_owner(
        &self,
        owner: &PasskeyOwner,
    ) -> impl Future<Output = Result<usize>> + Send;

    fn update_webauthn_credential(
        &self,
        id: &str,
        change: WebauthnCredentialUpdate,
    ) -> impl Future<Output = Result<()>> + Send;

    fn delete_webauthn_credential(&self, id: &str) -> impl Future<Output = Result<()>> + Send;

    // ── WebAuthn challenge state ──────────────────────────────────────────────

    fn put_webauthn_state(
        &self,
        id: &str,
        kind: &str,
        user_id: Option<&str>,
        state_json: &str,
        expires_at: u64,
    ) -> impl Future<Output = Result<()>> + Send;

    fn get_webauthn_state(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<WebauthnState>>> + Send;

    fn delete_webauthn_state(&self, id: &str) -> impl Future<Output = Result<()>> + Send;

    // ── Generic ephemeral state ───────────────────────────────────────────────

    /// Upsert a record into the `ephemeral_state` table (overwrites any existing
    /// item with the same `id`).
    fn put_ephemeral_state(
        &self,
        id: &str,
        kind: &str,
        payload: &str,
        expires_at: u64,
    ) -> impl Future<Output = Result<()>> + Send;

    fn get_ephemeral_state(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<EphemeralState>>> + Send;

    fn delete_ephemeral_state(&self, id: &str) -> impl Future<Output = Result<()>> + Send;

    // ── Tasks ─────────────────────────────────────────────────────────────────

    fn get_tasks<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Task>>>> + Send;
    /// Open tasks only, deleted ones never; in `show_date` order. Reads a GSI, so a
    /// write may take a moment to show up.
    fn list_tasks(&self, query: ListTasksQuery) -> impl Future<Output = Result<Vec<Task>>> + Send;
    fn create_task(&self, task: NewTask<'_>) -> impl Future<Output = Result<Task>> + Send;
    fn update_task(
        &self,
        id: &str,
        change: TaskUpdateShape<'_>,
    ) -> impl Future<Output = Result<Task>> + Send;

    fn get_task_lists<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<TaskList>>>> + Send;
    /// A location's task lists, deleted ones left out.
    fn list_task_lists(
        &self,
        location_id: &str,
    ) -> impl Future<Output = Result<Vec<TaskList>>> + Send;
    fn create_task_list(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> impl Future<Output = Result<TaskList>> + Send;
    fn update_task_list(
        &self,
        id: &str,
        change: TaskListUpdateShape<'_>,
    ) -> impl Future<Output = Result<TaskList>> + Send;

    // ── Notes ─────────────────────────────────────────────────────────────────

    fn get_notes<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Note>>>> + Send;
    /// A location's notes, deleted ones left out, in no particular order. Reads a GSI,
    /// so a write may take a moment to show up.
    fn list_notes(&self, location_id: &str) -> impl Future<Output = Result<Vec<Note>>> + Send;
    fn create_note(&self, note: NewNote<'_>) -> impl Future<Output = Result<Note>> + Send;
    fn update_note(
        &self,
        id: &str,
        change: NoteUpdateShape<'_>,
    ) -> impl Future<Output = Result<Note>> + Send;

    fn get_note_folders<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<NoteFolder>>>> + Send;
    /// A location's note folders, deleted ones left out.
    fn list_note_folders(
        &self,
        location_id: &str,
    ) -> impl Future<Output = Result<Vec<NoteFolder>>> + Send;
    fn create_note_folder(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> impl Future<Output = Result<NoteFolder>> + Send;
    fn update_note_folder(
        &self,
        id: &str,
        change: NoteFolderUpdateShape<'_>,
    ) -> impl Future<Output = Result<NoteFolder>> + Send;

    // ── Equipment ─────────────────────────────────────────────────────────────

    fn get_equipment<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Equipment>>>> + Send;
    /// A location's equipment, deleted ones left out, in no particular order. Reads a
    /// GSI, so a write may take a moment to show up.
    fn list_equipment(
        &self,
        location_id: &str,
    ) -> impl Future<Output = Result<Vec<Equipment>>> + Send;
    /// Add a piece of equipment together with its first log entry, atomically.
    fn create_equipment(
        &self,
        equipment: NewEquipment<'_>,
    ) -> impl Future<Output = Result<(Equipment, EquipmentLogEntry)>> + Send;
    fn update_equipment(
        &self,
        id: &str,
        change: EquipmentUpdateShape<'_>,
    ) -> impl Future<Output = Result<Equipment>> + Send;
    /// Append a log entry and, atomically with it, apply it to the equipment: a status
    /// change sets the status, a check stamps `last_checked_at`. Fails with
    /// [`Error::NotFound`] when the equipment does not exist, is deleted or is no longer
    /// in the status the entry starts from (`from`, or a check's `status`), so the log
    /// never records a change from a status it was not in. Returns the equipment after
    /// the write and the new entry.
    fn add_equipment_log_entry(
        &self,
        entry: NewEquipmentLogEntry<'_>,
    ) -> impl Future<Output = Result<(Equipment, EquipmentLogEntry)>> + Send;
    /// A piece of equipment's log, in no particular order. Kept when the equipment is
    /// deleted.
    fn list_equipment_log(
        &self,
        equipment_id: &str,
    ) -> impl Future<Output = Result<Vec<EquipmentLogEntry>>> + Send;

    // ── Equipment reservations ────────────────────────────────────────────────

    fn get_equipment_reservations<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<EquipmentReservation>>>> + Send;
    /// A location's reservations that end after `ending_after` (Unix seconds), deleted
    /// ones left out, in no particular order. Reads a GSI keyed on the end time, so past
    /// reservations cost nothing, and a write may take a moment to show up.
    fn list_equipment_reservations(
        &self,
        location_id: &str,
        ending_after: u64,
    ) -> impl Future<Output = Result<Vec<EquipmentReservation>>> + Send;
    fn create_equipment_reservation(
        &self,
        reservation: NewEquipmentReservation<'_>,
    ) -> impl Future<Output = Result<EquipmentReservation>> + Send;
    fn update_equipment_reservation(
        &self,
        id: &str,
        change: EquipmentReservationUpdateShape<'_>,
    ) -> impl Future<Output = Result<EquipmentReservation>> + Send;

    // ── Map ───────────────────────────────────────────────────────────────────

    fn get_map_pins<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<MapPin>>>> + Send;
    /// A location's map pins, deleted ones left out, in no particular order. Reads a
    /// GSI, so a write may take a moment to show up.
    fn list_map_pins(&self, location_id: &str) -> impl Future<Output = Result<Vec<MapPin>>> + Send;
    fn create_map_pin(&self, pin: NewMapPin<'_>) -> impl Future<Output = Result<MapPin>> + Send;
    fn update_map_pin(
        &self,
        id: &str,
        change: MapPinUpdateShape<'_>,
    ) -> impl Future<Output = Result<MapPin>> + Send;

    fn get_map_layers<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<MapLayer>>>> + Send;
    /// A location's map layers, deleted ones left out.
    fn list_map_layers(
        &self,
        location_id: &str,
    ) -> impl Future<Output = Result<Vec<MapLayer>>> + Send;
    fn create_map_layer(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> impl Future<Output = Result<MapLayer>> + Send;
    fn update_map_layer(
        &self,
        id: &str,
        change: MapLayerUpdateShape<'_>,
    ) -> impl Future<Output = Result<MapLayer>> + Send;

    // ── Calendar ──────────────────────────────────────────────────────────────

    fn get_calendars<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Calendar>>>> + Send;
    /// A location's calendars, deleted ones left out.
    fn list_calendars(
        &self,
        location_id: &str,
    ) -> impl Future<Output = Result<Vec<Calendar>>> + Send;
    fn create_calendar(
        &self,
        location_id: &str,
        name: &str,
        color: &str,
        created_by: Option<&str>,
    ) -> impl Future<Output = Result<Calendar>> + Send;
    fn update_calendar(
        &self,
        id: &str,
        change: CalendarUpdateShape<'_>,
    ) -> impl Future<Output = Result<Calendar>> + Send;

    fn get_calendar_events<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<CalendarEvent>>>> + Send;
    /// A location's events whose last occurrence ends after `ending_after` (Unix
    /// seconds), deleted ones left out, in no particular order. Reads a GSI keyed on
    /// `last_end`, so finished events cost nothing, and a write may take a moment to
    /// show up.
    fn list_calendar_events(
        &self,
        location_id: &str,
        ending_after: u64,
    ) -> impl Future<Output = Result<Vec<CalendarEvent>>> + Send;
    fn create_calendar_event(
        &self,
        event: NewCalendarEvent<'_>,
    ) -> impl Future<Output = Result<CalendarEvent>> + Send;
    fn update_calendar_event(
        &self,
        id: &str,
        change: CalendarEventUpdateShape<'_>,
    ) -> impl Future<Output = Result<CalendarEvent>> + Send;

    // ── On Call ───────────────────────────────────────────────────────────────

    fn get_on_call_roles<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<OnCallRole>>>> + Send;
    /// A location's on-call roles, deleted ones left out, in no particular order. Reads
    /// a GSI, so a write may take a moment to show up.
    fn list_on_call_roles(
        &self,
        location_id: &str,
    ) -> impl Future<Output = Result<Vec<OnCallRole>>> + Send;
    fn create_on_call_role(
        &self,
        role: NewOnCallRole<'_>,
    ) -> impl Future<Output = Result<OnCallRole>> + Send;
    fn update_on_call_role(
        &self,
        id: &str,
        change: OnCallRoleUpdateShape<'_>,
    ) -> impl Future<Output = Result<OnCallRole>> + Send;

    // ── Shared contacts ───────────────────────────────────────────────────────

    fn get_shared_contacts<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<SharedContact>>>> + Send;
    /// A location's shared contacts, or the statewide ones, deleted ones left out, in no
    /// particular order. Reads a GSI, so a write may take a moment to show up.
    fn list_shared_contacts(
        &self,
        scope: SharedContactScope<'_>,
    ) -> impl Future<Output = Result<Vec<SharedContact>>> + Send;
    /// Every live shared contact, every location's and the statewide ones, in no
    /// particular order: one strongly consistent scan, for the offline sync, which sends
    /// a member all of theirs at once, including a write made just before. The table is
    /// small (a few contacts per location).
    fn list_all_shared_contacts(&self) -> impl Future<Output = Result<Vec<SharedContact>>> + Send;
    fn create_shared_contact(
        &self,
        contact: NewSharedContact<'_>,
    ) -> impl Future<Output = Result<SharedContact>> + Send;
    fn update_shared_contact(
        &self,
        id: &str,
        change: SharedContactUpdateShape<'_>,
    ) -> impl Future<Output = Result<SharedContact>> + Send;

    // ── Locate ────────────────────────────────────────────────────────────────

    /// Shares by user ID, whether or not they have ended or lapsed.
    fn get_location_shares<T: AsRef<str> + Sync>(
        &self,
        person_ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<LocationShare>>>> + Send;
    /// A location's shares, in no particular order. Includes any that have lapsed but
    /// not yet been removed by the TTL, so callers check `expires_at`. Reads a GSI, so a
    /// write may take a moment to show up.
    fn list_location_shares(
        &self,
        location_id: &str,
    ) -> impl Future<Output = Result<Vec<LocationShare>>> + Send;
    /// Store a new share, replacing any the user already has.
    fn put_location_share(&self, share: &LocationShare) -> impl Future<Output = Result<()>> + Send;
    /// Record a newer position on a share. Fails with [`Error::NotFound`] when the user
    /// has no share, or it ended at or before `updated_at`. Returns the share as it is
    /// after the write.
    fn update_location_share_position(
        &self,
        person_id: &str,
        position: &SharedPosition,
        updated_at: u64,
        expires_at: u64,
    ) -> impl Future<Output = Result<LocationShare>> + Send;
    /// Remove the user's share; returns it, or None if they had none.
    fn delete_location_share(
        &self,
        person_id: &str,
    ) -> impl Future<Output = Result<Option<LocationShare>>> + Send;

    // ── Notifications ─────────────────────────────────────────────────────────

    /// Store notifications (one per recipient), in batches. Anything DynamoDB defers is
    /// retried; an error means some may not have been written.
    fn put_notifications(
        &self,
        records: Vec<NotificationRecord>,
    ) -> impl Future<Output = Result<()>> + Send;
    /// A user's notifications, newest first, starting after the `after_sk` cursor (a
    /// previous row's `sk`) when given. `unread_only` lists only unread ones. Unread
    /// rows come through a GSI, so a notification written or marked read a moment ago
    /// may not show yet. Includes any past `expires_at` the TTL hasn't removed.
    fn list_notifications(
        &self,
        person_id: &str,
        after_sk: Option<&str>,
        limit: usize,
        unread_only: bool,
    ) -> impl Future<Output = Result<Vec<NotificationRecord>>> + Send;
    /// How many notifications the user has unread, counting no further than `cap`.
    /// Reads a GSI, so it can lag a write by a moment.
    fn count_unread_notifications(
        &self,
        person_id: &str,
        cap: usize,
    ) -> impl Future<Output = Result<usize>> + Send;
    /// Mark the user's notifications with these `sk`s read at `read_at`. One that is
    /// already read, or no longer exists, is left alone and is not an error.
    fn mark_notifications_read(
        &self,
        person_id: &str,
        sks: &[String],
        read_at: u64,
    ) -> impl Future<Output = Result<()>> + Send;

    // ── Audit log ─────────────────────────────────────────────────────────────

    /// Append an entry to the `audit_log` table. Callers go through
    /// [`crate::audit::AuditingHandler`], which builds entries for business writes and
    /// treats a failure here as non-fatal; nothing else should call this directly.
    fn put_audit_entry(&self, entry: &AuditEntry) -> impl Future<Output = Result<()>> + Send;

    /// One page of the audit log. Each returned entry is a single table item: its
    /// `location_ids` holds that item's location (empty for a global entity), not every
    /// location the event touched. A row that cannot be read is logged and skipped
    /// rather than failing the page. An `after` cursor that is not an [`audit_sk`] is an
    /// error; validate with [`parse_audit_cursor`] first to report it as a user error.
    fn list_audit_entries(
        &self,
        query: ListAuditEntriesQuery,
    ) -> impl Future<Output = Result<Vec<AuditEntry>>> + Send;
}

#[cfg(test)]
mod tests {
    use super::{Person, Session, audit_sk, email_key, parse_audit_cursor};

    #[test]
    fn an_email_key_ignores_case_and_surrounding_space() {
        assert_eq!(
            email_key("  Alice.Anderson@Example.COM "),
            "alice.anderson@example.com"
        );
        assert_eq!(email_key("bob@example.com"), "bob@example.com");
    }

    #[test]
    fn an_audit_sk_is_a_valid_cursor_that_carries_its_id() {
        let sk = audit_sk(1_700_000_000, "Ab3dEf6hIj9k");
        assert_eq!(sk, "1700000000#Ab3dEf6hIj9k");
        assert_eq!(parse_audit_cursor(&sk), Some("Ab3dEf6hIj9k"));
        // Small timestamps are padded so lexical order stays time order.
        assert_eq!(audit_sk(5, "x"), "0000000005#x");
        assert!(audit_sk(9, "x") < audit_sk(10, "x"));
    }

    #[test]
    fn audit_cursors_that_are_not_an_sk_are_rejected() {
        for bad in [
            "",
            "#",
            "1700000000",
            "1700000000#",
            "#abc",
            "17x0#abc",
            "1700000000#a#b",
            "1700000000#a b",
            "1700000000#a/b",
            "-1#abc",
        ] {
            assert_eq!(parse_audit_cursor(bad), None, "{bad:?}");
        }
        assert_eq!(parse_audit_cursor(&format!("1#{}", "a".repeat(65))), None);
    }

    fn session(active: bool, key_released_at: Option<u64>) -> Session {
        Session {
            id: "s1".to_string(),
            name: "Front Desk".to_string(),
            location_id: "loc-a".to_string(),
            active,
            last_contact: Some(1_700_000_000),
            client_version: None,
            client_info: None,
            client_info_updated_at: None,
            config: serde_json::Map::new(),
            healthcheck_url: None,
            public_key: None,
            key_fingerprint: None,
            key_expires_at: None,
            key_released_at,
            retired_at: None,
            version: 1,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn a_normal_kiosk_is_live() {
        assert!(session(true, None).is_live());
    }

    #[test]
    fn a_soft_deleted_kiosk_is_not_live() {
        assert!(!session(false, None).is_live());
    }

    #[test]
    fn a_kiosk_whose_key_was_released_is_not_live_despite_keeping_active() {
        // The whole point of `key_released_at`: the record keeps `active` so it stays in
        // `active-location_id-index` and remains listed, but it must authenticate
        // nothing and count as no kiosk.
        let leftover = session(true, Some(1_700_000_100));
        assert!(leftover.active);
        assert!(!leftover.is_live());
    }

    fn member(home: &str, secondary: &[&str]) -> Person {
        Person {
            id: "p1".into(),
            location_id: home.into(),
            first_name: "First".into(),
            last_name: "Last".into(),
            registration_number: None,
            ses_api_person_id: None,
            email: None,
            mobile: None,
            phones: None,
            preferred_name: None,
            badge_state: None,
            capability_tags: None,
            deleted: None,
            missing_since: None,
            secondary_location_ids: secondary.iter().map(|s| s.to_string()).collect(),
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn a_member_belongs_to_their_home_unit_and_their_secondary_units() {
        let p = member("home", &["a", "b"]);
        assert_eq!(p.member_location_ids(), vec!["home", "a", "b"]);
        assert!(p.is_member_of("home"));
        assert!(p.is_member_of("b"));
        assert!(!p.is_member_of("c"));
        assert_eq!(member("home", &[]).member_location_ids(), vec!["home"]);
    }

    #[test]
    fn a_home_unit_also_in_the_secondary_set_is_listed_once() {
        // Member sync moved them home to `a` since directory sync last wrote the set.
        let p = member("a", &["a", "b"]);
        assert_eq!(p.member_location_ids(), vec!["a", "b"]);
    }
}
