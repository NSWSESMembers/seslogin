use anyhow::{Context, Result, anyhow};
use aws_sdk_sqs::Client as SqsClient;
use chrono_tz::Australia::Sydney;
use std::collections::HashMap;
use tracing::{info, warn};

use crate::db;
use crate::ses_api::{
    SesClient, SesNonIncidentCreate, SesNonIncidentUpdate, SesParticipantUpsert, SesPersonRef,
    SesTagRef,
};
use crate::sqs_dispatch;

#[derive(Debug, Clone)]
pub struct NitcConfig {
    pub dry_run: bool,
    /// Re-sync even when the period/event is already at the exported version.
    pub force: bool,
    /// Perform DB writes but skip enqueuing SQS messages. Lets you update the DB in
    /// Phase 1 and run Phase 2 locally instead of via the queue. Phase 2 does not enqueue
    /// anything, so this flag has no effect there.
    pub skip_queue: bool,
    pub ses_api_base_url: String,
    pub ses_api_key: String,
    pub nitc_queue_url: String,
    pub max_retries: usize,
}

#[derive(Debug, Clone)]
pub enum PeriodAssignOutcome {
    Assigned(String),
    /// Period is no longer NITC-eligible but was still attached to an event, so it was
    /// detached: the event was re-synced to drop its participant and the period's
    /// event/participant pointers were cleared. Holds the event ID it was removed from.
    Detached(String),
    /// Period is not NITC-eligible. The reason explains which check it failed.
    Skipped(SkipReason),
    /// Period's nitc_exported_version >= version — already fully exported, nothing to do.
    AlreadySynced,
}

/// Why a period was not assigned to an NITC event. Used for CLI/log debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// The period row does not exist (or is deleted).
    PeriodNotFound,
    /// The period has no category set.
    NoCategory,
    /// The period's category row does not exist.
    CategoryNotFound,
    /// The category has no nitc_group_id, so it is not NITC-enabled.
    CategoryNotNitcEnabled,
    /// The location is not NITC-enabled (no nitc_enabled cutover set).
    LocationNotNitcEnabled,
    /// The location has no ses_api_headquarters_id to export against.
    LocationNoHeadquartersId,
    /// The period started before the location's NITC cutover time.
    BeforeCutover,
    /// The period is still open and has no participant to clean up.
    OpenNoParticipant,
    /// The period's start time is not before its end time.
    StartNotBeforeEnd,
    /// The period's duration exceeds the maximum allowed length.
    DurationTooLong,
}

impl std::fmt::Display for SkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            SkipReason::PeriodNotFound => "period not found",
            SkipReason::NoCategory => "period has no category",
            SkipReason::CategoryNotFound => "category not found",
            SkipReason::CategoryNotNitcEnabled => "category is not NITC-enabled (no nitc_group_id)",
            SkipReason::LocationNotNitcEnabled => "location is not NITC-enabled",
            SkipReason::LocationNoHeadquartersId => "location has no ses_api_headquarters_id",
            SkipReason::BeforeCutover => "period started before the location's NITC cutover",
            SkipReason::OpenNoParticipant => "period is open with no participant",
            SkipReason::StartNotBeforeEnd => "period start time is not before its end time",
            SkipReason::DurationTooLong => "period duration exceeds the maximum allowed length",
        };
        f.write_str(msg)
    }
}

#[derive(Debug, Clone)]
pub enum EventSyncOutcome {
    Synced(i64),
    /// Message version doesn't match the event's current version — a newer change is pending.
    Stale,
    /// event was already synced at this version or later — nothing to do.
    AlreadySynced,
    NoLivePeriods,
    /// Event could not be synced. The reason explains which check it failed.
    Skipped(EventSkipReason),
}

/// Why a NITC event was not synced to SES. Used for CLI/log debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventSkipReason {
    /// The nitc_events row does not exist.
    EventNotFound,
    /// The event's location row does not exist.
    LocationNotFound,
    /// The event's location is not NITC-enabled.
    LocationNotNitcEnabled,
}

impl std::fmt::Display for EventSkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            EventSkipReason::EventNotFound => "NITC event not found",
            EventSkipReason::LocationNotFound => "location not found",
            EventSkipReason::LocationNotNitcEnabled => "location is not NITC-enabled",
        };
        f.write_str(msg)
    }
}

