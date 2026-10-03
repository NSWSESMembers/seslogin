//! Audit log: who changed what, recorded for every business database write.
//!
//! [`AuditingHandler`] wraps a [`db::Handler`] and implements it by delegating every
//! method. For each *business* write it reads the entity's "before" state through the
//! inner handler, performs the write, and — only if the write succeeded — records an
//! [`AuditEntry`] with the actor, the affected locations and a field-level diff via
//! [`db::Handler::put_audit_entry`].
//!
//! **Who** made the write is not a parameter of any `db::Handler` method, so it travels
//! in a task-local, [`AUDIT_CONTEXT`], set at each entry point (the HTTP/GraphQL request
//! handlers, the OAuth token endpoint, the sync jobs, the CLI). The DataLoader spawner
//! copies it into the tasks it spawns (see [`crate::request_metrics::request_spawner`]).
//! A write made with no context in scope is still recorded, with an
//! [`Actor::Unknown`] actor and a warning, rather than dropped.
//!
//! **Auditing is best-effort.** It must never fail or alter a business write: a failed
//! before-read is logged and the write carries on, and a failed `put_audit_entry` (the
//! table not yet created, throttling, …) is logged at `error!` and swallowed — the
//! mutation's own result is returned unchanged.
//!
//! **Adding a write method to [`db::Handler`]** — the trait has no default methods, so
//! the compiler forces a decision here. Classify it: audit it (read before, write,
//! record after), or pass it straight through *and say why* (bookkeeping such as
//! `last_used_at` touches and NITC export state is deliberately not audited).
//!
//! Secrets never reach an entry: token hashes, passkey material, session codes, kiosk
//! public keys and OAuth secrets are omitted, or replaced by the string `"[redacted]"`
//! where the fact that the field changed is worth showing.

use std::future::Future;

use serde_json::{Value, json};
use tracing::{error, warn};

use crate::auth::AuthInfo;
use crate::db::{
    self, ApiToken, AuditAction, AuditEntityType, AuditEntry, AuditFieldChange, Category,
    ListSessionsQuery, Location, OAuthGrant, Period, Person, Session, User,
};

/// Placeholder stored in place of a secret value.
pub const REDACTED: &str = "[redacted]";

// ── Actor and request context ────────────────────────────────────────────────

/// Who made a write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Actor {
    /// A signed-in user. `via` says how they came in when it is not an ordinary web
    /// session: `oauth_grant:<id>` (MCP), `user_token:<id>` (opaque token) or
    /// `oauth_client:<client_id>` (the OAuth token endpoint acting for them).
    User {
        id: String,
        via: Option<String>,
    },
    /// A kiosk.
    Session {
        id: String,
    },
    ApiToken {
        id: String,
    },
    /// A member's single-period edit link.
    PeriodLink {
        period_id: String,
    },
    /// A background job or the CLI, named by `job` (`member-sync`, `cli`, …).
    System {
        job: String,
    },
    /// A request that carried no valid credentials.
    Unauthenticated,
    /// No audit context was in scope — a code path that forgot to set one.
    Unknown,
}

impl Actor {
    /// Stable string stored as `actor_kind`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::User { .. } => "user",
            Self::Session { .. } => "session",
            Self::ApiToken { .. } => "api_token",
            Self::PeriodLink { .. } => "period_link",
            Self::System { .. } => "system",
            Self::Unauthenticated => "unauthenticated",
            Self::Unknown => "unknown",
        }
    }

    /// Stored as `actor_id`: the user/session/token id, the period id for a period
    /// link, or the job name for a system actor.
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::User { id, .. } | Self::Session { id } | Self::ApiToken { id } => Some(id),
            Self::PeriodLink { period_id } => Some(period_id),
            Self::System { job } => Some(job),
            Self::Unauthenticated | Self::Unknown => None,
        }
    }

    /// Stored as `actor_via`.
    pub fn via(&self) -> Option<&str> {
        match self {
            Self::User { via, .. } => via.as_deref(),
            _ => None,
        }
    }

    pub fn system(job: &str) -> Self {
        Self::System {
            job: job.to_string(),
        }
    }

    /// The actor behind a request's (optional) verified credentials.
    pub fn from_auth_info(auth: Option<&AuthInfo>) -> Self {
        match auth {
            None => Self::Unauthenticated,
            Some(AuthInfo::User {
                id,
                token_id,
                grant_id,
                ..
            }) => Self::User {
                id: id.clone(),
                via: match (grant_id, token_id) {
                    (Some(grant), _) => Some(format!("oauth_grant:{grant}")),
                    (None, Some(token)) => Some(format!("user_token:{token}")),
                    (None, None) => None,
                },
            },
            Some(AuthInfo::Session { id, .. }) => Self::Session { id: id.clone() },
            Some(AuthInfo::ApiToken { id, .. }) => Self::ApiToken { id: id.clone() },
            Some(AuthInfo::PeriodLink { period_id }) => Self::PeriodLink {
                period_id: period_id.clone(),
            },
        }
    }
}

/// The audit-relevant facts about the request or job a write is made under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditContext {
    pub actor: Actor,
    pub ip: Option<String>,
}

impl AuditContext {
    pub fn new(actor: Actor, ip: Option<String>) -> Self {
        Self { actor, ip }
    }

    /// A background job or CLI invocation: no IP.
    pub fn system(job: &str) -> Self {
        Self::new(Actor::system(job), None)
    }

    /// The context for a request, from its verified credentials (if any) and client IP.
    pub fn for_request(auth: Option<&AuthInfo>, ip: Option<&str>) -> Self {
        Self::new(Actor::from_auth_info(auth), ip.map(str::to_string))
    }
}

tokio::task_local! {
    /// Who the current task is acting for. See the module docs.
    pub static AUDIT_CONTEXT: AuditContext;
}

/// Run `f` with `ctx` as the audit context.
pub async fn scope<F: Future>(ctx: AuditContext, f: F) -> F::Output {
    AUDIT_CONTEXT.scope(ctx, f).await
}

/// Run `f` as the actor behind a request's credentials. The single entry-point helper
/// the HTTP servers, the MCP handler and the tests all use, so they cannot drift.
pub async fn scope_for_request<F: Future>(
    auth: Option<&AuthInfo>,
    ip: Option<&str>,
    f: F,
) -> F::Output {
    scope(AuditContext::for_request(auth, ip), f).await
}

/// The context in scope, or an [`Actor::Unknown`] one (with a warning) when none is.
pub fn current() -> AuditContext {
    AUDIT_CONTEXT.try_with(|c| c.clone()).unwrap_or_else(|_| {
        warn!("audited write made with no AUDIT_CONTEXT in scope; recording the actor as unknown");
        AuditContext::new(Actor::Unknown, None)
    })
}

/// Just the client IP of the context in scope, without the warning [`current`] gives.
pub fn current_ip() -> Option<String> {
    AUDIT_CONTEXT.try_with(|c| c.ip.clone()).ok().flatten()
}

// ── Field-level diffs ────────────────────────────────────────────────────────

