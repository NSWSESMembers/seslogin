//! What Tally's apps share: who the signed-in member is ([`member_id`]), how members
//! are named on what they did ([`member_name`]), and the two entry points the apps
//! start from — `Query.tallyViewer` (the member and the units they can use) and
//! `Query.tallyLocation` (one unit's shared tools).
//!
//! Tally acts for members, never users: everything a member writes records their
//! person ID, and a caller who isn't an [`AuthInfo::Member`] is refused by every
//! Tally check (see [`super::auth::require_member_location`]). The core `Location`
//! type carries none of Tally's tools; [`TallyLocation`] does.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, ID, Object};
use std::sync::Arc;

use super::auth::require_member_location;
use super::calendar::{self, Calendar, CalendarEventOccurrence};
use super::dataloader::DatabaseLoader;
use super::equipment::{self, Equipment};
use super::error::ApiError;
use super::location_share::{self, LocationShare};
use super::map::{self, MapLayer, MapPin};
use super::notes::{self, Note, NoteFolder};
use super::on_call::{self, OnCallRole};
use super::query::Person;
use super::reservations::{self, EquipmentReservation};
use super::shared_contacts::{self, SharedContact};
use super::tasks::{self, Task, TaskList, TaskView};
use super::{LocationId, PersonId};
use crate::app::{App, HasDb};
use crate::auth::AuthInfo;
use crate::db::{self, Handler as _};

/// The person ID of the member signed in to Tally, if the caller is one.
pub(crate) fn member_id<'a>(ctx: &'a Context<'_>) -> Option<&'a str> {
    match ctx.data_opt::<AuthInfo>() {
        Some(AuthInfo::Member { person_id, .. }) => Some(person_id.as_str()),
        _ => None,
    }
}

/// [`member_id`], refusing anyone who isn't a member signed in to Tally.
pub(crate) fn require_member<'a>(ctx: &'a Context<'_>) -> Result<&'a str> {
    member_id(ctx).ok_or_else(|| ApiError::forbidden("Only available in Tally").into())
}

/// "First Last" for the member `id`, as Tally names who made or changed something.
/// `None` for no ID, or a member who is gone.
pub(crate) async fn member_name<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    id: Option<&str>,
) -> Result<Option<String>> {
    let Some(id) = id else { return Ok(None) };
    let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
    Ok(loader
        .load_one(PersonId(ID(id.to_string())))
        .await
        .map_err(|e| anyhow!("Failed to load person via DataLoader: {}", e))?
        .flatten()
        .filter(|p| p.rec.deleted.is_none())
        .map(|p| full_name(&p.rec))
        .filter(|name| !name.is_empty()))
}

pub(crate) fn full_name(person: &db::Person) -> String {
    format!("{} {}", person.first_name.trim(), person.last_name.trim())
        .trim()
        .to_string()
}

// ── tallyViewer ──────────────────────────────────────────────────────────────

/// The member signed in to Tally.
pub struct TallyViewer<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    person: db::Person,
    user_email: Option<String>,
    is_super: bool,
    is_dev: bool,
    user_location_ids: Vec<String>,
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> TallyViewer<A> {
    /// The member.
    async fn member(&self) -> Person<A> {
        Person::new(self.person.clone())
    }

    /// "First Last".
    async fn name(&self) -> String {
        full_name(&self.person)
    }

    /// The member's primary unit: the one member sync lists them at in SES.
    async fn primary_location_id(&self) -> ID {
        ID(self.person.location_id.clone())
    }

    /// The member's primary unit. Kept for Tally builds that predate
    /// `primaryLocationId`.
    #[graphql(deprecation = "Use `primaryLocationId`.")]
    async fn home_location_id(&self) -> ID {
        ID(self.person.location_id.clone())
    }

    /// Every unit the member belongs to: their primary unit first, then their secondary
    /// units. Unlike `locations`, this leaves out units reached only through the user
    /// signed in alongside them, so Tally can tell "my units" from those.
    async fn member_location_ids(&self) -> Vec<ID> {
        self.person
            .member_location_ids()
            .into_iter()
            .map(|id| ID(id.to_string()))
            .collect()
    }

    /// The email of the SESLogin user signed in alongside the member, when their email
    /// matched one; null otherwise.
    async fn user_email(&self) -> Option<&str> {
        self.user_email.as_deref()
    }

    /// Whether the user signed in alongside the member is a super user, and so can use
    /// every unit. False when no user signed in alongside them.
    async fn is_super(&self) -> bool {
        self.is_super
    }

    /// Whether the user signed in alongside the member is a developer (`User.isDev`), so
    /// Tally offers them its environment picker. False when no user signed in alongside them.
    async fn is_dev(&self) -> bool {
        self.is_dev
    }

    /// Every unit the member can use in Tally, by name: every unit they belong to (primary
    /// or secondary), plus the units the user signed in alongside them holds either role
    /// at (every enabled unit for a super user). Disabled units are left out, except a
    /// member's own primary unit.
    async fn locations(&self, ctx: &Context<'_>) -> Result<Vec<TallyLocation<A>>> {
        let app = ctx.data_unchecked::<Arc<A>>();
        let mut locations = if self.is_super {
            app.db()
                .list_locations(db::ListLocationsFilter::EnabledOnly)
                .await?
        } else {
            let mut ids: Vec<&str> = self.user_location_ids.iter().map(String::as_str).collect();
            ids.extend(self.person.member_location_ids());
            ids.sort_unstable();
            ids.dedup();
            app.db()
                .get_locations(&ids)
                .await?
                .into_iter()
                .flatten()
                .filter(|l| l.enabled || l.id == self.person.location_id)
                .collect()
        };
        if !locations.iter().any(|l| l.id == self.person.location_id)
            && let Some(primary) = app
                .db()
                .get_locations(&[&self.person.location_id])
                .await?
                .into_iter()
                .next()
                .flatten()
        {
            locations.push(primary);
        }
        locations.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
        Ok(locations.into_iter().map(TallyLocation::new).collect())
    }
}

