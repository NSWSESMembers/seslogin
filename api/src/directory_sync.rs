//! Directory sync: members' contact details and unit assignments from the SES intranet
//! directory.
//!
//! One run fetches the whole directory (every person, ~13 requests) and every entity,
//! matches people to local members by registration number, and plans:
//!
//! - a [`db::PersonUpdateShape::Directory`] write when a member's email, mobile, phones
//!   or preferred name differ from the directory's, and
//! - a [`db::Handler::replace_person_units`] when the entities they are assigned to, or
//!   what they hold at them, differ from the `person_unit` rows.
//!
//! Nothing is cleared on absence. A local member missing from the directory keeps
//! everything; a matched member whose record lists no email (or no phone) keeps the stored
//! one. Units *are* replaced to exactly what the record lists, an empty list included,
//! because a record is complete for its person. `person_unit` rows of a member who is no
//! longer live (soft-deleted, or gone) are removed.
//!
//! Every write is planned before any is made, and the run aborts before writing anything
//! when the plan exceeds `max_mutations`.

use crate::audit::AuditingHandler;
use crate::db::{self, Handler as _};
use crate::dynamodb;
use crate::ses_api::{SesAssignment, SesContactDetail, SesSearchClient, SesSearchResult};
use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use tracing::{error, info, warn};

pub const DEFAULT_MAX_MUTATIONS: usize = 500;
/// The directory held 12,232 people when this was written; far fewer means a bad fetch.
pub const DEFAULT_MIN_PEOPLE: usize = 10_000;

#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub dry_run: bool,
    pub ses_intranet_search_api_base_url: String,
    pub ses_intranet_search_api_key: String,
    pub db_prefix: String,
    pub max_retries: usize,
    /// Abort before writing anything when more writes than this are planned.
    pub max_mutations: usize,
    /// Abort when the directory returns fewer people than this.
    pub min_people: usize,
}

#[derive(Default, Debug, Clone)]
pub struct RunStats {
    pub directory_people: usize,
    /// Directory people matched to a live local member.
    pub matched: usize,
    /// Directory people with no live local member (other units' members, staff).
    pub unmatched: usize,
    /// Live local members with a registration number the directory doesn't list. Left
    /// alone.
    pub local_absent: usize,
    /// Registration numbers held by more than one live local member. Skipped.
    pub duplicate_registration_numbers: usize,
    pub contact_updates: usize,
    pub unit_updates: usize,
    /// Members no longer live whose `person_unit` rows are removed.
    pub orphan_unit_deletes: usize,
    /// Matched members assigned to two or more entities.
    pub multi_unit_people: usize,
}

impl RunStats {
    pub fn total_mutations(&self) -> usize {
        self.contact_updates + self.unit_updates + self.orphan_unit_deletes
    }
}

// ── Contact details ──────────────────────────────────────────────────────────

/// Contact types that are phone numbers. Emails and faxes are everything else.
fn is_phone_type(t: &str) -> bool {
    let t = t.to_ascii_lowercase();
    !t.contains("fax")
        && !t.contains("mail")
        && (t.contains("phone") || t.contains("telephone") || t.contains("mobile") || t == "sms")
}

fn number(detail: &SesContactDetail) -> Option<String> {
    let n: String = detail
        .detail
        .as_deref()?
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    (!n.is_empty()).then_some(n)
}

/// The order [`pick_mobile`] tries contact types in. "Primary Telephone" is SES's flagged
/// primary, but it can be a landline, hence the fallbacks.
const MOBILE_PREFERENCE: &[&str] = &[
    "Primary Telephone",
    "Personal Mobile Phone",
    "SMS",
    "Work Mobile Phone",
];

