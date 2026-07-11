#![allow(clippy::too_many_arguments)]
use super::pagination::{build_connection, pagination_args};
use anyhow::Result;
use anyhow::anyhow;
use async_graphql::Context;
use async_graphql::Enum;
use async_graphql::ID;
use async_graphql::Json;
use async_graphql::Object;
use async_graphql::SimpleObject;
use async_graphql::connection::{Connection, EmptyFields};
use async_graphql::dataloader::DataLoader;
use chrono_tz::Australia::Sydney;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::warn;
use xxhash_rust::xxh64::xxh64;

use crate::app::App;
use crate::app::HasDb;
use crate::auth;
use crate::auth::AuthInfo;
use crate::db;
use crate::db::Handler;
use crate::db::ListSessionsQuery;
use crate::ses_api;

use super::auth::{AuthGuard, AuthRequirement, require_location_access};
use super::dataloader::DatabaseLoader;
use super::{CategoryId, LocationId, NitcEventId, PersonId, SessionId, UserId};

#[derive(Debug, PartialEq)]
pub struct User<A: App + HasDb + Send + Sync> {
    _marker: std::marker::PhantomData<A>,
    pub(super) rec: db::User,
}

impl<A: App + HasDb + Send + Sync> User<A> {
    pub fn new(rec: db::User) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

impl<A: App + HasDb + Send + Sync> Clone for User<A> {
    fn clone(&self) -> Self {
        Self {
            _marker: Default::default(),
            rec: self.rec.clone(),
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> User<A> {
    async fn id(&self) -> ID {
        async_graphql::ID(self.rec.id.clone())
    }
    async fn email(&self) -> &str {
        &self.rec.email
    }
    /// defaults to false if missing
    async fn is_super(&self) -> bool {
        self.rec.is_super
    }
    async fn is_dev(&self) -> bool {
        self.rec.is_dev
    }
    async fn enabled(&self) -> bool {
        self.rec.enabled
    }

    async fn access_time(&self) -> Option<i64> {
        self.rec.access_time.map(|t| t as i64)
    }

    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }

    async fn updated_at(&self) -> i64 {
        self.rec.updated_at as i64
    }

    async fn email_summary_location_ids(&self) -> Vec<String> {
        self.rec
            .email_config
            .iter()
            .filter_map(|(loc_id, val)| {
                val.as_object()
                    .filter(|m| m.contains_key("daily"))
                    .map(|_| loc_id.clone())
            })
            .collect()
    }

    async fn location_grant_ids(&self) -> Vec<ID> {
        self.rec
            .location_grants
            .iter()
            .map(|id| ID(id.clone()))
            .collect()
    }

    async fn locations(&self, ctx: &Context<'_>) -> Result<Vec<Location<A>>> {
        if self.rec.is_super {
            // superusers have access to all locations, so fetch full list of locations
            let app = ctx.data_unchecked::<Arc<A>>();
            let items = app
                .db()
                .list_locations(crate::db::ListLocationsFilter::EnabledOnly)
                .await?;

            return Ok(items.into_iter().map(|rec| Location::new_db(rec)).collect());
        }

        // all other users we list only the units they have grants for
        let locations = &self.rec.location_grants;

        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        let loaded_locations = loader
            .load_many(
                locations
                    .iter()
                    .map(|s| LocationId(ID(s.clone())))
                    .collect::<Vec<LocationId>>(),
            )
            .await
            .map_err(|e| anyhow!("Failed to load locations via DataLoader: {}", e))?;
        locations
            .iter()
            .map(|s| {
                loaded_locations
                    .get(&LocationId(ID(s.clone())))
                    .cloned()
                    .flatten()
                    .ok_or_else(|| anyhow!("Location with ID {} missing", s))
            })
            .collect::<Result<Vec<Location<A>>>>()
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn passkeys(&self, ctx: &Context<'_>) -> Result<Vec<PasskeyInfo>> {
        let app = ctx.data_unchecked::<Arc<A>>();
        let creds = app
            .db()
            .list_webauthn_credentials_by_user(&self.rec.id)
            .await?;
        Ok(creds
            .into_iter()
            .map(|c| PasskeyInfo {
                id: c.id,
                name: c.name,
                created_at: c.created_at as i64,
                last_used_at: c.last_used_at.map(|t| t as i64),
            })
            .collect())
    }
}

/// Metadata for a stored passkey credential (never returns the private key material).
#[derive(SimpleObject, Clone, Debug)]
pub struct PasskeyInfo {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
}

/// A still-valid pending kiosk enrollment, surfaced to the admin SessionEnroll page so
/// it can confirm the kiosk is still waiting before showing the enrollment form.
#[derive(SimpleObject, Clone, Debug)]
pub struct PendingEnrollmentKey {
    pub fingerprint: String,
    pub expires_at: i64,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct DashboardDailyPeriodSummary {
    pub day_start: i64,
    pub period_count: i64,
    pub total_time: i64,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct DashboardCategoryPeriodSummary {
    pub category_id: Option<String>,
    pub category_name: String,
    pub period_count: i64,
    pub total_time: i64,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct LocationDashboardSummary {
    pub as_of: i64,
    pub total_members: i64,
    pub active_members_24h: i64,
    pub active_members_30d: i64,
    pub check_ins_24h: i64,
    pub check_ins_7d: i64,
    pub total_time_7d: i64,
    pub avg_completed_duration_7d: i64,
    pub total_kiosks: i64,
    pub online_kiosks: i64,
    pub recently_active_kiosks: i64,
    pub last_successful_member_sync: Option<i64>,
    pub daily_periods_7d: Vec<DashboardDailyPeriodSummary>,
    pub top_categories_7d: Vec<DashboardCategoryPeriodSummary>,
}

#[derive(Debug, PartialEq)]
pub struct Category<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    pub rec: db::Category,
}

impl<A: App + HasDb + Send + Sync> Category<A> {
    pub fn new(rec: db::Category) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

impl<A: App + HasDb + Send + Sync> Clone for Category<A> {
    fn clone(&self) -> Self {
        Self {
            _marker: Default::default(),
            rec: self.rec.clone(),
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> Category<A> {
    async fn id(&self) -> ID {
        async_graphql::ID(self.rec.id.clone())
    }
    async fn name(&self) -> &String {
        &self.rec.name
    }
    async fn enabled(&self) -> bool {
        self.rec.enabled
    }
    async fn nitc_group_id(&self) -> Option<&String> {
        self.rec.nitc_group_id.as_ref()
    }
    async fn nitc_participant_type(&self) -> Option<&String> {
        self.rec.nitc_participant_type.as_ref()
    }
    async fn nitc_group(&self, ctx: &Context<'_>) -> Result<Option<NitcGroup<A>>> {
        let Some(ref gid) = self.rec.nitc_group_id else {
            return Ok(None);
        };
        let app = ctx.data_unchecked::<Arc<A>>();
        Ok(app.db().get_nitc_group(gid).await?.map(NitcGroup::new))
    }

    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }

    async fn updated_at(&self) -> i64 {
        self.rec.updated_at as i64
    }
}

#[derive(Debug, PartialEq)]
pub struct Person<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    pub(super) rec: db::Person,
}

impl<A: App + HasDb + Send + Sync> Person<A> {
    pub fn new(rec: db::Person) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

impl<A: App + HasDb + Send + Sync> Clone for Person<A> {
    fn clone(&self) -> Self {
        Self {
            _marker: Default::default(),
            rec: self.rec.clone(),
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync> Person<A> {
    async fn id(&self) -> ID {
        async_graphql::ID(self.rec.id.clone())
    }

    /// this will be "" if missing
    async fn first_name(&self) -> &String {
        &self.rec.first_name
    }

    /// this will be "" if missing
    async fn last_name(&self) -> &String {
        &self.rec.last_name
    }

    /// this can be null
    async fn registration_number(&self) -> &Option<String> {
        &self.rec.registration_number
    }
    /// for backwards compatibility
    async fn member_number(&self) -> &Option<String> {
        &self.rec.registration_number
    }

    /// this can be null
    async fn ses_api_person_id(&self) -> &Option<String> {
        &self.rec.ses_api_person_id
    }

    /// this can be null
    async fn email(&self) -> &Option<String> {
        &self.rec.email
    }

    async fn deleted(&self) -> bool {
        self.rec.deleted.is_some()
    }

    async fn created_at(&self) -> Option<i64> {
        self.rec.created_at.map(|t| t as i64)
    }

    async fn updated_at(&self) -> Option<i64> {
        self.rec.updated_at.map(|t| t as i64)
    }

    async fn periods<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        after: Option<String>,
        before: Option<String>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> Result<Connection<String, Period<A>, EmptyFields, EmptyFields>> {
        let after_cursor = after.as_deref().map(decode_period_cursor).transpose()?;
        let before_cursor = before.as_deref().map(decode_period_cursor).transpose()?;
        let has_after = after_cursor.is_some();
        let has_before = before_cursor.is_some();
        let (page_size, is_last_mode) =
            pagination_args(first, last, DEFAULT_PERIOD_PAGE_SIZE, MAX_PERIOD_PAGE_SIZE)?;
        let fetch_limit = i32::try_from(page_size.saturating_add(1))
            .map_err(|_| anyhow!("Requested page is too large"))?;

        require_location_access(ctx, &self.rec.location_id)?;
        let app = ctx.data_unchecked::<Arc<A>>();
        let items = app
            .db()
            .list_periods_for_person(
                &self.rec.id,
                None,
                None,
                db::ListPeriodsPage {
                    after: after_cursor,
                    before: before_cursor,
                    limit: fetch_limit,
                    descending: !is_last_mode,
                },
            )
            .await
            .map_err(|e| {
                warn!("db error: {:?}", e);
                e
            })?;

        Ok(build_connection(
            items,
            page_size,
            is_last_mode,
            has_after,
            has_before,
            |p| (encode_period_cursor(p), Period::new(p.clone())),
        ))
    }

    /// The person's most recent period (by start time), or null if they have none.
    async fn last_period(&self, ctx: &Context<'_>) -> Result<Option<Period<A>>> {
        require_location_access(ctx, &self.rec.location_id)?;
        let app = ctx.data_unchecked::<Arc<A>>();
        let mut items = app
            .db()
            .list_periods_for_person(
                &self.rec.id,
                None,
                None,
                db::ListPeriodsPage {
                    after: None,
                    before: None,
                    limit: 1,
                    descending: true,
                },
            )
            .await
            .map_err(|e| {
                warn!("db error: {:?}", e);
                e
            })?;
        Ok(items.drain(..).next().map(Period::new))
    }
}

#[derive(Debug, PartialEq)]
pub struct Period<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    pub(super) rec: db::Period,
}

impl<A: App + HasDb + Send + Sync> Period<A> {
    pub(crate) fn new(rec: db::Period) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

/// Load a session by its optional id via the DataLoader. Returns `None` when no
/// session id was recorded on the period (e.g. older periods, or admin-created
/// or admin-edited periods that have no kiosk session). Deleted sessions are
/// soft-deleted and still resolve, so a recorded id normally hydrates fine; the
/// final `flatten()` is just defensive against a genuinely missing row.
async fn load_optional_session<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    session_id: &Option<String>,
) -> Result<Option<Session<A>>> {
    let Some(session_id) = session_id else {
        return Ok(None);
    };
    let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
    Ok(loader
        .load_one(SessionId(ID(session_id.clone())))
        .await
        .map_err(|e| anyhow!("Failed to load session via DataLoader: {}", e))?
        .flatten())
}

#[Object]
impl<A: App + HasDb + Send + Sync> Period<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    async fn person_id(&self) -> ID {
        ID(self.rec.person_id.clone())
    }

    async fn person(&self, ctx: &Context<'_>) -> Result<Person<A>> {
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        loader
            .load_one(PersonId(ID(self.rec.person_id.clone())))
            .await
            .map_err(|e| anyhow!("Failed to load person via DataLoader: {}", e))?
            .flatten()
            .ok_or_else(|| anyhow!("Person with ID {} missing", self.rec.person_id))
    }

    async fn location_id(&self) -> ID {
        ID(self.rec.location_id.clone())
    }

    async fn location(&self, ctx: &Context<'_>) -> Result<Location<A>> {
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        loader
            .load_one(LocationId(ID(self.rec.location_id.clone())))
            .await
            .map_err(|e| anyhow!("Failed to load location via DataLoader: {}", e))?
            .flatten()
            .ok_or_else(|| anyhow!("Location with ID {} missing", self.rec.location_id))
    }

    async fn category_id(&self) -> Option<ID> {
        self.rec.category_id.clone().map(ID)
    }

    async fn category(&self, ctx: &Context<'_>) -> Result<Option<Category<A>>> {
        Ok(match &self.rec.category_id {
            None => None,
            Some(category_id) => {
                let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
                Some(
                    loader
                        .load_one(CategoryId(ID(category_id.clone())))
                        .await
                        .map_err(|e| anyhow!("Failed to load category via DataLoader: {}", e))?
                        .flatten()
                        .ok_or_else(|| anyhow!("Category with ID {} missing", category_id))?,
                )
            }
        })
    }

    async fn start_time(&self) -> i64 {
        self.rec.start_time as i64
    }

    async fn end_time(&self) -> Option<i64> {
        self.rec.end_time.map(|i| i as i64)
    }

    async fn created_at(&self) -> Option<i64> {
        self.rec.created_at.map(|t| t as i64)
    }

    async fn updated_at(&self) -> Option<i64> {
        self.rec.updated_at.map(|t| t as i64)
    }

    async fn signed_in_session_id(&self) -> Option<ID> {
        self.rec.signed_in_session_id.clone().map(ID)
    }

    /// The kiosk session that signed this period in, if recorded. `None` for
    /// periods with no recorded sign-in session (older or admin-created periods).
    async fn signed_in_session(&self, ctx: &Context<'_>) -> Result<Option<Session<A>>> {
        load_optional_session(ctx, &self.rec.signed_in_session_id).await
    }

    async fn signed_out_session_id(&self) -> Option<ID> {
        self.rec.signed_out_session_id.clone().map(ID)
    }

    /// The kiosk session that signed this period out, if recorded. `None` for
    /// periods that are still open or were signed out/edited by an admin.
    async fn signed_out_session(&self, ctx: &Context<'_>) -> Result<Option<Session<A>>> {
        load_optional_session(ctx, &self.rec.signed_out_session_id).await
    }

    async fn nitc_event_id(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        let Some(ref event_id) = self.rec.nitc_event_id else {
            return Ok(None);
        };
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        let event = loader
            .load_one(NitcEventId(event_id.clone()))
            .await
            .map_err(|e| anyhow!("Failed to load NITC event via DataLoader: {}", e))?;
        Ok(event
            .and_then(|e| e.ses_api_nitc_id)
            .map(|id| id.to_string()))
    }

    async fn nitc_export_status(&self, ctx: &Context<'_>) -> Result<Option<NitcExportStatus>> {
        let Some(ref cat_id) = self.rec.category_id else {
            return Ok(None);
        };
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        let cat = loader
            .load_one(CategoryId(ID(cat_id.clone())))
            .await
            .map_err(|e| anyhow!("Failed to load category via DataLoader: {}", e))?
            .flatten();
        if cat.is_none_or(|c| c.rec.nitc_group_id.is_none()) {
            return Ok(None);
        }
        // Period must be exported and its nitc_event must also be fully synced.
        let synced = if let Some(ref event_id) = self.rec.nitc_event_id {
            if self.rec.nitc_exported_version == Some(self.rec.version) {
                let event = loader
                    .load_one(NitcEventId(event_id.clone()))
                    .await
                    .map_err(|e| anyhow!("Failed to load NITC event via DataLoader: {}", e))?;
                event.is_some_and(|e| e.synced_version == Some(e.version))
            } else {
                false
            }
        } else {
            false
        };
        if synced {
            Ok(Some(NitcExportStatus::Synced))
        } else {
            let location = loader
                .load_one(LocationId(ID(self.rec.location_id.clone())))
                .await
                .map_err(|e| anyhow!("Failed to load location via DataLoader: {}", e))?
                .flatten()
                .ok_or_else(|| anyhow!("Location with ID {} missing", self.rec.location_id))?;
            match location.rec.nitc_enabled {
                Some(cutover) if self.rec.start_time >= cutover => {
                    Ok(Some(NitcExportStatus::Pending))
                }
                _ => Ok(None),
            }
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum NitcExportStatus {
    Synced,
    Pending,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MemberPeriodSummary<A: App + HasDb + Send + Sync> {
    _marker: std::marker::PhantomData<A>,
    person_id: String,
    total_time: i64,
}

impl<A: App + HasDb + Send + Sync> MemberPeriodSummary<A> {
    fn new(person_id: String, total_time: i64) -> Self {
        Self {
            _marker: Default::default(),
            person_id,
            total_time,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> MemberPeriodSummary<A> {
    async fn person_id(&self) -> ID {
        ID(self.person_id.clone())
    }

    async fn person(&self, ctx: &Context<'_>) -> Result<Person<A>> {
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        loader
            .load_one(PersonId(ID(self.person_id.clone())))
            .await
            .map_err(|e| anyhow!("Failed to load person via DataLoader: {}", e))?
            .flatten()
            .ok_or_else(|| anyhow!("Person with ID {} missing", self.person_id))
    }

    async fn total_time(&self) -> i64 {
        self.total_time
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CategoryPeriodSummary<A: App + HasDb + Send + Sync> {
    _marker: std::marker::PhantomData<A>,
    category_id: String,
    total_time: i64,
}

impl<A: App + HasDb + Send + Sync> CategoryPeriodSummary<A> {
    fn new(category_id: String, total_time: i64) -> Self {
        Self {
            _marker: Default::default(),
            category_id,
            total_time,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> CategoryPeriodSummary<A> {
    async fn category_id(&self) -> ID {
        ID(self.category_id.clone())
    }

    async fn category(&self, ctx: &Context<'_>) -> Result<Category<A>> {
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        loader
            .load_one(CategoryId(ID(self.category_id.clone())))
            .await
            .map_err(|e| anyhow!("Failed to load category via DataLoader: {}", e))?
            .flatten()
            .ok_or_else(|| anyhow!("Category with ID {} missing", self.category_id))
    }

    async fn total_time(&self) -> i64 {
        self.total_time
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MemberCategoryPeriodSummary<A: App + HasDb + Send + Sync> {
    _marker: std::marker::PhantomData<A>,
    person_id: String,
    total_time: i64,
    categories: Vec<CategoryPeriodSummary<A>>,
}

impl<A: App + HasDb + Send + Sync> MemberCategoryPeriodSummary<A> {
    fn new(person_id: String, total_time: i64, categories: Vec<CategoryPeriodSummary<A>>) -> Self {
        Self {
            _marker: Default::default(),
            person_id,
            total_time,
            categories,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> MemberCategoryPeriodSummary<A> {
    async fn person_id(&self) -> ID {
        ID(self.person_id.clone())
    }

    async fn person(&self, ctx: &Context<'_>) -> Result<Person<A>> {
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        loader
            .load_one(PersonId(ID(self.person_id.clone())))
            .await
            .map_err(|e| anyhow!("Failed to load person via DataLoader: {}", e))?
            .flatten()
            .ok_or_else(|| anyhow!("Person with ID {} missing", self.person_id))
    }

    async fn total_time(&self) -> i64 {
        self.total_time
    }

    async fn categories(&self) -> &Vec<CategoryPeriodSummary<A>> {
        &self.categories
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CategoryMemberPeriodSummary<A: App + HasDb + Send + Sync> {
    _marker: std::marker::PhantomData<A>,
    category_id: String,
    total_time: i64,
    members: Vec<MemberPeriodSummary<A>>,
}

impl<A: App + HasDb + Send + Sync> CategoryMemberPeriodSummary<A> {
    fn new(category_id: String, total_time: i64, members: Vec<MemberPeriodSummary<A>>) -> Self {
        Self {
            _marker: Default::default(),
            category_id,
            total_time,
            members,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> CategoryMemberPeriodSummary<A> {
    async fn category_id(&self) -> ID {
        ID(self.category_id.clone())
    }

    async fn category(&self, ctx: &Context<'_>) -> Result<Category<A>> {
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        loader
            .load_one(CategoryId(ID(self.category_id.clone())))
            .await
            .map_err(|e| anyhow!("Failed to load category via DataLoader: {}", e))?
            .flatten()
            .ok_or_else(|| anyhow!("Category with ID {} missing", self.category_id))
    }

    async fn total_time(&self) -> i64 {
        self.total_time
    }

    async fn members(&self) -> &Vec<MemberPeriodSummary<A>> {
        &self.members
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DayCategoryPeriodSummary<A: App + HasDb + Send + Sync> {
    _marker: std::marker::PhantomData<A>,
    date: String,
    total_time: i64,
    categories: Vec<CategoryMemberPeriodSummary<A>>,
}

impl<A: App + HasDb + Send + Sync> DayCategoryPeriodSummary<A> {
    fn new(date: String, total_time: i64, categories: Vec<CategoryMemberPeriodSummary<A>>) -> Self {
        Self {
            _marker: Default::default(),
            date,
            total_time,
            categories,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> DayCategoryPeriodSummary<A> {
    async fn date(&self) -> &str {
        &self.date
    }

    async fn total_time(&self) -> i64 {
        self.total_time
    }

    async fn categories(&self) -> &Vec<CategoryMemberPeriodSummary<A>> {
        &self.categories
    }
}

fn period_duration(period: &db::Period) -> Option<u64> {
    period
        .end_time
        .and_then(|end_time| end_time.checked_sub(period.start_time))
}

fn unix_to_sydney_date(unix: u64) -> String {
    chrono::DateTime::from_timestamp(unix as i64, 0)
        .unwrap_or(chrono::DateTime::UNIX_EPOCH)
        .with_timezone(&Sydney)
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

const DEFAULT_PERIOD_PAGE_SIZE: usize = 100;
const MAX_PERIOD_PAGE_SIZE: usize = 1000;
const DAY_SECONDS: u64 = 24 * 60 * 60;
const THIRTY_DAYS_SECONDS: u64 = 30 * DAY_SECONDS;
const SEVEN_DAYS_SECONDS: u64 = 7 * DAY_SECONDS;
const ONLINE_SESSION_SECONDS: u64 = 15 * 60;

fn encode_period_cursor(period: &db::Period) -> String {
    format!("{}:{}", period.start_time, period.id)
}

fn decode_period_cursor(cursor: &str) -> Result<db::PeriodCursor> {
    let mut parts = cursor.splitn(2, ':');
    let start_time = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid cursor"))?
        .parse::<u64>()
        .map_err(|_| anyhow!("Invalid cursor"))?;
    let id = parts.next().ok_or_else(|| anyhow!("Invalid cursor"))?;
    if id.is_empty() {
        return Err(anyhow!("Invalid cursor"));
    }

    Ok(db::PeriodCursor {
        start_time,
        id: id.to_string(),
    })
}

#[derive(Debug, PartialEq)]
pub struct Location<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::Location,
}

impl<A: App + HasDb + Send + Sync> Location<A> {
    pub fn new_db(rec: db::Location) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

impl<A: App + HasDb + Send + Sync> Clone for Location<A> {
    fn clone(&self) -> Self {
        Location::new_db(self.rec.clone())
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync> Location<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    async fn name(&self) -> String {
        self.rec.name.clone()
    }

    async fn enabled(&self) -> bool {
        self.rec.enabled
    }

    async fn nitc_enabled(&self) -> Option<i64> {
        self.rec.nitc_enabled.map(|ts| ts as i64)
    }

    async fn ses_api_headquarters_id(&self) -> Option<String> {
        self.rec.ses_api_headquarters_id.clone()
    }

    async fn last_successful_member_sync(&self) -> Option<i64> {
        self.rec.last_successful_member_sync.map(|t| t as i64)
    }

    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }

    async fn updated_at(&self) -> i64 {
        self.rec.updated_at as i64
    }

    async fn people(&self, ctx: &Context<'_>) -> Result<Vec<Person<A>>> {
        let app = ctx.data_unchecked::<Arc<A>>();
        let items = app
            .db()
            .list_people_for_location(&self.rec.id, true)
            .await
            .map_err(|e| {
                warn!("db error: {:?}", e);
                e
            })?;

        Ok(items.into_iter().map(|p| Person::new(p)).collect())
    }

    async fn periods(
        &self,
        ctx: &Context<'_>,
        only_active: Option<bool>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        after: Option<String>,
        before: Option<String>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> Result<Connection<String, Period<A>, EmptyFields, EmptyFields>> {
        let after_cursor = after.as_deref().map(decode_period_cursor).transpose()?;
        let before_cursor = before.as_deref().map(decode_period_cursor).transpose()?;
        let has_after = after_cursor.is_some();
        let has_before = before_cursor.is_some();
        let (page_size, is_last_mode) =
            pagination_args(first, last, DEFAULT_PERIOD_PAGE_SIZE, MAX_PERIOD_PAGE_SIZE)?;
        let fetch_limit = i32::try_from(page_size.saturating_add(1))
            .map_err(|_| anyhow!("Requested page is too large"))?;

        let range = match (start_time, end_time) {
            (None, None) => None,
            (Some(_), None) | (None, Some(_)) => {
                return Err(anyhow!("start_time and end_time must both be provided"));
            }
            (Some(start_time), Some(end_time)) => {
                if start_time >= end_time {
                    return Err(anyhow!("start_time must be before end_time"));
                }
                let range_start = u64::try_from(start_time)
                    .map_err(|_| anyhow!("start_time must be a non-negative unix timestamp"))?;
                let range_end = u64::try_from(end_time)
                    .map_err(|_| anyhow!("end_time must be a non-negative unix timestamp"))?;
                Some((range_start, range_end))
            }
        };

        require_location_access(ctx, &self.rec.id)?;
        let app = ctx.data_unchecked::<Arc<A>>();
        let items = app
            .db()
            .list_periods_for_location(
                &self.rec.id,
                only_active.unwrap_or(false),
                range,
                db::ListPeriodsPage {
                    after: after_cursor,
                    before: before_cursor,
                    limit: fetch_limit,
                    descending: !is_last_mode,
                },
            )
            .await
            .map_err(|e| {
                warn!("db error: {:?}", e);
                e
            })?;

        Ok(build_connection(
            items,
            page_size,
            is_last_mode,
            has_after,
            has_before,
            |p| (encode_period_cursor(p), Period::new(p.clone())),
        ))
    }

    async fn period_summary_by_member(
        &self,
        ctx: &Context<'_>,
        start_time: i64,
        end_time: i64,
        category: Option<ID>,
    ) -> Result<Vec<MemberPeriodSummary<A>>> {
        if start_time >= end_time {
            return Err(anyhow!("start_time must be before end_time"));
        }
        let range_start = u64::try_from(start_time)
            .map_err(|_| anyhow!("start_time must be a non-negative unix timestamp"))?;
        let range_end = u64::try_from(end_time)
            .map_err(|_| anyhow!("end_time must be a non-negative unix timestamp"))?;
        let category_filter = category.map(|c| c.0);

        let app = ctx.data_unchecked::<Arc<A>>();
        let periods = app
            .db()
            .list_periods_for_location(
                &self.rec.id,
                false,
                Some((range_start, range_end)),
                db::ListPeriodsPage {
                    after: None,
                    before: None,
                    limit: i32::MAX,
                    descending: true,
                },
            )
            .await
            .map_err(|e| {
                warn!("db error: {:?}", e);
                e
            })?;

        let mut totals_by_member: HashMap<String, u64> = HashMap::new();
        for period in periods {
            if let Some(ref wanted) = category_filter
                && period.category_id.as_deref() != Some(wanted.as_str())
            {
                continue;
            }
            if let Some(duration) = period_duration(&period) {
                *totals_by_member.entry(period.person_id).or_insert(0) += duration;
            }
        }

        let mut rows: Vec<MemberPeriodSummary<A>> = totals_by_member
            .into_iter()
            .map(|(person_id, total_time)| MemberPeriodSummary::new(person_id, total_time as i64))
            .collect();
        rows.sort_by_key(|b| std::cmp::Reverse(b.total_time));

        Ok(rows)
    }

    async fn period_summary_by_category(
        &self,
        ctx: &Context<'_>,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<CategoryPeriodSummary<A>>> {
        if start_time >= end_time {
            return Err(anyhow!("start_time must be before end_time"));
        }
        let range_start = u64::try_from(start_time)
            .map_err(|_| anyhow!("start_time must be a non-negative unix timestamp"))?;
        let range_end = u64::try_from(end_time)
            .map_err(|_| anyhow!("end_time must be a non-negative unix timestamp"))?;

        let app = ctx.data_unchecked::<Arc<A>>();
        let periods = app
            .db()
            .list_periods_for_location(
                &self.rec.id,
                false,
                Some((range_start, range_end)),
                db::ListPeriodsPage {
                    after: None,
                    before: None,
                    limit: i32::MAX,
                    descending: true,
                },
            )
            .await
            .map_err(|e| {
                warn!("db error: {:?}", e);
                e
            })?;

        let mut totals_by_category: HashMap<String, u64> = HashMap::new();
        for period in periods {
            if let (Some(category_id), Some(duration)) =
                (period.category_id.clone(), period_duration(&period))
            {
                *totals_by_category.entry(category_id).or_insert(0) += duration;
            }
        }

        let mut rows: Vec<CategoryPeriodSummary<A>> = totals_by_category
            .into_iter()
            .map(|(category_id, total_time)| {
                CategoryPeriodSummary::new(category_id, total_time as i64)
            })
            .collect();
        rows.sort_by_key(|b| std::cmp::Reverse(b.total_time));

        Ok(rows)
    }

    async fn period_summary_by_member_by_category(
        &self,
        ctx: &Context<'_>,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<MemberCategoryPeriodSummary<A>>> {
        if start_time >= end_time {
            return Err(anyhow!("start_time must be before end_time"));
        }
        let range_start = u64::try_from(start_time)
            .map_err(|_| anyhow!("start_time must be a non-negative unix timestamp"))?;
        let range_end = u64::try_from(end_time)
            .map_err(|_| anyhow!("end_time must be a non-negative unix timestamp"))?;

        let app = ctx.data_unchecked::<Arc<A>>();
        let periods = app
            .db()
            .list_periods_for_location(
                &self.rec.id,
                false,
                Some((range_start, range_end)),
                db::ListPeriodsPage {
                    after: None,
                    before: None,
                    limit: i32::MAX,
                    descending: true,
                },
            )
            .await
            .map_err(|e| {
                warn!("db error: {:?}", e);
                e
            })?;

        let mut totals_by_member: HashMap<String, HashMap<String, u64>> = HashMap::new();
        for period in periods {
            if let (Some(category_id), Some(duration)) =
                (period.category_id.clone(), period_duration(&period))
            {
                *totals_by_member
                    .entry(period.person_id)
                    .or_default()
                    .entry(category_id)
                    .or_insert(0) += duration;
            }
        }

        let mut rows = totals_by_member
            .into_iter()
            .map(|(person_id, totals_by_category)| {
                let mut categories: Vec<CategoryPeriodSummary<A>> = totals_by_category
                    .into_iter()
                    .map(|(category_id, total_time)| {
                        CategoryPeriodSummary::new(category_id, total_time as i64)
                    })
                    .collect();
                categories.sort_by_key(|b| std::cmp::Reverse(b.total_time));
                let total_time = categories.iter().map(|c| c.total_time).sum::<i64>();

                MemberCategoryPeriodSummary::new(person_id, total_time, categories)
            })
            .collect::<Vec<MemberCategoryPeriodSummary<A>>>();
        rows.sort_by_key(|b| std::cmp::Reverse(b.total_time));

        Ok(rows)
    }

    async fn period_summary_by_category_by_member(
        &self,
        ctx: &Context<'_>,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<CategoryMemberPeriodSummary<A>>> {
        if start_time >= end_time {
            return Err(anyhow!("start_time must be before end_time"));
        }
        let range_start = u64::try_from(start_time)
            .map_err(|_| anyhow!("start_time must be a non-negative unix timestamp"))?;
        let range_end = u64::try_from(end_time)
            .map_err(|_| anyhow!("end_time must be a non-negative unix timestamp"))?;

        let app = ctx.data_unchecked::<Arc<A>>();
        let periods = app
            .db()
            .list_periods_for_location(
                &self.rec.id,
                false,
                Some((range_start, range_end)),
                db::ListPeriodsPage {
                    after: None,
                    before: None,
                    limit: i32::MAX,
                    descending: true,
                },
            )
            .await
            .map_err(|e| {
                warn!("db error: {:?}", e);
                e
            })?;

        let mut totals_by_category: HashMap<String, HashMap<String, u64>> = HashMap::new();
        for period in periods {
            if let (Some(category_id), Some(duration)) =
                (period.category_id.clone(), period_duration(&period))
            {
                *totals_by_category
                    .entry(category_id)
                    .or_default()
                    .entry(period.person_id)
                    .or_insert(0) += duration;
            }
        }

        let mut rows = totals_by_category
            .into_iter()
            .map(|(category_id, totals_by_member)| {
                let mut members: Vec<MemberPeriodSummary<A>> = totals_by_member
                    .into_iter()
                    .map(|(person_id, total_time)| {
                        MemberPeriodSummary::new(person_id, total_time as i64)
                    })
                    .collect();
                members.sort_by_key(|b| std::cmp::Reverse(b.total_time));
                let total_time = members.iter().map(|m| m.total_time).sum::<i64>();

                CategoryMemberPeriodSummary::new(category_id, total_time, members)
            })
            .collect::<Vec<CategoryMemberPeriodSummary<A>>>();
        rows.sort_by_key(|b| std::cmp::Reverse(b.total_time));

        Ok(rows)
    }

    async fn period_summary_by_day_by_category_by_member(
        &self,
        ctx: &Context<'_>,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<DayCategoryPeriodSummary<A>>> {
        if start_time >= end_time {
            return Err(anyhow!("start_time must be before end_time"));
        }
        let range_start = u64::try_from(start_time)
            .map_err(|_| anyhow!("start_time must be a non-negative unix timestamp"))?;
        let range_end = u64::try_from(end_time)
            .map_err(|_| anyhow!("end_time must be a non-negative unix timestamp"))?;

        let app = ctx.data_unchecked::<Arc<A>>();
        let periods = app
            .db()
            .list_periods_for_location(
                &self.rec.id,
                false,
                Some((range_start, range_end)),
                db::ListPeriodsPage {
                    after: None,
                    before: None,
                    limit: i32::MAX,
                    descending: true,
                },
            )
            .await
            .map_err(|e| {
                warn!("db error: {:?}", e);
                e
            })?;

        let mut totals_by_day: HashMap<String, HashMap<String, HashMap<String, u64>>> =
            HashMap::new();
        for period in periods {
            if let (Some(category_id), Some(duration)) =
                (period.category_id.clone(), period_duration(&period))
            {
                let date = unix_to_sydney_date(period.start_time);
                *totals_by_day
                    .entry(date)
                    .or_default()
                    .entry(category_id)
                    .or_default()
                    .entry(period.person_id)
                    .or_insert(0) += duration;
            }
        }

        let mut rows = totals_by_day
            .into_iter()
            .map(|(date, totals_by_category)| {
                let mut categories: Vec<CategoryMemberPeriodSummary<A>> = totals_by_category
                    .into_iter()
                    .map(|(category_id, totals_by_member)| {
                        let mut members: Vec<MemberPeriodSummary<A>> = totals_by_member
                            .into_iter()
                            .map(|(person_id, total_time)| {
                                MemberPeriodSummary::new(person_id, total_time as i64)
                            })
                            .collect();
                        members.sort_by_key(|m| std::cmp::Reverse(m.total_time));
                        let total_time = members.iter().map(|m| m.total_time).sum::<i64>();

                        CategoryMemberPeriodSummary::new(category_id, total_time, members)
                    })
                    .collect();
                categories.sort_by_key(|c| std::cmp::Reverse(c.total_time));
                let total_time = categories.iter().map(|c| c.total_time).sum::<i64>();

                DayCategoryPeriodSummary::new(date, total_time, categories)
            })
            .collect::<Vec<DayCategoryPeriodSummary<A>>>();
        rows.sort_by(|a, b| b.date.cmp(&a.date));

        Ok(rows)
    }

    async fn sessions(&self, ctx: &Context<'_>) -> Result<Vec<Session<A>>> {
        require_location_access(ctx, &self.rec.id)?;
        let app = ctx.data_unchecked::<Arc<A>>();
        let items = app
            .db()
            .list_sessions(ListSessionsQuery::ByLocation(self.rec.id.to_string()))
            .await
            .map_err(|e| {
                warn!("db error: {:?}", e);
                e
            })?;

        Ok(items.into_iter().map(Session::new).collect())
    }

    async fn dashboard_summary(
        &self,
        ctx: &Context<'_>,
        as_of: Option<i64>,
    ) -> Result<LocationDashboardSummary> {
        require_location_access(ctx, &self.rec.id)?;

        let as_of_ts = match as_of {
            Some(ts) if ts > 0 => {
                u64::try_from(ts).map_err(|_| anyhow!("as_of must be positive"))?
            }
            Some(_) => return Err(anyhow!("as_of must be positive")),
            None => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| anyhow!("system clock is before unix epoch"))?
                .as_secs(),
        };

        let day_floor = as_of_ts / DAY_SECONDS;
        let day_start_floor = day_floor.saturating_sub(6);
        let range_30d_start = as_of_ts.saturating_sub(THIRTY_DAYS_SECONDS);
        let range_24h_start = as_of_ts.saturating_sub(DAY_SECONDS);
        let range_7d_start = as_of_ts.saturating_sub(SEVEN_DAYS_SECONDS);

        let app = ctx.data_unchecked::<Arc<A>>();

        let members = app
            .db()
            .list_people_for_location(&self.rec.id, true)
            .await
            .map_err(|e| {
                warn!("db error: {:?}", e);
                e
            })?;

        let categories = app.db().list_categories().await.map_err(|e| {
            warn!("db error: {:?}", e);
            e
        })?;
        let category_name_by_id: HashMap<String, String> = categories
            .into_iter()
            .map(|cat| (cat.id, cat.name))
            .collect();

        let periods_30d = app
            .db()
            .list_periods_for_location(
                &self.rec.id,
                false,
                Some((range_30d_start, as_of_ts)),
                db::ListPeriodsPage {
                    after: None,
                    before: None,
                    limit: i32::MAX,
                    descending: true,
                },
            )
            .await
            .map_err(|e| {
                warn!("db error: {:?}", e);
                e
            })?;

        let sessions = app
            .db()
            .list_sessions(ListSessionsQuery::ByLocation(self.rec.id.to_string()))
            .await
            .map_err(|e| {
                warn!("db error: {:?}", e);
                e
            })?;

        let mut active_members_30d: HashSet<String> = HashSet::new();
        let mut active_members_24h: HashSet<String> = HashSet::new();
        let mut check_ins_24h: i64 = 0;
        let mut check_ins_7d: i64 = 0;
        let mut total_time_7d: i64 = 0;
        let mut completed_total_time_7d: i64 = 0;
        let mut completed_count_7d: i64 = 0;
        let mut daily_counts: HashMap<u64, (i64, i64)> = HashMap::new();
        let mut category_totals_7d: HashMap<Option<String>, (i64, i64)> = HashMap::new();

        for period in periods_30d {
            active_members_30d.insert(period.person_id.clone());

            if period.start_time >= range_24h_start {
                active_members_24h.insert(period.person_id.clone());
                check_ins_24h += 1;
            }

            if period.start_time >= range_7d_start {
                check_ins_7d += 1;

                let day = period.start_time / DAY_SECONDS;
                if day >= day_start_floor && day <= day_floor {
                    let bucket = daily_counts.entry(day).or_insert((0, 0));
                    bucket.0 += 1;

                    let bounded_end = std::cmp::min(period.end_time.unwrap_or(as_of_ts), as_of_ts);
                    let duration = bounded_end.saturating_sub(period.start_time);
                    let duration_i64 =
                        i64::try_from(duration).map_err(|_| anyhow!("period duration overflow"))?;
                    bucket.1 += duration_i64;
                    total_time_7d += duration_i64;

                    let category_bucket = category_totals_7d
                        .entry(period.category_id.clone())
                        .or_insert((0, 0));
                    category_bucket.0 += 1;
                    category_bucket.1 += duration_i64;

                    if let Some(end_time) = period.end_time
                        && end_time >= period.start_time
                    {
                        let completed_duration = end_time - period.start_time;
                        let completed_duration_i64 = i64::try_from(completed_duration)
                            .map_err(|_| anyhow!("period duration overflow"))?;
                        completed_total_time_7d += completed_duration_i64;
                        completed_count_7d += 1;
                    }
                }
            }
        }

        let avg_completed_duration_7d = if completed_count_7d > 0 {
            completed_total_time_7d / completed_count_7d
        } else {
            0
        };

        let total_kiosks = i64::try_from(sessions.len()).map_err(|_| anyhow!("too many kiosks"))?;
        let online_kiosks = i64::try_from(
            sessions
                .iter()
                .filter(|session| {
                    session
                        .last_contact
                        .is_some_and(|last| as_of_ts.saturating_sub(last) <= ONLINE_SESSION_SECONDS)
                })
                .count(),
        )
        .map_err(|_| anyhow!("too many kiosks"))?;
        let recently_active_kiosks = i64::try_from(
            sessions
                .iter()
                .filter(|session| {
                    session
                        .last_contact
                        .is_some_and(|last| as_of_ts.saturating_sub(last) <= DAY_SECONDS)
                })
                .count(),
        )
        .map_err(|_| anyhow!("too many kiosks"))?;

        let mut daily_periods_7d: Vec<DashboardDailyPeriodSummary> = Vec::new();
        for day in day_start_floor..=day_floor {
            let day_start = day.saturating_mul(DAY_SECONDS);
            let (period_count, total_time) = daily_counts.get(&day).copied().unwrap_or((0, 0));
            daily_periods_7d.push(DashboardDailyPeriodSummary {
                day_start: i64::try_from(day_start).map_err(|_| anyhow!("timestamp overflow"))?,
                period_count,
                total_time,
            });
        }

        let mut top_categories_7d: Vec<DashboardCategoryPeriodSummary> = category_totals_7d
            .into_iter()
            .map(|(category_id, (period_count, total_time))| {
                let category_name = match category_id.as_ref() {
                    Some(id) => category_name_by_id
                        .get(id)
                        .cloned()
                        .unwrap_or_else(|| "Unknown category".to_string()),
                    None => "Uncategorised".to_string(),
                };

                DashboardCategoryPeriodSummary {
                    category_id,
                    category_name,
                    period_count,
                    total_time,
                }
            })
            .collect();
        top_categories_7d.sort_by(|a, b| {
            b.period_count
                .cmp(&a.period_count)
                .then_with(|| b.total_time.cmp(&a.total_time))
        });
        top_categories_7d.truncate(5);

        Ok(LocationDashboardSummary {
            as_of: i64::try_from(as_of_ts).map_err(|_| anyhow!("timestamp overflow"))?,
            total_members: i64::try_from(members.len()).map_err(|_| anyhow!("too many members"))?,
            active_members_24h: i64::try_from(active_members_24h.len())
                .map_err(|_| anyhow!("too many members"))?,
            active_members_30d: i64::try_from(active_members_30d.len())
                .map_err(|_| anyhow!("too many members"))?,
            check_ins_24h,
            check_ins_7d,
            total_time_7d,
            avg_completed_duration_7d,
            total_kiosks,
            online_kiosks,
            recently_active_kiosks,
            last_successful_member_sync: self.rec.last_successful_member_sync.map(|t| t as i64),
            daily_periods_7d,
            top_categories_7d,
        })
    }
}

#[derive(Debug, PartialEq)]
pub struct Session<A: App + HasDb + Send + Sync> {
    _marker: std::marker::PhantomData<A>,
    pub(super) rec: db::Session,
}

impl<A: App + HasDb + Send + Sync> Session<A> {
    pub(crate) fn new(rec: db::Session) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

impl<A: App + HasDb + Send + Sync> Clone for Session<A> {
    fn clone(&self) -> Self {
        Self {
            _marker: Default::default(),
            rec: self.rec.clone(),
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> Session<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    async fn last_contact(&self) -> Option<i64> {
        self.rec.last_contact.map(|ts| ts as i64)
    }

    async fn client_version(&self) -> Option<&str> {
        self.rec.client_version.as_deref()
    }

    async fn name(&self) -> &String {
        &self.rec.name
    }

    async fn location_id(&self) -> ID {
        ID(self.rec.location_id.clone())
    }

    async fn location(&self, ctx: &Context<'_>) -> Result<Location<A>> {
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        loader
            .load_one(LocationId(ID(self.rec.location_id.clone())))
            .await
            .map_err(|e| anyhow!("Failed to load location via DataLoader: {}", e))?
            .flatten()
            .ok_or_else(|| anyhow!("Location with ID {} missing", self.rec.location_id))
    }

    async fn code(&self) -> &Option<String> {
        // TODO: careful who we show this to
        &self.rec.code
    }

    async fn healthcheck_url(&self) -> Option<&str> {
        self.rec.healthcheck_url.as_deref()
    }

    async fn config(&self) -> Json<serde_json::Map<String, serde_json::Value>> {
        Json(self.rec.config.clone())
    }

    async fn created_at(&self) -> Option<i64> {
        self.rec.created_at.map(|t| t as i64)
    }

    async fn updated_at(&self) -> Option<i64> {
        self.rec.updated_at.map(|t| t as i64)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApiToken {
    pub(super) rec: db::ApiToken,
}

impl ApiToken {
    pub(crate) fn new(rec: db::ApiToken) -> Self {
        Self { rec }
    }
}

#[Object]
impl ApiToken {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }
    async fn name(&self) -> &str {
        &self.rec.name
    }
    async fn location_grants(&self) -> &Vec<String> {
        &self.rec.location_grants
    }
    async fn read_only(&self) -> bool {
        self.rec.read_only
    }
    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }
    async fn created_by_user_id(&self) -> ID {
        ID(self.rec.created_by_user_id.clone())
    }
    async fn expires_at(&self) -> Option<i64> {
        self.rec.expires_at.map(|t| t as i64)
    }
    async fn revoked_at(&self) -> Option<i64> {
        self.rec.revoked_at.map(|t| t as i64)
    }
    async fn last_used_at(&self) -> Option<i64> {
        self.rec.last_used_at.map(|t| t as i64)
    }
}

#[derive(Clone, Debug)]
struct SesHeadquarters {
    rec: ses_api::SesHeadquarters,
}

impl SesHeadquarters {
    fn new(rec: ses_api::SesHeadquarters) -> Self {
        Self { rec }
    }

    fn opaque_id_for_ses_id(ses_id: i64) -> ID {
        let input = format!("SES Headquarters {}", ses_id);
        let hash = xxh64(input.as_bytes(), 0);
        ID(format!("{:016x}", hash))
    }
}

#[Object]
impl SesHeadquarters {
    async fn id(&self) -> ID {
        let ses_id = self.rec.id.expect("sesId should always be present");
        Self::opaque_id_for_ses_id(ses_id)
    }

    async fn ses_id(&self) -> ID {
        ID(self
            .rec
            .id
            .expect("sesId should always be present")
            .to_string())
    }

    async fn name(&self) -> &Option<String> {
        &self.rec.name
    }

    async fn code(&self) -> &Option<String> {
        &self.rec.code
    }

    async fn latitude(&self) -> Option<f64> {
        self.rec.latitude
    }

    async fn longitude(&self) -> Option<f64> {
        self.rec.longitude
    }

    #[graphql(name = "type")]
    async fn type_(&self) -> &Option<String> {
        &self.rec.headquarters_type
    }

    async fn status(&self) -> &Option<String> {
        &self.rec.status
    }

    async fn zone(&self) -> Option<SesHeadquarters> {
        self.rec
            .zone
            .as_deref()
            .cloned()
            .and_then(|zone| zone.id.map(|_| SesHeadquarters::new(zone)))
    }
}

#[derive(Clone, Debug)]
struct SesNonIncidentTag {
    id: i32,
    name: String,
    primary_activity_name: String,
}

#[Object]
impl SesNonIncidentTag {
    async fn id(&self) -> String {
        self.id.to_string()
    }

    async fn name(&self) -> &str {
        &self.name
    }

    async fn primary_activity_name(&self) -> &str {
        &self.primary_activity_name
    }
}

pub struct NitcGroup<A: App + HasDb + Send + Sync> {
    _marker: std::marker::PhantomData<A>,
    pub rec: db::NitcGroup,
}

impl<A: App + HasDb + Send + Sync> NitcGroup<A> {
    pub fn new(rec: db::NitcGroup) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> NitcGroup<A> {
    async fn id(&self) -> &str {
        &self.rec.id
    }

    async fn nitc_type(&self) -> &str {
        &self.rec.nitc_type
    }

    async fn ses_tags(&self, ctx: &Context<'_>) -> Result<Vec<SesNonIncidentTag>> {
        let app = ctx.data_unchecked::<Arc<A>>();
        let tag_ids: std::collections::HashSet<i32> =
            self.rec.nitc_tag_ids.iter().copied().collect();
        let all_tags = app.db().list_nitc_tags().await?;
        let mut tags: Vec<SesNonIncidentTag> = all_tags
            .into_iter()
            .filter(|t| tag_ids.contains(&t.id))
            .map(|t| SesNonIncidentTag {
                id: t.id,
                name: t.name,
                primary_activity_name: t.primary_activity_name,
            })
            .collect();
        tags.sort_by_key(|t| t.id);
        Ok(tags)
    }

    async fn created_at(&self) -> Option<i64> {
        self.rec.created_at.map(|t| t as i64)
    }

    async fn updated_at(&self) -> Option<i64> {
        self.rec.updated_at.map(|t| t as i64)
    }
}

/// Lightweight result of looking a person up by registration number. Carries only
/// the values the GSI query already returns, so no additional record fetch is needed.
#[derive(SimpleObject)]
pub struct PersonRef {
    /// The matched person's ID.
    id: ID,
    /// The registration/member number that was looked up.
    member_number: String,
}

pub struct QueryRoot<A: App + HasDb + Send + Sync> {
    _marker: std::marker::PhantomData<A>,
}

impl<A: App + HasDb + Send + Sync> QueryRoot<A> {
    pub fn new() -> Self {
        Self {
            _marker: Default::default(),
        }
    }
}

impl<A: App + HasDb + Send + Sync> Default for QueryRoot<A> {
    fn default() -> Self {
        Self::new()
    }
}

fn make_ses_client() -> Result<ses_api::SesClient> {
    let base_url =
        std::env::var("SES_API_BASE_URL").map_err(|_| anyhow!("SES_API_BASE_URL is required"))?;
    let api_key = std::env::var("SES_API_KEY").map_err(|_| anyhow!("SES_API_KEY is required"))?;
    let page_limit = std::env::var("SES_PAGE_LIMIT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(500);
    let max_retries = std::env::var("SES_SYNC_MAX_RETRIES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(3);
    ses_api::SesClient::new(base_url, api_key, page_limit, max_retries)
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> QueryRoot<A> {
    #[graphql(guard = "AuthGuard::new(AuthRequirement::Authenticated)")]
    async fn user(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "ID for the user to look up")] id: Option<ID>,
    ) -> Result<User<A>> {
        let auth = ctx.data_opt::<AuthInfo>();
        let user_id = match id {
            Some(id) => id.to_string(),
            None => match auth {
                Some(AuthInfo::User { id, .. }) => id.to_string(),
                Some(AuthInfo::Session { .. }) | Some(AuthInfo::ApiToken { .. }) => {
                    return Err(anyhow!("Sessions cannot query without user ID"));
                }
                None => {
                    return Err(anyhow!("Cannot query without user ID if not logged in"));
                }
            },
        };

        if user_id.is_empty() {
            return Err(anyhow!("User ID cannot be empty"));
        }

        // Non-super users may only query their own record. Sessions and API
        // tokens have no associated user record, so they may not query users.
        match auth {
            Some(AuthInfo::User { id, is_super, .. }) => {
                if !is_super && *id != user_id {
                    return Err(anyhow!("Not authorized to query other users"));
                }
            }
            Some(AuthInfo::Session { .. }) | Some(AuthInfo::ApiToken { .. }) => {
                return Err(anyhow!("Not authorized to query users"));
            }
            None => return Err(anyhow!("Cannot query users if not logged in")),
        }

        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        let rec = loader
            .load_one(UserId(ID(user_id.clone())))
            .await
            .map_err(|e| anyhow!("Failed to load user via DataLoader: {}", e))?
            .flatten()
            .ok_or_else(|| anyhow!("User with ID {} missing", user_id))?;

        Ok(rec)
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::Authenticated)")]
    async fn location(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "ID for the location to look up")] id: ID,
    ) -> Result<Location<A>> {
        if id.is_empty() {
            return Err(anyhow!("Location ID cannot be empty"));
        }

        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        let rec = loader
            .load_one(LocationId(id.clone()))
            .await
            .map_err(|e| anyhow!("Failed to load location via DataLoader: {}", e))?
            .flatten()
            .ok_or_else(|| anyhow!("Location with ID {:?} missing", id))?;

        Ok(rec)
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::Authenticated)")]
    async fn person(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "ID for the person to look up")] id: ID,
    ) -> Result<Person<A>> {
        if id.is_empty() {
            return Err(anyhow!("Person ID cannot be empty"));
        }

        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        let rec = loader
            .load_one(PersonId(id.clone()))
            .await
            .map_err(|e| anyhow!("Failed to load person via DataLoader: {}", e))?
            .flatten()
            .ok_or_else(|| anyhow!("Person with ID {:?} missing", id))?;

        Ok(rec)
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::Authenticated)")]
    async fn person_by_registration_number(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "memberNumber", desc = "Registration/member number to look up")]
        registration_number: String,
    ) -> Result<Option<PersonRef>> {
        let registration_number = registration_number.trim();
        if registration_number.is_empty() {
            return Err(anyhow!("registration_number cannot be empty"));
        }

        let app = ctx.data_unchecked::<Arc<A>>().clone();
        let matches = app
            .db()
            .get_person_id_by_registration_number(registration_number)
            .await?;
        let Some(person_id) = db::at_most_one(matches, || {
            format!("Multiple people share registration number {registration_number}")
        })?
        else {
            return Ok(None);
        };

        Ok(Some(PersonRef {
            id: ID(person_id),
            member_number: registration_number.to_string(),
        }))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::Authenticated)")]
    async fn period(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "ID for the period to look up")] id: ID,
    ) -> Result<Period<A>> {
        if id.is_empty() {
            return Err(anyhow!("Period ID cannot be empty"));
        }
        let app = ctx.data_unchecked::<Arc<A>>().clone();
        let rec = app
            .db()
            .get_periods(&[&id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Period with ID {:?} missing", id))?;
        require_location_access(ctx, &rec.location_id)?;
        Ok(Period::new(rec))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::Authenticated)")]
    async fn session(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "ID for the session to look up")] id: Option<ID>,
    ) -> Result<Session<A>> {
        let auth = ctx.data_opt::<AuthInfo>();
        let session_id = match id {
            Some(id) => id.to_string(),
            None => match auth {
                Some(AuthInfo::Session { id, .. }) => id.to_string(),
                Some(AuthInfo::User { .. }) | Some(AuthInfo::ApiToken { .. }) => {
                    return Err(anyhow!("Cannot query without session ID"));
                }
                None => {
                    return Err(anyhow!("Cannot query without session ID if not logged in"));
                }
            },
        };

        if session_id.is_empty() {
            return Err(anyhow!("Session ID cannot be empty"));
        }
        let app = ctx.data_unchecked::<Arc<A>>().clone();
        let rec = app
            .db()
            .get_sessions(&[&session_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Session with ID {:?} missing", session_id))?;
        require_location_access(ctx, &rec.location_id)?;
        Ok(Session::new(rec))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::Session)")]
    async fn refresh_token(&self, ctx: &Context<'_>) -> Result<String> {
        let session_id = match ctx.data_opt::<AuthInfo>() {
            Some(AuthInfo::Session { id, .. }) => id,
            _ => return Err(anyhow!("Cannot refresh token without session auth")),
        };

        let app = ctx.data_unchecked::<Arc<A>>().clone();
        auth::issue_token_for_session_id(&*app, session_id)
    }

    /// Look up a pending kiosk enrollment by its key fingerprint (from the QR code).
    /// Returns `None` if it never existed or has expired — the admin SessionEnroll page
    /// uses this to tell the operator to rescan.
    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn pending_enrollment_key(
        &self,
        ctx: &Context<'_>,
        fingerprint: String,
    ) -> Result<Option<PendingEnrollmentKey>> {
        let app = ctx.data_unchecked::<Arc<A>>().clone();
        let now = crate::clock::now_sec();
        let state_id = crate::session_key::enroll_state_id(&fingerprint);
        let pending = app
            .db()
            .get_ephemeral_state(&state_id)
            .await?
            .filter(|s| s.kind == crate::session_key::ENROLL_STATE_KIND && s.expires_at > now);
        Ok(pending.map(|s| PendingEnrollmentKey {
            fingerprint,
            expires_at: s.expires_at as i64,
        }))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::Authenticated)")]
    async fn category(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "ID for the category to look up")] id: ID,
    ) -> Result<Category<A>> {
        if id.is_empty() {
            return Err(anyhow!("Category ID cannot be empty"));
        }
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        let item = loader
            .load_one(CategoryId(id.clone()))
            .await
            .map_err(|e| anyhow!("Failed to load category via DataLoader: {}", e))?
            .flatten()
            .ok_or_else(|| anyhow!("Category with ID {:?} missing", id))?;

        Ok(item)
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn locations(&self, ctx: &Context<'_>) -> Result<Vec<Location<A>>> {
        let app = ctx.data_unchecked::<Arc<A>>();
        let items = app
            .db()
            .list_locations(crate::db::ListLocationsFilter::EnabledOnly)
            .await?;

        Ok(items.into_iter().map(|rec| Location::new_db(rec)).collect())
    }
    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn users(&self, ctx: &Context<'_>) -> Result<Vec<User<A>>> {
        let app = ctx.data_unchecked::<Arc<A>>();
        let items = app.db().list_users().await?;
        Ok(items.into_iter().map(|rec| User::new(rec)).collect())
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::Authenticated)")]
    async fn categories(&self, ctx: &Context<'_>) -> Result<Vec<Category<A>>> {
        let app = ctx.data_unchecked::<Arc<A>>();
        let items = app.db().list_categories().await?;
        Ok(items.into_iter().map(Category::new).collect())
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn nitc_groups(&self, ctx: &Context<'_>) -> Result<Vec<NitcGroup<A>>> {
        let app = ctx.data_unchecked::<Arc<A>>();
        let items = app.db().list_nitc_groups().await?;
        Ok(items.into_iter().map(NitcGroup::new).collect())
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn nitc_group(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "ID for the NITC group to look up")] id: ID,
    ) -> Result<NitcGroup<A>> {
        let app = ctx.data_unchecked::<Arc<A>>();
        app.db()
            .get_nitc_group(&id)
            .await?
            .map(NitcGroup::new)
            .ok_or_else(|| anyhow!("NitcGroup with ID {:?} not found", id))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn api_tokens(&self, ctx: &Context<'_>) -> Result<Vec<ApiToken>> {
        let app = ctx.data_unchecked::<Arc<A>>();
        let items = app.db().list_api_tokens().await?;
        Ok(items.into_iter().map(ApiToken::new).collect())
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn api_token(&self, ctx: &Context<'_>, id: ID) -> Result<ApiToken> {
        let app = ctx.data_unchecked::<Arc<A>>();
        let rec = app
            .db()
            .get_api_token(&id)
            .await?
            .ok_or_else(|| anyhow!("ApiToken with ID {:?} not found", id))?;
        Ok(ApiToken::new(rec))
    }

    #[graphql(
        name = "ses_headquarters",
        guard = "AuthGuard::new(AuthRequirement::SuperUser)"
    )]
    async fn ses_headquarters(&self) -> Result<Vec<SesHeadquarters>> {
        let ses_client = make_ses_client()?;
        let headquarters = ses_client.list_headquarters_cached().await?;
        Ok(headquarters
            .iter()
            .filter_map(|hq| {
                if hq.id.is_none() {
                    warn!("Skipping SES headquarters with null id: {}", hq);
                    return None;
                }
                Some(SesHeadquarters::new(hq.clone()))
            })
            .collect())
    }

    #[graphql(
        name = "ses_nonincident_types",
        guard = "AuthGuard::new(AuthRequirement::SuperUser)"
    )]
    async fn ses_nonincident_types(&self) -> Result<Vec<String>> {
        let ses_client = make_ses_client()?;
        let types = ses_client.fetch_nonincident_types_cached().await?;
        Ok((*types).clone())
    }

    #[graphql(
        name = "ses_nonincident_tags",
        guard = "AuthGuard::new(AuthRequirement::SuperUser)"
    )]
    async fn ses_nonincident_tags(&self, ctx: &Context<'_>) -> Result<Vec<SesNonIncidentTag>> {
        let app = ctx.data_unchecked::<Arc<A>>();
        let tags = app.db().list_nitc_tags().await?;
        let mut result: Vec<SesNonIncidentTag> = tags
            .into_iter()
            .map(|t| SesNonIncidentTag {
                id: t.id,
                name: t.name,
                primary_activity_name: t.primary_activity_name,
            })
            .collect();
        result.sort_by_key(|t| t.id);
        Ok(result)
    }

    #[graphql(
        name = "ses_participant_types",
        guard = "AuthGuard::new(AuthRequirement::SuperUser)"
    )]
    async fn ses_participant_types(&self) -> Result<Vec<String>> {
        let ses_client = make_ses_client()?;
        let types = ses_client.fetch_participant_types_cached().await?;
        Ok((*types).clone())
    }
}
