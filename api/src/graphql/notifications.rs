//! The caller's in-app notification inbox, used by the Tally app. What notifications
//! are, who gets them and how they are stored is in [`crate::notifications`] and
//! SCHEMA.md's `notification` section. This module holds the GraphQL type and the
//! bodies of `Query.myNotifications` / `myUnreadNotificationCount` and the mark-read
//! mutations, which `query.rs` and `mutations.rs` call through thin resolvers.
//!
//! Every field is the caller's own: the table is keyed by the caller's user ID, so no
//! one can read or mark anyone else's. Only users may — any enabled user, including a
//! connected app acting for one — and kiosk sessions, API tokens and edit links are
//! refused. The one extra check is at read time: a notification about a location the
//! caller can no longer access is left out, so stored text never reaches someone who
//! lost access after it was sent.

use anyhow::Result;
use async_graphql::connection::{Connection, EmptyFields};
use async_graphql::{Context, ID, SimpleObject};
use std::sync::Arc;

use super::error::ApiError;
use super::pagination::{build_connection, pagination_args};
use crate::app::{App, HasDb};
use crate::auth::AuthInfo;
use crate::db::{self, Handler};
use crate::notifications::MAX_UNREAD_COUNT;

pub(super) const DEFAULT_PAGE_SIZE: usize = 50;
pub(super) const MAX_PAGE_SIZE: usize = 200;
/// The most notifications `markNotificationsRead` takes in one call.
const MAX_MARK_IDS: usize = 200;
/// How many times a page is topped up from the database after rows were left out for
/// being expired or for a location the caller lost access to, before giving up.
const MAX_PAGE_FETCHES: usize = 10;
/// How many unread notifications `markAllNotificationsRead` marks per round.
const MARK_ALL_BATCH: usize = 100;

/// One entry in the caller's notification inbox.
#[derive(SimpleObject, Clone, Debug)]
pub struct Notification {
    /// Unique per notification for the caller; also its paging cursor.
    pub id: ID,
    /// The app that produced it, e.g. `reservations`.
    pub source: String,
    /// What happened, e.g. `reservation.for_you`.
    pub kind: String,
    pub title: String,
    pub body: Option<String>,
    /// The location it is about, if any.
    pub location_id: Option<ID>,
    /// What to open when it is tapped, e.g. `reservation`, with `targetId`.
    pub target_type: Option<String>,
    pub target_id: Option<ID>,
    /// Unix seconds.
    pub created_at: i64,
    /// Unix seconds when it was marked read; null while unread.
    pub read_at: Option<i64>,
    pub read: bool,
}

impl From<&db::NotificationRecord> for Notification {
    fn from(r: &db::NotificationRecord) -> Self {
        Self {
            id: ID(r.sk.clone()),
            source: r.source.clone(),
            kind: r.kind.clone(),
            title: r.title.clone(),
            body: r.body.clone(),
            location_id: r.location_id.clone().map(ID),
            target_type: r.target_type.clone(),
            target_id: r.target_id.clone().map(ID),
            created_at: r.created_at as i64,
            read_at: r.read_at.map(|t| t as i64),
            read: r.is_read(),
        }
    }
}

/// The result of marking notifications read.
#[derive(SimpleObject, Clone, Debug)]
pub struct MarkNotificationsReadPayload {
    /// How many notifications are still unread, counting no further than 100 (show
    /// "99+" above 99). Read from an index that can lag a moment behind the write.
    pub unread_count: i32,
}

pub(super) type NotificationConnection = Connection<String, Notification, EmptyFields, EmptyFields>;

/// The calling user's ID; anyone but a user is refused.
fn require_user<'a>(ctx: &'a Context<'_>) -> Result<&'a str> {
    match ctx.data_opt::<AuthInfo>() {
        Some(AuthInfo::User { id, .. }) => Ok(id.as_str()),
        _ => Err(ApiError::forbidden("Notifications are only available to signed-in users").into()),
    }
}

/// Whether the caller may still see a notification about `location_id`: it has none, or
/// the caller is a super user or holds either role there.
fn can_see(ctx: &Context<'_>, location_id: Option<&str>) -> bool {
    let Some(location_id) = location_id else {
        return true;
    };
    match ctx.data_opt::<AuthInfo>() {
        Some(AuthInfo::User { is_super: true, .. }) => true,
        Some(AuthInfo::User {
            location_grants,
            location_read_only_grants,
            ..
        }) => location_grants
            .iter()
            .chain(location_read_only_grants)
            .any(|g| g == location_id),
        _ => false,
    }
}