/// An entity's audited fields, in display order. `None` means the field is absent.
#[derive(Clone, Debug, Default, PartialEq)]
struct Fields(Vec<(&'static str, Option<Value>)>);

impl Fields {
    fn set(&mut self, name: &'static str, value: Option<Value>) {
        match self.0.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = value,
            None => self.0.push((name, value)),
        }
    }

    fn with(mut self, name: &'static str, value: Option<Value>) -> Self {
        self.set(name, value);
        self
    }

    fn get(&self, name: &str) -> Option<&Value> {
        self.0
            .iter()
            .find(|(n, _)| *n == name)
            .and_then(|(_, v)| v.as_ref())
    }
}

/// The fields that differ between `before` and `after`, in `after`'s order (fields only
/// `before` has follow).
fn diff(before: &Fields, after: &Fields) -> Vec<AuditFieldChange> {
    let mut names: Vec<&'static str> = after.0.iter().map(|(n, _)| *n).collect();
    for (name, _) in &before.0 {
        if !names.contains(name) {
            names.push(name);
        }
    }
    names
        .into_iter()
        .filter_map(|name| {
            let (b, a) = (before.get(name), after.get(name));
            (b != a).then(|| AuditFieldChange {
                field: name.to_string(),
                before: b.cloned(),
                after: a.cloned(),
            })
        })
        .collect()
}

/// Everything an entity has, as changes from nothing.
fn creation(after: &Fields) -> Vec<AuditFieldChange> {
    diff(&Fields::default(), after)
}

/// Everything an entity had, as changes to nothing.
fn removal(before: &Fields) -> Vec<AuditFieldChange> {
    diff(before, &Fields::default())
}

fn text(v: &str) -> Option<Value> {
    Some(Value::String(v.to_string()))
}

fn opt_text(v: Option<&str>) -> Option<Value> {
    v.and_then(text)
}

fn num(v: u64) -> Option<Value> {
    Some(json!(v))
}

fn opt_num(v: Option<u64>) -> Option<Value> {
    v.and_then(num)
}

fn flag(v: bool) -> Option<Value> {
    Some(Value::Bool(v))
}

/// A set of strings as a sorted JSON array, or absent when empty (an empty set is not
/// stored, so "no grants" and "grants attribute missing" must not diff).
fn string_set(v: &[String]) -> Option<Value> {
    if v.is_empty() {
        return None;
    }
    let mut sorted = v.to_vec();
    sorted.sort();
    Some(json!(sorted))
}

fn map(v: &serde_json::Map<String, Value>) -> Option<Value> {
    Some(Value::Object(v.clone()))
}

fn redacted() -> Option<Value> {
    text(REDACTED)
}

fn full_name(first: &str, last: &str) -> String {
    format!("{first} {last}").trim().to_string()
}

// ── User ─────────────────────────────────────────────────────────────────────

fn user_fields(u: &User) -> Fields {
    Fields::default()
        .with("email", text(&u.email))
        .with("is_super", flag(u.is_super))
        .with("is_dev", flag(u.is_dev))
        .with("enabled", flag(u.enabled))
        .with("location_grants", string_set(&u.location_grants))
        .with(
            "location_read_only_grants",
            string_set(&u.location_read_only_grants),
        )
        .with("email_config", {
            // An empty config is the default, not something worth listing.
            (!u.email_config.is_empty())
                .then(|| map(&u.email_config))
                .flatten()
        })
        .with(
            "disaggregate_virtual_periods",
            flag(u.disaggregate_virtual_periods),
        )
}

pub fn user_create_changes(u: &User) -> Vec<AuditFieldChange> {
    creation(&user_fields(u))
}

/// Whether `update_user` with this shape is audited. `AccessTime` is a bookkeeping touch
/// made on authentication and is deliberately not.
pub fn user_shape_is_audited(shape: &db::UserUpdateShape<'_>) -> bool {
    !matches!(shape, db::UserUpdateShape::AccessTime)
}

pub fn user_update_changes(
    before: Option<&User>,
    shape: &db::UserUpdateShape<'_>,
) -> Vec<AuditFieldChange> {
    let before = before.map(user_fields).unwrap_or_default();
    let mut after = before.clone();
    match shape {
        db::UserUpdateShape::Fields {
            email,
            is_super,
            is_dev,
            enabled,
            location_grants,
            location_read_only_grants,
        } => {
            after.set("email", text(email));
            after.set("is_super", flag(*is_super));
            after.set("is_dev", flag(*is_dev));
            after.set("enabled", flag(*enabled));
            after.set("location_grants", string_set(location_grants));
            if let Some(grants) = location_read_only_grants {
                after.set("location_read_only_grants", string_set(grants));
            }
        }
        db::UserUpdateShape::AccessTime => {}
        db::UserUpdateShape::EmailConfig { email_config } => {
            after.set(
                "email_config",
                (!email_config.is_empty())
                    .then(|| map(email_config))
                    .flatten(),
            );
        }
        db::UserUpdateShape::DisaggregateVirtualPeriods { value } => {
            after.set("disaggregate_virtual_periods", flag(*value));
        }
    }
    diff(&before, &after)
}

fn user_label_after(before: Option<&User>, shape: &db::UserUpdateShape<'_>) -> Option<String> {
    match shape {
        db::UserUpdateShape::Fields { email, .. } => Some(email.to_string()),
        _ => before.map(|u| u.email.clone()),
    }
}

// ── Person ───────────────────────────────────────────────────────────────────

fn person_fields(p: &Person) -> Fields {
    Fields::default()
        .with("first_name", text(&p.first_name))
        .with("last_name", text(&p.last_name))
        .with(
            "registration_number",
            opt_text(p.registration_number.as_deref()),
        )
        .with("location_id", text(&p.location_id))
        .with(
            "ses_api_person_id",
            opt_text(p.ses_api_person_id.as_deref()),
        )
        .with("email", opt_text(p.email.as_deref()))
        .with("deleted", opt_num(p.deleted))
        .with("missing_since", opt_num(p.missing_since))
}

pub fn person_create_changes(p: &Person) -> Vec<AuditFieldChange> {
    creation(&person_fields(p))
}

/// What a person update does and the field changes it makes. `now` stamps the soft-delete
/// time, which the database sets itself. `Undelete` is a [`AuditAction::Restore`] and
/// `Delete` a [`AuditAction::Delete`]; everything else is an update.
pub fn person_update_changes(
    before: Option<&Person>,
    shape: &db::PersonUpdateShape<'_>,
    now: u64,
) -> (AuditAction, Vec<AuditFieldChange>) {
    let before = before.map(person_fields).unwrap_or_default();
    let mut after = before.clone();
    let mut action = AuditAction::Update;
    match shape {
        db::PersonUpdateShape::Fields {
            first_name,
            last_name,
            registration_number,
        } => {
            after.set("first_name", text(first_name));
            after.set("last_name", text(last_name));
            after.set("registration_number", text(registration_number));
        }
        db::PersonUpdateShape::Location { location_id } => {
            after.set("location_id", text(location_id));
        }
        db::PersonUpdateShape::SesApiPersonId { ses_api_person_id } => {
            after.set("ses_api_person_id", opt_text(*ses_api_person_id));
        }
        db::PersonUpdateShape::Email { email } => {
            after.set("email", opt_text(*email));
        }
        db::PersonUpdateShape::MissingSince { missing_since } => {
            after.set("missing_since", opt_num(*missing_since));
        }
        db::PersonUpdateShape::Undelete => {
            action = AuditAction::Restore;
            after.set("deleted", None);
            after.set("missing_since", None);
        }
        db::PersonUpdateShape::Delete => {
            action = AuditAction::Delete;
            after.set("deleted", num(now));
            after.set("missing_since", None);
        }
    }
    (action, diff(&before, &after))
}

/// The locations a person update affects: the person's current one, plus the new one
/// when the update moves them.
pub fn person_update_locations(
    before: Option<&Person>,
    shape: &db::PersonUpdateShape<'_>,
) -> Vec<String> {
    let mut locations: Vec<String> = before.map(|p| p.location_id.clone()).into_iter().collect();
    if let db::PersonUpdateShape::Location { location_id } = shape {
        locations.push(location_id.to_string());
    }
    locations
}

fn person_label_after(
    before: Option<&Person>,
    shape: &db::PersonUpdateShape<'_>,
) -> Option<String> {
    match shape {
        db::PersonUpdateShape::Fields {
            first_name,
            last_name,
            ..
        } => Some(full_name(first_name, last_name)),
        _ => before.map(|p| full_name(&p.first_name, &p.last_name)),
    }
}

// ── Period ───────────────────────────────────────────────────────────────────

fn period_fields(p: &Period) -> Fields {
    Fields::default()
        .with("person_id", opt_text(p.person_id.as_deref()))
        .with("guest_name", opt_text(p.guest_name.as_deref()))
        .with("comment", opt_text(p.comment.as_deref()))
        .with("location_id", text(&p.location_id))
        .with("category_id", opt_text(p.category_id.as_deref()))
        .with("start_time", num(p.start_time))
        .with("end_time", opt_num(p.end_time))
        .with(
            "signed_in_session_id",
            opt_text(p.signed_in_session_id.as_deref()),
        )
        .with(
            "signed_out_session_id",
            opt_text(p.signed_out_session_id.as_deref()),
        )
        .with("deleted", opt_num(p.deleted))
}

pub fn period_create_changes(p: &Period) -> Vec<AuditFieldChange> {
    creation(&period_fields(p))
}

/// The changes `end_period` made, from the period it was given and the one it returned.
pub fn period_end_changes(before: &Period, after: &Period) -> Vec<AuditFieldChange> {
    diff(&period_fields(before), &period_fields(after))
}

fn apply_comment(after: &mut Fields, comment: &Option<Option<&str>>) {
    // `None` leaves the comment alone, `Some(None)` clears it, `Some(Some(_))` sets it.
    if let Some(comment) = comment {
        after.set("comment", opt_text(*comment));
    }
}

/// The action and field changes of an `update_period`. Times arrive signed; the stored
/// value is a non-negative number of seconds, so they are recorded as numbers as given.
pub fn period_update_changes(
    before: Option<&Period>,
    shape: &db::PeriodUpdateShape<'_>,
    now: u64,
) -> (AuditAction, Vec<AuditFieldChange>) {
    let before = before.map(period_fields).unwrap_or_default();
    let mut after = before.clone();
    let mut action = AuditAction::Update;
    match shape {
        db::PeriodUpdateShape::Fields {
            person_id,
            location_id,
            category_id,
            start_time,
            end_time,
            comment,
        } => {
            after.set("person_id", text(person_id));
            after.set("location_id", text(location_id));
            after.set("category_id", text(category_id));
            after.set("start_time", Some(json!(start_time)));
            after.set("end_time", Some(json!(end_time)));
            apply_comment(&mut after, comment);
        }
        db::PeriodUpdateShape::TimeCategory {
            start_time,
            end_time,
            category_id,
            signed_out_session_id,
            comment,
        } => {
            after.set("start_time", Some(json!(start_time)));
            after.set("end_time", Some(json!(end_time)));
            after.set("category_id", text(category_id));
            if let Some(session) = signed_out_session_id {
                after.set("signed_out_session_id", text(session));
            }
            apply_comment(&mut after, comment);
        }
        db::PeriodUpdateShape::Guest {
            guest_name,
            start_time,
            end_time,
            comment,
        } => {
            after.set("guest_name", text(guest_name));
            after.set("start_time", Some(json!(start_time)));
            after.set("end_time", Some(json!(end_time)));
            apply_comment(&mut after, comment);
        }
        db::PeriodUpdateShape::Delete => {
            action = AuditAction::Delete;
            after.set("deleted", num(now));
        }
    }
    (action, diff(&before, &after))
}

/// The locations a period update affects: the period's current one, plus the new one when
/// the update moves it.
pub fn period_update_locations(
    before: Option<&Period>,
    shape: &db::PeriodUpdateShape<'_>,
) -> Vec<String> {
    let mut locations: Vec<String> = before.map(|p| p.location_id.clone()).into_iter().collect();
    if let db::PeriodUpdateShape::Fields { location_id, .. } = shape {
        locations.push(location_id.to_string());
    }
    locations
}

// ── Session ──────────────────────────────────────────────────────────────────

/// `code` and `public_key` are secrets: only their presence is shown, as `"[redacted]"`.
/// `active` is carried so a soft delete reads `true → false`; it is dropped from a
/// creation, where it would only say the obvious.
fn session_fields(s: &Session) -> Fields {
    Fields::default()
        .with("name", text(&s.name))
        .with("location_id", text(&s.location_id))
        .with("config", map(&s.config))
        .with("healthcheck_url", opt_text(s.healthcheck_url.as_deref()))
        .with("active", flag(s.active))
        .with("code", s.code.as_ref().and(redacted()))
        .with("public_key", s.public_key.as_ref().and(redacted()))
        .with("key_expires_at", opt_num(s.key_expires_at))
        .with("key_released_at", opt_num(s.key_released_at))
}

pub fn session_create_changes(s: &Session) -> Vec<AuditFieldChange> {
    creation(&session_fields(s))
        .into_iter()
        .filter(|c| c.field != "active")
        .collect()
}

/// Redeeming a kiosk's one-time scan code removes it.
pub fn session_wipe_code_changes() -> Vec<AuditFieldChange> {
    vec![AuditFieldChange {
        field: "code".to_string(),
        before: redacted(),
        after: None,
    }]
}

/// Whether `update_session` with this shape is audited. `Info` is the kiosk's own
/// `last_contact`/client-info heartbeat and is deliberately not.
pub fn session_shape_is_audited(shape: &db::SessionUpdateShape<'_>) -> bool {
    !matches!(shape, db::SessionUpdateShape::Info { .. })
}

pub fn session_update_changes(
    before: Option<&Session>,
    shape: &db::SessionUpdateShape<'_>,
    now: u64,
) -> (AuditAction, Vec<AuditFieldChange>) {
    let before = before.map(session_fields).unwrap_or_default();
    let mut after = before.clone();
    let mut action = AuditAction::Update;
    match shape {
        db::SessionUpdateShape::Fields {
            name,
            config,
            healthcheck_url,
        } => {
            after.set("name", text(name));
            after.set("config", map(config));
            after.set("healthcheck_url", opt_text(*healthcheck_url));
        }
        db::SessionUpdateShape::Info { .. } => {}
        db::SessionUpdateShape::ExtendKey { expires_at } => {
            after.set("key_expires_at", num(*expires_at));
        }
        db::SessionUpdateShape::ReleaseKey { .. } => {
            after.set("public_key", None);
            after.set("key_expires_at", None);
            after.set("key_released_at", num(now));
        }
        db::SessionUpdateShape::Delete => {
            action = AuditAction::Delete;
            after.set("active", flag(false));
            after.set("public_key", None);
            after.set("key_expires_at", None);
        }
    }
    (action, diff(&before, &after))
}

fn session_label_after(
    before: Option<&Session>,
    shape: &db::SessionUpdateShape<'_>,
) -> Option<String> {
    match shape {
        db::SessionUpdateShape::Fields { name, .. } => Some(name.to_string()),
        _ => before.map(|s| s.name.clone()),
    }
}

// ── API token ────────────────────────────────────────────────────────────────

/// `token_hash` is never listed.
fn api_token_fields(t: &ApiToken) -> Fields {
    Fields::default()
        .with("name", text(&t.name))
        .with("location_grants", string_set(&t.location_grants))
        .with("read_only", flag(t.read_only))
        .with("expires_at", opt_num(t.expires_at))
        .with("created_by_user_id", text(&t.created_by_user_id))
        .with("revoked_at", opt_num(t.revoked_at))
}

pub fn api_token_create_changes(t: &ApiToken) -> Vec<AuditFieldChange> {
    creation(&api_token_fields(t))
}

/// Whether `update_api_token` with this shape is audited. `TouchLastUsed` is not.
pub fn api_token_shape_is_audited(shape: &db::ApiTokenUpdateShape<'_>) -> bool {
    !matches!(shape, db::ApiTokenUpdateShape::TouchLastUsed)
}

pub fn api_token_update_changes(
    before: Option<&ApiToken>,
    shape: &db::ApiTokenUpdateShape<'_>,
    now: u64,
) -> Vec<AuditFieldChange> {
    let before = before.map(api_token_fields).unwrap_or_default();
    let mut after = before.clone();
    match shape {
        db::ApiTokenUpdateShape::Fields {
            name,
            location_grants,
            read_only,
            expires_at,
        } => {
            after.set("name", text(name));
            after.set("location_grants", string_set(location_grants));
            after.set("read_only", flag(*read_only));
            after.set("expires_at", opt_num(*expires_at));
        }
        db::ApiTokenUpdateShape::TouchLastUsed => {}
        db::ApiTokenUpdateShape::Revoke => {
            after.set("revoked_at", num(now));
        }
    }
    diff(&before, &after)
}

fn api_token_label_after(
    before: Option<&ApiToken>,
    shape: &db::ApiTokenUpdateShape<'_>,
) -> Option<String> {
    match shape {
        db::ApiTokenUpdateShape::Fields { name, .. } => Some(name.to_string()),
        _ => before.map(|t| t.name.clone()),
    }
}

// ── Location ─────────────────────────────────────────────────────────────────

fn location_fields(l: &Location) -> Fields {
    Fields::default()
        .with("name", text(&l.name))
        .with("enabled", flag(l.enabled))
        .with("nitc_enabled", opt_num(l.nitc_enabled))
        .with("nitc_complete_on_export", flag(l.nitc_complete_on_export))
        .with(
            "ses_api_headquarters_id",
            opt_text(l.ses_api_headquarters_id.as_deref()),
        )
}

pub fn location_create_changes(l: &Location) -> Vec<AuditFieldChange> {
    creation(&location_fields(l))
}

/// Whether `update_location` with this shape is audited. `LastSyncTime` is the member
/// sync's heartbeat and is not.
pub fn location_shape_is_audited(shape: &db::LocationUpdateShape<'_>) -> bool {
    !matches!(shape, db::LocationUpdateShape::LastSyncTime { .. })
}

pub fn location_update_changes(
    before: Option<&Location>,
    shape: &db::LocationUpdateShape<'_>,
) -> Vec<AuditFieldChange> {
    let before = before.map(location_fields).unwrap_or_default();
    let mut after = before.clone();
    match shape {
        db::LocationUpdateShape::Fields {
            name,
            enabled,
            nitc_enabled,
            nitc_complete_on_export,
        } => {
            after.set("name", text(name));
            after.set("enabled", flag(*enabled));
            after.set("nitc_enabled", opt_num(*nitc_enabled));
            if let Some(complete) = nitc_complete_on_export {
                after.set("nitc_complete_on_export", flag(*complete));
            }
        }
        db::LocationUpdateShape::LastSyncTime { .. } => {}
        db::LocationUpdateShape::Name { name } => {
            after.set("name", text(name));
        }
    }
    diff(&before, &after)
}

fn location_label_after(
    before: Option<&Location>,
    shape: &db::LocationUpdateShape<'_>,
) -> Option<String> {
    match shape {
        db::LocationUpdateShape::Fields { name, .. } | db::LocationUpdateShape::Name { name } => {
            Some(name.to_string())
        }
        _ => before.map(|l| l.name.clone()),
    }
}

// ── Category, NITC group and tag ─────────────────────────────────────────────

fn category_fields(c: &Category) -> Fields {
    Fields::default()
        .with("name", text(&c.name))
        .with("enabled", flag(c.enabled))
        .with("is_virtual", flag(c.is_virtual))
        .with("nitc_group_id", opt_text(c.nitc_group_id.as_deref()))
        .with(
            "nitc_participant_type",
            opt_text(c.nitc_participant_type.as_deref()),
        )
}

pub fn category_create_changes(c: &Category) -> Vec<AuditFieldChange> {
    creation(&category_fields(c))
}

pub fn category_update_changes(
    before: Option<&Category>,
    name: &str,
    enabled: bool,
    is_virtual: bool,
    nitc_group_id: Option<&str>,
    nitc_participant_type: Option<&str>,
) -> Vec<AuditFieldChange> {
    let before = before.map(category_fields).unwrap_or_default();
    let after = before
        .clone()
        .with("name", text(name))
        .with("enabled", flag(enabled))
        .with("is_virtual", flag(is_virtual))
        .with("nitc_group_id", opt_text(nitc_group_id))
        .with("nitc_participant_type", opt_text(nitc_participant_type));
    diff(&before, &after)
}

fn nitc_group_fields(nitc_type: &str, tag_ids: &[i32]) -> Fields {
    let mut tags = tag_ids.to_vec();
    tags.sort_unstable();
    Fields::default()
        .with("nitc_type", text(nitc_type))
        .with("nitc_tag_ids", (!tags.is_empty()).then(|| json!(tags)))
}

pub fn nitc_group_update_changes(
    before: Option<&db::NitcGroup>,
    nitc_type: &str,
    nitc_tag_ids: &[i32],
) -> Vec<AuditFieldChange> {
    let before = before
        .map(|g| nitc_group_fields(&g.nitc_type, &g.nitc_tag_ids))
        .unwrap_or_default();
    diff(&before, &nitc_group_fields(nitc_type, nitc_tag_ids))
}

fn nitc_tag_fields(t: &db::NitcTag) -> Fields {
    Fields::default()
        .with("name", text(&t.name))
        .with("primary_activity_name", text(&t.primary_activity_name))
}

// ── Credentials and grants ───────────────────────────────────────────────────

/// Neither the token nor its hash is listed — just whose it is and when it lapses.
pub fn user_token_create_changes(t: &db::UserToken) -> Vec<AuditFieldChange> {
    creation(
        &Fields::default()
            .with("user_id", text(&t.user_id))
            .with("expires_at", num(t.expires_at)),
    )
}

fn oauth_grant_fields(g: &OAuthGrant) -> Fields {
    Fields::default()
        .with("user_id", text(&g.user_id))
        .with("client_id", text(&g.client_id))
        .with("client_name", text(&g.client_name))
        .with("redirect_uri", text(&g.redirect_uri))
        .with("resource", text(&g.resource))
        .with("scope", text(&g.scope))
        .with("expires_at", num(g.expires_at))
}

pub fn oauth_grant_create_changes(g: &OAuthGrant) -> Vec<AuditFieldChange> {
    creation(&oauth_grant_fields(g))
}

/// A revoked grant: who it was for and which app it was, not its secrets.
pub fn oauth_grant_delete_changes(g: &OAuthGrant) -> Vec<AuditFieldChange> {
    removal(
        &Fields::default()
            .with("user_id", text(&g.user_id))
            .with("client_id", text(&g.client_id))
            .with("client_name", text(&g.client_name)),
    )
}

fn webauthn_fields(c: &db::WebauthnCredential) -> Fields {
    // `passkey_json` holds the public key and signature counter: never listed.
    Fields::default()
        .with("user_id", text(&c.user_id))
        .with("name", text(&c.name))
}

pub fn webauthn_create_changes(c: &db::WebauthnCredential) -> Vec<AuditFieldChange> {
    creation(&webauthn_fields(c))
}

pub fn webauthn_rename_changes(before: Option<&str>, new_name: &str) -> Vec<AuditFieldChange> {
    diff(
        &Fields::default().with("name", opt_text(before)),
        &Fields::default().with("name", text(new_name)),
    )
}

pub fn webauthn_delete_changes(c: &db::WebauthnCredential) -> Vec<AuditFieldChange> {
    removal(&webauthn_fields(c))
}

// ── The wrapper ──────────────────────────────────────────────────────────────

/// A [`db::Handler`] that records an [`AuditEntry`] for each successful business write
/// to the handler it wraps. See the module docs.
#[derive(Clone, Debug)]
pub struct AuditingHandler<D: db::Handler> {
    inner: D,
}

/// Everything about a write that is known once it has succeeded.
struct Recorded<'a> {
    entity_type: AuditEntityType,
    entity_id: &'a str,
    action: AuditAction,
    label: Option<String>,
    location_ids: Vec<String>,
    changes: Vec<AuditFieldChange>,
}

