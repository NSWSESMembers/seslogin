use crate::db::{self, Handler as _};
use crate::dynamodb;
use crate::ses_api::{SesClient, SesPerson, SesSearchClient};
use anyhow::{Context, Result, anyhow};
use std::collections::{HashMap, HashSet};
use tracing::{error, info, warn};

const BLOCKED_UNIT_SES_IDS: &[i64] = &[
    307, // 'Volunteer Membership Unit'
    269, // 'Interstate' zone (empty at last check)
    277, // 'State Units' zone (empty at last check)
];

#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub dry_run: bool,
    pub adopt: bool,
    pub ses_api_base_url: String,
    pub ses_api_key: String,
    pub ses_intranet_search_api_base_url: String,
    pub ses_intranet_search_api_key: String,
    pub db_prefix: String,
    pub page_limit: usize,
    pub max_retries: usize,
    pub location_ids: Vec<String>,
    pub max_mutations: usize,
    pub absence: AbsencePolicy,
}

/// Governs the two-phase deletion of members who stop appearing in their location's SES
/// payload. Phase one stamps `Person.missing_since`; phase two soft-deletes once that
/// marker has aged past `grace_secs` — giving a member who transfers to another unit time
/// to be picked up (and their marker cleared) by the new unit's sync.
#[derive(Debug, Clone, Copy)]
pub struct AbsencePolicy {
    /// Kill switch. Off by default; enable per environment after reviewing a dry run.
    pub enabled: bool,
    /// How long a marker must age before the member is soft-deleted.
    pub grace_secs: u64,
    /// Always allow at least this many absence candidates, so small units still work.
    pub min_candidates: usize,
    /// Above that floor, cap candidates at this percentage of the synced roster.
    pub max_candidate_percent: usize,
    /// Suppress deletions when the previous successful sync for the location is older
    /// than this (or absent) — a marker is one observation plus a clock, and a location
    /// recovering from a long outage must not delete on its first sight of the roster.
    pub max_sync_staleness_secs: u64,
}

pub const DEFAULT_ABSENCE_GRACE_SECS: u64 = 7 * 24 * 3600;
pub const DEFAULT_ABSENCE_MIN_CANDIDATES: usize = 5;
/// Raised from 20% after a unit hit the cap on a genuine round of departures. Note the
/// floor above is what binds for any roster under 20 members, not this percentage.
pub const DEFAULT_ABSENCE_MAX_CANDIDATE_PERCENT: usize = 25;
/// 36h: the checker lambda alarms on a location that has not synced for 30h, so anything
/// beyond this is already a known-bad location.
pub const DEFAULT_ABSENCE_MAX_SYNC_STALENESS_SECS: u64 = 36 * 3600;

impl Default for AbsencePolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            grace_secs: DEFAULT_ABSENCE_GRACE_SECS,
            min_candidates: DEFAULT_ABSENCE_MIN_CANDIDATES,
            max_candidate_percent: DEFAULT_ABSENCE_MAX_CANDIDATE_PERCENT,
            max_sync_staleness_secs: DEFAULT_ABSENCE_MAX_SYNC_STALENESS_SECS,
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct RunStats {
    pub processed_locations: usize,
    pub skipped_locations: usize,
    pub ses_people_seen: usize,
    pub adopts: usize,
    pub creates: usize,
    pub updates: usize,
    pub undeletes: usize,
    pub soft_deletes: usize,
    pub noops: usize,
    pub blocked_manual_conflicts: usize,
    pub emails_seen: usize,
    pub emails_updated: usize,
    pub emails_unmatched: usize,
    pub emails_noops: usize,
    /// SES rows carrying `deleted: true`. Not expected to happen; we only warn.
    pub ses_deleted_flags_seen: usize,
    pub missing_marked: usize,
    pub missing_cleared: usize,
    /// Candidates holding a marker that has not yet aged past the grace window.
    pub missing_waiting: usize,
    /// Deletions held back because the location's previous sync was missing or stale.
    pub absence_deletes_suppressed: usize,
    /// Locations where a guard suppressed the whole absence pass.
    pub absence_skipped_locations: usize,
}

impl RunStats {
    /// Feeds the `max_mutations` abort, which is a tripwire for "SES handed us garbage and
    /// we are about to rewrite the member database".
    ///
    /// Absence writes are deliberately excluded. They are governed by the per-location
    /// candidate cap instead, so that a location with a large but *legal* backlog of
    /// departed members does not trip a global tripwire and abort every other location's
    /// legitimate creates and updates along with it. Exceeding that cap is itself fatal for
    /// the offending location — see `absence_skip_is_fatal`.
    pub fn total_mutations(&self) -> usize {
        self.adopts + self.creates + self.updates + self.undeletes
    }
}

#[derive(Debug, PartialEq, Eq)]
enum PlannedChange {
    AdoptSesApiPersonId {
        person_id: String,
        location_id: String,
        ses_api_person_id: String,
        registration_number: String,
    },
    Create {
        location_id: String,
        ses_api_person_id: String,
        registration_number: String,
        first_name: String,
        last_name: String,
    },
    Update {
        person_id: String,
        location_id: String,
        current_location_id: String,
        ses_api_person_id: String,
        registration_number: String,
        first_name: String,
        current_first_name: String,
        last_name: String,
        current_last_name: String,
    },
    UndeleteAndUpdate {
        person_id: String,
        location_id: String,
        current_location_id: String,
        ses_api_person_id: String,
        registration_number: String,
        first_name: String,
        current_first_name: String,
        last_name: String,
        current_last_name: String,
    },
    /// Stamp the missing marker on a person absent from their location's SES payload.
    MarkMissing {
        person_id: String,
        location_id: String,
        ses_api_person_id: String,
        registration_number: Option<String>,
        missing_since: u64,
    },
    /// Drop the missing marker from a person SES mentioned again.
    ClearMissing {
        person_id: String,
        location_id: String,
    },
    SoftDelete {
        person_id: String,
        location_id: String,
        ses_api_person_id: String,
        registration_number: Option<String>,
    },
}

fn normalize_names(person: &SesPerson) -> Result<(String, String)> {
    let first = person
        .first_name
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string();
    let last = person
        .last_name
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string();

    if first.is_empty() && last.is_empty() {
        Err(anyhow!(
            "SES person has empty first and last name after trimming: {} (fullName='{}')",
            person,
            person.full_name.as_deref().unwrap_or("")
        ))
    } else {
        Ok((first, last))
    }
}

fn build_location_filter(location_ids: &[String]) -> HashSet<String> {
    location_ids
        .iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect()
}

fn print_message(message: &str) {
    println!("{}", message);
}

fn print_planned_change(change: &PlannedChange, dry_run: bool) {
    let mode = if dry_run { "DRY-RUN" } else { "APPLY" };
    match change {
        PlannedChange::AdoptSesApiPersonId {
            person_id,
            location_id,
            ses_api_person_id,
            registration_number,
        } => {
            println!(
                "[{mode}] adopt sesApiPersonId for person id={} location={} sesApiPersonId={} registrationNumber={}",
                person_id, location_id, ses_api_person_id, registration_number
            );
        }
        PlannedChange::Create {
            location_id,
            ses_api_person_id,
            registration_number,
            first_name,
            last_name,
        } => {
            println!(
                "[{mode}] create person location={} sesApiPersonId={} registrationNumber={} firstName='{}' lastName='{}'",
                location_id, ses_api_person_id, registration_number, first_name, last_name
            );
        }
        PlannedChange::Update {
            person_id,
            location_id,
            current_location_id,
            ses_api_person_id,
            registration_number,
            first_name,
            current_first_name,
            last_name,
            current_last_name,
        } => {
            println!(
                "[{mode}] update person id={} location={}=>{} sesApiPersonId={} registrationNumber={} firstName='{}'=>'{}' lastName='{}'=>'{}'",
                person_id,
                current_location_id,
                location_id,
                ses_api_person_id,
                registration_number,
                current_first_name,
                first_name,
                current_last_name,
                last_name
            );
        }
        PlannedChange::UndeleteAndUpdate {
            person_id,
            location_id,
            current_location_id,
            ses_api_person_id,
            registration_number,
            first_name,
            current_first_name,
            last_name,
            current_last_name,
        } => {
            println!(
                "[{mode}] undelete+update person id={} location={}=>{} sesApiPersonId={} registrationNumber={} firstName='{}'=>'{}' lastName='{}'=>'{}'",
                person_id,
                current_location_id,
                location_id,
                ses_api_person_id,
                registration_number,
                current_first_name,
                first_name,
                current_last_name,
                last_name
            );
        }
        PlannedChange::MarkMissing {
            person_id,
            location_id,
            ses_api_person_id,
            registration_number,
            missing_since,
        } => {
            println!(
                "[{mode}] mark member missing id={} location={} sesApiPersonId={} registrationNumber={} missingSince={}",
                person_id,
                location_id,
                ses_api_person_id,
                registration_number.as_deref().unwrap_or("-"),
                missing_since
            );
        }
        PlannedChange::ClearMissing {
            person_id,
            location_id,
        } => {
            println!(
                "[{mode}] clear missing marker id={} location={}",
                person_id, location_id
            );
        }
        PlannedChange::SoftDelete {
            person_id,
            location_id,
            ses_api_person_id,
            registration_number,
        } => {
            println!(
                "[{mode}] soft-delete person id={} location={} sesApiPersonId={} registrationNumber={}",
                person_id,
                location_id,
                ses_api_person_id,
                registration_number.as_deref().unwrap_or("-")
            );
        }
    }
}

