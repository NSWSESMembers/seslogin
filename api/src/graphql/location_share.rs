//! Locate: a user sharing their phone's position with a location's members for a fixed
//! time, used by the Tally app. The rules are in [`crate::location_share`]; storage is
//! described in SCHEMA.md. This module holds the GraphQL type and the bodies of the
//! `Location.locationShares` / `Query.myLocationShare` fields and the sharing
//! mutations, which `query.rs` and `mutations.rs` call through thin resolvers.
//!
//! Access is [`require_location_share_access`] on the share's location: any user with
//! Admin or Read only access, and super users, may share with it and see who is
//! sharing there. Each user has at most one share, and only they can change it.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, ID, InputObject, Object};
use std::sync::Arc;

use super::auth::require_location_share_access;
use super::dataloader::DatabaseLoader;
use super::error::ApiError;
use super::query::{Location, Person};
use super::{LocationId, PersonId, UserId};
use crate::app::{App, HasDb};
use crate::auth::AuthInfo;
use crate::db::{self, Handler};
use crate::location_share::{self, PositionInput};

/// Where the phone is, as it reports it. Leave out what it doesn't know.
#[derive(InputObject, Clone, Copy)]
pub struct LocationSharePositionInput {
    /// WGS 84 degrees.
    pub latitude: f64,
    pub longitude: f64,
    /// Radius of uncertainty, in metres.
    pub accuracy: Option<f64>,
    /// Metres above the WGS 84 ellipsoid.
    pub altitude: Option<f64>,
    pub altitude_accuracy: Option<f64>,
    /// Direction of travel, degrees clockwise from north; negative for unknown.
    pub heading: Option<f64>,
    /// Metres per second; negative for unknown.
    pub speed: Option<f64>,
    /// When the phone took the fix (Unix seconds); now when omitted.
    pub recorded_at: Option<i64>,
}

impl LocationSharePositionInput {
    fn validate(self, now: u64) -> Result<db::SharedPosition> {
        location_share::validate_position(
            PositionInput {
                latitude: self.latitude,
                longitude: self.longitude,
                accuracy: self.accuracy,
                altitude: self.altitude,
                altitude_accuracy: self.altitude_accuracy,
                heading: self.heading,
                speed: self.speed,
                recorded_at: self.recorded_at,
            },
            now,
        )
        .map_err(|m| ApiError::bad_request(m).into())
    }
}

pub struct LocationShare<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::LocationShare,
    /// When the share was read, so every field agrees on what "now" is.
    now: u64,
}