/// `Query.myNotifications`: the caller's notifications, newest first.
pub(super) async fn my_notifications<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    first: Option<i32>,
    after: Option<String>,
    unread_only: Option<bool>,
) -> Result<NotificationConnection> {
    let user_id = require_user(ctx)?;
    let (page_size, _) = pagination_args(first, None, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let unread_only = unread_only.unwrap_or(false);
    let has_after = after.is_some();
    let app = ctx.data_unchecked::<Arc<A>>();
    let now = crate::clock::now_sec();

    // One more than the page, to know whether there is a next one. Rows that are
    // expired (the TTL is lazy) or about a location the caller lost access to are left
    // out, so top the page up from where the last read ended. If every round is
    // dropped to nothing the page just ends short.
    let want = page_size + 1;
    let mut rows: Vec<db::NotificationRecord> = Vec::new();
    let mut cursor = after;
    for _ in 0..MAX_PAGE_FETCHES {
        let batch = app
            .db()
            .list_notifications(user_id, cursor.as_deref(), want - rows.len(), unread_only)
            .await?;
        let asked = want - rows.len();
        let exhausted = batch.len() < asked;
        cursor = batch.last().map(|r| r.sk.clone());
        rows.extend(
            batch
                .into_iter()
                .filter(|r| r.expires_at > now && can_see(ctx, r.location_id.as_deref())),
        );
        if rows.len() >= want || exhausted {
            break;
        }
    }

    Ok(build_connection(
        rows,
        page_size,
        false,
        has_after,
        false,
        |r| (r.sk.clone(), Notification::from(r)),
    ))
}

/// `Query.myUnreadNotificationCount`, capped at 100. It counts from an index without
/// reading the rows, so a notification about a location the caller has since lost access
/// to is still counted although `myNotifications` hides it.
pub(super) async fn my_unread_notification_count<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
) -> Result<i32> {
    let user_id = require_user(ctx)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    unread_count(app.as_ref(), user_id).await
}

async fn unread_count<A: HasDb + Sync>(app: &A, user_id: &str) -> Result<i32> {
    let count = app
        .db()
        .count_unread_notifications(user_id, MAX_UNREAD_COUNT)
        .await?;
    Ok(i32::try_from(count).unwrap_or(i32::MAX))
}

/// `markNotificationsRead`: mark these of the caller's notifications read. An ID that is
/// already read, or that is not one of the caller's, is ignored.
pub(super) async fn mark_notifications_read<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    ids: Vec<ID>,
) -> Result<MarkNotificationsReadPayload> {
    let user_id = require_user(ctx)?;
    if ids.len() > MAX_MARK_IDS {
        return Err(ApiError::bad_request(format!(
            "At most {MAX_MARK_IDS} notifications can be marked read at once"
        ))
        .into());
    }
    let sks: Vec<String> = ids.into_iter().map(|id| id.0).collect();
    app.db()
        .mark_notifications_read(user_id, &sks, crate::clock::now_sec())
        .await?;
    Ok(MarkNotificationsReadPayload {
        unread_count: unread_count(app, user_id).await?,
    })
}

/// `markAllNotificationsRead`: mark everything the caller has unread as read.
pub(super) async fn mark_all_notifications_read<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
) -> Result<MarkNotificationsReadPayload> {
    let user_id = require_user(ctx)?;
    let now = crate::clock::now_sec();
    // Page through the unread index by cursor rather than starting over each time, so
    // an index that is slow to drop a row just marked cannot make this loop.
    let mut cursor: Option<String> = None;
    loop {
        let page = app
            .db()
            .list_notifications(user_id, cursor.as_deref(), MARK_ALL_BATCH, true)
            .await?;
        let Some(last) = page.last() else {
            break;
        };
        cursor = Some(last.sk.clone());
        let sks: Vec<String> = page.iter().map(|r| r.sk.clone()).collect();
        app.db().mark_notifications_read(user_id, &sks, now).await?;
        if page.len() < MARK_ALL_BATCH {
            break;
        }
    }
    Ok(MarkNotificationsReadPayload {
        unread_count: unread_count(app, user_id).await?,
    })
}