#[derive(Debug)]
struct SesPersonWorkItem {
    ses_api_person_id: String,
    registration_number: String,
    first_name: String,
    last_name: String,
    unit_ses_id: Option<i64>,
}

/// One location's SES payload, reduced to the work items the planner acts on plus the
/// record of everyone the payload mentioned at all.
#[derive(Debug, Default)]
struct ParsedPayload {
    items: Vec<SesPersonWorkItem>,
    /// Every SES person id in the payload, recorded before any skip below it.
    present_ses_ids: HashSet<String>,
    /// Every non-empty registration number in the payload, likewise.
    present_registration_numbers: HashSet<String>,
    /// Rows as returned by SES, before any filtering. Backs the unusable-payload guard.
    raw_count: usize,
}

/// Reduce a location's SES payload to work items.
///
/// The two `present_*` sets are the exemption sets for the absence pass and are populated
/// at the very top of the loop, above every skip: anyone SES mentioned — even in a row we
/// then discard as malformed, duplicated, foreign or flagged deleted — must never become a
/// deletion candidate. Both sets are needed because the two identifiers fail
/// independently: a row with a null `id` still carries a usable registration number, and a
/// row whose registration number collides with a local person holding a *different* SES id
/// would otherwise leave that local person unexempted.
fn parse_ses_payload(
    location_id: &str,
    headquarters_id: &str,
    ses_people: &[SesPerson],
    stats: &mut RunStats,
) -> Result<ParsedPayload> {
    let mut ses_items_by_registration_number: HashMap<String, SesPersonWorkItem> = HashMap::new();
    let mut seen_ses_ids: HashSet<String> = HashSet::new();
    let mut present_ses_ids: HashSet<String> = HashSet::new();
    let mut present_registration_numbers: HashSet<String> = HashSet::new();

    for ses_person in ses_people {
        stats.ses_people_seen += 1;

        if let Some(raw) = ses_person
            .registration_number
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            present_registration_numbers.insert(raw.to_string());
        }

        let Some(ses_id_raw) = ses_person.id else {
            warn!(
                "Skipping SES person for location={} because id is null: {}",
                location_id, ses_person
            );
            continue;
        };
        let ses_api_person_id = ses_id_raw.to_string();
        present_ses_ids.insert(ses_api_person_id.clone());

        // skip SES people that are present in this headquarters but it is not their primary headquarters
        if let Some(primary_headquarters_id) = ses_person.headquarters_id()
            && primary_headquarters_id.to_string() != headquarters_id
        {
            info!(
                "Skipping SES person for location={} because primary headquarters {} does not match location headquarters {}: {}",
                location_id, primary_headquarters_id, headquarters_id, ses_person,
            );
            continue;
        }

        // SES is not expected to hand back deleted people at all — departures show up as
        // absence from the payload, which the absence pass handles. Surface it loudly and
        // otherwise leave the local record alone.
        if ses_person.deleted.unwrap_or(false) {
            stats.ses_deleted_flags_seen += 1;
            warn!(
                "SES reports deleted=true for location={} sesApiPersonId={} registrationNumber={} — taking no action: {}",
                location_id,
                ses_api_person_id,
                ses_person.registration_number.as_deref().unwrap_or("-"),
                ses_person,
            );
            continue;
        }

        if !seen_ses_ids.insert(ses_api_person_id.clone()) {
            warn!(
                "Duplicate SES person in payload for location={} with sesApiPersonId={}: {}",
                location_id, ses_api_person_id, ses_person,
            );
            continue;
        }

        let Some(registration_number_raw) = ses_person.registration_number.as_deref() else {
            warn!(
                "Skipping SES person for location={} because registrationNumber is null: {}",
                location_id, ses_person,
            );
            continue;
        };

        let registration_number = registration_number_raw.trim().to_string();
        if registration_number.is_empty() {
            warn!(
                "Skipping SES person for location={} because registrationNumber is empty: {}",
                location_id, ses_person,
            );
            continue;
        }

        let (first_name, last_name) = normalize_names(ses_person)?;

        let new_item = SesPersonWorkItem {
            ses_api_person_id,
            registration_number: registration_number.clone(),
            first_name,
            last_name,
            unit_ses_id: ses_person.headquarters_id(),
        };

        if let Some(existing_item) = ses_items_by_registration_number.get_mut(&registration_number)
        {
            let existing_ses_id = existing_item
                .ses_api_person_id
                .parse::<i64>()
                .with_context(|| {
                    format!(
                        "Invalid SES API person id '{}' for registrationNumber={} in location={}",
                        existing_item.ses_api_person_id, registration_number, location_id
                    )
                })?;

            let new_ses_id = new_item.ses_api_person_id.parse::<i64>().with_context(|| {
                format!(
                    "Invalid SES API person id '{}' for registrationNumber={} in location={}",
                    new_item.ses_api_person_id, registration_number, location_id
                )
            })?;

            if new_ses_id < existing_ses_id {
                warn!(
                    "Duplicate SES registrationNumber in payload for location={} registrationNumber={} keeping lower sesApiPersonId={} and discarding sesApiPersonId={} ({} {} -> {} {})",
                    location_id,
                    registration_number,
                    new_ses_id,
                    existing_ses_id,
                    new_item.first_name,
                    new_item.last_name,
                    existing_item.first_name,
                    existing_item.last_name,
                );
                *existing_item = new_item;
            } else {
                warn!(
                    "Duplicate SES registrationNumber in payload for location={} registrationNumber={} keeping lower sesApiPersonId={} and discarding sesApiPersonId={} ({} {} -> {} {})",
                    location_id,
                    registration_number,
                    existing_ses_id,
                    new_ses_id,
                    existing_item.first_name,
                    existing_item.last_name,
                    new_item.first_name,
                    new_item.last_name,
                );
            }

            continue;
        }

        ses_items_by_registration_number.insert(registration_number, new_item);
    }

    Ok(ParsedPayload {
        items: ses_items_by_registration_number.into_values().collect(),
        present_ses_ids,
        present_registration_numbers,
        raw_count: ses_people.len(),
    })
}