impl<D: db::Handler> AuditingHandler<D> {
    pub fn new(inner: D) -> Self {
        Self { inner }
    }

    /// The wrapped handler, for the rare caller that must bypass auditing.
    pub fn inner(&self) -> &D {
        &self.inner
    }

    /// Store the entry for a write that has already succeeded. Never fails: a problem
    /// recording it is logged and swallowed so it cannot undo or mask the write.
    async fn record(&self, write: Recorded<'_>) {
        // An update that changed nothing (an idempotent re-sync, a form saved unedited)
        // leaves nothing worth recording.
        if write.action == AuditAction::Update && write.changes.is_empty() {
            return;
        }
        let ctx = current();
        let mut location_ids: Vec<String> = Vec::new();
        for id in write.location_ids {
            if !id.is_empty() && !location_ids.contains(&id) {
                location_ids.push(id);
            }
        }
        let entry = AuditEntry {
            id: db::new_id(),
            event_id: db::new_id(),
            ts: crate::clock::now_sec(),
            action: write.action,
            entity_type: write.entity_type,
            entity_id: write.entity_id.to_string(),
            entity_label: write.label.filter(|l| !l.is_empty()),
            location_ids,
            actor_kind: ctx.actor.kind().to_string(),
            actor_id: ctx.actor.id().map(str::to_string),
            actor_via: ctx.actor.via().map(str::to_string),
            ip: ctx.ip,
            changes: write.changes,
        };
        if let Err(e) = self.inner.put_audit_entry(&entry).await {
            error!(
                entity_type = entry.entity_type.as_str(),
                entity_id = %entry.entity_id,
                action = entry.action.as_str(),
                "failed to record audit entry (the write itself succeeded): {e}"
            );
        }
    }

    async fn record_create(
        &self,
        entity_type: AuditEntityType,
        entity_id: &str,
        label: Option<String>,
        location_ids: Vec<String>,
        changes: Vec<AuditFieldChange>,
    ) {
        self.record(Recorded {
            entity_type,
            entity_id,
            action: AuditAction::Create,
            label,
            location_ids,
            changes,
        })
        .await;
    }

    /// The "before" state of an entity, or `None` if it is missing *or* could not be read:
    /// a failed read must never stop the write, so it only costs the diff its "before"
    /// side.
    fn before<T>(&self, entity: &str, id: &str, read: db::Result<Option<T>>) -> Option<T> {
        match read {
            Ok(found) => found,
            Err(e) => {
                warn!(
                    "audit: could not read {entity} {id} before writing it, \
                     so its diff will lack the old values: {e}"
                );
                None
            }
        }
    }

    async fn before_user(&self, id: &str) -> Option<User> {
        self.before("user", id, first(self.inner.get_users(&[id]).await))
    }

    async fn before_person(&self, id: &str) -> Option<Person> {
        self.before("person", id, first(self.inner.get_persons(&[id]).await))
    }

    async fn before_period(&self, id: &str) -> Option<Period> {
        self.before("period", id, first(self.inner.get_periods(&[id]).await))
    }

    async fn before_session(&self, id: &str) -> Option<Session> {
        self.before("session", id, first(self.inner.get_sessions(&[id]).await))
    }

    async fn before_location(&self, id: &str) -> Option<Location> {
        self.before("location", id, first(self.inner.get_locations(&[id]).await))
    }

    async fn before_category(&self, id: &str) -> Option<Category> {
        self.before(
            "category",
            id,
            first(self.inner.get_categories(&[id]).await),
        )
    }

    /// A period's label: the member's name, or the guest's.
    async fn period_label(
        &self,
        person_id: Option<&str>,
        guest_name: Option<&str>,
    ) -> Option<String> {
        if let Some(person_id) = person_id {
            if let Some(person) = self.before_person(person_id).await {
                return Some(full_name(&person.first_name, &person.last_name));
            }
            return None;
        }
        guest_name.map(str::to_string)
    }

    async fn record_period_create(&self, period: &Period) {
        let label = self
            .period_label(period.person_id.as_deref(), period.guest_name.as_deref())
            .await;
        self.record_create(
            AuditEntityType::Period,
            &period.id,
            label,
            vec![period.location_id.clone()],
            period_create_changes(period),
        )
        .await;
    }
}