/// SES rejects event names longer than this many characters.
const MAX_EVENT_NAME_LEN: usize = 50;

/// Periods longer than this (96 hours, in seconds) are treated as bad data and skipped.
const MAX_PERIOD_DURATION_SECS: u64 = 96 * 60 * 60;

fn make_event_name(category_name: Option<&str>) -> String {
    let name = category_name.unwrap_or("unknown");
    let compressed = name.replace(" - ", "-");
    let full = format!("SESLOGIN: {}", compressed);

    if full.chars().count() <= MAX_EVENT_NAME_LEN {
        return full;
    }
    // Keep the first 49 chars and append the ellipsis (counted as one char by SES).
    let mut truncated: String = full.chars().take(MAX_EVENT_NAME_LEN - 1).collect();
    truncated.push('…');
    truncated
}

pub struct SqsClients {
    pub client: SqsClient,
    pub queue_url: String,
}

pub struct NitcClients<D: db::Handler> {
    pub db: D,
    pub ses: SesClient,
    pub sqs: SqsClients,
}

pub async fn make_dynamodb_clients(
    config: &NitcConfig,
    db_prefix: String,
) -> Result<NitcClients<crate::dynamodb::Handler>> {
    let db = crate::dynamodb::Handler::new(&db_prefix, false).await;
    let ses = SesClient::new(
        config.ses_api_base_url.clone(),
        config.ses_api_key.clone(),
        100,
        config.max_retries,
    )?;
    let aws_cfg = crate::aws_config_loader().load().await;
    let sqs = SqsClients {
        client: SqsClient::new(&aws_cfg),
        queue_url: config.nitc_queue_url.clone(),
    };
    Ok(NitcClients { db, ses, sqs })
}

fn unix_to_sydney_rfc3339(unix: u64) -> String {
    let utc =
        chrono::DateTime::from_timestamp(unix as i64, 0).unwrap_or(chrono::DateTime::UNIX_EPOCH);
    utc.with_timezone(&Sydney).to_rfc3339()
}

fn unix_to_sydney_date(unix: u64) -> chrono::NaiveDate {
    let utc =
        chrono::DateTime::from_timestamp(unix as i64, 0).unwrap_or(chrono::DateTime::UNIX_EPOCH);
    utc.with_timezone(&Sydney).date_naive()
}

// ── Phase 1: Period assignment ────────────────────────────────────────────────

/// Handle a period that is not (or no longer) NITC-eligible. If it is still attached to an
/// NITC event, detach it: bump and re-sync that event so SES drops the participant (by its
/// absence from the next PUT), and clear the period's event/participant pointers so it is no
/// longer listed for the event. Otherwise it is a plain skip with the given reason.
async fn skip_or_detach<D: db::Handler>(
    period: &db::Period,
    reason: SkipReason,
    config: &NitcConfig,
    clients: &NitcClients<D>,
) -> Result<PeriodAssignOutcome> {
    let Some(old_event_id) = period.nitc_event_id.as_deref() else {
        return Ok(PeriodAssignOutcome::Skipped(reason));
    };

    if config.dry_run {
        info!(
            "[dry-run] Would detach period {} from event {} (now ineligible: {}) and re-sync the event to remove its participant",
            period.id, old_event_id, reason
        );
        return Ok(PeriodAssignOutcome::Detached(old_event_id.to_string()));
    }

    info!(
        "Detaching period {} from event {} (now ineligible: {})",
        period.id, old_event_id, reason
    );
    // Bump + enqueue the event sync first so Phase 2 re-PUTs the participant list without this
    // period, then clear the period's pointers so it drops off the event's period index. The
    // event_export is delayed, so the clear lands well before Phase 2 reads the list.
    let new_version = clients.db.bump_nitc_event_version(old_event_id).await?;
    if config.skip_queue {
        info!(
            "[skip-queue] Not enqueuing Phase 2 export for detached event {} (version {}); run it locally",
            old_event_id, new_version
        );
    } else {
        sqs_dispatch::enqueue_nitc_event_export(
            &clients.sqs.client,
            &clients.sqs.queue_url,
            old_event_id,
            new_version,
        )
        .await?;
    }
    clients
        .db
        .clear_period_nitc_participant(&period.id, period.version)
        .await?;

    Ok(PeriodAssignOutcome::Detached(old_event_id.to_string()))
}