/// Body of `Query.tallyViewer`.
pub(super) async fn viewer<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
) -> Result<TallyViewer<A>> {
    let Some(AuthInfo::Member {
        person_id,
        user_id,
        is_super,
        user_location_ids,
        ..
    }) = ctx.data_opt::<AuthInfo>()
    else {
        return Err(ApiError::forbidden("Only available in Tally").into());
    };
    let app = ctx.data_unchecked::<Arc<A>>();
    let person = app
        .db()
        .get_persons(&[person_id])
        .await?
        .into_iter()
        .next()
        .flatten()
        .ok_or_else(|| ApiError::not_found("Member", person_id))?;
    let user = match user_id {
        Some(id) => app
            .db()
            .get_users(&[id])
            .await?
            .into_iter()
            .next()
            .flatten(),
        None => None,
    };
    let is_dev = user.as_ref().is_some_and(|u| u.is_dev);
    let user_email = user.map(|u| u.email);
    Ok(TallyViewer {
        _marker: Default::default(),
        person,
        user_email,
        is_super: *is_super,
        is_dev,
        user_location_ids: user_location_ids.clone(),
    })
}

// ── tallyLocation ────────────────────────────────────────────────────────────

/// A unit as Tally sees it: its name and its shared tools. Every list field checks the
/// caller may use this unit (see [`require_member_location`]).
pub struct TallyLocation<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::Location,
}