fn build_plans_for_location(
    location_id: &str,
    ses_items: &[SesPersonWorkItem],
    people_by_id: &HashMap<String, db::Person>,
    person_id_by_ses_id: &HashMap<String, String>,
    person_id_by_registration_number: &HashMap<String, String>,
    adopt: bool,
    stats: &mut RunStats,
) -> Result<Vec<PlannedChange>> {
    let mut plans = Vec::new();

    for item in ses_items {
        if let Some(existing_person_id) = person_id_by_ses_id.get(&item.ses_api_person_id) {
            let existing = people_by_id.get(existing_person_id).ok_or_else(|| {
                anyhow!(
                    "Mapped person id {} missing in batch fetched records",
                    existing_person_id
                )
            })?;

            let needs_update = existing.deleted.is_some()
                || existing.first_name != item.first_name
                || existing.last_name != item.last_name
                || existing.registration_number.as_deref()
                    != Some(item.registration_number.as_str())
                || existing.location_id != location_id;

            if !needs_update {
                stats.noops += 1;
                continue;
            }

            if existing.deleted.is_some() {
                plans.push(PlannedChange::UndeleteAndUpdate {
                    person_id: existing.id.clone(),
                    location_id: location_id.to_string(),
                    current_location_id: existing.location_id.clone(),
                    ses_api_person_id: item.ses_api_person_id.clone(),
                    registration_number: item.registration_number.clone(),
                    first_name: item.first_name.clone(),
                    current_first_name: existing.first_name.clone(),
                    last_name: item.last_name.clone(),
                    current_last_name: existing.last_name.clone(),
                });
            } else {
                plans.push(PlannedChange::Update {
                    person_id: existing.id.clone(),
                    location_id: location_id.to_string(),
                    current_location_id: existing.location_id.clone(),
                    ses_api_person_id: item.ses_api_person_id.clone(),
                    registration_number: item.registration_number.clone(),
                    first_name: item.first_name.clone(),
                    current_first_name: existing.first_name.clone(),
                    last_name: item.last_name.clone(),
                    current_last_name: existing.last_name.clone(),
                });
            }
            continue;
        }

        if let Some(existing_member_id) =
            person_id_by_registration_number.get(&item.registration_number)
        {
            let existing = people_by_id.get(existing_member_id).ok_or_else(|| {
                anyhow!(
                    "Mapped person id {} missing in batch fetched records",
                    existing_member_id
                )
            })?;

            match existing.ses_api_person_id.as_deref() {
                None => {
                    if adopt {
                        plans.push(PlannedChange::AdoptSesApiPersonId {
                            person_id: existing.id.clone(),
                            location_id: location_id.to_string(),
                            ses_api_person_id: item.ses_api_person_id.clone(),
                            registration_number: item.registration_number.clone(),
                        });
                        continue;
                    }

                    print_message(&format!(
                        "SKIP location={} registrationNumber={} because local member id={} has no ses_api_person_id {} {} => {} {} ({})",
                        location_id,
                        item.registration_number,
                        existing.id,
                        item.first_name,
                        item.last_name,
                        existing.first_name,
                        existing.last_name,
                        existing.location_id
                    ));
                    stats.blocked_manual_conflicts += 1;
                    continue;
                }
                Some(existing_ses_id) if existing_ses_id != item.ses_api_person_id => {
                    print_message(&format!(
                        "SKIP location={} registrationNumber={} because local member id={} has different ses_api_person_id={} (SES has {})",
                        location_id,
                        item.registration_number,
                        existing.id,
                        existing_ses_id,
                        item.ses_api_person_id
                    ));
                    stats.blocked_manual_conflicts += 1;
                    continue;
                }
                Some(_) => {}
            }
        }

        if let Some(unit_id) = item.unit_ses_id
            && BLOCKED_UNIT_SES_IDS.contains(&unit_id)
        {
            info!(
                "Skipping create for location={} registrationNumber={} because unit {} is blocked",
                location_id, item.registration_number, unit_id,
            );
            stats.noops += 1;
            continue;
        }

        plans.push(PlannedChange::Create {
            location_id: location_id.to_string(),
            ses_api_person_id: item.ses_api_person_id.clone(),
            registration_number: item.registration_number.clone(),
            first_name: item.first_name.clone(),
            last_name: item.last_name.clone(),
        });
    }

    Ok(plans)
}

/// What to do about a person absent from their location's SES payload.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum MissingDecision {
    /// No marker yet — start the clock.
    Mark,
    /// Marker still inside the grace window. Emit nothing: re-stamping would reset the
    /// clock on every run and the person would never be deleted.
    Wait,
    Delete,
}

fn classify_missing(marker: Option<u64>, now: u64, grace_secs: u64) -> MissingDecision {
    match marker {
        None => MissingDecision::Mark,
        // saturating_sub so a marker stamped in the future (clock skew) waits rather
        // than underflowing into an immediate delete.
        Some(t) if now.saturating_sub(t) >= grace_secs => MissingDecision::Delete,
        Some(_) => MissingDecision::Wait,
    }
}

/// Integer arithmetic throughout — a float percentage would truncate unpredictably.
fn absence_candidate_cap(synced_roster: usize, policy: &AbsencePolicy) -> usize {
    std::cmp::max(
        policy.min_candidates,
        synced_roster * policy.max_candidate_percent / 100,
    )
}

/// Why the absence pass declined to mark or delete anyone at a location.
#[derive(Debug, PartialEq, Eq)]
enum AbsenceSkip {
    Disabled,
    /// SES returned rows but none survived parsing into work items. A payload with *no*
    /// rows is not this case — plenty of units are legitimately empty in SES, so those run
    /// the pass normally and rely on the candidate cap and staleness guards below.
    NoUsableItems,
    /// More of the roster has vanished from SES than the cap allows. Fatal outside dry-run
    /// — see `absence_skip_is_fatal`.
    OverCap {
        candidates: usize,
        cap: usize,
        synced_roster: usize,
    },
}

/// Whether a suppressed absence pass should abort the location's sync outright.
///
/// Only `OverCap` is fatal. It means the local roster and the SES payload have diverged so
/// far that we cannot distinguish a genuine mass departure from a bad payload, so the run
/// refuses to touch the location at all — no creates, no updates, no last-sync stamp — and
/// returns an error. In the lambda that means SQS retries and then the DLQ, which is the
/// alarm; a human decides whether the departure is real. The other variants are ordinary
/// conditions that only stand the pass down, and dry-run never aborts so that a review pass
/// still reports on every remaining location.
fn absence_skip_is_fatal(skip: &AbsenceSkip, dry_run: bool) -> bool {
    !dry_run && matches!(skip, AbsenceSkip::OverCap { .. })
}

#[derive(Debug, Default)]
struct AbsenceOutcome {
    /// Clears first, then marks, then deletes — least destructive first, so a partial
    /// `apply_changes` failure leaves the safe half applied.
    changes: Vec<PlannedChange>,
    marked: usize,
    cleared: usize,
    deleted: usize,
    waiting: usize,
    deletes_suppressed: usize,
    skipped: Option<AbsenceSkip>,
}

struct AbsenceInput<'a> {
    location_id: &'a str,
    /// Non-deleted local rows at this location.
    roster: &'a [db::Person],
    present_ses_ids: &'a HashSet<String>,
    present_registration_numbers: &'a HashSet<String>,
    ses_payload_raw_count: usize,
    ses_usable_item_count: usize,
    /// Output of `build_plans_for_location`, used to clear markers on people this run is
    /// moving *into* the location (they are not in its roster snapshot yet).
    plans: &'a [PlannedChange],
    people_by_id: &'a HashMap<String, db::Person>,
    /// The location's previous successful sync, before this run updates it.
    last_successful_sync: Option<u64>,
    now: u64,
    policy: AbsencePolicy,
}

/// Plan the marker and deletion work for members who have stopped appearing in SES.
///
/// Clearing runs even when a guard suppresses the rest of the pass: dropping a stale
/// marker is always safe and always desirable.
fn plan_absence_changes(input: AbsenceInput<'_>) -> AbsenceOutcome {
    let AbsenceInput {
        location_id,
        roster,
        present_ses_ids,
        present_registration_numbers,
        ses_payload_raw_count,
        ses_usable_item_count,
        plans,
        people_by_id,
        last_successful_sync,
        now,
        policy,
    } = input;

    let seen_in_ses = |p: &db::Person| -> bool {
        p.ses_api_person_id
            .as_deref()
            .is_some_and(|id| present_ses_ids.contains(id))
            || p.registration_number
                .as_deref()
                .is_some_and(|rn| present_registration_numbers.contains(rn))
    };

    let mut outcome = AbsenceOutcome::default();
    let mut cleared_ids: HashSet<&str> = HashSet::new();
    let mut clears: Vec<PlannedChange> = Vec::new();

    // C1, roster-driven. This must fire on the steady-state no-op path, not just where a
    // plan was produced: if a transfer is marked at the old location off a lagged GSI read
    // and then drops out of that location's roster, only the new location ever sees the
    // person again, and it usually has nothing to update.
    for person in roster {
        if person.missing_since.is_some() && seen_in_ses(person) && cleared_ids.insert(&person.id) {
            clears.push(PlannedChange::ClearMissing {
                person_id: person.id.clone(),
                location_id: location_id.to_string(),
            });
        }
    }

    // C2, plan-driven: covers people being moved into this location, who are not in its
    // roster snapshot.
    for change in plans {
        let person_id = match change {
            PlannedChange::AdoptSesApiPersonId { person_id, .. }
            | PlannedChange::Update { person_id, .. }
            | PlannedChange::UndeleteAndUpdate { person_id, .. } => person_id,
            _ => continue,
        };
        let Some(person) = people_by_id.get(person_id) else {
            continue;
        };
        if person.missing_since.is_some() && cleared_ids.insert(&person.id) {
            clears.push(PlannedChange::ClearMissing {
                person_id: person.id.clone(),
                location_id: location_id.to_string(),
            });
        }
    }

    outcome.cleared = clears.len();
    outcome.changes = clears;

    if !policy.enabled {
        outcome.skipped = Some(AbsenceSkip::Disabled);
        return outcome;
    }
    // Rows that all fail to parse point at a malformed payload or a misconfigured
    // headquarters id, so the pass stands down. A payload with no rows at all is taken at
    // face value: many units are genuinely empty in SES, and the candidate cap plus the
    // staleness guard below carry the protection for anything larger than a handful.
    if ses_payload_raw_count > 0 && ses_usable_item_count == 0 {
        outcome.skipped = Some(AbsenceSkip::NoUsableItems);
        return outcome;
    }

    // Only rows we have previously matched to SES can be judged absent from it. Members
    // created by hand and never adopted have no SES identity to be missing from.
    let synced_roster = roster
        .iter()
        .filter(|p| p.ses_api_person_id.is_some())
        .count();
    let candidates: Vec<&db::Person> = roster
        .iter()
        .filter(|p| p.ses_api_person_id.is_some() && !seen_in_ses(p))
        .collect();

    let cap = absence_candidate_cap(synced_roster, &policy);
    // Fatal for the location outside dry-run — the caller turns this into an error rather
    // than merely standing the pass down. See `absence_skip_is_fatal`.
    if candidates.len() > cap {
        outcome.skipped = Some(AbsenceSkip::OverCap {
            candidates: candidates.len(),
            cap,
            synced_roster,
        });
        return outcome;
    }

    // A marker records one observation plus a clock. If the location has not synced
    // recently, every marker looks stale on the recovery run and the whole roster would
    // delete on a single sighting — so hold deletions until sync is healthy again.
    let sync_is_fresh = last_successful_sync
        .is_some_and(|t| now.saturating_sub(t) <= policy.max_sync_staleness_secs);

    let mut marks: Vec<PlannedChange> = Vec::new();
    let mut deletes: Vec<PlannedChange> = Vec::new();

    for person in candidates {
        match classify_missing(person.missing_since, now, policy.grace_secs) {
            MissingDecision::Mark => marks.push(PlannedChange::MarkMissing {
                person_id: person.id.clone(),
                location_id: location_id.to_string(),
                ses_api_person_id: person.ses_api_person_id.clone().unwrap_or_default(),
                registration_number: person.registration_number.clone(),
                missing_since: now,
            }),
            MissingDecision::Wait => outcome.waiting += 1,
            MissingDecision::Delete => {
                if sync_is_fresh {
                    deletes.push(PlannedChange::SoftDelete {
                        person_id: person.id.clone(),
                        location_id: location_id.to_string(),
                        ses_api_person_id: person.ses_api_person_id.clone().unwrap_or_default(),
                        registration_number: person.registration_number.clone(),
                    });
                } else {
                    outcome.deletes_suppressed += 1;
                }
            }
        }
    }

    outcome.marked = marks.len();
    outcome.deleted = deletes.len();
    outcome.changes.extend(marks);
    outcome.changes.extend(deletes);
    outcome
}

