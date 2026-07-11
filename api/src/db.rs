use chrono::NaiveDate;
use std::future::Future;
use thiserror::Error;

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
    pub location_grants: Vec<String>,
    pub access_time: Option<u64>,
    pub email_config: serde_json::Map<String, serde_json::Value>,
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
    },
    AccessTime,
    EmailConfig {
        email_config: serde_json::Map<String, serde_json::Value>,
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
    pub deleted: Option<u64>,
    pub created_at: Option<u64>,
    pub updated_at: Option<u64>,
}

impl HasID for Person {
    fn id(&self) -> &str {
        &self.id
    }
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
    Email {
        email: Option<&'a str>,
    },
    Undelete,
    Delete,
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
    pub code: Option<String>,
    pub config: serde_json::Map<String, serde_json::Value>,
    pub healthcheck_url: Option<String>,
    /// Base64 (standard) SPKI DER of the kiosk's enrolled ECDSA P-256 public key.
    /// Present only for key-enrolled sessions (the QR/public-key flow); code-enrolled
    /// sessions leave this unset.
    pub public_key: Option<String>,
    /// Hex SHA-256 of the SPKI DER above. Backs the `key_fingerprint-index` GSI, so it
    /// must be omitted (never written as Null) when absent.
    pub key_fingerprint: Option<String>,
    /// Unix seconds after which the enrolled key no longer authenticates; extended on
    /// each verified request. A kiosk offline past this must re-enroll.
    pub key_expires_at: Option<u64>,
    pub created_at: Option<u64>,
    pub updated_at: Option<u64>,
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
    },
    LastSyncTime {
        time: u64,
    },
    Name {
        name: &'a str,
    },
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
        /// When set, also bump `key_expires_at` to this value (the sliding 14-day
        /// window for key-enrolled sessions). Written in the same update as the
        /// throttled `last_contact` refresh, so it costs no extra write.
        extend_key_expires_at: Option<u64>,
    },
    Delete,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Period {
    pub id: String,
    pub person_id: String,
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
    },
    TimeCategory {
        start_time: i64,
        end_time: i64,
        category_id: &'a str,
        signed_out_session_id: Option<&'a str>,
    },
    Delete,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Location {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub nitc_enabled: Option<u64>,
    pub ses_api_headquarters_id: Option<String>,
    pub last_successful_member_sync: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
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
    pub nitc_participant_type: Option<String>,
    pub nitc_group_id: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
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

#[derive(Clone, Debug, PartialEq)]
pub struct WebauthnCredential {
    /// Credential ID (base64url) — partition key in DynamoDB.
    pub id: String,
    pub user_id: String,
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

pub trait Handler {
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
    fn get_sessions<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Session>>>> + Send;
    /// Returns the IDs of every session whose scan code matches (from the code GSI).
    /// Expected to be at most one; returns multiple only when the uniqueness invariant
    /// has been violated. Does not verify the underlying rows — callers should fetch each
    /// ID with [`get_sessions`](Self::get_sessions) to confirm it exists.
    fn get_session_id_by_code(
        &self,
        code: &str,
    ) -> impl Future<Output = Result<Vec<String>>> + Send;
    fn wipe_session_code(&self, id: &str) -> impl Future<Output = Result<()>> + Send;
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
        page: ListPeriodsPage,
    ) -> impl Future<Output = Result<Vec<Period>>> + Send;
    fn get_periods<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Period>>>> + Send;
    /// Ends the given period by setting its end_time to the current time
    fn end_period(&self, period: &Period) -> impl Future<Output = Result<Period>> + Send;
    /// Create a new period starting now for the given person at the given location
    fn start_period_for_person_location(
        &self,
        person_id: &str,
        location_id: &str,
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
    fn create_period(
        &self,
        person_id: &str,
        location_id: &str,
        category_id: &str,
        start_time: u64,
        end_time: u64,
    ) -> impl Future<Output = Result<Period>> + Send;
    fn update_period(
        &self,
        id: &str,
        change: PeriodUpdateShape<'_>,
    ) -> impl Future<Output = Result<()>> + Send;
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
        key: Option<SessionKeyParams<'_>>,
    ) -> impl Future<Output = Result<Session>> + Send;
    fn update_session(
        &self,
        id: &str,
        change: SessionUpdateShape<'_>,
    ) -> impl Future<Output = Result<()>> + Send;

    fn get_api_token(&self, id: &str) -> impl Future<Output = Result<Option<ApiToken>>> + Send;
    fn get_api_token_by_hash(
        &self,
        token_hash: &str,
    ) -> impl Future<Output = Result<Option<ApiToken>>> + Send;
    fn list_api_tokens(&self) -> impl Future<Output = Result<Vec<ApiToken>>> + Send;
    fn create_api_token(
        &self,
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
        name: &str,
        nitc_group_id: Option<&str>,
        nitc_participant_type: Option<&str>,
    ) -> impl Future<Output = Result<Category>> + Send;
    fn update_category(
        &self,
        id: &str,
        name: &str,
        active: bool,
        nitc_group_id: Option<&str>,
        nitc_participant_type: Option<&str>,
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

    fn get_nitc_events_by_ids<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<NitcEvent>>> + Send;

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

    fn create_user_token(
        &self,
        token_hash: &str,
        user_id: &str,
        expires_at: u64,
    ) -> impl Future<Output = Result<UserToken>> + Send;

    fn get_user_token_by_hash(
        &self,
        token_hash: &str,
    ) -> impl Future<Output = Result<Option<UserToken>>> + Send;

    fn update_user_token(
        &self,
        id: &str,
        change: UserTokenUpdateShape,
    ) -> impl Future<Output = Result<()>> + Send;

    fn delete_user_token(&self, id: &str) -> impl Future<Output = Result<()>> + Send;

    // ── WebAuthn passkey credentials ─────────────────────────────────────────

    fn create_webauthn_credential(
        &self,
        id: &str,
        user_id: &str,
        name: &str,
        passkey_json: &str,
    ) -> impl Future<Output = Result<WebauthnCredential>> + Send;

    fn get_webauthn_credential(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<WebauthnCredential>>> + Send;

    fn list_webauthn_credentials_by_user(
        &self,
        user_id: &str,
    ) -> impl Future<Output = Result<Vec<WebauthnCredential>>> + Send;

    fn count_webauthn_credentials_by_user(
        &self,
        user_id: &str,
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
}
