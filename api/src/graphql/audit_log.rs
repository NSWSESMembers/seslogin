//! GraphQL read API for the audit log (`{prefix}_audit_log`, see SCHEMA.md and
//! `audit::AuditingHandler`, which writes it).
//!
//! Exposed in two places, both newest first:
//! - `Location.auditLog` — that location's entries. Any *user* who can view the location
//!   (Admin or Read only, or a super user) may read it. Kiosk sessions, API tokens and
//!   edit-link tokens are rejected even though they can read other location data: who
//!   changed what is an administrative view, not something a device needs.
//! - `Query.auditLog` — every event once, including those about global entities (users,
//!   categories, …). Super users only.
//!
//! Two attributes are personal data and shown to super users only: the client IP and the
//! `via` of an actor (which OAuth grant / user token acted).

use anyhow::{Result, anyhow};
use async_graphql::connection::{Connection, EmptyFields};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, Enum, ID, Object, SimpleObject};
use std::sync::Arc;

use super::auth::require_location_access;
use super::dataloader::DatabaseLoader;
use super::error::ApiError;
use super::pagination::{build_connection, pagination_args};
use super::query::Location;
use super::{ApiTokenNameId, LocationId, SessionId, UserId};
use crate::app::{App, HasDb};
use crate::auth::AuthInfo;
use crate::db::{self, Handler};

pub(super) const DEFAULT_AUDIT_PAGE_SIZE: usize = 50;
pub(super) const MAX_AUDIT_PAGE_SIZE: usize = 200;

/// What a recorded write did to its entity.
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum AuditAction {
    Create,
    Update,
    Delete,
    Restore,
}

impl From<db::AuditAction> for AuditAction {
    fn from(a: db::AuditAction) -> Self {
        match a {
            db::AuditAction::Create => Self::Create,
            db::AuditAction::Update => Self::Update,
            db::AuditAction::Delete => Self::Delete,
            db::AuditAction::Restore => Self::Restore,
        }
    }
}

/// The kind of record an entry is about.
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
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
    #[graphql(name = "OAUTH_GRANT")]
    OAuthGrant,
    WebauthnCredential,
}

impl From<db::AuditEntityType> for AuditEntityType {
    fn from(t: db::AuditEntityType) -> Self {
        match t {
            db::AuditEntityType::User => Self::User,
            db::AuditEntityType::Person => Self::Person,
            db::AuditEntityType::Period => Self::Period,
            db::AuditEntityType::Session => Self::Session,
            db::AuditEntityType::ApiToken => Self::ApiToken,
            db::AuditEntityType::Location => Self::Location,
            db::AuditEntityType::Category => Self::Category,
            db::AuditEntityType::NitcGroup => Self::NitcGroup,
            db::AuditEntityType::NitcTag => Self::NitcTag,
            db::AuditEntityType::UserToken => Self::UserToken,
            db::AuditEntityType::OAuthGrant => Self::OAuthGrant,
            db::AuditEntityType::WebauthnCredential => Self::WebauthnCredential,
        }
    }
}

impl From<AuditEntityType> for db::AuditEntityType {
    fn from(t: AuditEntityType) -> Self {
        match t {
            AuditEntityType::User => Self::User,
            AuditEntityType::Person => Self::Person,
            AuditEntityType::Period => Self::Period,
            AuditEntityType::Session => Self::Session,
            AuditEntityType::ApiToken => Self::ApiToken,
            AuditEntityType::Location => Self::Location,
            AuditEntityType::Category => Self::Category,
            AuditEntityType::NitcGroup => Self::NitcGroup,
            AuditEntityType::NitcTag => Self::NitcTag,
            AuditEntityType::UserToken => Self::UserToken,
            AuditEntityType::OAuthGrant => Self::OAuthGrant,
            AuditEntityType::WebauthnCredential => Self::WebauthnCredential,
        }
    }
}