/// The `max_mutations` tripwire: "SES handed us garbage and we are about to rewrite the
/// member database".
///
/// Called twice per location — once for the member changes before they are applied, then
/// again with that location's email updates folded into the running total — so the ceiling
/// still governs the run as a whole even though the two are applied in separate phases.
/// The consequence of the split is deliberate: a location whose member changes fit but
/// whose emails push it over now commits the member changes and aborts before the emails,
/// rather than abandoning both.
fn check_max_mutations(applied_so_far: usize, planned: usize, max_mutations: usize) -> Result<()> {
    if applied_so_far + planned > max_mutations {
        return Err(anyhow!(
            "Aborting sync: planned mutations exceed max_mutations (current_total={} planned_for_location={} max_mutations={})",
            applied_so_far,
            planned,
            max_mutations
        ));
    }
    Ok(())
}

async fn apply_changes<H: db::Handler>(
    db: &H,
    changes: &[PlannedChange],
    dry_run: bool,
) -> Result<()> {
    for change in changes {
        print_planned_change(change, dry_run);

        if dry_run {
            continue;
        }

        match change {
            PlannedChange::AdoptSesApiPersonId {
                person_id,
                ses_api_person_id,
                ..
            } => {
                db.update_person(
                    person_id,
                    db::PersonUpdateShape::SesApiPersonId {
                        ses_api_person_id: Some(ses_api_person_id),
                    },
                )
                .await
                .with_context(|| {
                    format!(
                        "Adopting ses_api_person_id for person id={} sesApiPersonId={}",
                        person_id, ses_api_person_id
                    )
                })?;
            }
            PlannedChange::Create {
                location_id,
                ses_api_person_id,
                registration_number,
                first_name,
                last_name,
            } => {
                let person = db
                    .create_person(location_id, first_name, last_name, registration_number)
                    .await
                    .with_context(|| {
                        format!(
                            "Creating person for location={} registrationNumber={}",
                            location_id, registration_number
                        )
                    })?;
                db.update_person(
                    &person.id,
                    db::PersonUpdateShape::SesApiPersonId {
                        ses_api_person_id: Some(ses_api_person_id),
                    },
                )
                .await
                .with_context(|| {
                    format!(
                        "Setting ses_api_person_id for person id={} sesApiPersonId={}",
                        person.id, ses_api_person_id
                    )
                })?;
            }
            PlannedChange::Update {
                person_id,
                location_id,
                registration_number,
                first_name,
                last_name,
                ..
            } => {
                db.update_person(person_id, db::PersonUpdateShape::Location { location_id })
                    .await
                    .with_context(|| {
                        format!(
                            "Updating person location id={} location={}",
                            person_id, location_id
                        )
                    })?;
                db.update_person(
                    person_id,
                    db::PersonUpdateShape::Fields {
                        first_name,
                        last_name,
                        registration_number,
                    },
                )
                .await
                .with_context(|| {
                    format!(
                        "Updating person id={} registrationNumber={}",
                        person_id, registration_number
                    )
                })?;
            }
            PlannedChange::UndeleteAndUpdate {
                person_id,
                location_id,
                registration_number,
                first_name,
                last_name,
                ..
            } => {
                db.update_person(person_id, db::PersonUpdateShape::Undelete)
                    .await
                    .with_context(|| format!("Undeleting person id={}", person_id))?;
                db.update_person(person_id, db::PersonUpdateShape::Location { location_id })
                    .await
                    .with_context(|| {
                        format!(
                            "Updating undeleted person location id={} location={}",
                            person_id, location_id
                        )
                    })?;
                db.update_person(
                    person_id,
                    db::PersonUpdateShape::Fields {
                        first_name,
                        last_name,
                        registration_number,
                    },
                )
                .await
                .with_context(|| {
                    format!(
                        "Updating undeleted person id={} registrationNumber={}",
                        person_id, registration_number
                    )
                })?;
            }
            PlannedChange::MarkMissing {
                person_id,
                missing_since,
                ..
            } => {
                db.update_person(
                    person_id,
                    db::PersonUpdateShape::MissingSince {
                        missing_since: Some(*missing_since),
                    },
                )
                .await
                .with_context(|| {
                    format!(
                        "Marking person id={} missing since {}",
                        person_id, missing_since
                    )
                })?;
            }
            PlannedChange::ClearMissing { person_id, .. } => {
                db.update_person(
                    person_id,
                    db::PersonUpdateShape::MissingSince {
                        missing_since: None,
                    },
                )
                .await
                .with_context(|| format!("Clearing missing marker for person id={}", person_id))?;
            }
            PlannedChange::SoftDelete { person_id, .. } => {
                db.update_person(person_id, db::PersonUpdateShape::Delete)
                    .await
                    .with_context(|| format!("Soft deleting person id={}", person_id))?;
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
struct PlannedEmailUpdate {
    person_id: String,
    registration_number: String,
    current_email: Option<String>,
    new_email: String,
}

/// Read-only: looks up member emails for a location's unit via the SES search API and diffs
/// them against local `Person` rows by `registration_number`. Does not write anything.
async fn plan_email_updates<H: db::Handler>(
    db: &H,
    search_client: &SesSearchClient,
    location: &db::Location,
    stats: &mut RunStats,
) -> Result<Vec<PlannedEmailUpdate>> {
    let results = search_client
        .fetch_unit_members(&location.name)
        .await
        .with_context(|| {
            format!(
                "Fetching SES directory search results for location={} unit='{}'",
                location.id, location.name
            )
        })?;

    let mut updates = Vec::new();

    for result in &results {
        let Some(registration_number) = result
            .id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };

        let Some(new_email) = result.email() else {
            continue;
        };

        stats.emails_seen += 1;

        let matches = db
            .get_person_id_by_registration_number(registration_number)
            .await
            .with_context(|| {
                format!(
                    "Lookup local person by registration number for email sync location={}",
                    location.id
                )
            })?;
        let Some(person_id) = db::at_most_one(matches, || {
            format!(
                "Multiple people share registration number {} (location={})",
                registration_number, location.id
            )
        })?
        else {
            stats.emails_unmatched += 1;
            continue;
        };

        let existing = db
            .get_persons(&[person_id.as_str()])
            .await
            .with_context(|| format!("Fetching person id={} for email sync", person_id))?
            .into_iter()
            .flatten()
            .next();

        let Some(existing) = existing else {
            stats.emails_unmatched += 1;
            continue;
        };

        if existing.email.as_deref() == Some(new_email) {
            stats.emails_noops += 1;
            continue;
        }

        updates.push(PlannedEmailUpdate {
            person_id,
            registration_number: registration_number.to_string(),
            current_email: existing.email.clone(),
            new_email: new_email.to_string(),
        });
    }

    Ok(updates)
}

async fn apply_email_updates<H: db::Handler>(
    db: &H,
    updates: &[PlannedEmailUpdate],
    location_id: &str,
    dry_run: bool,
) -> Result<()> {
    let mode = if dry_run { "DRY-RUN" } else { "APPLY" };

    for update in updates {
        println!(
            "[{mode}] update person email id={} location={} registrationNumber={} email={:?}=>{:?}",
            update.person_id,
            location_id,
            update.registration_number,
            update.current_email,
            update.new_email
        );

        if dry_run {
            continue;
        }

        db.update_person(
            &update.person_id,
            db::PersonUpdateShape::Email {
                email: Some(&update.new_email),
            },
        )
        .await
        .with_context(|| format!("Updating email for person id={}", update.person_id))?;
    }

    Ok(())
}

pub async fn run(config: SyncConfig) -> Result<RunStats> {
    if config.page_limit == 0 {
        return Err(anyhow!("SES_PAGE_LIMIT must be greater than 0"));
    }

    let ses_client = SesClient::new(
        config.ses_api_base_url,
        config.ses_api_key,
        config.page_limit,
        config.max_retries,
    )?;

    let search_client = SesSearchClient::new(
        config.ses_intranet_search_api_base_url,
        config.ses_intranet_search_api_key,
        config.max_retries,
    )?;

    let db = dynamodb::Handler::new(&config.db_prefix, false).await;
    let all_locations = db
        .list_locations(db::ListLocationsFilter::EnabledOnly)
        .await
        .context("Listing locations")?;
    let location_filter = build_location_filter(&config.location_ids);

    let mut stats = RunStats::default();

    for location in all_locations {
        if !location_filter.is_empty() && !location_filter.contains(&location.id) {
            stats.skipped_locations += 1;
            continue;
        }

        let Some(headquarters_id) = location.ses_api_headquarters_id.as_deref() else {
            stats.skipped_locations += 1;
            continue;
        };

        let headquarters_id = headquarters_id.trim();
        if headquarters_id.is_empty() {
            stats.skipped_locations += 1;
            continue;
        }

        stats.processed_locations += 1;

        let sync_start_time = crate::clock::now_sec();

        info!(
            "Syncing location={} name='{}' ses_api_headquarters_id={} dry_run={}",
            location.id, location.name, headquarters_id, config.dry_run
        );

        let ses_people = ses_client
            .fetch_people_for_headquarters(headquarters_id)
            .await
            .with_context(|| {
                format!(
                    "Fetching SES people for location={} headquarters_id={}",
                    location.id, headquarters_id
                )
            })?;

        let parsed = parse_ses_payload(&location.id, headquarters_id, &ses_people, &mut stats)?;
        let ses_items = &parsed.items;

        // iterating here is not ideal but Dynamo gives us no choice
        let mut person_id_by_ses_id: HashMap<String, String> = HashMap::new();
        for item in ses_items {
            let matches = db
                .get_person_id_by_ses_api_person_id(&item.ses_api_person_id)
                .await
                .with_context(|| {
                    format!("Lookup local person by SES ID for location={}", location.id)
                })?;
            if let Some(id) = crate::db::at_most_one(matches, || {
                format!(
                    "Multiple people share ses_api_person_id {} (location={})",
                    item.ses_api_person_id, location.id
                )
            })? {
                person_id_by_ses_id.insert(item.ses_api_person_id.clone(), id);
            }
        }

        // iterating here is not ideal but Dynamo gives us no choice
        let mut person_id_by_registration_number: HashMap<String, String> = HashMap::new();
        for item in ses_items {
            let matches = db
                .get_person_id_by_registration_number(&item.registration_number)
                .await
                .with_context(|| {
                    format!(
                        "Lookup local person by registration number for location={}",
                        location.id
                    )
                })?;
            if let Some(id) = crate::db::at_most_one(matches, || {
                format!(
                    "Multiple people share registration number {} (location={})",
                    item.registration_number, location.id
                )
            })? {
                person_id_by_registration_number.insert(item.registration_number.clone(), id);
            }
        }

        let mut unique_person_ids = HashSet::new();
        unique_person_ids.extend(person_id_by_ses_id.values().cloned());
        unique_person_ids.extend(person_id_by_registration_number.values().cloned());

        let person_id_vec: Vec<String> = unique_person_ids.into_iter().collect();
        let person_id_refs: Vec<&str> = person_id_vec.iter().map(|s| s.as_str()).collect();
        let existing_people = if person_id_refs.is_empty() {
            vec![]
        } else {
            db.get_persons(&person_id_refs).await.with_context(|| {
                format!(
                    "Batch fetch existing people rows for location={}",
                    location.id
                )
            })?
        };
        let people_by_id: HashMap<String, db::Person> = existing_people
            .into_iter()
            .flatten()
            .map(|p| (p.id.clone(), p))
            .collect();

        let mut plans = build_plans_for_location(
            &location.id,
            ses_items,
            &people_by_id,
            &person_id_by_ses_id,
            &person_id_by_registration_number,
            config.adopt,
            &mut stats,
        )?;

        // Read the roster before applying anything. The pre- and post-apply snapshots are
        // equivalent for picking candidates — a location's own sync only ever moves people
        // *in*, and everyone it creates or moves in is by construction exempt — so take the
        // snapshot that is consistent with `people_by_id` and leaves nothing attempted if
        // `apply_changes` later fails.
        let roster = db
            .list_people_for_location(&location.id, true)
            .await
            .with_context(|| format!("Listing local roster for location={}", location.id))?;

        let absence = plan_absence_changes(AbsenceInput {
            location_id: &location.id,
            roster: &roster,
            present_ses_ids: &parsed.present_ses_ids,
            present_registration_numbers: &parsed.present_registration_numbers,
            ses_payload_raw_count: parsed.raw_count,
            ses_usable_item_count: parsed.items.len(),
            plans: &plans,
            people_by_id: &people_by_id,
            last_successful_sync: location.last_successful_member_sync,
            now: sync_start_time,
            policy: config.absence,
        });

        if let Some(reason) = &absence.skipped {
            // Disabled is the configured default, not an anomaly worth an error log.
            if matches!(reason, AbsenceSkip::Disabled) {
                info!(
                    "Absence pass disabled for location={} name='{}'",
                    location.id, location.name
                );
            } else {
                stats.absence_skipped_locations += 1;

                if let AbsenceSkip::OverCap {
                    candidates,
                    cap,
                    synced_roster,
                } = reason
                {
                    let detail = format!(
                        "location={} name='{}' has {} absence candidate(s), over the cap of {} for a synced roster of {}. SES and the local roster have diverged too far to act on, so no changes are applied for this location at all. Investigate the SES payload for headquarters {}; if the departure is genuine, raise SES_SYNC_ABSENCE_PERCENT / SES_SYNC_ABSENCE_MIN or remove the members by hand",
                        location.id, location.name, candidates, cap, synced_roster, headquarters_id,
                    );
                    if absence_skip_is_fatal(reason, config.dry_run) {
                        return Err(anyhow!("Aborting sync: {}", detail));
                    }
                    error!("[DRY-RUN] would abort sync: {}", detail);
                } else {
                    error!(
                        "Skipping absence pass for location={} name='{}': {:?}",
                        location.id, location.name, reason
                    );
                }
            }
        }
        if absence.deletes_suppressed > 0 {
            error!(
                "Suppressed {} absence deletion(s) for location={} because the previous successful sync is missing or too old (last_successful_member_sync={:?})",
                absence.deletes_suppressed, location.id, location.last_successful_member_sync
            );
        }

        let absence_marked = absence.marked;
        let absence_cleared = absence.cleared;
        let absence_deleted = absence.deleted;
        let absence_waiting = absence.waiting;
        let absence_deletes_suppressed = absence.deletes_suppressed;
        plans.extend(absence.changes);

        let mut adopts = 0usize;
        let mut creates = 0usize;
        let mut updates = 0usize;
        let mut undeletes = 0usize;

        for change in &plans {
            match change {
                PlannedChange::AdoptSesApiPersonId { .. } => adopts += 1,
                PlannedChange::Create { .. } => creates += 1,
                PlannedChange::Update { .. } => updates += 1,
                PlannedChange::UndeleteAndUpdate { .. } => undeletes += 1,
                // Absence writes are counted by the absence pass and deliberately kept out
                // of the max_mutations tripwire — see RunStats::total_mutations.
                PlannedChange::MarkMissing { .. }
                | PlannedChange::ClearMissing { .. }
                | PlannedChange::SoftDelete { .. } => {}
            }
        }

        let member_mutations = adopts + creates + updates + undeletes;
        if !config.dry_run {
            check_max_mutations(
                stats.total_mutations(),
                member_mutations,
                config.max_mutations,
            )?;
        }

        apply_changes(&db, &plans, config.dry_run)
            .await
            .with_context(|| format!("Applying sync changes for location={}", location.id))?;

        // Member stats land as soon as their writes do, so the running total the tripwire
        // reads below reflects what is actually in the database.
        stats.adopts += adopts;
        stats.creates += creates;
        stats.updates += updates;
        stats.undeletes += undeletes;
        stats.soft_deletes += absence_deleted;
        stats.missing_marked += absence_marked;
        stats.missing_cleared += absence_cleared;
        stats.missing_waiting += absence_waiting;
        stats.absence_deletes_suppressed += absence_deletes_suppressed;

        // Stamp before the email phase. The member sync for this location has now
        // succeeded, and a failure in the secondary email API below must not leave the
        // location looking un-synced — that would suppress its absence deletions on the
        // next run and light up the checker lambda's stale-location digest for a reason
        // that has nothing to do with member sync.
        if !config.dry_run {
            db.update_location(
                &location.id,
                db::LocationUpdateShape::LastSyncTime {
                    time: sync_start_time,
                },
            )
            .await
            .with_context(|| format!("Updating last sync time for location={}", location.id))?;
        }

        // Email sync runs last because it depends on a different API with its own
        // credential. It still errors, retries and DLQs on failure — but by then the member
        // changes are committed, so an outage of the secondary API cannot stop every
        // location's primary sync and bury a real single-location failure in the DLQ.
        let email_updates = plan_email_updates(&db, &search_client, &location, &mut stats)
            .await
            .with_context(|| format!("Planning email sync for location={}", location.id))?;

        if !config.dry_run {
            check_max_mutations(
                stats.total_mutations(),
                email_updates.len(),
                config.max_mutations,
            )?;
        }

        apply_email_updates(&db, &email_updates, &location.id, config.dry_run)
            .await
            .with_context(|| format!("Applying email sync for location={}", location.id))?;

        stats.emails_updated += email_updates.len();
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ses_api::SesPersonHeadquarters;

    const NOW: u64 = 1_000_000_000;
    const GRACE: u64 = DEFAULT_ABSENCE_GRACE_SECS;
    const HQ: &str = "42";

    fn person(
        id: &str,
        ses_id: Option<&str>,
        regnum: Option<&str>,
        missing_since: Option<u64>,
    ) -> db::Person {
        db::Person {
            id: id.to_string(),
            location_id: "loc1".to_string(),
            first_name: "First".to_string(),
            last_name: "Last".to_string(),
            registration_number: regnum.map(str::to_string),
            ses_api_person_id: ses_id.map(str::to_string),
            email: None,
            deleted: None,
            missing_since,
            created_at: None,
            updated_at: None,
        }
    }

    fn ses_person(id: Option<i64>, regnum: Option<&str>) -> SesPerson {
        SesPerson {
            id,
            registration_number: regnum.map(str::to_string),
            first_name: Some("First".to_string()),
            last_name: Some("Last".to_string()),
            full_name: None,
            deleted: None,
            headquarters: Some(SesPersonHeadquarters {
                id: Some(HQ.parse().unwrap()),
            }),
        }
    }

    fn set(values: &[&str]) -> HashSet<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    /// Assembles an `AbsenceInput` with everything healthy, so each test only states the
    /// one condition it is about.
    struct Fixture {
        roster: Vec<db::Person>,
        present_ses_ids: HashSet<String>,
        present_registration_numbers: HashSet<String>,
        plans: Vec<PlannedChange>,
        people_by_id: HashMap<String, db::Person>,
        raw_count: usize,
        usable: usize,
        last_sync: Option<u64>,
        policy: AbsencePolicy,
    }

    impl Default for Fixture {
        fn default() -> Self {
            Self {
                roster: vec![],
                present_ses_ids: HashSet::new(),
                present_registration_numbers: HashSet::new(),
                plans: vec![],
                people_by_id: HashMap::new(),
                raw_count: 50,
                usable: 50,
                last_sync: Some(NOW - 3600),
                policy: AbsencePolicy {
                    enabled: true,
                    ..AbsencePolicy::default()
                },
            }
        }
    }

    impl Fixture {
        fn plan(&self) -> AbsenceOutcome {
            plan_absence_changes(AbsenceInput {
                location_id: "loc1",
                roster: &self.roster,
                present_ses_ids: &self.present_ses_ids,
                present_registration_numbers: &self.present_registration_numbers,
                ses_payload_raw_count: self.raw_count,
                ses_usable_item_count: self.usable,
                plans: &self.plans,
                people_by_id: &self.people_by_id,
                last_successful_sync: self.last_sync,
                now: NOW,
                policy: self.policy,
            })
        }
    }

    fn count_marks(outcome: &AbsenceOutcome) -> usize {
        outcome
            .changes
            .iter()
            .filter(|c| matches!(c, PlannedChange::MarkMissing { .. }))
            .count()
    }

    fn count_clears(outcome: &AbsenceOutcome) -> usize {
        outcome
            .changes
            .iter()
            .filter(|c| matches!(c, PlannedChange::ClearMissing { .. }))
            .count()
    }

    fn count_deletes(outcome: &AbsenceOutcome) -> usize {
        outcome
            .changes
            .iter()
            .filter(|c| matches!(c, PlannedChange::SoftDelete { .. }))
            .count()
    }

    // ── classify_missing ────────────────────────────────────────────────────

    #[test]
    fn no_marker_starts_the_clock() {
        assert_eq!(classify_missing(None, NOW, GRACE), MissingDecision::Mark);
    }

    #[test]
    fn fresh_marker_waits() {
        assert_eq!(
            classify_missing(Some(NOW), NOW, GRACE),
            MissingDecision::Wait
        );
    }

    #[test]
    fn marker_exactly_at_grace_deletes() {
        assert_eq!(
            classify_missing(Some(NOW - GRACE), NOW, GRACE),
            MissingDecision::Delete
        );
    }

    #[test]
    fn marker_one_second_short_of_grace_waits() {
        assert_eq!(
            classify_missing(Some(NOW - GRACE + 1), NOW, GRACE),
            MissingDecision::Wait
        );
    }

    /// Clock skew must not underflow into an immediate delete.
    #[test]
    fn marker_in_the_future_waits() {
        assert_eq!(
            classify_missing(Some(NOW + 5000), NOW, GRACE),
            MissingDecision::Wait
        );
    }

    // ── absence_candidate_cap ───────────────────────────────────────────────

    #[test]
    fn cap_uses_floor_for_small_rosters() {
        let policy = AbsencePolicy::default();
        assert_eq!(absence_candidate_cap(0, &policy), 5);
        assert_eq!(absence_candidate_cap(10, &policy), 5);
    }

    /// Pins the shipped default: a 100-member roster may shed 25 in one cycle.
    #[test]
    fn cap_uses_percentage_for_large_rosters() {
        assert_eq!(absence_candidate_cap(100, &AbsencePolicy::default()), 25);
    }

    /// 20% of 26 is 5.2; integer arithmetic must land on 5, not 6. Uses an explicit
    /// percentage rather than the default so it keeps testing the arithmetic if the
    /// shipped cap is retuned.
    #[test]
    fn cap_truncates_rather_than_rounds() {
        let policy = AbsencePolicy {
            max_candidate_percent: 20,
            ..AbsencePolicy::default()
        };
        assert_eq!(absence_candidate_cap(26, &policy), 5);
    }

    // ── guards ──────────────────────────────────────────────────────────────

    #[test]
    fn disabled_policy_plans_nothing() {
        let f = Fixture {
            roster: vec![person("p1", Some("1"), Some("R1"), None)],
            policy: AbsencePolicy::default(),
            ..Default::default()
        };
        let outcome = f.plan();
        assert_eq!(outcome.skipped, Some(AbsenceSkip::Disabled));
        assert_eq!(count_marks(&outcome), 0);
        assert_eq!(count_deletes(&outcome), 0);
    }

    /// A unit that is legitimately empty in SES is common, so an empty payload runs the
    /// pass like any other and leans on the cap for protection.
    #[test]
    fn empty_payload_marks_a_small_roster() {
        let f = Fixture {
            roster: vec![person("p1", Some("1"), Some("R1"), None)],
            raw_count: 0,
            usable: 0,
            ..Default::default()
        };
        let outcome = f.plan();
        assert_eq!(outcome.skipped, None);
        assert_eq!(count_marks(&outcome), 1);
    }

    /// The cap is what keeps an empty payload from wiping a unit big enough for the loss to
    /// look like a fetch failure rather than a disbandment.
    #[test]
    fn empty_payload_still_trips_the_cap_for_a_large_roster() {
        let roster: Vec<db::Person> = (0..100)
            .map(|i| person(&format!("p{i}"), Some(&format!("{i}")), None, None))
            .collect();
        let f = Fixture {
            roster,
            raw_count: 0,
            usable: 0,
            ..Default::default()
        };
        let outcome = f.plan();
        assert_eq!(
            outcome.skipped,
            Some(AbsenceSkip::OverCap {
                candidates: 100,
                cap: 25,
                synced_roster: 100
            })
        );
        assert_eq!(count_marks(&outcome), 0);
    }

    /// A payload whose rows all belong to another headquarters is not the same as one with
    /// no rows — it yields no work items but plenty of exemptions.
    #[test]
    fn payload_with_no_usable_items_plans_nothing() {
        let f = Fixture {
            roster: vec![person("p1", Some("1"), Some("R1"), None)],
            raw_count: 50,
            usable: 0,
            ..Default::default()
        };
        let outcome = f.plan();
        assert_eq!(outcome.skipped, Some(AbsenceSkip::NoUsableItems));
        assert_eq!(count_marks(&outcome), 0);
    }

    #[test]
    fn too_many_candidates_trips_the_cap() {
        let roster: Vec<db::Person> = (0..100)
            .map(|i| person(&format!("p{i}"), Some(&format!("{i}")), None, None))
            .collect();
        // Everyone from index 30 up is still in SES; the first 30 are absent.
        let present_ses_ids: HashSet<String> = (30..100).map(|i| i.to_string()).collect();
        let f = Fixture {
            roster,
            present_ses_ids,
            ..Default::default()
        };
        let outcome = f.plan();
        assert_eq!(
            outcome.skipped,
            Some(AbsenceSkip::OverCap {
                candidates: 30,
                cap: 25,
                synced_roster: 100
            })
        );
        assert_eq!(count_marks(&outcome), 0);
        assert_eq!(count_deletes(&outcome), 0);
    }

    // ── max_mutations tripwire ──────────────────────────────────────────────

    #[test]
    fn mutations_exactly_at_the_ceiling_are_allowed() {
        assert!(check_max_mutations(90, 10, 100).is_ok());
    }

    #[test]
    fn one_mutation_over_the_ceiling_aborts() {
        assert!(check_max_mutations(90, 11, 100).is_err());
    }

    /// The two phases share one ceiling: emails are checked against a running total that
    /// already includes the member changes applied moments earlier.
    #[test]
    fn email_phase_counts_against_the_member_changes_already_applied() {
        let applied_member_mutations = 95;
        assert!(check_max_mutations(applied_member_mutations, 5, 100).is_ok());
        assert!(check_max_mutations(applied_member_mutations, 6, 100).is_err());
    }

    // ── fatal guards ────────────────────────────────────────────────────────

    /// Over-cap is the one guard that aborts the location instead of standing the pass
    /// down, so that the lambda DLQs and a human looks at the payload.
    #[test]
    fn over_cap_aborts_the_location() {
        let over_cap = AbsenceSkip::OverCap {
            candidates: 30,
            cap: 20,
            synced_roster: 100,
        };
        assert!(absence_skip_is_fatal(&over_cap, false));
    }

    /// A review pass must still report on every remaining location.
    #[test]
    fn over_cap_does_not_abort_a_dry_run() {
        let over_cap = AbsenceSkip::OverCap {
            candidates: 30,
            cap: 20,
            synced_roster: 100,
        };
        assert!(!absence_skip_is_fatal(&over_cap, true));
    }

    /// The remaining guards are ordinary conditions; they must not stop the location's
    /// creates and updates.
    #[test]
    fn other_guards_are_not_fatal() {
        assert!(!absence_skip_is_fatal(&AbsenceSkip::Disabled, false));
        assert!(!absence_skip_is_fatal(&AbsenceSkip::NoUsableItems, false));
    }

    /// Clears are unconditionally safe, so a guard must not suppress them.
    #[test]
    fn guards_still_clear_markers() {
        let f = Fixture {
            roster: vec![person("p1", Some("1"), Some("R1"), Some(NOW - 100))],
            present_ses_ids: set(&["1"]),
            raw_count: 50,
            usable: 0,
            ..Default::default()
        };
        let outcome = f.plan();
        assert_eq!(outcome.skipped, Some(AbsenceSkip::NoUsableItems));
        assert_eq!(count_clears(&outcome), 1);
        assert_eq!(outcome.cleared, 1);
    }

    // ── cap boundaries ──────────────────────────────────────────────────────

    #[test]
    fn candidates_exactly_at_cap_are_allowed() {
        let roster: Vec<db::Person> = (0..100)
            .map(|i| person(&format!("p{i}"), Some(&format!("{i}")), None, None))
            .collect();
        // 25 absent out of a synced roster of 100 lands exactly on the cap, which is
        // allowed — only *exceeding* it trips the guard.
        let present: Vec<String> = (25..100).map(|i| i.to_string()).collect();
        let f = Fixture {
            roster,
            present_ses_ids: present.iter().cloned().collect(),
            ..Default::default()
        };
        let outcome = f.plan();
        assert_eq!(outcome.skipped, None);
        assert_eq!(count_marks(&outcome), 25);
    }

    /// A three-person unit losing everyone is within the floor and must still work.
    #[test]
    fn tiny_roster_is_allowed_via_the_floor() {
        let f = Fixture {
            roster: vec![
                person("p1", Some("1"), None, None),
                person("p2", Some("2"), None, None),
                person("p3", Some("3"), None, None),
            ],
            ..Default::default()
        };
        let outcome = f.plan();
        assert_eq!(outcome.skipped, None);
        assert_eq!(count_marks(&outcome), 3);
    }

    // ── marker state machine ────────────────────────────────────────────────

    #[test]
    fn absent_person_without_marker_is_marked() {
        let f = Fixture {
            roster: vec![person("p1", Some("1"), Some("R1"), None)],
            ..Default::default()
        };
        let outcome = f.plan();
        assert_eq!(outcome.marked, 1);
        assert_eq!(
            outcome.changes,
            vec![PlannedChange::MarkMissing {
                person_id: "p1".to_string(),
                location_id: "loc1".to_string(),
                ses_api_person_id: "1".to_string(),
                registration_number: Some("R1".to_string()),
                missing_since: NOW,
            }]
        );
    }

    /// Re-stamping would reset the clock every run, so nothing would ever be deleted.
    #[test]
    fn absent_person_with_fresh_marker_is_left_alone() {
        let f = Fixture {
            roster: vec![person("p1", Some("1"), None, Some(NOW - 100))],
            ..Default::default()
        };
        let outcome = f.plan();
        assert!(outcome.changes.is_empty());
        assert_eq!(outcome.waiting, 1);
        assert_eq!(outcome.marked, 0);
    }

    #[test]
    fn absent_person_past_grace_is_deleted() {
        let f = Fixture {
            roster: vec![person("p1", Some("1"), Some("R1"), Some(NOW - GRACE - 1))],
            ..Default::default()
        };
        let outcome = f.plan();
        assert_eq!(outcome.deleted, 1);
        assert_eq!(
            outcome.changes,
            vec![PlannedChange::SoftDelete {
                person_id: "p1".to_string(),
                location_id: "loc1".to_string(),
                ses_api_person_id: "1".to_string(),
                registration_number: Some("R1".to_string()),
            }]
        );
    }

    /// A location that has never synced cannot have observed anyone as absent.
    #[test]
    fn deletion_suppressed_when_location_never_synced() {
        let f = Fixture {
            roster: vec![person("p1", Some("1"), None, Some(NOW - GRACE - 1))],
            last_sync: None,
            ..Default::default()
        };
        let outcome = f.plan();
        assert_eq!(count_deletes(&outcome), 0);
        assert_eq!(outcome.deletes_suppressed, 1);
    }

    /// Recovering from a long outage, every marker looks stale — delete on more evidence.
    #[test]
    fn deletion_suppressed_when_last_sync_is_stale() {
        let f = Fixture {
            roster: vec![person("p1", Some("1"), None, Some(NOW - GRACE - 1))],
            last_sync: Some(NOW - 40 * 3600),
            ..Default::default()
        };
        let outcome = f.plan();
        assert_eq!(count_deletes(&outcome), 0);
        assert_eq!(outcome.deletes_suppressed, 1);
    }

    // ── exemption and clearing ──────────────────────────────────────────────

    #[test]
    fn present_person_without_marker_costs_no_write() {
        let f = Fixture {
            roster: vec![person("p1", Some("1"), Some("R1"), None)],
            present_ses_ids: set(&["1"]),
            ..Default::default()
        };
        assert!(f.plan().changes.is_empty());
    }

    /// The steady-state no-op path. Without this, a marker stamped off a lagged GSI read
    /// can never be cleared and eventually deletes the person with no grace at all.
    #[test]
    fn present_person_with_marker_is_cleared() {
        let f = Fixture {
            roster: vec![person("p1", Some("1"), Some("R1"), Some(NOW - 100))],
            present_ses_ids: set(&["1"]),
            ..Default::default()
        };
        let outcome = f.plan();
        assert_eq!(
            outcome.changes,
            vec![PlannedChange::ClearMissing {
                person_id: "p1".to_string(),
                location_id: "loc1".to_string(),
            }]
        );
        assert_eq!(outcome.marked, 0);
    }

    /// SES named this registration number under a different person id — the local row is
    /// still demonstrably in SES and must not be deleted.
    #[test]
    fn registration_number_alone_exempts() {
        let f = Fixture {
            roster: vec![person("p1", Some("111"), Some("R1"), Some(NOW - 100))],
            present_ses_ids: set(&["222"]),
            present_registration_numbers: set(&["R1"]),
            ..Default::default()
        };
        let outcome = f.plan();
        assert_eq!(count_marks(&outcome), 0);
        assert_eq!(count_deletes(&outcome), 0);
        assert_eq!(count_clears(&outcome), 1);
    }

    #[test]
    fn person_without_ses_id_is_never_a_candidate() {
        let f = Fixture {
            roster: vec![person("p1", None, Some("R1"), None)],
            ..Default::default()
        };
        assert!(f.plan().changes.is_empty());
    }

    #[test]
    fn transfer_into_location_clears_via_plans() {
        let moved = person("p1", Some("1"), Some("R1"), Some(NOW - 100));
        let mut people_by_id = HashMap::new();
        people_by_id.insert("p1".to_string(), moved);
        let f = Fixture {
            // Not in this location's roster yet — it is being moved in by this run.
            roster: vec![],
            people_by_id,
            plans: vec![PlannedChange::Update {
                person_id: "p1".to_string(),
                location_id: "loc1".to_string(),
                current_location_id: "loc2".to_string(),
                ses_api_person_id: "1".to_string(),
                registration_number: "R1".to_string(),
                first_name: "First".to_string(),
                current_first_name: "First".to_string(),
                last_name: "Last".to_string(),
                current_last_name: "Last".to_string(),
            }],
            present_ses_ids: set(&["1"]),
            ..Default::default()
        };
        let outcome = f.plan();
        assert_eq!(count_clears(&outcome), 1);
    }

    #[test]
    fn clear_is_not_duplicated_when_both_rules_match() {
        let p = person("p1", Some("1"), Some("R1"), Some(NOW - 100));
        let mut people_by_id = HashMap::new();
        people_by_id.insert("p1".to_string(), p.clone());
        let f = Fixture {
            roster: vec![p],
            people_by_id,
            plans: vec![PlannedChange::AdoptSesApiPersonId {
                person_id: "p1".to_string(),
                location_id: "loc1".to_string(),
                ses_api_person_id: "1".to_string(),
                registration_number: "R1".to_string(),
            }],
            present_ses_ids: set(&["1"]),
            ..Default::default()
        };
        assert_eq!(f.plan().cleared, 1);
    }

    /// A Create plan has no `people_by_id` entry; the lookup must not panic.
    #[test]
    fn create_plan_is_ignored_by_the_absence_pass() {
        let f = Fixture {
            plans: vec![PlannedChange::Create {
                location_id: "loc1".to_string(),
                ses_api_person_id: "9".to_string(),
                registration_number: "R9".to_string(),
                first_name: "First".to_string(),
                last_name: "Last".to_string(),
            }],
            ..Default::default()
        };
        assert!(f.plan().changes.is_empty());
    }

    /// Least destructive first, so a partial apply leaves the safe half applied.
    #[test]
    fn changes_are_ordered_clear_then_mark_then_delete() {
        let f = Fixture {
            roster: vec![
                person("seen", Some("1"), None, Some(NOW - 100)),
                person("fresh", Some("2"), None, None),
                person("stale", Some("3"), None, Some(NOW - GRACE - 1)),
            ],
            present_ses_ids: set(&["1"]),
            ..Default::default()
        };
        let outcome = f.plan();
        let kinds: Vec<&str> = outcome
            .changes
            .iter()
            .map(|c| match c {
                PlannedChange::ClearMissing { .. } => "clear",
                PlannedChange::MarkMissing { .. } => "mark",
                PlannedChange::SoftDelete { .. } => "delete",
                _ => "other",
            })
            .collect();
        assert_eq!(kinds, vec!["clear", "mark", "delete"]);
    }

    // ── parse_ses_payload ───────────────────────────────────────────────────

    fn parse(people: &[SesPerson]) -> (ParsedPayload, RunStats) {
        let mut stats = RunStats::default();
        let parsed = parse_ses_payload("loc1", HQ, people, &mut stats).expect("payload parses");
        (parsed, stats)
    }

    #[test]
    fn deleted_flag_is_warned_about_and_exempts() {
        let mut p = ses_person(Some(1), Some("R1"));
        p.deleted = Some(true);
        let (parsed, stats) = parse(&[p]);
        assert!(parsed.items.is_empty());
        assert!(parsed.present_ses_ids.contains("1"));
        assert!(parsed.present_registration_numbers.contains("R1"));
        assert_eq!(stats.ses_deleted_flags_seen, 1);
    }

    #[test]
    fn foreign_headquarters_row_exempts_without_producing_work() {
        let mut p = ses_person(Some(1), Some("R1"));
        p.headquarters = Some(SesPersonHeadquarters { id: Some(999) });
        let (parsed, _) = parse(&[p]);
        assert!(parsed.items.is_empty());
        assert!(parsed.present_ses_ids.contains("1"));
    }

    #[test]
    fn null_registration_number_row_still_exempts_by_ses_id() {
        let (parsed, _) = parse(&[ses_person(Some(1), None)]);
        assert!(parsed.items.is_empty());
        assert!(parsed.present_ses_ids.contains("1"));
    }

    #[test]
    fn blank_registration_number_row_still_exempts_by_ses_id() {
        let (parsed, _) = parse(&[ses_person(Some(1), Some("   "))]);
        assert!(parsed.items.is_empty());
        assert!(parsed.present_ses_ids.contains("1"));
        assert!(parsed.present_registration_numbers.is_empty());
    }

    /// A row SES sent without an id still names a registration number, and the local
    /// person holding it must stay exempt.
    #[test]
    fn null_ses_id_row_exempts_by_registration_number() {
        let (parsed, _) = parse(&[ses_person(None, Some("R1"))]);
        assert!(parsed.items.is_empty());
        assert!(parsed.present_ses_ids.is_empty());
        assert!(parsed.present_registration_numbers.contains("R1"));
    }

    #[test]
    fn duplicate_ses_id_yields_one_item_and_one_exemption() {
        let (parsed, _) = parse(&[
            ses_person(Some(1), Some("R1")),
            ses_person(Some(1), Some("R2")),
        ]);
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(parsed.present_ses_ids.len(), 1);
        assert_eq!(parsed.present_registration_numbers.len(), 2);
    }

    /// The unusable-payload guard reads this, so it must count rows SES sent, not survivors.
    #[test]
    fn raw_count_counts_every_row_including_skipped() {
        let mut foreign = ses_person(Some(2), Some("R2"));
        foreign.headquarters = Some(SesPersonHeadquarters { id: Some(999) });
        let (parsed, _) = parse(&[
            ses_person(Some(1), Some("R1")),
            foreign,
            ses_person(None, None),
        ]);
        assert_eq!(parsed.raw_count, 3);
        assert_eq!(parsed.items.len(), 1);
    }

    // ── planner regressions ─────────────────────────────────────────────────

    fn work_item(ses_id: &str, regnum: &str) -> SesPersonWorkItem {
        SesPersonWorkItem {
            ses_api_person_id: ses_id.to_string(),
            registration_number: regnum.to_string(),
            first_name: "First".to_string(),
            last_name: "Last".to_string(),
            unit_ses_id: Some(HQ.parse().unwrap()),
        }
    }

    /// Rows that used to hit the removed `deleted: true` branch now flow through the
    /// normal create path rather than being stranded.
    #[test]
    fn unmatched_item_creates() {
        let mut stats = RunStats::default();
        let plans = build_plans_for_location(
            "loc1",
            &[work_item("1", "R1")],
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            false,
            &mut stats,
        )
        .expect("plans build");
        assert_eq!(plans.len(), 1);
        assert!(matches!(plans[0], PlannedChange::Create { .. }));
    }

    #[test]
    fn unchanged_person_is_a_noop() {
        let existing = person("p1", Some("1"), Some("R1"), None);
        let mut people_by_id = HashMap::new();
        people_by_id.insert("p1".to_string(), existing);
        let mut by_ses_id = HashMap::new();
        by_ses_id.insert("1".to_string(), "p1".to_string());

        let mut stats = RunStats::default();
        let plans = build_plans_for_location(
            "loc1",
            &[work_item("1", "R1")],
            &people_by_id,
            &by_ses_id,
            &HashMap::new(),
            false,
            &mut stats,
        )
        .expect("plans build");
        assert!(plans.is_empty());
        assert_eq!(stats.noops, 1);
    }
}
