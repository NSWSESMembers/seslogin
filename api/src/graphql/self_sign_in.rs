//! Self sign-in: a member signed in to Tally signs themselves in and out at a unit,
//! exactly as they would by typing their member number at a kiosk there. The unit is
//! their primary unit unless they pick another they can use in Tally (see
//! [`sign_in_location_id`]), as a member may sign in at another unit's kiosk.
//!
//! The authority is being that member (an [`AuthInfo::Member`]), **not** a location
//! grant: a member needs no role anywhere to sign themselves in at their primary unit. So this
//! deliberately uses neither `require_location_access` nor
//! `require_location_write_access`; [`linked_person`] is the check, plus
//! `require_member_location` for a unit other than their primary unit. Only Tally members qualify —
//! users on their own, kiosk sessions, API tokens and period links are refused — and
//! only the chosen unit's periods are ever considered, as at a kiosk, which scopes by
//! location. See `mutations.rs` for `selfSignIn` / `selfSignOut`.

use anyhow::{Result, anyhow};
use async_graphql::{Context, ID, Object, SimpleObject};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::warn;

use super::auth::require_member_location;
use super::error::ApiError;
use super::query::{Location, Period, Person, QuickPick, build_quick_pick};
use crate::app::{App, HasDb};
use crate::auth::AuthInfo;
use crate::db::{self, Handler};

/// Why a mutation could not find the caller's member.
const NO_MEMBER: &str = "Your member record is no longer available";

/// The person ID of the member signed in to Tally, refusing every other kind of caller.
/// A user on their own, a kiosk, an API token and an edit link are FORBIDDEN (they are
/// not a member acting for themselves).
pub(super) fn require_member_id(ctx: &Context<'_>) -> Result<String> {
    match ctx.data_opt::<AuthInfo>() {
        Some(AuthInfo::Member { person_id, .. }) => Ok(person_id.clone()),
        Some(_) => Err(ApiError::forbidden(
            "Only a member signed in to Tally can sign themselves in or out",
        )
        .into()),
        None => Err(ApiError::new(
            super::error::ErrorCode::Unauthenticated,
            "Must be authenticated",
        )
        .into()),
    }
}

/// The caller's member, read fresh: `None` when they have since been deleted.
pub(super) async fn linked_person<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
) -> Result<Option<db::Person>> {
    let person_id = require_member_id(ctx)?;
    Ok(app
        .db()
        .get_persons(&[&person_id])
        .await?
        .into_iter()
        .next()
        .flatten()
        .filter(|p| p.deleted.is_none()))
}

/// [`linked_person`] for a mutation, where "none" is an error rather than a null.
pub(super) async fn require_linked_person<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
) -> Result<db::Person> {
    linked_person(ctx, app)
        .await?
        .ok_or_else(|| ApiError::bad_request(NO_MEMBER).into())
}

/// The unit to sign `person` in at: `location_id` when given, which must be a unit the
/// member may use in Tally (any unit they belong to, or one the user signed in alongside
/// them holds either role at; see [`require_member_location`]), else their primary unit.
pub(super) fn sign_in_location_id(
    ctx: &Context<'_>,
    person: &db::Person,
    location_id: Option<&str>,
) -> Result<String> {
    match location_id {
        Some(id) => {
            require_member_location(ctx, id, "Only a member signed in to Tally can sign in")?;
            Ok(id.to_string())
        }
        None => Ok(person.location_id.clone()),
    }
}

/// The member's newest open period at `location_id`, as `scanRegister2` looks it up
/// at that unit's kiosk. A period open at any other unit is ignored.
pub(super) async fn open_period_at(
    db: &impl Handler,
    person: &db::Person,
    location_id: &str,
) -> Result<Option<db::Period>> {
    Ok(db
        .list_periods_for_person(
            &person.id,
            Some(location_id),
            Some(true),
            None,
            db::ListPeriodsPage {
                after: None,
                before: None,
                limit: 10,
                descending: true,
            },
        )
        .await?
        .into_iter()
        .find(|p| p.deleted.is_none() && p.end_time.is_none()))
}

/// The most open sign-ins a unit's who's-here list returns. The kiosk's list is far
/// shorter in practice; this only bounds a pathological unit.
const WHOS_HERE_LIMIT: i32 = 500;

/// One open sign-in at the unit, as the kiosk's Who's here list shows it. Deliberately
/// narrow: no guest reason, member number or other period fields.
#[derive(SimpleObject)]
pub struct WhosHereEntry {
    /// The period id.
    id: ID,
    /// "First Last" for a member, the guest's name for a guest ("Guest" if blank).
    name: String,
    guest: bool,
    /// Unix seconds.
    start_time: u64,
    /// This is the calling member's own period.
    is_me: bool,
}