/// Phase 1: assign a period to the correct nitc_events DB row (creating if needed),
/// bump the event version, and enqueue a delayed event_sync SQS message for each
/// affected event. Returns Skipped if the period is not NITC-eligible.
pub async fn assign_period<D: db::Handler>(
    period_id: &str,
    config: &NitcConfig,
    clients: &NitcClients<D>,
) -> Result<PeriodAssignOutcome> {
    // Fetch and validate the period + its category + location for NITC eligibility
    let Some(period) = clients
        .db
        .get_periods(&[period_id])
        .await?
        .into_iter()
        .next()
        .flatten()
    else {
        return Ok(PeriodAssignOutcome::Skipped(SkipReason::PeriodNotFound));
    };

    let Some(category_id) = &period.category_id else {
        return skip_or_detach(&period, SkipReason::NoCategory, config, clients).await;
    };

    let Some(category) = clients
        .db
        .get_categories(&[category_id])
        .await?
        .into_iter()
        .next()
        .flatten()
    else {
        return skip_or_detach(&period, SkipReason::CategoryNotFound, config, clients).await;
    };

    let Some(nitc_group_id) = category.nitc_group_id else {
        return skip_or_detach(&period, SkipReason::CategoryNotNitcEnabled, config, clients).await;
    };

    let location = clients
        .db
        .get_locations(&[&period.location_id])
        .await?
        .into_iter()
        .next()
        .flatten();
    let Some(nitc_cutover) = location.as_ref().and_then(|l| l.nitc_enabled) else {
        return skip_or_detach(&period, SkipReason::LocationNotNitcEnabled, config, clients).await;
    };
    if location
        .as_ref()
        .and_then(|l| l.ses_api_headquarters_id.as_ref())
        .is_none()
    {
        return skip_or_detach(
            &period,
            SkipReason::LocationNoHeadquartersId,
            config,
            clients,
        )
        .await;
    }
    if period.start_time < nitc_cutover {
        return skip_or_detach(&period, SkipReason::BeforeCutover, config, clients).await;
    }

    // Validate the period's bounds. These only apply to closed periods (those with an
    // end_time); open periods are handled below.
    if let Some(end_time) = period.end_time {
        if period.start_time >= end_time {
            return skip_or_detach(&period, SkipReason::StartNotBeforeEnd, config, clients).await;
        }
        if end_time - period.start_time > MAX_PERIOD_DURATION_SECS {
            return skip_or_detach(&period, SkipReason::DurationTooLong, config, clients).await;
        }
    }

    // Skip open periods that have no participant to clean up
    if period.end_time.is_none() && period.nitc_participant_id.is_none() {
        if !config.dry_run {
            clients
                .db
                .set_period_nitc_exported_version(period_id, period.version)
                .await?;
        }
        return Ok(PeriodAssignOutcome::Skipped(SkipReason::OpenNoParticipant));
    }

    if !config.force && period.nitc_exported_version >= Some(period.version) {
        return Ok(PeriodAssignOutcome::AlreadySynced);
    }

    let event_date = unix_to_sydney_date(period.start_time);

    if config.dry_run {
        let existing = db::at_most_one(
            clients
                .db
                .list_nitc_events_for_day(&period.location_id, &nitc_group_id, event_date)
                .await?,
            || {
                format!(
                    "Multiple nitc_events for location {} nitc_group {} date {}",
                    period.location_id, nitc_group_id, event_date
                )
            },
        )?;
        match &existing {
            Some(r) => info!(
                "[dry-run] Would assign period {} to existing NITC event {} (location={}, nitc_group={}, date={})",
                period_id, r.id, period.location_id, nitc_group_id, event_date
            ),
            None => info!(
                "[dry-run] Would create and assign period {} to new NITC event (location={}, nitc_group={}, date={})",
                period_id, period.location_id, nitc_group_id, event_date
            ),
        }
        if let Some(ref old_event_id) = period.nitc_event_id
            && existing.as_ref().map(|r| &r.id) != Some(old_event_id)
        {
            info!(
                "[dry-run] Would move period {} from event {} (old event's Phase 2 will omit the participant)",
                period_id, old_event_id
            );
        }
        return Ok(PeriodAssignOutcome::Assigned(
            existing.map_or_else(String::new, |r| r.id),
        ));
    }

    let desired_event = clients
        .db
        .get_or_create_nitc_event_for_day(&period.location_id, &nitc_group_id, event_date)
        .await?;

    let mut events_to_sync: Vec<(String, u64)> = Vec::new();

    // Handle category/date change: the period was previously on a different event
    if let Some(old_event_id) = period.nitc_event_id {
        if old_event_id != desired_event.id {
            // Bump the version of the event the period is moving off so that the participant gets
            // removed and the start/end times get recalculated.
            let old_version = clients.db.bump_nitc_event_version(&old_event_id).await?;
            events_to_sync.push((old_event_id, old_version));

            // Reassign period to the new event
            clients
                .db
                .set_period_nitc_event(period_id, &desired_event.id)
                .await?;
        }
    } else {
        // First-time assignment
        clients
            .db
            .set_period_nitc_event(period_id, &desired_event.id)
            .await?;
    }

    // Bump the desired event's version
    let new_version = clients
        .db
        .bump_nitc_event_version(&desired_event.id)
        .await?;
    events_to_sync.push((desired_event.id.clone(), new_version));

    for (event_id, version) in events_to_sync {
        if config.skip_queue {
            info!(
                "[skip-queue] Not enqueuing Phase 2 export for event {} (version {}); run it locally",
                event_id, version
            );
            continue;
        }
        sqs_dispatch::enqueue_nitc_event_export(
            &clients.sqs.client,
            &clients.sqs.queue_url,
            &event_id,
            version,
        )
        .await?;
    }

    clients
        .db
        .set_period_nitc_exported_version(period_id, period.version)
        .await?;

    Ok(PeriodAssignOutcome::Assigned(desired_event.id))
}