/// The first Australian mobile (`+614…`) among the preferred contact types.
fn pick_mobile(contacts: &[SesContactDetail]) -> Option<String> {
    MOBILE_PREFERENCE.iter().find_map(|wanted| {
        contacts
            .iter()
            .filter(|c| c.detail_type.as_deref() == Some(*wanted))
            .filter_map(number)
            .find(|n| n.starts_with("+614"))
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct Phone {
    number: String,
    types: Vec<String>,
}

/// Every distinct phone number with the contact types that list it, the mobile first and
/// the rest in the order they appear. A type listing the same number twice counts once.
fn dedupe_phones(contacts: &[SesContactDetail], mobile: Option<&str>) -> Vec<Phone> {
    let mut phones: Vec<Phone> = Vec::new();
    for c in contacts {
        let Some(t) = c.detail_type.as_deref().filter(|t| is_phone_type(t)) else {
            continue;
        };
        let Some(n) = number(c) else { continue };
        match phones.iter_mut().find(|p| p.number == n) {
            Some(p) => {
                if !p.types.iter().any(|x| x == t) {
                    p.types.push(t.to_string());
                }
            }
            None => phones.push(Phone {
                number: n,
                types: vec![t.to_string()],
            }),
        }
    }
    if let Some(i) = mobile.and_then(|m| phones.iter().position(|p| p.number == m)) {
        let p = phones.remove(i);
        phones.insert(0, p);
    }
    phones
}

/// What directory sync stores in a member's four directory fields.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Contact {
    email: Option<String>,
    mobile: Option<String>,
    phones: Option<String>,
    preferred_name: Option<String>,
}

impl Contact {
    fn of(p: &db::Person) -> Self {
        Self {
            email: p.email.clone(),
            mobile: p.mobile.clone(),
            phones: p.phones.clone(),
            preferred_name: p.preferred_name.clone(),
        }
    }
}

/// The member's contact fields after this run. An email or phones the record lacks keep
/// the stored values; a blank preferred name clears it.
fn plan_contact(person: &db::Person, record: &SesSearchResult) -> Contact {
    let current = Contact::of(person);
    let mobile = pick_mobile(&record.contact_details);
    let phones = dedupe_phones(&record.contact_details, mobile.as_deref());
    let (mobile, phones) = if phones.is_empty() {
        (current.mobile.clone(), current.phones.clone())
    } else {
        (
            mobile,
            Some(serde_json::to_string(&phones).expect("phones serialize")),
        )
    };
    Contact {
        email: record.email().map(str::to_string).or(current.email.clone()),
        mobile,
        phones,
        preferred_name: record
            .preferred_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    }
}

// ── Assignments ──────────────────────────────────────────────────────────────

fn non_blank(v: Option<&str>) -> Option<String> {
    v.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// A member's assignments grouped into one `person_unit` row per entity, sorted by
/// entity id.
///
/// `entity_codes` maps directory entity ids to their codes, `entity_locations` maps them
/// to our location ids, and `primary_location_id` is `Person.location_id`.
fn group_assignments(
    person_id: &str,
    primary_location_id: &str,
    assignments: &[SesAssignment],
    entity_codes: &HashMap<String, String>,
    entity_locations: &HashMap<String, String>,
) -> Vec<db::PersonUnit> {
    let mut units: BTreeMap<String, db::PersonUnit> = BTreeMap::new();
    for a in assignments {
        let Some(entity_id) = non_blank(a.entity_id.as_deref()) else {
            continue;
        };
        let unit = units.entry(entity_id.clone()).or_insert_with(|| {
            let location_id = entity_locations.get(&entity_id).cloned();
            db::PersonUnit {
                person_id: person_id.to_string(),
                is_primary: location_id.as_deref() == Some(primary_location_id),
                location_id,
                entity_code: entity_codes.get(&entity_id).cloned(),
                entity_id,
                ..Default::default()
            }
        });
        if unit.entity_name.is_none() {
            unit.entity_name = non_blank(a.entity_name.as_deref());
        }
        if unit.zone.is_none() {
            unit.zone = non_blank(a.location.as_deref());
        }
        let Some(detail) = non_blank(a.detail.as_deref()) else {
            continue;
        };
        let list = match a.assignment_type.as_deref() {
            // Nobody holds two at one entity; should it happen, the first one wins.
            Some("Volunteer") => {
                if unit.membership_type.is_none() {
                    unit.membership_type = Some(detail);
                }
                continue;
            }
            Some("Rank") => &mut unit.ranks,
            Some("Role") => &mut unit.roles,
            Some("Position") => &mut unit.positions,
            _ => continue,
        };
        if !list.contains(&detail) {
            list.push(detail);
        }
    }
    units
        .into_values()
        .map(|mut u| {
            u.ranks.sort();
            u.roles.sort();
            u.positions.sort();
            u
        })
        .collect()
}

/// Whether the stored rows differ from the desired ones in anything directory sync
/// writes.
fn units_differ(stored: &[db::PersonUnit], desired: &[db::PersonUnit]) -> bool {
    let mut stored: Vec<&db::PersonUnit> = stored.iter().collect();
    stored.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
    stored.len() != desired.len()
        || stored
            .iter()
            .zip(desired)
            .any(|(s, d)| s.entity_id != d.entity_id || !s.same_content(d))
}

// ── Planning ─────────────────────────────────────────────────────────────────

#[derive(Debug)]
enum PlannedChange {
    Contact {
        person_id: String,
        registration_number: String,
        before: Contact,
        after: Contact,
    },
    Units {
        person_id: String,
        registration_number: String,
        before: Vec<db::PersonUnit>,
        after: Vec<db::PersonUnit>,
    },
    /// Remove the rows of a member who is no longer live.
    OrphanUnits { person_id: String },
}

struct PlanInput<'a> {
    /// Every person row (deleted ones included).
    people: &'a [db::Person],
    stored_units: &'a [db::PersonUnit],
    records: &'a [SesSearchResult],
    entity_codes: &'a HashMap<String, String>,
    entity_locations: &'a HashMap<String, String>,
}

fn plan(input: PlanInput<'_>, stats: &mut RunStats) -> Vec<PlannedChange> {
    let mut units_by_person: HashMap<&str, Vec<db::PersonUnit>> = HashMap::new();
    for u in input.stored_units {
        units_by_person
            .entry(u.person_id.as_str())
            .or_default()
            .push(u.clone());
    }

    let live: Vec<&db::Person> = input
        .people
        .iter()
        .filter(|p| p.deleted.is_none())
        .collect();
    let live_ids: HashSet<&str> = live.iter().map(|p| p.id.as_str()).collect();

    let mut by_registration: HashMap<&str, Vec<&db::Person>> = HashMap::new();
    for p in &live {
        if let Some(rn) = p
            .registration_number
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            by_registration.entry(rn).or_default().push(p);
        }
    }
    by_registration.retain(|rn, people| {
        if people.len() > 1 {
            warn!(
                registration_number = rn,
                people = ?people.iter().map(|p| &p.id).collect::<Vec<_>>(),
                "several live members share this registration number; skipping them"
            );
            stats.duplicate_registration_numbers += 1;
            return false;
        }
        true
    });

    let mut changes = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    stats.directory_people = input.records.len();

    for record in input.records {
        let Some(rn) = record
            .id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            stats.unmatched += 1;
            continue;
        };
        let Some(person) = by_registration.get(rn).map(|p| p[0]) else {
            stats.unmatched += 1;
            continue;
        };
        if !seen.insert(rn) {
            // The fetch's completeness check rules this out; be safe regardless.
            continue;
        }
        stats.matched += 1;

        let before = Contact::of(person);
        let after = plan_contact(person, record);
        if before != after {
            changes.push(PlannedChange::Contact {
                person_id: person.id.clone(),
                registration_number: rn.to_string(),
                before,
                after,
            });
        }

        let desired = group_assignments(
            &person.id,
            &person.location_id,
            &record.assignments,
            input.entity_codes,
            input.entity_locations,
        );
        if desired.len() >= 2 {
            stats.multi_unit_people += 1;
        }
        let stored = units_by_person
            .remove(person.id.as_str())
            .unwrap_or_default();
        if units_differ(&stored, &desired) {
            changes.push(PlannedChange::Units {
                person_id: person.id.clone(),
                registration_number: rn.to_string(),
                before: stored,
                after: desired,
            });
        }
    }

    stats.local_absent = by_registration
        .keys()
        .filter(|rn| !seen.contains(*rn))
        .count();

    // What's left in `units_by_person` belongs to members the directory didn't match:
    // live ones keep theirs, the rest are removed.
    let mut orphans: Vec<&str> = units_by_person
        .keys()
        .copied()
        .filter(|id| !live_ids.contains(id))
        .collect();
    orphans.sort();
    for person_id in orphans {
        changes.push(PlannedChange::OrphanUnits {
            person_id: person_id.to_string(),
        });
    }

    for change in &changes {
        match change {
            PlannedChange::Contact { .. } => stats.contact_updates += 1,
            PlannedChange::Units { .. } => stats.unit_updates += 1,
            PlannedChange::OrphanUnits { .. } => stats.orphan_unit_deletes += 1,
        }
    }
    changes
}