/// `get_*(&[id])` returns `Vec<Option<T>>` aligned with its one id; reduce it to that entry.
fn first<T>(result: db::Result<Vec<Option<T>>>) -> db::Result<Option<T>> {
    result.map(|rows| rows.into_iter().next().flatten())
}

impl<D: db::Handler> db::Handler for AuditingHandler<D> {
    // ── Audited writes ───────────────────────────────────────────────────────

    async fn create_user(
        &self,
        email: &str,
        is_super: bool,
        location_grants: Vec<String>,
        location_read_only_grants: Vec<String>,
    ) -> db::Result<User> {
        let user = self
            .inner
            .create_user(email, is_super, location_grants, location_read_only_grants)
            .await?;
        self.record_create(
            AuditEntityType::User,
            &user.id,
            Some(user.email.clone()),
            vec![],
            user_create_changes(&user),
        )
        .await;
        Ok(user)
    }

    async fn update_user(&self, id: &str, change: db::UserUpdateShape<'_>) -> db::Result<()> {
        if !user_shape_is_audited(&change) {
            return self.inner.update_user(id, change).await;
        }
        let before = self.before_user(id).await;
        let changes = user_update_changes(before.as_ref(), &change);
        let label = user_label_after(before.as_ref(), &change);
        self.inner.update_user(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::User,
            entity_id: id,
            action: AuditAction::Update,
            label,
            location_ids: vec![],
            changes,
        })
        .await;
        Ok(())
    }

    async fn create_person(
        &self,
        location_id: &str,
        first_name: &str,
        last_name: &str,
        registration_number: &str,
    ) -> db::Result<Person> {
        let person = self
            .inner
            .create_person(location_id, first_name, last_name, registration_number)
            .await?;
        self.record_create(
            AuditEntityType::Person,
            &person.id,
            Some(full_name(&person.first_name, &person.last_name)),
            vec![person.location_id.clone()],
            person_create_changes(&person),
        )
        .await;
        Ok(person)
    }

    async fn update_person(&self, id: &str, change: db::PersonUpdateShape<'_>) -> db::Result<()> {
        let before = self.before_person(id).await;
        let (action, changes) =
            person_update_changes(before.as_ref(), &change, crate::clock::now_sec());
        let location_ids = person_update_locations(before.as_ref(), &change);
        let label = person_label_after(before.as_ref(), &change);
        self.inner.update_person(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::Person,
            entity_id: id,
            action,
            label,
            location_ids,
            changes,
        })
        .await;
        Ok(())
    }

    async fn create_period(
        &self,
        person_id: &str,
        location_id: &str,
        category_id: &str,
        start_time: u64,
        end_time: u64,
        comment: Option<&str>,
    ) -> db::Result<Period> {
        let period = self
            .inner
            .create_period(
                person_id,
                location_id,
                category_id,
                start_time,
                end_time,
                comment,
            )
            .await?;
        self.record_period_create(&period).await;
        Ok(period)
    }

    async fn start_period_for_person_location(
        &self,
        person_id: &str,
        location_id: &str,
        signed_in_session_id: Option<&str>,
        start_time: Option<u64>,
    ) -> db::Result<Period> {
        let period = self
            .inner
            .start_period_for_person_location(
                person_id,
                location_id,
                signed_in_session_id,
                start_time,
            )
            .await?;
        self.record_period_create(&period).await;
        Ok(period)
    }

    async fn start_guest_period(
        &self,
        location_id: &str,
        guest_name: &str,
        comment: Option<&str>,
        signed_in_session_id: &str,
    ) -> db::Result<Period> {
        let period = self
            .inner
            .start_guest_period(location_id, guest_name, comment, signed_in_session_id)
            .await?;
        self.record_period_create(&period).await;
        Ok(period)
    }

    async fn end_period(
        &self,
        period: &Period,
        signed_out_session_id: Option<&str>,
    ) -> db::Result<Period> {
        let ended = self.inner.end_period(period, signed_out_session_id).await?;
        let label = self
            .period_label(ended.person_id.as_deref(), ended.guest_name.as_deref())
            .await;
        self.record(Recorded {
            entity_type: AuditEntityType::Period,
            entity_id: &ended.id,
            action: AuditAction::Update,
            label,
            location_ids: vec![ended.location_id.clone()],
            changes: period_end_changes(period, &ended),
        })
        .await;
        Ok(ended)
    }

    async fn update_period(&self, id: &str, change: db::PeriodUpdateShape<'_>) -> db::Result<u64> {
        let before = self.before_period(id).await;
        let (action, changes) =
            period_update_changes(before.as_ref(), &change, crate::clock::now_sec());
        let location_ids = period_update_locations(before.as_ref(), &change);
        // Name the period after whoever it belongs to once the write has landed.
        let person_id = match &change {
            db::PeriodUpdateShape::Fields { person_id, .. } => Some(person_id.to_string()),
            _ => before.as_ref().and_then(|p| p.person_id.clone()),
        };
        let guest_name = match &change {
            db::PeriodUpdateShape::Guest { guest_name, .. } => Some(guest_name.to_string()),
            _ => before.as_ref().and_then(|p| p.guest_name.clone()),
        };
        let version = self.inner.update_period(id, change).await?;
        let label = self
            .period_label(person_id.as_deref(), guest_name.as_deref())
            .await;
        self.record(Recorded {
            entity_type: AuditEntityType::Period,
            entity_id: id,
            action,
            label,
            location_ids,
            changes,
        })
        .await;
        Ok(version)
    }

    async fn create_session(
        &self,
        location_id: &str,
        name: &str,
        config: &serde_json::Map<String, serde_json::Value>,
        healthcheck_url: Option<&str>,
        key: Option<db::SessionKeyParams<'_>>,
    ) -> db::Result<Session> {
        let session = self
            .inner
            .create_session(location_id, name, config, healthcheck_url, key)
            .await?;
        self.record_create(
            AuditEntityType::Session,
            &session.id,
            Some(session.name.clone()),
            vec![session.location_id.clone()],
            session_create_changes(&session),
        )
        .await;
        Ok(session)
    }

    async fn wipe_session_code(&self, id: &str) -> db::Result<()> {
        let before = self.before_session(id).await;
        self.inner.wipe_session_code(id).await?;
        // A session that had no code to wipe changed nothing.
        if before.as_ref().is_some_and(|s| s.code.is_none()) {
            return Ok(());
        }
        self.record(Recorded {
            entity_type: AuditEntityType::Session,
            entity_id: id,
            action: AuditAction::Update,
            label: before.as_ref().map(|s| s.name.clone()),
            location_ids: before.iter().map(|s| s.location_id.clone()).collect(),
            changes: session_wipe_code_changes(),
        })
        .await;
        Ok(())
    }

    async fn update_session(&self, id: &str, change: db::SessionUpdateShape<'_>) -> db::Result<()> {
        if !session_shape_is_audited(&change) {
            return self.inner.update_session(id, change).await;
        }
        let before = self.before_session(id).await;
        let (action, changes) =
            session_update_changes(before.as_ref(), &change, crate::clock::now_sec());
        let label = session_label_after(before.as_ref(), &change);
        let location_ids: Vec<String> = before.iter().map(|s| s.location_id.clone()).collect();
        self.inner.update_session(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::Session,
            entity_id: id,
            action,
            label,
            location_ids,
            changes,
        })
        .await;
        Ok(())
    }

    async fn create_api_token(
        &self,
        name: &str,
        token_hash: &str,
        location_grants: Vec<String>,
        read_only: bool,
        expires_at: Option<u64>,
        created_by_user_id: &str,
    ) -> db::Result<ApiToken> {
        let token = self
            .inner
            .create_api_token(
                name,
                token_hash,
                location_grants,
                read_only,
                expires_at,
                created_by_user_id,
            )
            .await?;
        self.record_create(
            AuditEntityType::ApiToken,
            &token.id,
            Some(token.name.clone()),
            vec![],
            api_token_create_changes(&token),
        )
        .await;
        Ok(token)
    }

    async fn update_api_token(
        &self,
        id: &str,
        change: db::ApiTokenUpdateShape<'_>,
    ) -> db::Result<()> {
        if !api_token_shape_is_audited(&change) {
            return self.inner.update_api_token(id, change).await;
        }
        let before = self.before("api token", id, self.inner.get_api_token(id).await);
        let changes = api_token_update_changes(before.as_ref(), &change, crate::clock::now_sec());
        let label = api_token_label_after(before.as_ref(), &change);
        self.inner.update_api_token(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::ApiToken,
            entity_id: id,
            action: AuditAction::Update,
            label,
            location_ids: vec![],
            changes,
        })
        .await;
        Ok(())
    }

    async fn create_location(
        &self,
        name: &str,
        nitc_enabled: Option<u64>,
        ses_api_headquarters_id: Option<&str>,
    ) -> db::Result<Location> {
        let location = self
            .inner
            .create_location(name, nitc_enabled, ses_api_headquarters_id)
            .await?;
        self.record_create(
            AuditEntityType::Location,
            &location.id,
            Some(location.name.clone()),
            vec![location.id.clone()],
            location_create_changes(&location),
        )
        .await;
        Ok(location)
    }

    async fn update_location(
        &self,
        id: &str,
        change: db::LocationUpdateShape<'_>,
    ) -> db::Result<()> {
        if !location_shape_is_audited(&change) {
            return self.inner.update_location(id, change).await;
        }
        let before = self.before_location(id).await;
        let changes = location_update_changes(before.as_ref(), &change);
        let label = location_label_after(before.as_ref(), &change);
        self.inner.update_location(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::Location,
            entity_id: id,
            action: AuditAction::Update,
            label,
            location_ids: vec![id.to_string()],
            changes,
        })
        .await;
        Ok(())
    }

    async fn create_category(
        &self,
        id: Option<&str>,
        name: &str,
        is_virtual: bool,
        nitc_group_id: Option<&str>,
        nitc_participant_type: Option<&str>,
    ) -> db::Result<Category> {
        let category = self
            .inner
            .create_category(id, name, is_virtual, nitc_group_id, nitc_participant_type)
            .await?;
        self.record_create(
            AuditEntityType::Category,
            &category.id,
            Some(category.name.clone()),
            vec![],
            category_create_changes(&category),
        )
        .await;
        Ok(category)
    }

    async fn update_category(
        &self,
        id: &str,
        name: &str,
        active: bool,
        is_virtual: bool,
        nitc_group_id: Option<&str>,
        nitc_participant_type: Option<&str>,
    ) -> db::Result<()> {
        let before = self.before_category(id).await;
        let changes = category_update_changes(
            before.as_ref(),
            name,
            active,
            is_virtual,
            nitc_group_id,
            nitc_participant_type,
        );
        self.inner
            .update_category(
                id,
                name,
                active,
                is_virtual,
                nitc_group_id,
                nitc_participant_type,
            )
            .await?;
        self.record(Recorded {
            entity_type: AuditEntityType::Category,
            entity_id: id,
            action: AuditAction::Update,
            label: Some(name.to_string()),
            location_ids: vec![],
            changes,
        })
        .await;
        Ok(())
    }

    async fn create_nitc_group(
        &self,
        id: Option<&str>,
        nitc_type: &str,
        nitc_tag_ids: &[i32],
    ) -> db::Result<db::NitcGroup> {
        let group = self
            .inner
            .create_nitc_group(id, nitc_type, nitc_tag_ids)
            .await?;
        self.record_create(
            AuditEntityType::NitcGroup,
            &group.id,
            Some(group.nitc_type.clone()),
            vec![],
            nitc_group_update_changes(None, &group.nitc_type, &group.nitc_tag_ids),
        )
        .await;
        Ok(group)
    }

    async fn update_nitc_group(
        &self,
        id: &str,
        nitc_type: &str,
        nitc_tag_ids: &[i32],
    ) -> db::Result<()> {
        let before = self.before("NITC group", id, self.inner.get_nitc_group(id).await);
        let changes = nitc_group_update_changes(before.as_ref(), nitc_type, nitc_tag_ids);
        self.inner
            .update_nitc_group(id, nitc_type, nitc_tag_ids)
            .await?;
        self.record(Recorded {
            entity_type: AuditEntityType::NitcGroup,
            entity_id: id,
            action: AuditAction::Update,
            label: Some(nitc_type.to_string()),
            location_ids: vec![],
            changes,
        })
        .await;
        Ok(())
    }

    async fn delete_nitc_group(&self, id: &str) -> db::Result<()> {
        let existing = self.inner.get_nitc_group(id).await;
        self.inner.delete_nitc_group(id).await?;
        let (label, changes) = match existing {
            // Deleting a group that was not there removed nothing.
            Ok(None) => return Ok(()),
            Ok(Some(group)) => (
                Some(group.nitc_type.clone()),
                removal(&nitc_group_fields(&group.nitc_type, &group.nitc_tag_ids)),
            ),
            Err(e) => {
                warn!("audit: could not read NITC group {id} before deleting it: {e}");
                (None, vec![])
            }
        };
        self.record(Recorded {
            entity_type: AuditEntityType::NitcGroup,
            entity_id: id,
            action: AuditAction::Delete,
            label,
            location_ids: vec![],
            changes,
        })
        .await;
        Ok(())
    }

    async fn put_nitc_tag(&self, tag: &db::NitcTag) -> db::Result<()> {
        // There is no get-by-id for tags, so this scans the (small) table — fine for the
        // one tool that writes tags, `load-nitc-tags`.
        let existing = match self.inner.list_nitc_tags().await {
            Ok(tags) => tags.into_iter().find(|t| t.id == tag.id),
            Err(e) => {
                warn!(
                    "audit: could not list NITC tags before writing tag {}: {e}",
                    tag.id
                );
                None
            }
        };
        self.inner.put_nitc_tag(tag).await?;
        let before = existing.as_ref().map(nitc_tag_fields).unwrap_or_default();
        self.record(Recorded {
            entity_type: AuditEntityType::NitcTag,
            entity_id: &tag.id.to_string(),
            action: if existing.is_some() {
                AuditAction::Update
            } else {
                AuditAction::Create
            },
            label: Some(tag.name.clone()),
            location_ids: vec![],
            changes: diff(&before, &nitc_tag_fields(tag)),
        })
        .await;
        Ok(())
    }

    async fn create_user_token(
        &self,
        token_hash: &str,
        user_id: &str,
        expires_at: u64,
    ) -> db::Result<db::UserToken> {
        let token = self
            .inner
            .create_user_token(token_hash, user_id, expires_at)
            .await?;
        let label = self.before_user(user_id).await.map(|u| u.email);
        self.record_create(
            AuditEntityType::UserToken,
            &token.id,
            label,
            vec![],
            user_token_create_changes(&token),
        )
        .await;
        Ok(token)
    }

    async fn delete_user_token(&self, id: &str) -> db::Result<()> {
        // Tokens can only be looked up by hash, so there is nothing to read first.
        self.inner.delete_user_token(id).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::UserToken,
            entity_id: id,
            action: AuditAction::Delete,
            label: None,
            location_ids: vec![],
            changes: vec![],
        })
        .await;
        Ok(())
    }

    async fn create_oauth_grant(&self, grant: &OAuthGrant) -> db::Result<()> {
        self.inner.create_oauth_grant(grant).await?;
        self.record_create(
            AuditEntityType::OAuthGrant,
            &grant.id,
            Some(grant.client_name.clone()),
            vec![],
            oauth_grant_create_changes(grant),
        )
        .await;
        Ok(())
    }

    async fn delete_oauth_grant(&self, id: &str) -> db::Result<()> {
        let existing = self.inner.get_oauth_grant(id).await;
        self.inner.delete_oauth_grant(id).await?;
        let (label, changes) = match existing {
            Ok(None) => return Ok(()),
            Ok(Some(grant)) => (
                Some(grant.client_name.clone()),
                oauth_grant_delete_changes(&grant),
            ),
            Err(e) => {
                warn!("audit: could not read OAuth grant {id} before deleting it: {e}");
                (None, vec![])
            }
        };
        self.record(Recorded {
            entity_type: AuditEntityType::OAuthGrant,
            entity_id: id,
            action: AuditAction::Delete,
            label,
            location_ids: vec![],
            changes,
        })
        .await;
        Ok(())
    }

    async fn create_webauthn_credential(
        &self,
        id: &str,
        user_id: &str,
        name: &str,
        passkey_json: &str,
    ) -> db::Result<db::WebauthnCredential> {
        let credential = self
            .inner
            .create_webauthn_credential(id, user_id, name, passkey_json)
            .await?;
        self.record_create(
            AuditEntityType::WebauthnCredential,
            &credential.id,
            Some(credential.name.clone()),
            vec![],
            webauthn_create_changes(&credential),
        )
        .await;
        Ok(credential)
    }

    async fn update_webauthn_credential(
        &self,
        id: &str,
        change: db::WebauthnCredentialUpdate,
    ) -> db::Result<()> {
        // `TouchLastUsed` rewrites the signature counter on every login: bookkeeping.
        let db::WebauthnCredentialUpdate::Rename(new_name) = &change else {
            return self.inner.update_webauthn_credential(id, change).await;
        };
        let new_name = new_name.clone();
        let before = self.before("passkey", id, self.inner.get_webauthn_credential(id).await);
        self.inner.update_webauthn_credential(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::WebauthnCredential,
            entity_id: id,
            action: AuditAction::Update,
            label: Some(new_name.clone()),
            location_ids: vec![],
            changes: webauthn_rename_changes(before.as_ref().map(|c| c.name.as_str()), &new_name),
        })
        .await;
        Ok(())
    }

    async fn delete_webauthn_credential(&self, id: &str) -> db::Result<()> {
        let existing = self.inner.get_webauthn_credential(id).await;
        self.inner.delete_webauthn_credential(id).await?;
        let (label, changes) = match existing {
            Ok(None) => return Ok(()),
            Ok(Some(credential)) => (
                Some(credential.name.clone()),
                webauthn_delete_changes(&credential),
            ),
            Err(e) => {
                warn!("audit: could not read passkey {id} before deleting it: {e}");
                (None, vec![])
            }
        };
        self.record(Recorded {
            entity_type: AuditEntityType::WebauthnCredential,
            entity_id: id,
            action: AuditAction::Delete,
            label,
            location_ids: vec![],
            changes,
        })
        .await;
        Ok(())
    }

    // ── Deliberately not audited ─────────────────────────────────────────────
    //
    // Bookkeeping that is not a business change, or that is high-volume plumbing:
    //   * `update_oauth_grant`: token rotation and `last_used_at`.
    //   * `update_user_token`: `last_used_at`.
    //   * login codes, WebAuthn challenge state and ephemeral state: short-lived
    //     scratch records with their own TTL.
    //   * the NITC export state machine (`get_or_create_nitc_event_for_day`,
    //     `bump_*_version`, `set_*`, `*_synced`, `update_period_nitc_exported`,
    //     `clear_period_nitc_participant`): written by the export worker, not by a
    //     person, and replayed on every export.
    // The excluded *shapes* of audited methods (`AccessTime`, session `Info`,
    // `TouchLastUsed`, `LastSyncTime`) are handled inside those methods above.
    // `put_audit_entry` itself must of course not recurse into auditing.

    async fn put_audit_entry(&self, entry: &AuditEntry) -> db::Result<()> {
        self.inner.put_audit_entry(entry).await
    }

    // ── Everything else: straight delegation ─────────────────────────────────

    async fn get_users<T: AsRef<str> + Sync>(&self, ids: &[T]) -> db::Result<Vec<Option<User>>> {
        self.inner.get_users(ids).await
    }

    async fn get_user_id_by_email(&self, email: &str) -> db::Result<Vec<String>> {
        self.inner.get_user_id_by_email(email).await
    }

    async fn list_users(&self) -> db::Result<Vec<User>> {
        self.inner.list_users().await
    }

    async fn get_persons<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<Person>>> {
        self.inner.get_persons(ids).await
    }

    async fn get_person_id_by_registration_number(
        &self,
        registration_number: &str,
    ) -> db::Result<Vec<String>> {
        self.inner
            .get_person_id_by_registration_number(registration_number)
            .await
    }

    async fn get_person_id_by_ses_api_person_id(
        &self,
        ses_api_person_id: &str,
    ) -> db::Result<Vec<String>> {
        self.inner
            .get_person_id_by_ses_api_person_id(ses_api_person_id)
            .await
    }

    async fn get_sessions<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<Session>>> {
        self.inner.get_sessions(ids).await
    }

    async fn get_session_id_by_code(&self, code: &str) -> db::Result<Vec<String>> {
        self.inner.get_session_id_by_code(code).await
    }

    async fn get_session_id_by_key_fingerprint(
        &self,
        fingerprint: &str,
    ) -> db::Result<Vec<String>> {
        self.inner
            .get_session_id_by_key_fingerprint(fingerprint)
            .await
    }

    async fn list_sessions(&self, query: ListSessionsQuery) -> db::Result<Vec<Session>> {
        self.inner.list_sessions(query).await
    }

    async fn list_people_for_location(
        &self,
        location_id: &str,
        skip_deleted: bool,
    ) -> db::Result<Vec<Person>> {
        self.inner
            .list_people_for_location(location_id, skip_deleted)
            .await
    }

    async fn list_periods_for_location(
        &self,
        location_id: &str,
        only_active: bool,
        timestamp_range: Option<(u64, u64)>,
        category_ids: Option<&[String]>,
        page: db::ListPeriodsPage,
    ) -> db::Result<Vec<Period>> {
        self.inner
            .list_periods_for_location(
                location_id,
                only_active,
                timestamp_range,
                category_ids,
                page,
            )
            .await
    }

    async fn list_periods_for_person(
        &self,
        person_id: &str,
        location_id: Option<&str>,
        only_unfinished: Option<bool>,
        category_ids: Option<&[String]>,
        page: db::ListPeriodsPage,
    ) -> db::Result<Vec<Period>> {
        self.inner
            .list_periods_for_person(person_id, location_id, only_unfinished, category_ids, page)
            .await
    }

    async fn get_periods<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<Period>>> {
        self.inner.get_periods(ids).await
    }

    async fn get_api_token(&self, id: &str) -> db::Result<Option<ApiToken>> {
        self.inner.get_api_token(id).await
    }

    async fn get_api_token_by_hash(&self, token_hash: &str) -> db::Result<Option<ApiToken>> {
        self.inner.get_api_token_by_hash(token_hash).await
    }

    async fn list_api_tokens(&self, filter: db::ListApiTokensFilter) -> db::Result<Vec<ApiToken>> {
        self.inner.list_api_tokens(filter).await
    }

    async fn get_locations<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<Location>>> {
        self.inner.get_locations(ids).await
    }

    async fn list_locations(&self, filter: db::ListLocationsFilter) -> db::Result<Vec<Location>> {
        self.inner.list_locations(filter).await
    }

    async fn list_categories(&self) -> db::Result<Vec<Category>> {
        self.inner.list_categories().await
    }

    async fn get_categories<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<Category>>> {
        self.inner.get_categories(ids).await
    }

    async fn list_nitc_events_for_day(
        &self,
        location_id: &str,
        nitc_group_id: &str,
        date: chrono::NaiveDate,
    ) -> db::Result<Vec<db::NitcEvent>> {
        self.inner
            .list_nitc_events_for_day(location_id, nitc_group_id, date)
            .await
    }

    async fn get_or_create_nitc_event_for_day(
        &self,
        location_id: &str,
        nitc_group_id: &str,
        date: chrono::NaiveDate,
    ) -> db::Result<db::NitcEvent> {
        self.inner
            .get_or_create_nitc_event_for_day(location_id, nitc_group_id, date)
            .await
    }

    async fn get_nitc_event_by_id(&self, id: &str) -> db::Result<Option<db::NitcEvent>> {
        self.inner.get_nitc_event_by_id(id).await
    }

    async fn get_nitc_events_by_ids<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::NitcEvent>>> {
        self.inner.get_nitc_events_by_ids(ids).await
    }

    async fn list_nitc_events_for_location(
        &self,
        location_id: &str,
    ) -> db::Result<Vec<db::NitcEvent>> {
        self.inner.list_nitc_events_for_location(location_id).await
    }

    async fn scan_persons(
        &self,
        cursor: Option<db::ScanCursor>,
        limit: i32,
    ) -> db::Result<db::ScanPage<Person>> {
        self.inner.scan_persons(cursor, limit).await
    }

    async fn scan_periods(
        &self,
        cursor: Option<db::ScanCursor>,
        limit: i32,
    ) -> db::Result<db::ScanPage<Period>> {
        self.inner.scan_periods(cursor, limit).await
    }

    async fn scan_sessions(
        &self,
        cursor: Option<db::ScanCursor>,
        limit: i32,
    ) -> db::Result<db::ScanPage<Session>> {
        self.inner.scan_sessions(cursor, limit).await
    }

    async fn scan_user_tokens(
        &self,
        cursor: Option<db::ScanCursor>,
        limit: i32,
    ) -> db::Result<db::ScanPage<db::UserToken>> {
        self.inner.scan_user_tokens(cursor, limit).await
    }

    async fn scan_nitc_events(
        &self,
        cursor: Option<db::ScanCursor>,
        limit: i32,
    ) -> db::Result<db::ScanPage<db::NitcEvent>> {
        self.inner.scan_nitc_events(cursor, limit).await
    }

    async fn get_nitc_group(&self, id: &str) -> db::Result<Option<db::NitcGroup>> {
        self.inner.get_nitc_group(id).await
    }

    async fn list_nitc_groups(&self) -> db::Result<Vec<db::NitcGroup>> {
        self.inner.list_nitc_groups().await
    }

    async fn bump_period_version(&self, period_id: &str) -> db::Result<u64> {
        self.inner.bump_period_version(period_id).await
    }

    async fn bump_nitc_event_version(&self, event_id: &str) -> db::Result<u64> {
        self.inner.bump_nitc_event_version(event_id).await
    }

    async fn set_period_nitc_event(&self, period_id: &str, event_id: &str) -> db::Result<()> {
        self.inner.set_period_nitc_event(period_id, event_id).await
    }

    async fn list_period_ids_for_nitc_event(&self, event_id: &str) -> db::Result<Vec<String>> {
        self.inner.list_period_ids_for_nitc_event(event_id).await
    }

    async fn set_nitc_event_ses_id(&self, event_id: &str, ses_api_nitc_id: i64) -> db::Result<()> {
        self.inner
            .set_nitc_event_ses_id(event_id, ses_api_nitc_id)
            .await
    }

    async fn set_period_nitc_exported_version(
        &self,
        period_id: &str,
        synced_version: u64,
    ) -> db::Result<()> {
        self.inner
            .set_period_nitc_exported_version(period_id, synced_version)
            .await
    }

    async fn update_period_nitc_exported(
        &self,
        period_id: &str,
        nitc_event_id: &str,
        nitc_participant_id: i64,
        synced_version: u64,
    ) -> db::Result<()> {
        self.inner
            .update_period_nitc_exported(
                period_id,
                nitc_event_id,
                nitc_participant_id,
                synced_version,
            )
            .await
    }

    async fn clear_period_nitc_participant(
        &self,
        period_id: &str,
        synced_version: u64,
    ) -> db::Result<()> {
        self.inner
            .clear_period_nitc_participant(period_id, synced_version)
            .await
    }

    async fn mark_nitc_event_synced(&self, event_id: &str, synced_version: u64) -> db::Result<()> {
        self.inner
            .mark_nitc_event_synced(event_id, synced_version)
            .await
    }

    async fn list_test_pagination(
        &self,
        page: db::ListTestPaginationPage,
    ) -> db::Result<Vec<db::TestPaginationRow>> {
        self.inner.list_test_pagination(page).await
    }

    async fn put_login_code(
        &self,
        email: &str,
        code_hash: &str,
        expires_at: u64,
        last_sent_at: u64,
    ) -> db::Result<()> {
        self.inner
            .put_login_code(email, code_hash, expires_at, last_sent_at)
            .await
    }

    async fn get_login_code(&self, email: &str) -> db::Result<Option<db::LoginCode>> {
        self.inner.get_login_code(email).await
    }

    async fn delete_login_code(&self, email: &str) -> db::Result<()> {
        self.inner.delete_login_code(email).await
    }

    async fn increment_login_code_attempts(&self, email: &str) -> db::Result<()> {
        self.inner.increment_login_code_attempts(email).await
    }

    async fn get_user_token_by_hash(&self, token_hash: &str) -> db::Result<Option<db::UserToken>> {
        self.inner.get_user_token_by_hash(token_hash).await
    }

    async fn update_user_token(
        &self,
        id: &str,
        change: db::UserTokenUpdateShape,
    ) -> db::Result<()> {
        self.inner.update_user_token(id, change).await
    }

    async fn get_oauth_grant(&self, id: &str) -> db::Result<Option<OAuthGrant>> {
        self.inner.get_oauth_grant(id).await
    }

    async fn update_oauth_grant(
        &self,
        id: &str,
        change: db::OAuthGrantUpdateShape,
    ) -> db::Result<()> {
        self.inner.update_oauth_grant(id, change).await
    }

    async fn list_oauth_grants_by_user(&self, user_id: &str) -> db::Result<Vec<OAuthGrant>> {
        self.inner.list_oauth_grants_by_user(user_id).await
    }

    async fn get_webauthn_credential(
        &self,
        id: &str,
    ) -> db::Result<Option<db::WebauthnCredential>> {
        self.inner.get_webauthn_credential(id).await
    }

    async fn list_webauthn_credentials_by_user(
        &self,
        user_id: &str,
    ) -> db::Result<Vec<db::WebauthnCredential>> {
        self.inner.list_webauthn_credentials_by_user(user_id).await
    }

    async fn count_webauthn_credentials_by_user(&self, user_id: &str) -> db::Result<usize> {
        self.inner.count_webauthn_credentials_by_user(user_id).await
    }

    async fn put_webauthn_state(
        &self,
        id: &str,
        kind: &str,
        user_id: Option<&str>,
        state_json: &str,
        expires_at: u64,
    ) -> db::Result<()> {
        self.inner
            .put_webauthn_state(id, kind, user_id, state_json, expires_at)
            .await
    }

    async fn get_webauthn_state(&self, id: &str) -> db::Result<Option<db::WebauthnState>> {
        self.inner.get_webauthn_state(id).await
    }

    async fn delete_webauthn_state(&self, id: &str) -> db::Result<()> {
        self.inner.delete_webauthn_state(id).await
    }

    async fn put_ephemeral_state(
        &self,
        id: &str,
        kind: &str,
        payload: &str,
        expires_at: u64,
    ) -> db::Result<()> {
        self.inner
            .put_ephemeral_state(id, kind, payload, expires_at)
            .await
    }

    async fn get_ephemeral_state(&self, id: &str) -> db::Result<Option<db::EphemeralState>> {
        self.inner.get_ephemeral_state(id).await
    }

    async fn delete_ephemeral_state(&self, id: &str) -> db::Result<()> {
        self.inner.delete_ephemeral_state(id).await
    }

    async fn list_nitc_tags(&self) -> db::Result<Vec<db::NitcTag>> {
        self.inner.list_nitc_tags().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person() -> Person {
        Person {
            id: "p1".to_string(),
            location_id: "loc-a".to_string(),
            first_name: "Alice".to_string(),
            last_name: "Anderson".to_string(),
            registration_number: Some("1001".to_string()),
            ses_api_person_id: None,
            email: None,
            deleted: None,
            missing_since: None,
            created_at: Some(1),
            updated_at: Some(1),
        }
    }

    fn period() -> Period {
        Period {
            id: "per1".to_string(),
            person_id: Some("p1".to_string()),
            guest_name: None,
            comment: None,
            location_id: "loc-a".to_string(),
            category_id: Some("cat1".to_string()),
            start_time: 1000,
            end_time: Some(2000),
            signed_in_session_id: None,
            signed_out_session_id: None,
            version: 3,
            nitc_event_id: None,
            nitc_participant_id: None,
            nitc_exported_version: None,
            deleted: None,
            created_at: Some(1),
            updated_at: Some(1),
        }
    }

    fn session() -> Session {
        Session {
            id: "s1".to_string(),
            name: "Front desk".to_string(),
            location_id: "loc-a".to_string(),
            active: true,
            last_contact: Some(5),
            client_version: None,
            client_info: None,
            client_info_updated_at: None,
            code: Some("123456".to_string()),
            config: serde_json::Map::new(),
            healthcheck_url: None,
            public_key: None,
            key_fingerprint: None,
            key_expires_at: None,
            key_released_at: None,
            created_at: Some(1),
            updated_at: Some(1),
        }
    }

    fn user() -> User {
        User {
            id: "u1".to_string(),
            email: "a@example.com".to_string(),
            is_super: false,
            is_dev: false,
            enabled: true,
            location_grants: vec!["loc-b".to_string(), "loc-a".to_string()],
            location_read_only_grants: vec![],
            access_time: Some(9),
            email_config: serde_json::Map::new(),
            disaggregate_virtual_periods: false,
            created_at: 1,
            updated_at: 1,
        }
    }

    fn fields(changes: &[AuditFieldChange]) -> Vec<&str> {
        changes.iter().map(|c| c.field.as_str()).collect()
    }

    fn change<'a>(changes: &'a [AuditFieldChange], field: &str) -> &'a AuditFieldChange {
        changes
            .iter()
            .find(|c| c.field == field)
            .unwrap_or_else(|| panic!("no change to {field} in {changes:?}"))
    }

    // ── Actor ────────────────────────────────────────────────────────────────

    #[test]
    fn actor_kinds_and_ids_are_stable() {
        let cases = [
            (
                Actor::User {
                    id: "u".into(),
                    via: None,
                },
                "user",
                Some("u"),
            ),
            (Actor::Session { id: "s".into() }, "session", Some("s")),
            (Actor::ApiToken { id: "t".into() }, "api_token", Some("t")),
            (
                Actor::PeriodLink {
                    period_id: "per".into(),
                },
                "period_link",
                Some("per"),
            ),
            (Actor::system("cli"), "system", Some("cli")),
            (Actor::Unauthenticated, "unauthenticated", None),
            (Actor::Unknown, "unknown", None),
        ];
        for (actor, kind, id) in cases {
            assert_eq!(actor.kind(), kind);
            assert_eq!(actor.id(), id);
        }
    }

    #[test]
    fn actor_from_auth_info_names_how_a_user_came_in() {
        let user = |token_id: Option<&str>, grant_id: Option<&str>| AuthInfo::User {
            id: "u1".to_string(),
            is_super: false,
            location_grants: vec![],
            location_read_only_grants: vec![],
            token_id: token_id.map(str::to_string),
            grant_id: grant_id.map(str::to_string),
        };
        assert_eq!(
            Actor::from_auth_info(Some(&user(None, None))),
            Actor::User {
                id: "u1".into(),
                via: None
            }
        );
        assert_eq!(
            Actor::from_auth_info(Some(&user(Some("tok"), None))).via(),
            Some("user_token:tok")
        );
        assert_eq!(
            Actor::from_auth_info(Some(&user(None, Some("g")))).via(),
            Some("oauth_grant:g")
        );
        // A grant wins: an MCP caller is an OAuth client first.
        assert_eq!(
            Actor::from_auth_info(Some(&user(Some("tok"), Some("g")))).via(),
            Some("oauth_grant:g")
        );
    }

    #[test]
    fn actor_from_auth_info_covers_every_credential() {
        assert_eq!(Actor::from_auth_info(None), Actor::Unauthenticated);
        assert_eq!(
            Actor::from_auth_info(Some(&AuthInfo::Session {
                id: "s".into(),
                location: "loc".into()
            })),
            Actor::Session { id: "s".into() }
        );
        assert_eq!(
            Actor::from_auth_info(Some(&AuthInfo::ApiToken {
                id: "t".into(),
                location_grants: vec![],
                read_only: false
            })),
            Actor::ApiToken { id: "t".into() }
        );
        assert_eq!(
            Actor::from_auth_info(Some(&AuthInfo::PeriodLink {
                period_id: "per".into()
            })),
            Actor::PeriodLink {
                period_id: "per".into()
            }
        );
    }

    #[tokio::test]
    async fn current_is_unknown_without_a_scope_and_the_scoped_context_within_one() {
        assert_eq!(current().actor, Actor::Unknown);
        let ctx = AuditContext::new(Actor::system("job"), Some("198.51.100.1".to_string()));
        let seen = scope(ctx.clone(), async { current() }).await;
        assert_eq!(seen, ctx);
        assert_eq!(current_ip(), None);
    }

    #[tokio::test]
    async fn an_inner_scope_replaces_the_outer_one() {
        let seen = scope(AuditContext::system("outer"), async {
            scope(AuditContext::system("inner"), async { current() }).await
        })
        .await;
        assert_eq!(seen.actor, Actor::system("inner"));
    }

    // ── Person ───────────────────────────────────────────────────────────────

    #[test]
    fn person_create_lists_every_set_field_with_no_before() {
        let changes = person_create_changes(&person());
        assert_eq!(
            fields(&changes),
            vec![
                "first_name",
                "last_name",
                "registration_number",
                "location_id"
            ]
        );
        assert!(changes.iter().all(|c| c.before.is_none()));
        assert_eq!(change(&changes, "first_name").after, Some(json!("Alice")));
    }

    #[test]
    fn person_fields_update_emits_only_what_differs() {
        let (action, changes) = person_update_changes(
            Some(&person()),
            &db::PersonUpdateShape::Fields {
                first_name: "Alice",
                last_name: "Andersen",
                registration_number: "1001",
            },
            100,
        );
        assert_eq!(action, AuditAction::Update);
        assert_eq!(
            changes,
            vec![AuditFieldChange {
                field: "last_name".into(),
                before: Some(json!("Anderson")),
                after: Some(json!("Andersen")),
            }]
        );
    }

    #[test]
    fn person_update_with_no_before_lists_the_new_values_alone() {
        let (_, changes) = person_update_changes(
            None,
            &db::PersonUpdateShape::Email {
                email: Some("a@example.com"),
            },
            100,
        );
        assert_eq!(
            changes,
            vec![AuditFieldChange {
                field: "email".into(),
                before: None,
                after: Some(json!("a@example.com")),
            }]
        );
    }

    #[test]
    fn person_optional_fields_clear_to_none() {
        let mut p = person();
        p.email = Some("a@example.com".into());
        p.ses_api_person_id = Some("ses-1".into());
        p.missing_since = Some(50);

        let (_, changes) =
            person_update_changes(Some(&p), &db::PersonUpdateShape::Email { email: None }, 100);
        assert_eq!(
            change(&changes, "email").before,
            Some(json!("a@example.com"))
        );
        assert_eq!(change(&changes, "email").after, None);

        let (_, changes) = person_update_changes(
            Some(&p),
            &db::PersonUpdateShape::SesApiPersonId {
                ses_api_person_id: None,
            },
            100,
        );
        assert_eq!(fields(&changes), vec!["ses_api_person_id"]);

        let (_, changes) = person_update_changes(
            Some(&p),
            &db::PersonUpdateShape::MissingSince {
                missing_since: None,
            },
            100,
        );
        assert_eq!(change(&changes, "missing_since").before, Some(json!(50)));
        assert_eq!(change(&changes, "missing_since").after, None);
    }

    #[test]
    fn person_delete_and_undelete_are_a_delete_and_a_restore() {
        let mut p = person();
        p.missing_since = Some(50);
        let (action, changes) =
            person_update_changes(Some(&p), &db::PersonUpdateShape::Delete, 100);
        assert_eq!(action, AuditAction::Delete);
        assert_eq!(change(&changes, "deleted").after, Some(json!(100)));
        // The missing marker never outlives the live row.
        assert_eq!(change(&changes, "missing_since").after, None);

        p.deleted = Some(100);
        p.missing_since = None;
        let (action, changes) =
            person_update_changes(Some(&p), &db::PersonUpdateShape::Undelete, 200);
        assert_eq!(action, AuditAction::Restore);
        assert_eq!(fields(&changes), vec!["deleted"]);
        assert_eq!(change(&changes, "deleted").before, Some(json!(100)));
        assert_eq!(change(&changes, "deleted").after, None);
    }

    #[test]
    fn a_person_move_affects_both_locations() {
        let shape = db::PersonUpdateShape::Location {
            location_id: "loc-b",
        };
        assert_eq!(
            person_update_locations(Some(&person()), &shape),
            vec!["loc-a".to_string(), "loc-b".to_string()]
        );
        // Without a readable before only the destination is known.
        assert_eq!(
            person_update_locations(None, &shape),
            vec!["loc-b".to_string()]
        );
        // Anything else stays where it is.
        assert_eq!(
            person_update_locations(Some(&person()), &db::PersonUpdateShape::Delete),
            vec!["loc-a".to_string()]
        );
    }

    #[test]
    fn person_labels_follow_the_name_after_the_write() {
        assert_eq!(
            person_label_after(
                Some(&person()),
                &db::PersonUpdateShape::Fields {
                    first_name: "Alicia",
                    last_name: "Anderson",
                    registration_number: "1001"
                }
            )
            .as_deref(),
            Some("Alicia Anderson")
        );
        assert_eq!(
            person_label_after(Some(&person()), &db::PersonUpdateShape::Delete).as_deref(),
            Some("Alice Anderson")
        );
        assert_eq!(full_name("", "Solo"), "Solo");
    }

    // ── Period ───────────────────────────────────────────────────────────────

    #[test]
    fn period_create_includes_the_person_and_omits_bookkeeping() {
        let changes = period_create_changes(&period());
        assert_eq!(
            fields(&changes),
            vec![
                "person_id",
                "location_id",
                "category_id",
                "start_time",
                "end_time"
            ]
        );
        assert_eq!(change(&changes, "person_id").after, Some(json!("p1")));
        assert!(!fields(&changes).contains(&"version"));
        assert!(!fields(&changes).contains(&"updated_at"));
    }

    #[test]
    fn ending_a_period_diffs_the_returned_period() {
        let mut open = period();
        open.end_time = None;
        let mut ended = open.clone();
        ended.end_time = Some(5000);
        ended.signed_out_session_id = Some("s1".into());
        ended.version = 4;
        let changes = period_end_changes(&open, &ended);
        assert_eq!(fields(&changes), vec!["end_time", "signed_out_session_id"]);
    }

    #[test]
    fn period_time_category_update_and_three_state_comment() {
        let mut p = period();
        p.comment = Some("late".into());
        let shape = |comment| db::PeriodUpdateShape::TimeCategory {
            start_time: 1000,
            end_time: 2500,
            category_id: "cat1",
            signed_out_session_id: None,
            comment,
        };

        // `None` leaves the comment alone.
        let (_, changes) = period_update_changes(Some(&p), &shape(None), 9);
        assert_eq!(fields(&changes), vec!["end_time"]);
        assert_eq!(change(&changes, "end_time").before, Some(json!(2000)));
        assert_eq!(change(&changes, "end_time").after, Some(json!(2500)));

        // `Some(None)` clears it.
        let (_, changes) = period_update_changes(Some(&p), &shape(Some(None)), 9);
        assert_eq!(change(&changes, "comment").before, Some(json!("late")));
        assert_eq!(change(&changes, "comment").after, None);

        // `Some(Some(_))` sets it.
        let (_, changes) = period_update_changes(Some(&p), &shape(Some(Some("on time"))), 9);
        assert_eq!(change(&changes, "comment").after, Some(json!("on time")));
    }

    #[test]
    fn period_fields_update_that_moves_it_affects_both_locations() {
        let shape = db::PeriodUpdateShape::Fields {
            person_id: "p1",
            location_id: "loc-b",
            category_id: "cat1",
            start_time: 1000,
            end_time: 2000,
            comment: None,
        };
        let (action, changes) = period_update_changes(Some(&period()), &shape, 9);
        assert_eq!(action, AuditAction::Update);
        assert_eq!(fields(&changes), vec!["location_id"]);
        assert_eq!(
            period_update_locations(Some(&period()), &shape),
            vec!["loc-a".to_string(), "loc-b".to_string()]
        );
    }

    #[test]
    fn period_guest_edit_and_delete() {
        let mut guest = period();
        guest.person_id = None;
        guest.category_id = None;
        guest.guest_name = Some("Visitor".into());
        let (_, changes) = period_update_changes(
            Some(&guest),
            &db::PeriodUpdateShape::Guest {
                guest_name: "Visitor Two",
                start_time: 1000,
                end_time: 2000,
                comment: None,
            },
            9,
        );
        assert_eq!(fields(&changes), vec!["guest_name"]);

        let (action, changes) =
            period_update_changes(Some(&period()), &db::PeriodUpdateShape::Delete, 77);
        assert_eq!(action, AuditAction::Delete);
        assert_eq!(change(&changes, "deleted").after, Some(json!(77)));
    }

    // ── User ─────────────────────────────────────────────────────────────────

    #[test]
    fn user_grants_compare_as_sets() {
        let same_grants_other_order = db::UserUpdateShape::Fields {
            email: "a@example.com",
            is_super: false,
            is_dev: false,
            enabled: true,
            location_grants: vec!["loc-a".into(), "loc-b".into()],
            location_read_only_grants: None,
        };
        assert!(user_update_changes(Some(&user()), &same_grants_other_order).is_empty());

        let promoted = db::UserUpdateShape::Fields {
            email: "a@example.com",
            is_super: true,
            is_dev: false,
            enabled: true,
            location_grants: vec!["loc-a".into()],
            location_read_only_grants: Some(vec!["loc-c".into()]),
        };
        let changes = user_update_changes(Some(&user()), &promoted);
        assert_eq!(
            fields(&changes),
            vec!["is_super", "location_grants", "location_read_only_grants"]
        );
        assert_eq!(
            change(&changes, "location_grants").before,
            Some(json!(["loc-a", "loc-b"]))
        );
    }

    #[test]
    fn user_access_time_is_not_audited_but_the_other_shapes_are() {
        assert!(!user_shape_is_audited(&db::UserUpdateShape::AccessTime));
        assert!(user_shape_is_audited(&db::UserUpdateShape::EmailConfig {
            email_config: serde_json::Map::new()
        }));
        assert!(user_shape_is_audited(
            &db::UserUpdateShape::DisaggregateVirtualPeriods { value: true }
        ));
    }

    #[test]
    fn user_email_config_and_disaggregation_diffs() {
        let mut config = serde_json::Map::new();
        config.insert("daily".into(), json!(true));
        let changes = user_update_changes(
            Some(&user()),
            &db::UserUpdateShape::EmailConfig {
                email_config: config,
            },
        );
        assert_eq!(fields(&changes), vec!["email_config"]);
        assert_eq!(
            change(&changes, "email_config").after,
            Some(json!({"daily": true}))
        );

        let changes = user_update_changes(
            Some(&user()),
            &db::UserUpdateShape::DisaggregateVirtualPeriods { value: true },
        );
        assert_eq!(fields(&changes), vec!["disaggregate_virtual_periods"]);
    }

    #[test]
    fn user_create_omits_empty_sets_and_labels_by_email() {
        let changes = user_create_changes(&user());
        assert_eq!(
            fields(&changes),
            vec![
                "email",
                "is_super",
                "is_dev",
                "enabled",
                "location_grants",
                "disaggregate_virtual_periods"
            ]
        );
        assert_eq!(
            user_label_after(
                Some(&user()),
                &db::UserUpdateShape::Fields {
                    email: "new@example.com",
                    is_super: false,
                    is_dev: false,
                    enabled: true,
                    location_grants: vec![],
                    location_read_only_grants: None
                }
            )
            .as_deref(),
            Some("new@example.com")
        );
    }

    // ── Session ──────────────────────────────────────────────────────────────

    #[test]
    fn session_create_redacts_the_code_and_the_key() {
        let mut keyed = session();
        keyed.code = None;
        keyed.public_key = Some("MFkwEwYHKoZIzj0CAQ-secret-public-key".into());
        keyed.key_fingerprint = Some("deadbeef".into());
        keyed.key_expires_at = Some(5000);

        let coded = session_create_changes(&session());
        assert_eq!(change(&coded, "code").after, Some(json!(REDACTED)));
        assert!(!fields(&coded).contains(&"active"));
        let coded_json = serde_json::to_string(&coded).unwrap();
        assert!(!coded_json.contains("123456"), "{coded_json}");

        let keyed_changes = session_create_changes(&keyed);
        assert_eq!(
            change(&keyed_changes, "public_key").after,
            Some(json!(REDACTED))
        );
        let keyed_json = serde_json::to_string(&keyed_changes).unwrap();
        assert!(!keyed_json.contains("secret-public-key"), "{keyed_json}");
        assert!(!keyed_json.contains("deadbeef"), "{keyed_json}");
    }

    #[test]
    fn wiping_a_session_code_records_the_redaction() {
        assert_eq!(
            session_wipe_code_changes(),
            vec![AuditFieldChange {
                field: "code".into(),
                before: Some(json!(REDACTED)),
                after: None,
            }]
        );
    }

    #[test]
    fn session_update_shapes() {
        let mut keyed = session();
        keyed.code = None;
        keyed.public_key = Some("pk".into());
        keyed.key_expires_at = Some(5000);

        let (action, changes) = session_update_changes(
            Some(&keyed),
            &db::SessionUpdateShape::ExtendKey { expires_at: 9000 },
            1,
        );
        assert_eq!(action, AuditAction::Update);
        assert_eq!(fields(&changes), vec!["key_expires_at"]);

        let (action, changes) = session_update_changes(
            Some(&keyed),
            &db::SessionUpdateShape::ReleaseKey { fingerprint: "fp" },
            77,
        );
        assert_eq!(action, AuditAction::Update);
        assert_eq!(
            fields(&changes),
            vec!["public_key", "key_expires_at", "key_released_at"]
        );
        assert_eq!(change(&changes, "public_key").before, Some(json!(REDACTED)));
        assert_eq!(change(&changes, "key_released_at").after, Some(json!(77)));

        let (action, changes) =
            session_update_changes(Some(&keyed), &db::SessionUpdateShape::Delete, 1);
        assert_eq!(action, AuditAction::Delete);
        assert_eq!(change(&changes, "active").before, Some(json!(true)));
        assert_eq!(change(&changes, "active").after, Some(json!(false)));

        let mut config = serde_json::Map::new();
        config.insert("theme".into(), json!("dark"));
        let (_, changes) = session_update_changes(
            Some(&session()),
            &db::SessionUpdateShape::Fields {
                name: "Front desk",
                config: &config,
                healthcheck_url: Some("https://hc.example.com/x"),
            },
            1,
        );
        assert_eq!(fields(&changes), vec!["config", "healthcheck_url"]);
    }

    #[test]
    fn session_info_heartbeats_are_not_audited() {
        assert!(!session_shape_is_audited(&db::SessionUpdateShape::Info {
            client_version: None,
            client_info: None,
            extend_key_expires_at: None
        }));
        assert!(session_shape_is_audited(&db::SessionUpdateShape::Delete));
        assert!(session_shape_is_audited(
            &db::SessionUpdateShape::ExtendKey { expires_at: 1 }
        ));
    }

    // ── API token, location, category, NITC ──────────────────────────────────

    fn api_token() -> ApiToken {
        ApiToken {
            id: "t1".to_string(),
            name: "Reporting".to_string(),
            token_hash: "super-secret-hash".to_string(),
            location_grants: vec!["loc-a".to_string()],
            read_only: true,
            created_at: 1,
            created_by_user_id: "u1".to_string(),
            expires_at: None,
            revoked_at: None,
            last_used_at: None,
        }
    }

    #[test]
    fn api_token_changes_never_carry_the_hash() {
        let created = api_token_create_changes(&api_token());
        assert!(
            !serde_json::to_string(&created)
                .unwrap()
                .contains("secret-hash")
        );
        assert!(!fields(&created).contains(&"token_hash"));

        let revoked =
            api_token_update_changes(Some(&api_token()), &db::ApiTokenUpdateShape::Revoke, 500);
        assert_eq!(fields(&revoked), vec!["revoked_at"]);
        assert_eq!(change(&revoked, "revoked_at").after, Some(json!(500)));

        let edited = api_token_update_changes(
            Some(&api_token()),
            &db::ApiTokenUpdateShape::Fields {
                name: "Reporting",
                location_grants: vec!["loc-a".into(), "loc-b".into()],
                read_only: true,
                expires_at: Some(9),
            },
            500,
        );
        assert_eq!(fields(&edited), vec!["location_grants", "expires_at"]);
        assert!(!api_token_shape_is_audited(
            &db::ApiTokenUpdateShape::TouchLastUsed
        ));
    }

    fn location() -> Location {
        Location {
            id: "loc-a".to_string(),
            name: "Unit A".to_string(),
            enabled: true,
            nitc_enabled: None,
            nitc_complete_on_export: true,
            ses_api_headquarters_id: Some("77".to_string()),
            last_successful_member_sync: Some(5),
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn location_updates_ignore_the_sync_heartbeat() {
        assert!(!location_shape_is_audited(
            &db::LocationUpdateShape::LastSyncTime { time: 9 }
        ));
        let changes = location_update_changes(
            Some(&location()),
            &db::LocationUpdateShape::Name { name: "Unit Alpha" },
        );
        assert_eq!(fields(&changes), vec!["name"]);

        let changes = location_update_changes(
            Some(&location()),
            &db::LocationUpdateShape::Fields {
                name: "Unit A",
                enabled: false,
                nitc_enabled: Some(123),
                nitc_complete_on_export: None,
            },
        );
        assert_eq!(fields(&changes), vec!["enabled", "nitc_enabled"]);
        // `last_successful_member_sync` is the heartbeat, never part of the diff.
        assert!(
            !fields(&location_create_changes(&location())).contains(&"last_successful_member_sync")
        );
    }

    #[test]
    fn category_and_nitc_group_diffs() {
        let category = Category {
            id: "c1".into(),
            name: "Training".into(),
            enabled: true,
            is_virtual: false,
            nitc_participant_type: None,
            nitc_group_id: None,
            created_at: 1,
            updated_at: 1,
        };
        let changes =
            category_update_changes(Some(&category), "Training", false, false, Some("g1"), None);
        assert_eq!(fields(&changes), vec!["enabled", "nitc_group_id"]);

        let group = db::NitcGroup {
            id: "g1".into(),
            nitc_type: "attendee".into(),
            nitc_tag_ids: vec![3, 1],
            created_at: None,
            updated_at: None,
        };
        // Tag order is irrelevant, so the same tags in another order are no change.
        assert!(nitc_group_update_changes(Some(&group), "attendee", &[1, 3]).is_empty());
        let changes = nitc_group_update_changes(Some(&group), "attendee", &[1, 2]);
        assert_eq!(change(&changes, "nitc_tag_ids").before, Some(json!([1, 3])));
        assert_eq!(change(&changes, "nitc_tag_ids").after, Some(json!([1, 2])));
        let created = nitc_group_update_changes(None, "trainer", &[]);
        assert_eq!(fields(&created), vec!["nitc_type"]);
    }

    // ── Credentials and grants ───────────────────────────────────────────────

    #[test]
    fn credential_entries_carry_no_secrets() {
        let token = db::UserToken {
            id: "ut1".into(),
            token_hash: "user-token-hash-secret".into(),
            user_id: "u1".into(),
            created_at: 1,
            expires_at: 99,
            last_used_at: None,
        };
        let grant = OAuthGrant {
            id: "g1".into(),
            user_id: "u1".into(),
            client_id: "client".into(),
            client_name: "Claude".into(),
            redirect_uri: "https://claude.ai/cb".into(),
            resource: "https://x/mcp".into(),
            scope: "seslogin".into(),
            access_token_hash: "access-hash-secret".into(),
            access_expires_at: 5,
            refresh_token_hash: "refresh-hash-secret".into(),
            refresh_expires_at: 6,
            expires_at: 7,
            created_at: 1,
            last_used_at: None,
        };
        let credential = db::WebauthnCredential {
            id: "cred".into(),
            user_id: "u1".into(),
            name: "MacBook".into(),
            passkey_json: "{\"cred\":\"passkey-secret\"}".into(),
            created_at: 1,
            last_used_at: None,
        };
        let all = [
            user_token_create_changes(&token),
            oauth_grant_create_changes(&grant),
            oauth_grant_delete_changes(&grant),
            webauthn_create_changes(&credential),
            webauthn_delete_changes(&credential),
            webauthn_rename_changes(Some("MacBook"), "Work laptop"),
        ];
        for changes in &all {
            let json = serde_json::to_string(changes).unwrap();
            assert!(!json.contains("secret"), "{json}");
        }
        assert_eq!(
            fields(&all[0]),
            vec!["user_id", "expires_at"],
            "user token entries are the user and the expiry only"
        );
        assert_eq!(fields(&all[2]), vec!["user_id", "client_id", "client_name"]);
        assert_eq!(change(&all[2], "client_name").after, None);
        assert_eq!(change(&all[5], "name").before, Some(json!("MacBook")));
    }

    // ── Serialisation ────────────────────────────────────────────────────────

    #[test]
    fn a_change_serialises_without_its_absent_sides_and_round_trips() {
        let created = AuditFieldChange {
            field: "name".into(),
            before: None,
            after: Some(json!("x")),
        };
        assert_eq!(
            serde_json::to_value(&created).unwrap(),
            json!({"field": "name", "after": "x"})
        );
        let back: AuditFieldChange =
            serde_json::from_value(json!({"field": "name", "before": "y"})).unwrap();
        assert_eq!(back.before, Some(json!("y")));
        assert_eq!(back.after, None);
    }

    #[test]
    fn enum_strings_are_stable() {
        assert_eq!(AuditAction::Restore.as_str(), "restore");
        let entity_types = [
            (AuditEntityType::User, "user"),
            (AuditEntityType::Person, "person"),
            (AuditEntityType::Period, "period"),
            (AuditEntityType::Session, "session"),
            (AuditEntityType::ApiToken, "api_token"),
            (AuditEntityType::Location, "location"),
            (AuditEntityType::Category, "category"),
            (AuditEntityType::NitcGroup, "nitc_group"),
            (AuditEntityType::NitcTag, "nitc_tag"),
            (AuditEntityType::UserToken, "user_token"),
            (AuditEntityType::OAuthGrant, "oauth_grant"),
            (AuditEntityType::WebauthnCredential, "webauthn_credential"),
        ];
        for (entity, name) in entity_types {
            assert_eq!(entity.as_str(), name);
        }
    }
}