impl<A: App + HasDb + Send + Sync + 'static> TallyLocation<A> {
    pub fn new(rec: db::Location) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> TallyLocation<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    async fn name(&self) -> &str {
        &self.rec.name
    }

    /// The SES zone this unit is in, itself a location: its `id` and `name` are for
    /// anyone who can see this unit, and its tools keep their own checks. Units only.
    async fn zone(&self, ctx: &Context<'_>) -> Result<Option<TallyLocation<A>>> {
        let Some(zone_id) = &self.rec.ses_details.zone_location_id else {
            return Ok(None);
        };
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        Ok(loader
            .load_one(LocationId(ID(zone_id.clone())))
            .await
            .map_err(|e| anyhow!("Failed to load location via DataLoader: {}", e))?
            .flatten()
            .map(|zone| TallyLocation::new(zone.rec)))
    }

    /// The unit's open tasks (its shared to-do list): by default those due now,
    /// overdue first. `listId` narrows to one of its task lists; without it every
    /// task at the unit is listed, whichever list it is on. See `graphql::tasks`.
    async fn tasks(
        &self,
        ctx: &Context<'_>,
        #[graphql(default)] view: TaskView,
        list_id: Option<ID>,
    ) -> Result<Vec<Task<A>>> {
        tasks::location_tasks(ctx, &self.rec.id, view, list_id).await
    }

    /// The unit's task lists, by name.
    async fn task_lists(&self, ctx: &Context<'_>) -> Result<Vec<TaskList<A>>> {
        tasks::location_task_lists(ctx, &self.rec.id).await
    }

    /// The unit's notes (its shared notebook), most recently changed first.
    /// `folderId` narrows to one of its note folders; without it every note at the
    /// unit is listed, whichever folder it is in. `search` keeps notes whose title
    /// contains it, ignoring case. See `graphql::notes`.
    async fn notes(
        &self,
        ctx: &Context<'_>,
        folder_id: Option<ID>,
        search: Option<String>,
    ) -> Result<Vec<Note<A>>> {
        notes::location_notes(ctx, &self.rec.id, folder_id, search).await
    }

    /// The unit's note folders, by name.
    async fn note_folders(&self, ctx: &Context<'_>) -> Result<Vec<NoteFolder<A>>> {
        notes::location_note_folders(ctx, &self.rec.id).await
    }

    /// The unit's equipment register, by name. `search` keeps equipment whose name or
    /// description contains it, ignoring case. See `graphql::equipment`.
    async fn equipment(
        &self,
        ctx: &Context<'_>,
        search: Option<String>,
    ) -> Result<Vec<Equipment<A>>> {
        equipment::location_equipment(ctx, &self.rec.id, search).await
    }

    /// The unit's equipment reservations that end after `endingAfter` (Unix seconds,
    /// default now) — those still to come or under way — by start time.
    /// `equipmentId` narrows to reservations of one piece of equipment. See
    /// `graphql::reservations`.
    async fn equipment_reservations(
        &self,
        ctx: &Context<'_>,
        ending_after: Option<i64>,
        equipment_id: Option<ID>,
    ) -> Result<Vec<EquipmentReservation<A>>> {
        reservations::location_reservations(ctx, &self.rec.id, ending_after, equipment_id).await
    }

    /// The unit's reservations a reservation of `equipmentIds` from `startTime` to
    /// `endTime` would overlap, by start time, leaving out `excludeId` (the one being
    /// edited). Overlaps are allowed; this is so a client can warn before saving.
    async fn equipment_reservation_conflicts(
        &self,
        ctx: &Context<'_>,
        equipment_ids: Vec<ID>,
        start_time: i64,
        end_time: i64,
        exclude_id: Option<ID>,
    ) -> Result<Vec<EquipmentReservation<A>>> {
        reservations::location_reservation_conflicts(
            ctx,
            &self.rec.id,
            equipment_ids,
            start_time,
            end_time,
            exclude_id,
        )
        .await
    }

    /// The pins on the unit's shared map, by title. `layerId` narrows to one of its map
    /// layers; without it every pin at the unit is listed, whichever layer it is on.
    /// See `graphql::map`.
    async fn map_pins(&self, ctx: &Context<'_>, layer_id: Option<ID>) -> Result<Vec<MapPin<A>>> {
        map::location_map_pins(ctx, &self.rec.id, layer_id).await
    }

    /// The unit's map layers, by name.
    async fn map_layers(&self, ctx: &Context<'_>) -> Result<Vec<MapLayer<A>>> {
        map::location_map_layers(ctx, &self.rec.id).await
    }

    /// The unit's calendars, by name. See `graphql::calendar`.
    async fn calendars(&self, ctx: &Context<'_>) -> Result<Vec<Calendar<A>>> {
        calendar::location_calendars(ctx, &self.rec.id).await
    }

    /// Every occurrence of the unit's events that overlaps `from`–`to` (Unix seconds,
    /// at most 400 days apart), repeats expanded, by start; at most 5,000. All-day
    /// occurrences are included when one of their dates overlaps the window in any
    /// time zone, so a client should check them against its own days. Events in every
    /// calendar, and in none, are listed: a client picking calendars filters on
    /// `event.calendarId`.
    async fn calendar_events(
        &self,
        ctx: &Context<'_>,
        from: i64,
        to: i64,
    ) -> Result<Vec<CalendarEventOccurrence<A>>> {
        calendar::location_calendar_events(ctx, &self.rec.id, from, to).await
    }

    /// Everyone sharing their position with the unit ("Locate"), most recently updated
    /// first. A share shows until 7 minutes after its latest position, so this
    /// includes shares that have just ended (`active: false`). See
    /// `graphql::location_share`.
    async fn location_shares(&self, ctx: &Context<'_>) -> Result<Vec<LocationShare<A>>> {
        location_share::location_shares(ctx, &self.rec.id).await
    }

    /// The unit's on-call roles (Duty Officer, …) and who holds each now, by name. See
    /// `graphql::on_call`.
    async fn on_call_roles(&self, ctx: &Context<'_>) -> Result<Vec<OnCallRole<A>>> {
        on_call::location_on_call_roles(ctx, &self.rec.id).await
    }

    /// The contacts added by hand to the unit (its Duty Officer phone, …), by name. The
    /// statewide ones are `Query.stateContacts`. See `graphql::shared_contacts`.
    async fn shared_contacts(&self, ctx: &Context<'_>) -> Result<Vec<SharedContact<A>>> {
        shared_contacts::location_shared_contacts(ctx, &self.rec.id).await
    }
}

/// Body of `Query.tallyLocation`: a unit the member may use, or an error.
pub(super) async fn location<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    id: &str,
) -> Result<TallyLocation<A>> {
    require_member_location(ctx, id, "Only available in Tally")?;
    let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
    let location = loader
        .load_one(LocationId(ID(id.to_string())))
        .await
        .map_err(|e| anyhow!("Failed to load location via DataLoader: {}", e))?
        .flatten()
        .ok_or_else(|| ApiError::not_found("Location", id))?;
    Ok(TallyLocation::new(location.rec))
}