fn units_line(units: &[db::PersonUnit]) -> String {
    if units.is_empty() {
        return "-".to_string();
    }
    units
        .iter()
        .map(|u| {
            let held: Vec<&str> = u
                .membership_type
                .iter()
                .chain(&u.ranks)
                .chain(&u.roles)
                .chain(&u.positions)
                .map(String::as_str)
                .collect();
            format!(
                "{}{}{}: {}",
                u.entity_name.as_deref().unwrap_or(&u.entity_id),
                if u.location_id.is_some() {
                    ""
                } else {
                    " (no location)"
                },
                if u.is_primary { " (primary)" } else { "" },
                held.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn print_planned_change(change: &PlannedChange, dry_run: bool) {
    let mode = if dry_run { "DRY-RUN" } else { "APPLY" };
    match change {
        PlannedChange::Contact {
            person_id,
            registration_number,
            before,
            after,
        } => {
            let mut fields = Vec::new();
            let mut cmp = |name: &str, b: &Option<String>, a: &Option<String>| {
                if b != a {
                    fields.push(format!("{name}={b:?}=>{a:?}"));
                }
            };
            cmp("email", &before.email, &after.email);
            cmp("mobile", &before.mobile, &after.mobile);
            cmp("phones", &before.phones, &after.phones);
            cmp(
                "preferred_name",
                &before.preferred_name,
                &after.preferred_name,
            );
            println!(
                "[{mode}] update contact person id={person_id} registrationNumber={registration_number} {}",
                fields.join(" ")
            );
        }
        PlannedChange::Units {
            person_id,
            registration_number,
            before,
            after,
        } => {
            println!(
                "[{mode}] set units person id={person_id} registrationNumber={registration_number} {} => {}",
                units_line(before),
                units_line(after)
            );
        }
        PlannedChange::OrphanUnits { person_id } => {
            println!("[{mode}] remove units of non-live person id={person_id}");
        }
    }
}

// ── Running ──────────────────────────────────────────────────────────────────

/// Run the directory sync, recording every write it makes in the audit log as the
/// `directory-sync` system actor.
pub async fn run(config: SyncConfig) -> Result<RunStats> {
    crate::audit::scope(
        crate::audit::AuditContext::system("directory-sync"),
        run_inner(config),
    )
    .await
}

async fn scan_all_persons<H: db::Handler>(db: &H) -> Result<Vec<db::Person>> {
    let mut people = Vec::new();
    let mut cursor = None;
    loop {
        let page = db
            .scan_persons(cursor, 1000)
            .await
            .context("Scanning people")?;
        for row in page.rows {
            match row {
                Ok(p) => people.push(p),
                // One corrupt row is skipped: it could only ever be left alone anyway.
                Err(e) => warn!("Skipping unreadable person row: {}", e),
            }
        }
        match page.next {
            Some(next) => cursor = Some(next),
            None => return Ok(people),
        }
    }
}

async fn run_inner(config: SyncConfig) -> Result<RunStats> {
    let search = SesSearchClient::new(
        config.ses_intranet_search_api_base_url,
        config.ses_intranet_search_api_key,
        config.max_retries,
    )?;
    let db = AuditingHandler::new(dynamodb::Handler::new(&config.db_prefix, config.dry_run).await);

    let records = search
        .fetch_all_people()
        .await
        .context("Fetching every person from the SES directory")?;
    info!("SES directory returned {} people", records.len());
    if records.len() < config.min_people {
        return Err(anyhow!(
            "SES directory returned {} people, fewer than the floor of {}; re-run with --min-people to proceed",
            records.len(),
            config.min_people
        ));
    }

    let entities = search
        .list_entities()
        .await
        .context("Fetching SES directory entities")?;
    let entity_codes: HashMap<String, String> = entities
        .iter()
        .filter_map(|e| Some((non_blank(e.id.as_deref())?, non_blank(e.code.as_deref())?)))
        .collect();

    let locations = db
        .list_locations(db::ListLocationsFilter::All)
        .await
        .context("Listing locations")?;
    let entity_locations: HashMap<String, String> = locations
        .iter()
        .filter_map(|l| Some((l.directory_details.entity_id.clone()?, l.id.clone())))
        .collect();
    info!(
        "{} of {} locations have a directory entity",
        entity_locations.len(),
        locations.len()
    );

    let people = scan_all_persons(&db).await?;
    let stored_units = db
        .scan_person_units()
        .await
        .context("Scanning person_unit")?;
    info!(
        "{} person rows, {} person_unit rows",
        people.len(),
        stored_units.len()
    );

    let mut stats = RunStats::default();
    let changes = plan(
        PlanInput {
            people: &people,
            stored_units: &stored_units,
            records: &records,
            entity_codes: &entity_codes,
            entity_locations: &entity_locations,
        },
        &mut stats,
    );

    let total = stats.total_mutations();
    if total > config.max_mutations {
        let detail = format!(
            "{} writes planned, over max_mutations {}; re-run with --max-mutations {} to proceed",
            total, config.max_mutations, total
        );
        if !config.dry_run {
            return Err(anyhow!("Aborting directory sync: {detail}"));
        }
        error!("[DRY-RUN] would abort: {detail}");
    }

    for change in &changes {
        print_planned_change(change, config.dry_run);
        if config.dry_run {
            continue;
        }
        match change {
            PlannedChange::Contact {
                person_id, after, ..
            } => {
                db.update_person(
                    person_id,
                    db::PersonUpdateShape::Directory {
                        email: after.email.as_deref(),
                        mobile: after.mobile.as_deref(),
                        phones: after.phones.as_deref(),
                        preferred_name: after.preferred_name.as_deref(),
                    },
                )
                .await
                .with_context(|| format!("Updating contact details for person id={person_id}"))?;
            }
            PlannedChange::Units {
                person_id, after, ..
            } => {
                db.replace_person_units(person_id, after)
                    .await
                    .with_context(|| format!("Replacing units for person id={person_id}"))?;
            }
            PlannedChange::OrphanUnits { person_id } => {
                db.replace_person_units(person_id, &[])
                    .await
                    .with_context(|| format!("Removing units for person id={person_id}"))?;
            }
        }
    }

    info!(
        "directory sync complete mode={} directory_people={} matched={} unmatched={} local_absent={} duplicate_registration_numbers={} contact_updates={} unit_updates={} orphan_unit_deletes={} multi_unit_people={}",
        if config.dry_run { "dry-run" } else { "apply" },
        stats.directory_people,
        stats.matched,
        stats.unmatched,
        stats.local_absent,
        stats.duplicate_registration_numbers,
        stats.contact_updates,
        stats.unit_updates,
        stats.orphan_unit_deletes,
        stats.multi_unit_people,
    );
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contact(t: &str, d: &str) -> SesContactDetail {
        SesContactDetail {
            detail_type: Some(t.to_string()),
            detail: Some(d.to_string()),
        }
    }

    fn assignment(t: &str, detail: &str, entity_id: &str, name: &str) -> SesAssignment {
        SesAssignment {
            assignment_type: Some(t.to_string()),
            detail: Some(detail.to_string()),
            location: Some("Northern Zone".to_string()),
            entity_id: Some(entity_id.to_string()),
            entity_name: Some(name.to_string()),
        }
    }

    fn record(
        id: &str,
        contacts: Vec<SesContactDetail>,
        assignments: Vec<SesAssignment>,
    ) -> SesSearchResult {
        SesSearchResult {
            id: Some(id.to_string()),
            result_type: Some("Person".to_string()),
            preferred_name: None,
            assignments,
            contact_details: contacts,
        }
    }

    fn person(id: &str, rn: &str, location_id: &str) -> db::Person {
        db::Person {
            id: id.to_string(),
            location_id: location_id.to_string(),
            first_name: "First".to_string(),
            last_name: "Last".to_string(),
            registration_number: Some(rn.to_string()),
            ses_api_person_id: None,
            email: None,
            mobile: None,
            phones: None,
            preferred_name: None,
            badge_state: None,
            capability_tags: None,
            deleted: None,
            missing_since: None,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn landline_primary_falls_through_to_the_mobile() {
        let contacts = [
            contact("Primary Telephone", "+61299990000"),
            contact("Personal Mobile Phone", "+61400000001"),
        ];
        assert_eq!(pick_mobile(&contacts).as_deref(), Some("+61400000001"));
    }

    #[test]
    fn primary_mobile_wins_and_no_mobile_is_none() {
        let contacts = [
            contact("SMS", "+61400000002"),
            contact("Primary Telephone", "+61 400 000 001"),
        ];
        assert_eq!(pick_mobile(&contacts).as_deref(), Some("+61400000001"));
        let contacts = [
            contact("Home Telephone", "+61299990000"),
            contact("Work Telephone", "+61299990001"),
        ];
        assert_eq!(pick_mobile(&contacts), None);
    }

    #[test]
    fn phones_are_deduped_with_their_types_mobile_first() {
        // Two SMS entries with the same number, and a second, different SMS.
        let contacts = [
            contact("Home Telephone", "+61299990000"),
            contact("Primary Telephone", "+61400000001"),
            contact("SMS", "+61400000001"),
            contact("SMS", "+61400000001"),
            contact("SMS", "+61400000009"),
            contact("Fax", "+61299990005"),
            contact("Agency Email Address", "a@example.test"),
        ];
        let mobile = pick_mobile(&contacts);
        let phones = dedupe_phones(&contacts, mobile.as_deref());
        assert_eq!(
            phones,
            vec![
                Phone {
                    number: "+61400000001".into(),
                    types: vec!["Primary Telephone".into(), "SMS".into()],
                },
                Phone {
                    number: "+61299990000".into(),
                    types: vec!["Home Telephone".into()],
                },
                Phone {
                    number: "+61400000009".into(),
                    types: vec!["SMS".into()],
                },
            ]
        );
    }

    #[test]
    fn contact_keeps_stored_email_and_phones_the_record_lacks() {
        let mut p = person("p1", "40000001", "loc1");
        p.email = Some("old@example.test".into());
        p.mobile = Some("+61400000001".into());
        p.phones = Some(r#"[{"number":"+61400000001","types":["SMS"]}]"#.into());
        p.preferred_name = Some("Lex".into());
        let r = record("40000001", vec![], vec![]);
        let c = plan_contact(&p, &r);
        assert_eq!(c.email, p.email);
        assert_eq!(c.mobile, p.mobile);
        assert_eq!(c.phones, p.phones);
        // A blank preferred name is the record saying there is none.
        assert_eq!(c.preferred_name, None);
    }

    #[test]
    fn contact_takes_the_records_values() {
        let p = person("p1", "40000001", "loc1");
        let mut r = record(
            "40000001",
            vec![
                contact("Volunteers Primary E-Mail", "vol@example.test"),
                contact("Agency Email Address", "agency@example.test"),
                contact("Home Telephone", "+61299990000"),
            ],
            vec![],
        );
        r.preferred_name = Some(" Lex ".into());
        let c = plan_contact(&p, &r);
        assert_eq!(c.email.as_deref(), Some("agency@example.test"));
        // A record with phones but no mobile clears the mobile.
        assert_eq!(c.mobile, None);
        assert_eq!(
            c.phones.as_deref(),
            Some(r#"[{"number":"+61299990000","types":["Home Telephone"]}]"#)
        );
        assert_eq!(c.preferred_name.as_deref(), Some("Lex"));
    }

    fn maps() -> (HashMap<String, String>, HashMap<String, String>) {
        let codes = HashMap::from([
            ("70000047".to_string(), "ABD".to_string()),
            ("70000099".to_string(), "HMC".to_string()),
        ]);
        let locations = HashMap::from([("70000047".to_string(), "loc1".to_string())]);
        (codes, locations)
    }

    #[test]
    fn assignments_group_per_entity() {
        let (codes, locations) = maps();
        let units = group_assignments(
            "p1",
            "loc1",
            &[
                assignment("Role", "SES Team Leader", "70000047", "Aberdeen Unit"),
                assignment("Volunteer", "Member", "70000047", "Aberdeen Unit"),
                assignment("Rank", "SES Operator", "70000047", "Aberdeen Unit"),
                assignment("Role", "Deputy", "70000047", "Aberdeen Unit"),
                assignment("Volunteer", "Member", "70000099", "Hume - HMC"),
            ],
            &codes,
            &locations,
        );
        assert_eq!(units.len(), 2);
        let abd = &units[0];
        assert_eq!(abd.entity_id, "70000047");
        assert_eq!(abd.location_id.as_deref(), Some("loc1"));
        assert!(abd.is_primary);
        assert_eq!(abd.entity_code.as_deref(), Some("ABD"));
        assert_eq!(abd.zone.as_deref(), Some("Northern Zone"));
        assert_eq!(abd.membership_type.as_deref(), Some("Member"));
        assert_eq!(abd.ranks, vec!["SES Operator"]);
        assert_eq!(abd.roles, vec!["Deputy", "SES Team Leader"]);
        // A cluster is not one of our locations.
        let hmc = &units[1];
        assert_eq!(hmc.location_id, None);
        assert!(!hmc.is_primary);
    }

    #[test]
    fn staff_positions_and_secondary_units() {
        let (codes, locations) = maps();
        let units = group_assignments(
            "p1",
            "elsewhere",
            &[assignment(
                "Position",
                "Unit Support Officer",
                "70000047",
                "Aberdeen Unit",
            )],
            &codes,
            &locations,
        );
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].membership_type, None);
        assert_eq!(units[0].positions, vec!["Unit Support Officer"]);
        assert!(!units[0].is_primary);
        assert!(group_assignments("p1", "loc1", &[], &codes, &locations).is_empty());
    }

    #[test]
    fn units_compare_order_insensitively_and_ignore_updated_at() {
        let (codes, locations) = maps();
        let desired = group_assignments(
            "p1",
            "loc1",
            &[
                assignment("Volunteer", "Member", "70000047", "Aberdeen Unit"),
                assignment("Volunteer", "Member", "70000099", "Hume - HMC"),
            ],
            &codes,
            &locations,
        );
        let mut stored: Vec<db::PersonUnit> = desired.iter().rev().cloned().collect();
        stored[0].updated_at = Some(5);
        assert!(!units_differ(&stored, &desired));
        stored[0].roles.push("Deputy".into());
        assert!(units_differ(&stored, &desired));
        assert!(units_differ(&stored[..1], &desired));
    }

    fn run_plan(
        people: &[db::Person],
        stored: &[db::PersonUnit],
        records: &[SesSearchResult],
    ) -> (Vec<PlannedChange>, RunStats) {
        let (codes, locations) = maps();
        let mut stats = RunStats::default();
        let changes = plan(
            PlanInput {
                people,
                stored_units: stored,
                records,
                entity_codes: &codes,
                entity_locations: &locations,
            },
            &mut stats,
        );
        (changes, stats)
    }

    #[test]
    fn absent_members_are_left_alone_and_unmatched_people_skipped() {
        let mut p = person("p1", "40000001", "loc1");
        p.email = Some("old@example.test".into());
        let stored = vec![db::PersonUnit {
            person_id: "p1".into(),
            entity_id: "70000047".into(),
            ..Default::default()
        }];
        let records = [record(
            "49999999",
            vec![contact("SMS", "+61400000001")],
            vec![],
        )];
        let (changes, stats) = run_plan(&[p], &stored, &records);
        assert!(changes.is_empty());
        assert_eq!(stats.local_absent, 1);
        assert_eq!(stats.unmatched, 1);
    }

    #[test]
    fn a_record_with_no_assignments_removes_the_members_units() {
        let p = person("p1", "40000001", "loc1");
        let stored = vec![db::PersonUnit {
            person_id: "p1".into(),
            entity_id: "70000047".into(),
            ..Default::default()
        }];
        let records = [record("40000001", vec![], vec![])];
        let (changes, stats) = run_plan(&[p], &stored, &records);
        assert!(matches!(
            &changes[..],
            [PlannedChange::Units { after, .. }] if after.is_empty()
        ));
        assert_eq!(stats.unit_updates, 1);
        assert_eq!(stats.contact_updates, 0);
    }

    #[test]
    fn deleted_members_units_are_removed_and_they_are_never_matched() {
        let mut gone = person("p2", "40000002", "loc1");
        gone.deleted = Some(100);
        let stored = vec![db::PersonUnit {
            person_id: "p2".into(),
            entity_id: "70000047".into(),
            ..Default::default()
        }];
        let records = [record(
            "40000002",
            vec![contact("SMS", "+61400000001")],
            vec![assignment(
                "Volunteer",
                "Member",
                "70000047",
                "Aberdeen Unit",
            )],
        )];
        let (changes, stats) = run_plan(&[gone], &stored, &records);
        assert!(matches!(
            &changes[..],
            [PlannedChange::OrphanUnits { person_id }] if person_id == "p2"
        ));
        assert_eq!(stats.matched, 0);
    }

    #[test]
    fn shared_registration_numbers_are_skipped() {
        let records = [record(
            "40000001",
            vec![contact("SMS", "+61400000001")],
            vec![],
        )];
        let (changes, stats) = run_plan(
            &[
                person("p1", "40000001", "loc1"),
                person("p2", "40000001", "loc1"),
            ],
            &[],
            &records,
        );
        assert!(changes.is_empty());
        assert_eq!(stats.duplicate_registration_numbers, 1);
    }

    #[test]
    fn a_matched_member_gets_contact_and_units_in_one_plan() {
        let records = [record(
            "40000001",
            vec![contact("Primary Telephone", "+61400000001")],
            vec![assignment(
                "Volunteer",
                "Member",
                "70000047",
                "Aberdeen Unit",
            )],
        )];
        let (changes, stats) = run_plan(&[person("p1", "40000001", "loc1")], &[], &records);
        assert_eq!(changes.len(), 2);
        assert_eq!(stats.total_mutations(), 2);
        assert_eq!(stats.matched, 1);
    }
}