/// Who made a write.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum AuditActorKind {
    /// A signed-in user (web, MCP or a user token).
    User,
    /// A kiosk device.
    Session,
    ApiToken,
    /// A member using a single-period edit link.
    PeriodLink,
    /// A background job or the CLI.
    System,
    /// A write made before anyone was identified (e.g. the first login code request).
    Unauthenticated,
    /// Not recorded, or a kind this server version does not know.
    Unknown,
}

impl AuditActorKind {
    /// Map the stored `actor_kind` string; anything unrecognised is `Unknown`, so an old
    /// server can still page through rows written by a newer one.
    fn from_stored(kind: &str) -> Self {
        match kind {
            "user" => Self::User,
            "session" => Self::Session,
            "api_token" => Self::ApiToken,
            "period_link" => Self::PeriodLink,
            "system" => Self::System,
            "unauthenticated" => Self::Unauthenticated,
            _ => Self::Unknown,
        }
    }
}

/// One field a write changed.
#[derive(SimpleObject, Clone, Debug, PartialEq)]
pub struct AuditFieldChange {
    /// The field's name as stored, e.g. `first_name`.
    pub field: String,
    /// The value before the write, or null if the field was absent (or the entity was just
    /// created). A JSON string is returned as its raw text, anything else as compact JSON
    /// (`true`, `42`, `["a","b"]`), so the text `5` is a string or a number — a viewer that
    /// cares can tell from the field. A secret that changed shows as `[redacted]`.
    pub before: Option<String>,
    /// The value after the write, or null if the field was removed. Same encoding as
    /// `before`.
    pub after: Option<String>,
}

/// Render a stored JSON value for display: a string as its raw text, anything else as
/// compact JSON.
fn render_value(v: &Option<serde_json::Value>) -> Option<String> {
    match v {
        None => None,
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(other) => Some(other.to_string()),
    }
}

impl From<&db::AuditFieldChange> for AuditFieldChange {
    fn from(c: &db::AuditFieldChange) -> Self {
        Self {
            field: c.field.clone(),
            before: render_value(&c.before),
            after: render_value(&c.after),
        }
    }
}

/// Whether the caller is a super user. Gates the personal-data fields (`ip`, `via`).
fn viewer_is_super(ctx: &Context<'_>) -> bool {
    matches!(
        ctx.data_opt::<AuthInfo>(),
        Some(AuthInfo::User { is_super: true, .. })
    )
}

/// The identity behind a write.
pub struct AuditActor<A: App + HasDb + Send + Sync + 'static> {
    _marker: std::marker::PhantomData<A>,
    kind: AuditActorKind,
    id: Option<String>,
    via: Option<String>,
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> AuditActor<A> {
    async fn kind(&self) -> AuditActorKind {
        self.kind
    }

    /// The actor's ID: a user, kiosk session or API token ID, the period ID of an edit
    /// link, or the job name for `SYSTEM` (`member-sync`, `cli`, …). Null when none was
    /// recorded.
    async fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// How a user acted when not through an ordinary web login: `oauth_grant:<id>` (an AI
    /// client over MCP), `user_token:<id>` or `oauth_client:<id>`. Super users only;
    /// null for everyone else.
    async fn via(&self, ctx: &Context<'_>) -> Option<&str> {
        if viewer_is_super(ctx) {
            self.via.as_deref()
        } else {
            None
        }
    }

    /// A name for the actor, looked up now rather than recorded: a user's email, a kiosk's
    /// name, an API token's name, the job name for `SYSTEM`, "Period edit link" for an
    /// edit link. Null when there is nothing to show or the record no longer exists.
    async fn label(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        let Some(id) = self.id.clone() else {
            return Ok(match self.kind {
                AuditActorKind::PeriodLink => Some("Period edit link".to_string()),
                _ => None,
            });
        };
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        Ok(match self.kind {
            AuditActorKind::User => loader
                .load_one(UserId(ID(id)))
                .await
                .map_err(|e| anyhow!("Failed to load user via DataLoader: {}", e))?
                .flatten()
                .map(|u| u.rec.email),
            AuditActorKind::Session => loader
                .load_one(SessionId(ID(id)))
                .await
                .map_err(|e| anyhow!("Failed to load session via DataLoader: {}", e))?
                .flatten()
                .map(|s| s.rec.name),
            AuditActorKind::ApiToken => loader
                .load_one(ApiTokenNameId(id))
                .await
                .map_err(|e| anyhow!("Failed to load API token via DataLoader: {}", e))?,
            AuditActorKind::System => Some(id),
            AuditActorKind::PeriodLink => Some("Period edit link".to_string()),
            AuditActorKind::Unauthenticated | AuditActorKind::Unknown => None,
        })
    }
}