impl<A: App + HasDb + Send + Sync + 'static> LocationShare<A> {
    pub fn new(rec: db::LocationShare, now: u64) -> Self {
        Self {
            _marker: Default::default(),
            rec,
            now,
        }
    }

    /// The member the sharing user is linked to, if any and not deleted.
    async fn linked_person(&self, ctx: &Context<'_>) -> Result<Option<Person<A>>> {
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        let Some(person_id) = loader
            .load_one(UserId(ID(self.rec.user_id.clone())))
            .await
            .map_err(|e| anyhow!("Failed to load user via DataLoader: {}", e))?
            .flatten()
            .and_then(|u| u.rec.person_id)
        else {
            return Ok(None);
        };
        Ok(loader
            .load_one(PersonId(ID(person_id)))
            .await
            .map_err(|e| anyhow!("Failed to load person via DataLoader: {}", e))?
            .flatten()
            .filter(|p| p.rec.deleted.is_none()))
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> LocationShare<A> {
    /// The sharing user's ID: a user has at most one share.
    async fn id(&self) -> ID {
        ID(self.rec.user_id.clone())
    }

    /// Who is sharing: their member's name, or their email if their account isn't
    /// linked to a member.
    async fn name(&self, ctx: &Context<'_>) -> Result<String> {
        if let Some(person) = self.linked_person(ctx).await? {
            let name = format!("{} {}", person.rec.first_name, person.rec.last_name);
            if !name.trim().is_empty() {
                return Ok(name.trim().to_string());
            }
        }
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        Ok(loader
            .load_one(UserId(ID(self.rec.user_id.clone())))
            .await
            .map_err(|e| anyhow!("Failed to load user via DataLoader: {}", e))?
            .flatten()
            .map(|u| u.rec.email)
            .unwrap_or_else(|| "Someone".to_string()))
    }

    /// The member the sharing user's account is linked to; null if it isn't.
    async fn person(&self, ctx: &Context<'_>) -> Result<Option<Person<A>>> {
        self.linked_person(ctx).await
    }

    /// Whether the caller is the one sharing.
    async fn is_mine(&self, ctx: &Context<'_>) -> bool {
        caller_id(ctx) == Some(self.rec.user_id.as_str())
    }

    async fn location_id(&self) -> ID {
        ID(self.rec.location_id.clone())
    }

    /// The location it is shared with.
    async fn location(&self, ctx: &Context<'_>) -> Result<Location<A>> {
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        loader
            .load_one(LocationId(ID(self.rec.location_id.clone())))
            .await
            .map_err(|e| anyhow!("Failed to load location via DataLoader: {}", e))?
            .flatten()
            .ok_or_else(|| anyhow!("Location with ID {} missing", self.rec.location_id))
    }

    /// WGS 84 degrees.
    async fn latitude(&self) -> f64 {
        self.rec.position.latitude
    }

    async fn longitude(&self) -> f64 {
        self.rec.position.longitude
    }

    /// Radius of uncertainty of the position, in metres; null if the phone didn't say.
    async fn accuracy(&self) -> Option<f64> {
        self.rec.position.accuracy
    }

    /// Metres above the WGS 84 ellipsoid.
    async fn altitude(&self) -> Option<f64> {
        self.rec.position.altitude
    }

    async fn altitude_accuracy(&self) -> Option<f64> {
        self.rec.position.altitude_accuracy
    }

    /// Direction of travel, degrees clockwise from north; null when unknown.
    async fn heading(&self) -> Option<f64> {
        self.rec.position.heading
    }

    /// Metres per second; null when unknown.
    async fn speed(&self) -> Option<f64> {
        self.rec.position.speed
    }

    /// When the phone took the fix (Unix seconds).
    async fn recorded_at(&self) -> i64 {
        self.rec.position.recorded_at as i64
    }

    /// When the latest position arrived (Unix seconds).
    async fn updated_at(&self) -> i64 {
        self.rec.updated_at as i64
    }

    /// How often the phone reports in, in seconds (60 to 300).
    async fn interval_seconds(&self) -> i32 {
        self.rec.interval_secs as i32
    }

    async fn started_at(&self) -> i64 {
        self.rec.started_at as i64
    }

    /// When sharing stops (Unix seconds); later positions are refused.
    async fn ends_at(&self) -> i64 {
        self.rec.ends_at as i64
    }

    /// When the share stops showing: 7 minutes after the latest position.
    async fn visible_until(&self) -> i64 {
        self.rec.expires_at as i64
    }

    /// Whether it is still being shared, i.e. `endsAt` has not passed. An ended share
    /// still shows until `visibleUntil`.
    async fn active(&self) -> bool {
        self.now < self.rec.ends_at
    }
}

// ── Reads ────────────────────────────────────────────────────────────────────

fn caller_id<'a>(ctx: &'a Context<'_>) -> Option<&'a str> {
    match ctx.data_opt::<AuthInfo>() {
        Some(AuthInfo::User { id, .. }) => Some(id.as_str()),
        _ => None,
    }
}

fn require_caller<'a>(ctx: &'a Context<'_>) -> Result<&'a str> {
    caller_id(ctx).ok_or_else(|| {
        ApiError::forbidden("Location sharing is only available to signed-in users").into()
    })
}

/// `Location.locationShares`: everyone whose share still shows there, most recently
/// updated first.
pub(super) async fn location_shares<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    location_id: &str,
) -> Result<Vec<LocationShare<A>>> {
    require_location_share_access(ctx, location_id)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let now = crate::clock::now_sec();
    let mut shares: Vec<_> = app
        .db()
        .list_location_shares(location_id)
        .await?
        .into_iter()
        .filter(|s| s.expires_at > now)
        .collect();
    shares.sort_by(|a, b| (b.updated_at, &a.user_id).cmp(&(a.updated_at, &b.user_id)));
    Ok(shares
        .into_iter()
        .map(|s| LocationShare::new(s, now))
        .collect())
}