// ── Backfill: scan for unsynced periods and enqueue them ─────────────────────

#[derive(Debug, Default)]
pub struct BackfillStats {
    pub locations_checked: usize,
    pub periods_enqueued: usize,
    pub periods_already_synced: usize,
    pub periods_skipped_no_nitc_category: usize,
}

/// Scan all NITC-enabled locations (or a single location if `location_id_filter` is given)
/// for periods that are not yet exported (`nitc_exported_version < version`) and enqueue
/// a Phase 1 SQS message for each one. Periods with no category or a category without a
/// nitc_group_id are skipped.
pub async fn backfill_unsynced_periods<D: db::Handler>(
    location_id_filter: Option<&str>,
    config: &NitcConfig,
    clients: &NitcClients<D>,
) -> Result<BackfillStats> {
    let locations: Vec<db::Location> = if let Some(loc_id) = location_id_filter {
        clients
            .db
            .get_locations(&[loc_id])
            .await?
            .into_iter()
            .flatten()
            .collect()
    } else {
        clients
            .db
            .list_locations(crate::db::ListLocationsFilter::EnabledOnly)
            .await?
    };

    // Build a set of category IDs that are NITC-eligible (have nitc_group_id set).
    let nitc_category_ids: std::collections::HashSet<String> = clients
        .db
        .list_categories()
        .await?
        .into_iter()
        .filter(|c| c.nitc_group_id.is_some())
        .map(|c| c.id)
        .collect();

    let mut stats = BackfillStats::default();

    for location in &locations {
        let Some(nitc_cutover) = location.nitc_enabled else {
            continue;
        };
        stats.locations_checked += 1;
        info!(
            "Scanning location {} ({}) for unsynced periods since {}",
            location.id, location.name, nitc_cutover
        );

        let mut after_cursor: Option<db::PeriodCursor> = None;
        loop {
            let page = db::ListPeriodsPage {
                after: after_cursor.clone(),
                before: None,
                limit: 500,
                descending: false,
            };
            let batch = clients
                .db
                .list_periods_for_location(
                    &location.id,
                    false,
                    Some((nitc_cutover, u64::MAX)),
                    page,
                )
                .await?;
            let done = batch.len() < 500;
            after_cursor = batch.last().map(|p| db::PeriodCursor {
                id: p.id.clone(),
                start_time: p.start_time,
            });

            for period in &batch {
                let has_nitc_category = period
                    .category_id
                    .as_ref()
                    .is_some_and(|id| nitc_category_ids.contains(id));
                if !has_nitc_category {
                    stats.periods_skipped_no_nitc_category += 1;
                    continue;
                }
                if !config.force && period.nitc_exported_version >= Some(period.version) {
                    stats.periods_already_synced += 1;
                    continue;
                }
                if config.dry_run {
                    info!(
                        "[dry-run] Would enqueue period {} (location={}, version={}, nitc_exported_version={:?})",
                        period.id, location.id, period.version, period.nitc_exported_version
                    );
                } else {
                    sqs_dispatch::enqueue_period_nitc_export(
                        &clients.sqs.client,
                        &clients.sqs.queue_url,
                        &period.id,
                    )
                    .await?;
                    info!(
                        "Enqueued period {} (location={}, version={}, nitc_exported_version={:?})",
                        period.id, location.id, period.version, period.nitc_exported_version
                    );
                }
                stats.periods_enqueued += 1;
            }

            if done {
                break;
            }
        }
    }

    Ok(stats)
}