/// One recorded write, as it appears in a particular view of the log.
pub struct AuditEntry<A: App + HasDb + Send + Sync + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::AuditEntry,
}

impl<A: App + HasDb + Send + Sync + 'static> AuditEntry<A> {
    fn new(rec: db::AuditEntry) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> AuditEntry<A> {
    /// Unique per entry *in a view*. A write that touched two locations is two entries
    /// (one in each location's log) with different IDs.
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    /// When the write happened, in Unix seconds.
    async fn timestamp(&self) -> i64 {
        self.rec.ts as i64
    }

    async fn action(&self) -> AuditAction {
        self.rec.action.into()
    }

    async fn entity_type(&self) -> AuditEntityType {
        self.rec.entity_type.into()
    }

    /// The ID of the record that was changed.
    async fn entity_id(&self) -> ID {
        ID(self.rec.entity_id.clone())
    }

    /// A human-readable name for the record as it was at the time (e.g. a member's full
    /// name), so the entry still reads sensibly after the record is renamed or deleted.
    async fn entity_label(&self) -> Option<&str> {
        self.rec.entity_label.as_deref()
    }

    /// The location this entry belongs to. In a location's log, that location. In the
    /// all-locations view, the first location the write touched (a write that moved a
    /// member between units appears once there). Null for a global record (user,
    /// category, …) and if the location no longer exists.
    async fn location(&self, ctx: &Context<'_>) -> Result<Option<Location<A>>> {
        let Some(location_id) = self.rec.location_ids.first() else {
            return Ok(None);
        };
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        Ok(loader
            .load_one(LocationId(ID(location_id.clone())))
            .await
            .map_err(|e| anyhow!("Failed to load location via DataLoader: {}", e))?
            .flatten())
    }

    async fn actor(&self) -> AuditActor<A> {
        AuditActor {
            _marker: Default::default(),
            kind: AuditActorKind::from_stored(&self.rec.actor_kind),
            id: self.rec.actor_id.clone(),
            via: self.rec.actor_via.clone(),
        }
    }

    /// The client IP address of the request. Super users only; null for everyone else,
    /// because an IP address is personal data.
    async fn ip(&self, ctx: &Context<'_>) -> Option<&str> {
        if viewer_is_super(ctx) {
            self.rec.ip.as_deref()
        } else {
            None
        }
    }

    /// The fields the write changed, before and after. Empty when the write has no
    /// field-level detail to show.
    async fn changes(&self) -> Vec<AuditFieldChange> {
        self.rec.changes.iter().map(Into::into).collect()
    }
}

pub(super) type AuditEntryConnection<A> =
    Connection<String, AuditEntry<A>, EmptyFields, EmptyFields>;