/// `Query.myLocationShare`: the caller's share while it still shows; null otherwise.
pub(super) async fn my_location_share<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
) -> Result<Option<LocationShare<A>>> {
    let user_id = require_caller(ctx)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let now = crate::clock::now_sec();
    let share = first(app.db().get_location_shares(&[user_id]).await?);
    Ok(share
        .filter(|s| s.expires_at > now)
        .map(|s| LocationShare::new(s, now)))
}

// ── Writes ───────────────────────────────────────────────────────────────────

fn first<T>(rows: Vec<Option<T>>) -> Option<T> {
    rows.into_iter().next().flatten()
}

fn bad_request(message: String) -> anyhow::Error {
    ApiError::bad_request(message).into()
}

pub(super) struct StartLocationShareArgs {
    pub location_id: ID,
    pub duration_minutes: i32,
    pub interval_seconds: i32,
    pub position: LocationSharePositionInput,
}

/// `startLocationShare`: share with a location from now, replacing any share the
/// caller already has (at any location).
pub(super) async fn start_location_share<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: StartLocationShareArgs,
) -> Result<LocationShare<A>> {
    let user_id = require_caller(ctx)?;
    require_location_share_access(ctx, &args.location_id)?;
    first(app.db().get_locations(&[&args.location_id]).await?)
        .ok_or_else(|| ApiError::not_found("Location", args.location_id.as_str()))?;
    let duration = location_share::validate_duration(args.duration_minutes).map_err(bad_request)?;
    let interval = location_share::validate_interval(args.interval_seconds).map_err(bad_request)?;
    let now = crate::clock::now_sec();
    let position = args.position.validate(now)?;
    let share = db::LocationShare {
        user_id: user_id.to_string(),
        location_id: args.location_id.to_string(),
        position,
        interval_secs: interval,
        started_at: now,
        ends_at: now + duration,
        updated_at: now,
        expires_at: location_share::expires_at(now),
    };
    app.db().put_location_share(&share).await?;
    Ok(LocationShare::new(share, now))
}

/// `updateLocationShare`: record where the caller is now. `NOT_FOUND` once the share
/// has ended or been stopped, which tells the phone to stop sending.
pub(super) async fn update_location_share<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    position: LocationSharePositionInput,
) -> Result<LocationShare<A>> {
    let user_id = require_caller(ctx)?;
    let now = crate::clock::now_sec();
    let position = position.validate(now)?;
    let not_sharing = || ApiError::not_found("Location share", user_id);
    let share = first(app.db().get_location_shares(&[user_id]).await?)
        .filter(|s| s.ends_at > now)
        .ok_or_else(not_sharing)?;
    // Access may have been taken away since the share started.
    require_location_share_access(ctx, &share.location_id)?;
    let share = app
        .db()
        .update_location_share_position(user_id, &position, now, location_share::expires_at(now))
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => not_sharing().into(),
            e => anyhow::Error::from(e),
        })?;
    Ok(LocationShare::new(share, now))
}

/// `stopLocationShare`: stop sharing and stop showing at once. False if the caller
/// wasn't sharing.
pub(super) async fn stop_location_share<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
) -> Result<bool> {
    let user_id = require_caller(ctx)?;
    Ok(app.db().delete_location_share(user_id).await?.is_some())
}