// ── Phase 2: event sync ──────────────────────────────────────────────────────

/// Phase 2: sync a NITC event to SES if the version matches (no newer changes pending).
pub async fn sync_nitc_event<D: db::Handler>(
    event_id: &str,
    expected_version: Option<u64>,
    config: &NitcConfig,
    clients: &NitcClients<D>,
) -> Result<EventSyncOutcome> {
    let Some(event) = clients.db.get_nitc_event_by_id(event_id).await? else {
        warn!("NITC event {} not found, skipping sync", event_id);
        return Ok(EventSyncOutcome::Skipped(EventSkipReason::EventNotFound));
    };

    if expected_version.is_some_and(|v| event.version != v) {
        return Ok(EventSyncOutcome::Stale);
    }

    if !config.force && event.synced_version.is_some_and(|v| v >= event.version) {
        return Ok(EventSyncOutcome::AlreadySynced);
    }

    // Fetch period IDs and then the full period records
    let period_ids = clients.db.list_period_ids_for_nitc_event(event_id).await?;
    let all_periods: Vec<db::Period> = clients
        .db
        .get_periods(&period_ids)
        .await
        .context("Getting periods for NITC event")?
        .into_iter()
        .flatten()
        .collect();
    for period in &all_periods {
        info!(
            "Period {} (person {}, category {:?}, start {}, end {:?}, deleted {})",
            period.id,
            period.person_id.as_deref().unwrap_or("guest"),
            period.category_id,
            unix_to_sydney_rfc3339(period.start_time),
            period.end_time.map(unix_to_sydney_rfc3339),
            period.deleted.is_some()
        );
    }

    // Batch-fetch persons and categories needed by the periods
    let person_ids: Vec<&str> = all_periods
        .iter()
        .filter_map(|p| p.person_id.as_deref())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let persons: HashMap<String, db::Person> = clients
        .db
        .get_persons(&person_ids)
        .await
        .context("Getting persons for NITC event")?
        .into_iter()
        .flatten()
        .map(|p| (p.id.clone(), p))
        .collect();

    let category_ids: Vec<&str> = {
        let mut seen = std::collections::HashSet::new();
        all_periods
            .iter()
            .filter_map(|p| p.category_id.as_deref())
            .filter(|&id| seen.insert(id))
            .collect()
    };
    let categories: HashMap<String, db::Category> = if !category_ids.is_empty() {
        clients
            .db
            .get_categories(&category_ids)
            .await
            .context("Getting categories for NITC event")?
            .into_iter()
            .flatten()
            .map(|c| (c.id.clone(), c))
            .collect()
    } else {
        HashMap::new()
    };

    let Some(location) = clients
        .db
        .get_locations(&[&event.location_id])
        .await
        .context("Getting location for NITC event")?
        .into_iter()
        .next()
        .flatten()
    else {
        warn!(
            "NITC event {} location {} not found, skipping sync",
            event_id, event.location_id
        );
        return Ok(EventSyncOutcome::Skipped(EventSkipReason::LocationNotFound));
    };

    // Only live, ended periods are sent to SES; deleted periods are removed implicitly by
    // their absence from the PUT participants list.
    let live_periods: Vec<&db::Period> = all_periods
        .iter()
        .filter(|p| p.deleted.is_none() && p.end_time.is_some())
        .collect();

    // Defensive guard: every live period's category must map to this event's nitc_group.
    // A period whose category maps to a different group would carry an incompatible
    // participant type and make SES reject the whole update. Error out before we talk to
    // SES so the offending event can be investigated rather than silently corrupting it.
    let mismatched: Vec<String> = live_periods
        .iter()
        .filter(|p| {
            let group = p
                .category_id
                .as_deref()
                .and_then(|id| categories.get(id))
                .and_then(|c| c.nitc_group_id.as_ref());
            group != Some(&event.nitc_group_id)
        })
        .map(|p| {
            let group = p
                .category_id
                .as_deref()
                .and_then(|id| categories.get(id))
                .and_then(|c| c.nitc_group_id.clone());
            format!(
                "period {} (category {:?}, nitc_group {:?})",
                p.id, p.category_id, group
            )
        })
        .collect();
    if !mismatched.is_empty() {
        return Err(anyhow!(
            "NITC event {} (nitc_group {}) has live periods whose category maps to a different nitc_group: {}",
            event_id,
            event.nitc_group_id,
            mismatched.join(", ")
        ));
    }

    // Validate location NITC eligibility and resolve SES HQ ID
    if location.nitc_enabled.is_none() {
        warn!(
            "NITC event {} location {} not NITC-enabled, skipping sync",
            event_id, event.location_id
        );
        return Ok(EventSyncOutcome::Skipped(
            EventSkipReason::LocationNotNitcEnabled,
        ));
    }
    let ses_hq_id = location
        .ses_api_headquarters_id
        .as_ref()
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| {
            anyhow!(
                "Location {} has invalid ses_api_headquarters_id",
                location.id
            )
        })?;
    let nitc_location = location.name.clone();

    // Fetch NITC group config (type, tags) from the event's nitc_group_id
    let Some(nitc_group) = clients
        .db
        .get_nitc_group(&event.nitc_group_id)
        .await
        .context("Getting NITC group for event")?
    else {
        warn!(
            "NITC event {} nitc_group {} not found, skipping sync",
            event_id, event.nitc_group_id
        );
        return Ok(EventSyncOutcome::NoLivePeriods);
    };
    let nitc_type = nitc_group.nitc_type.clone();
    let nitc_tags: Vec<i32> = nitc_group.nitc_tag_ids.clone();

    // (period, ses_person_id) pairs for periods whose participants were successfully resolved
    let mut resolved: Vec<(&db::Period, i64)> = Vec::new();

    let (
        event_name,
        event_description,
        event_start_date,
        event_end_date,
        event_participants,
        event_tags,
    ) = if live_periods.is_empty() {
        // We can't delete NITC events in SES, so when all participants are removed we zero out the
        // time window and participant list while keeping the type/location/tags unchanged.
        let event_start = unix_to_sydney_rfc3339(
            event
                .event_date
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp() as u64,
        );
        let event_end = unix_to_sydney_rfc3339(
            event
                .event_date
                .and_hms_opt(0, 0, 1)
                .unwrap()
                .and_utc()
                .timestamp() as u64,
        );
        (
            "SESLOGIN - unused".to_string(),
            "Event created by SES Activity NITC export - previously had entries but they have all been removed".to_string(),
            event_start,
            event_end,
            Vec::<SesParticipantUpsert>::new(),
            nitc_tags.iter().map(|&id| SesTagRef::new(id)).collect::<Vec<_>>(),
        )
    } else {
        let min_start = live_periods.iter().map(|p| p.start_time).min().unwrap();
        let max_end = live_periods
            .iter()
            .filter_map(|p| p.end_time)
            .max()
            .unwrap();

        let start_rfc = unix_to_sydney_rfc3339(min_start);
        let end_rfc = unix_to_sydney_rfc3339(max_end);

        // Use the first live period's category for the event name and tags
        let rep_cat = live_periods[0]
            .category_id
            .as_deref()
            .and_then(|id| categories.get(id));
        let tags: Vec<SesTagRef> = nitc_tags.iter().map(|&id| SesTagRef::new(id)).collect();
        let event_name = make_event_name(rep_cat.map(|c| c.name.as_str()));

        // Resolve ses_person_id for each live period, skipping those we can't resolve
        for period in &live_periods {
            let person = period.person_id.as_ref().and_then(|id| persons.get(id));
            let ses_api_person_id: Option<i64> = person
                .and_then(|p| p.ses_api_person_id.as_deref())
                .and_then(|s| s.parse().ok());

            let Some(ses_person_id) = ses_api_person_id else {
                warn!(
                    "Period {} has no ses_api_person_id, skipping participant sync",
                    period.id
                );
                continue;
            };
            resolved.push((period, ses_person_id));
        }

        let participants: Vec<SesParticipantUpsert> = resolved
            .iter()
            .map(|(period, ses_person_id)| {
                let nitc_participant_type = period
                    .category_id
                    .as_deref()
                    .and_then(|id| categories.get(id))
                    .and_then(|c| c.nitc_participant_type.clone())
                    .unwrap_or_default();
                SesParticipantUpsert {
                    id: period.nitc_participant_id,
                    participant_type: nitc_participant_type,
                    start_date: unix_to_sydney_rfc3339(period.start_time),
                    end_date: unix_to_sydney_rfc3339(period.end_time.unwrap()),
                    person: SesPersonRef { id: *ses_person_id },
                }
            })
            .collect();

        (
            event_name,
            "Event created by SES Activity NITC export".to_string(),
            start_rfc,
            end_rfc,
            participants,
            tags,
        )
    };

    if config.dry_run {
        info!(
            "[dry-run] Would {} NITC event {} (ses_id={:?}) with {} participants",
            if event.ses_api_nitc_id.is_none() {
                "create+update"
            } else {
                "update"
            },
            event_id,
            event.ses_api_nitc_id,
            event_participants.len()
        );
        return Ok(EventSyncOutcome::Synced(event.ses_api_nitc_id.unwrap_or(0)));
    }

    let ses_nitc_id = if let Some(existing_id) = event.ses_api_nitc_id {
        existing_id
    } else {
        let create_body = SesNonIncidentCreate {
            name: event_name.clone(),
            description: event_description.clone(),
            nitc_type: nitc_type.clone(),
            location: nitc_location.clone(),
            start_date: event_start_date.clone(),
            end_date: event_end_date.clone(),
            tags: event_tags.clone(),
        };
        let new_id = clients
            .ses
            .create_nitc_event(ses_hq_id, &create_body)
            .await?;
        // make sure this happens immediately after we ge the new ID from the SES API
        // because we don't want to end up hitting an error and retrying the above create call,
        // which would result in multiple NITC events being created in SES for the same event.
        clients.db.set_nitc_event_ses_id(event_id, new_id).await?;
        new_id
    };

    let result = clients
        .ses
        .update_nitc_event(
            ses_hq_id,
            &SesNonIncidentUpdate {
                id: ses_nitc_id,
                name: event_name,
                description: event_description,
                nitc_type,
                location: nitc_location,
                start_date: event_start_date,
                end_date: event_end_date,
                participants: event_participants,
                tags: event_tags,
                completed: true,
            },
        )
        .await?;

    // Match returned participants back to periods by person_id to store participant IDs
    let participant_by_person: HashMap<i64, i64> = result
        .participants
        .into_iter()
        .filter_map(|p| p.person.map(|person| (person.id, p.id)))
        .collect();

    for (period, ses_person_id) in &resolved {
        let Some(&participant_id) = participant_by_person.get(ses_person_id) else {
            warn!(
                "Period {} person {} not found in SES upsert response, participant ID not stored",
                period.id, ses_person_id
            );
            continue;
        };
        clients
            .db
            .update_period_nitc_exported(&period.id, event_id, participant_id, period.version)
            .await?;
    }

    // Clear DB state for deleted periods whose participants were removed from SES implicitly
    for period in all_periods
        .iter()
        .filter(|p| p.deleted.is_some() && p.nitc_participant_id.is_some())
    {
        clients
            .db
            .clear_period_nitc_participant(&period.id, period.version)
            .await?;
    }

    clients
        .db
        .mark_nitc_event_synced(event_id, event.version)
        .await?;

    Ok(EventSyncOutcome::Synced(ses_nitc_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_name_unchanged() {
        assert_eq!(make_event_name(Some("Boxing")), "SESLOGIN: Boxing");
    }

    #[test]
    fn long_name_truncated_with_ellipsis() {
        let long = "A".repeat(80);
        let result = make_event_name(Some(&long));
        assert_eq!(result.chars().count(), MAX_EVENT_NAME_LEN);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn truncation_handles_multibyte_chars() {
        let long = "é".repeat(80);
        let result = make_event_name(Some(&long));
        assert_eq!(result.chars().count(), MAX_EVENT_NAME_LEN);
        assert!(result.ends_with('…'));
    }
}