/// Run one page of an audit log query and shape it as a Relay connection. Authorisation
/// is the caller's job and must happen before this.
///
/// The connection is newest first. `first`/`after` page towards older entries;
/// `last`/`before` page back towards newer ones. `first` with `before`, or `last` with
/// `after`, is rejected: the opposite pairing has no use here and would need a window
/// bounded on both sides.
pub(super) async fn audit_log_connection<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    scope: db::AuditScope,
    first: Option<i32>,
    after: Option<String>,
    last: Option<i32>,
    before: Option<String>,
    entity_type: Option<AuditEntityType>,
) -> Result<AuditEntryConnection<A>> {
    let (page_size, is_last_mode) =
        pagination_args(first, last, DEFAULT_AUDIT_PAGE_SIZE, MAX_AUDIT_PAGE_SIZE)?;
    if is_last_mode && after.is_some() {
        return Err(ApiError::bad_request("`last` pages with `before`, not `after`").into());
    }
    if !is_last_mode && before.is_some() {
        return Err(
            ApiError::bad_request("`before` needs `last`; use `after` with `first`").into(),
        );
    }
    let (has_after, has_before) = (after.is_some(), before.is_some());
    let cursor = after.or(before);
    if let Some(cursor) = &cursor
        && db::parse_audit_cursor(cursor).is_none()
    {
        return Err(ApiError::bad_request("Invalid cursor").into());
    }
    let limit = i32::try_from(page_size.saturating_add(1))
        .map_err(|_| anyhow!("Requested page is too large"))?;

    let app = ctx.data_unchecked::<Arc<A>>();
    let rows = app
        .db()
        .list_audit_entries(db::ListAuditEntriesQuery {
            scope,
            entity_type: entity_type.map(Into::into),
            // Backwards paging is the same query in the opposite order: the rows nearest
            // the cursor come first and `build_connection` flips them back.
            page: db::ListAuditEntriesPage {
                after: cursor,
                limit,
                descending: !is_last_mode,
            },
        })
        .await
        .map_err(|e| {
            tracing::warn!("db error: {:?}", e);
            e
        })?;

    Ok(build_connection(
        rows,
        page_size,
        is_last_mode,
        has_after,
        has_before,
        |e| (e.sk(), AuditEntry::new(e.clone())),
    ))
}

/// `Location.auditLog`: authorise, then read that location's log. Only users — see the
/// module docs.
pub(super) async fn location_audit_log<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    location_id: &str,
    first: Option<i32>,
    after: Option<String>,
    last: Option<i32>,
    before: Option<String>,
    entity_type: Option<AuditEntityType>,
) -> Result<AuditEntryConnection<A>> {
    if !matches!(ctx.data_opt::<AuthInfo>(), Some(AuthInfo::User { .. })) {
        return Err(ApiError::forbidden("The audit log is only available to users").into());
    }
    require_location_access(ctx, location_id)?;
    audit_log_connection(
        ctx,
        db::AuditScope::Location(location_id.to_string()),
        first,
        after,
        last,
        before,
        entity_type,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strings_render_raw_and_everything_else_as_compact_json() {
        assert_eq!(render_value(&None), None);
        assert_eq!(
            render_value(&Some(json!("Anderson"))).as_deref(),
            Some("Anderson")
        );
        assert_eq!(render_value(&Some(json!(42))).as_deref(), Some("42"));
        assert_eq!(render_value(&Some(json!(true))).as_deref(), Some("true"));
        assert_eq!(
            render_value(&Some(json!(["a", "b"]))).as_deref(),
            Some(r#"["a","b"]"#)
        );
        assert_eq!(render_value(&Some(json!(""))).as_deref(), Some(""));
    }

    #[test]
    fn unknown_actor_kinds_map_to_unknown() {
        assert_eq!(AuditActorKind::from_stored("user"), AuditActorKind::User);
        assert_eq!(
            AuditActorKind::from_stored("period_link"),
            AuditActorKind::PeriodLink
        );
        assert_eq!(
            AuditActorKind::from_stored("something_new"),
            AuditActorKind::Unknown
        );
    }

    #[test]
    fn enums_round_trip_with_the_db_types() {
        use db::AuditEntityType as Db;
        for t in [
            Db::User,
            Db::Person,
            Db::Period,
            Db::Session,
            Db::ApiToken,
            Db::Location,
            Db::Category,
            Db::NitcGroup,
            Db::NitcTag,
            Db::UserToken,
            Db::OAuthGrant,
            Db::WebauthnCredential,
        ] {
            assert_eq!(Db::from(AuditEntityType::from(t)), t);
            assert_eq!(Db::parse(t.as_str()), Some(t));
        }
    }
}