/// Everyone signed in at `location_id`, oldest sign-in first, marking `person`'s own
/// periods. The location is one [`sign_in_location_id`] allowed.
async fn whos_here(
    db: &impl Handler,
    location_id: &str,
    person: &db::Person,
) -> Result<Vec<WhosHereEntry>> {
    let mut periods: Vec<db::Period> = db
        .list_periods_for_location(
            location_id,
            true,
            None,
            None,
            db::ListPeriodsPage {
                after: None,
                before: None,
                limit: WHOS_HERE_LIMIT,
                descending: false,
            },
        )
        .await?
        .into_iter()
        .filter(|p| p.deleted.is_none() && p.end_time.is_none())
        .collect();
    periods.sort_by(|a, b| {
        a.start_time
            .cmp(&b.start_time)
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut ids: Vec<&str> = periods
        .iter()
        .filter(|p| p.guest_name.is_none())
        .filter_map(|p| p.person_id.as_deref())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let names: HashMap<String, String> = if ids.is_empty() {
        HashMap::new()
    } else {
        db.get_persons(&ids)
            .await?
            .into_iter()
            .flatten()
            .filter(|p| p.deleted.is_none())
            .map(|p| (p.id, format!("{} {}", p.first_name, p.last_name)))
            .collect()
    };

    Ok(periods
        .into_iter()
        .map(|p| {
            let guest = p.guest_name.is_some();
            let name = match (&p.guest_name, &p.person_id) {
                (Some(g), _) if g.trim().is_empty() => "Guest".to_string(),
                (Some(g), _) => g.clone(),
                (None, Some(id)) => names
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| "Unknown member".to_string()),
                (None, None) => "Unknown member".to_string(),
            };
            WhosHereEntry {
                is_me: p.person_id.as_deref() == Some(person.id.as_str()),
                id: ID(p.id),
                name,
                guest,
                start_time: p.start_time,
            }
        })
        .collect())
}

/// A member's own sign-in state at one unit.
pub struct SelfSignInStatus<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    pub(super) person: db::Person,
    pub(super) location: db::Location,
    pub(super) open_period: Option<db::Period>,
}

/// A member's own sign-in state at one unit: their primary unit, or the one asked for.
#[Object]
impl<A: App + HasDb + Send + Sync + 'static> SelfSignInStatus<A> {
    /// The member.
    async fn person(&self) -> Person<A> {
        Person::new(self.person.clone())
    }

    /// The unit this status is for, where `selfSignIn` with the same `locationId`
    /// signs in: the member's primary unit unless another was asked for.
    async fn location(&self) -> Location<A> {
        Location::new_db(self.location.clone())
    }

    /// The member's newest open period at this unit; null when signed out here. A
    /// period open at another unit is not shown.
    async fn open_period(&self) -> Option<Period<A>> {
        self.open_period.clone().map(Period::new)
    }

    /// The same sign-out shortcuts a kiosk gets. Only computed when selected; null
    /// if they could not be built.
    async fn quick_pick(&self, ctx: &Context<'_>) -> Option<QuickPick<A>> {
        let app = ctx.data_unchecked::<Arc<A>>();
        match build_quick_pick(app.as_ref(), &self.location.id, &self.person.id).await {
            Ok(qp) => Some(qp),
            Err(e) => {
                warn!(
                    "failed to build quick pick for person {}: {e:?}",
                    self.person.id
                );
                None
            }
        }
    }

    /// Everyone currently signed in at this unit (open, not deleted periods, members
    /// and guests), oldest sign-in first. Computed only when selected.
    async fn whos_here(&self, ctx: &Context<'_>) -> Result<Vec<WhosHereEntry>> {
        let app = ctx.data_unchecked::<Arc<A>>();
        whos_here(app.db(), &self.location.id, &self.person).await
    }
}

/// Body of `Query.selfSignInStatus`.
pub(super) async fn status<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    location_id: Option<&str>,
) -> Result<Option<SelfSignInStatus<A>>> {
    let Some(person) = linked_person(ctx, app).await? else {
        return Ok(None);
    };
    let location_id = sign_in_location_id(ctx, &person, location_id)?;
    let location = app
        .db()
        .get_locations(&[&location_id])
        .await?
        .into_iter()
        .next()
        .flatten()
        .ok_or_else(|| -> anyhow::Error {
            if location_id == person.location_id {
                anyhow!("Location with ID {location_id} missing")
            } else {
                ApiError::not_found("Location", &location_id).into()
            }
        })?;
    let open_period = open_period_at(app.db(), &person, &location.id).await?;
    Ok(Some(SelfSignInStatus {
        _marker: Default::default(),
        person,
        location,
        open_period,
    }))
}
