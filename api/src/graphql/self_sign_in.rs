//! Self sign-in: a signed-in user signs themselves in and out at their linked
//! member's home unit, from the Tally app, exactly as they would by typing their
//! member number at a kiosk there.
//!
//! The authority is the user's `person_id` link (set by a super user with
//! `setUserMember`), **not** a location grant: a member needs no role anywhere to
//! sign themselves in. So this deliberately uses neither `require_location_access`
//! nor `require_location_write_access`; [`linked_person`] is the whole check. Only
//! people qualify — kiosk sessions, API tokens and period links are refused — and
//! only the home unit's periods are ever considered, as at a kiosk, which scopes
//! by location. See `mutations.rs` for `selfSignIn` / `selfSignOut`.

use anyhow::{Result, anyhow};
use async_graphql::{Context, ID, Object, SimpleObject};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::warn;

use super::error::ApiError;
use super::query::{Location, Period, Person, QuickPick, build_quick_pick};
use crate::app::{App, HasDb};
use crate::auth::AuthInfo;
use crate::db::{self, Handler};

/// Why a mutation could not find the caller's member.
const NO_MEMBER: &str = "No member is linked to your account";

/// The id of the signed-in user, refusing every other kind of caller. A kiosk, an API
/// token and an edit link are FORBIDDEN (they are not a member acting for themselves).
pub(super) fn require_user_id(ctx: &Context<'_>) -> Result<String> {
    match ctx.data_opt::<AuthInfo>() {
        Some(AuthInfo::User { id, .. }) => Ok(id.clone()),
        Some(_) => {
            Err(ApiError::forbidden("Only a signed-in user can sign themselves in or out").into())
        }
        None => Err(ApiError::new(
            super::error::ErrorCode::Unauthenticated,
            "Must be authenticated",
        )
        .into()),
    }
}

/// The caller's linked member, read fresh from their user record: `None` when no
/// member is linked or the member has been deleted.
pub(super) async fn linked_person<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
) -> Result<Option<db::Person>> {
    let user_id = require_user_id(ctx)?;
    let Some(person_id) = app
        .db()
        .get_users(&[&user_id])
        .await?
        .into_iter()
        .next()
        .flatten()
        .and_then(|u| u.person_id)
    else {
        return Ok(None);
    };
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

/// The member's newest open period at their home unit, as `scanRegister2` looks it
/// up. A period open at any other unit is ignored.
pub(super) async fn open_home_period(
    db: &impl Handler,
    person: &db::Person,
) -> Result<Option<db::Period>> {
    Ok(db
        .list_periods_for_person(
            &person.id,
            Some(&person.location_id),
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
    /// This is the caller's linked member's own period.
    is_me: bool,
}

/// Everyone signed in at `person`'s home unit, oldest sign-in first. The location is
/// always the member's own, never caller-supplied.
async fn whos_here(db: &impl Handler, person: &db::Person) -> Result<Vec<WhosHereEntry>> {
    let mut periods: Vec<db::Period> = db
        .list_periods_for_location(
            &person.location_id,
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

/// A signed-in user's own sign-in state at their linked member's home unit.
pub struct SelfSignInStatus<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    pub(super) person: db::Person,
    pub(super) location: db::Location,
    pub(super) open_period: Option<db::Period>,
}

/// A signed-in user's own sign-in state at their linked member's home unit.
#[Object]
impl<A: App + HasDb + Send + Sync + 'static> SelfSignInStatus<A> {
    /// The linked member.
    async fn person(&self) -> Person<A> {
        Person::new(self.person.clone())
    }

    /// The member's home unit; sign-ins happen here.
    async fn location(&self) -> Location<A> {
        Location::new_db(self.location.clone())
    }

    /// The member's newest open period at the home unit; null when signed out. A
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

    /// Everyone currently signed in at the member's home unit (open, not deleted
    /// periods, members and guests), oldest sign-in first. Computed only when selected.
    async fn whos_here(&self, ctx: &Context<'_>) -> Result<Vec<WhosHereEntry>> {
        let app = ctx.data_unchecked::<Arc<A>>();
        whos_here(app.db(), &self.person).await
    }
}

/// Body of `Query.selfSignInStatus`.
pub(super) async fn status<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
) -> Result<Option<SelfSignInStatus<A>>> {
    let Some(person) = linked_person(ctx, app).await? else {
        return Ok(None);
    };
    let location = app
        .db()
        .get_locations(&[&person.location_id])
        .await?
        .into_iter()
        .next()
        .flatten()
        .ok_or_else(|| anyhow!("Location with ID {} missing", person.location_id))?;
    let open_period = open_home_period(app.db(), &person).await?;
    Ok(Some(SelfSignInStatus {
        _marker: Default::default(),
        person,
        location,
        open_period,
    }))
}
