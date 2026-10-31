use crate::audit::AuditingHandler;
use crate::db::{self, Handler as _, ListLocationsFilter};
use crate::dynamodb;
use crate::ses_api::{
    SesAccreditation, SesAddress, SesClient, SesDirectoryEntity, SesHeadquarters, SesSearchClient,
};
use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use std::collections::HashMap;
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub dry_run: bool,
    pub ses_api_base_url: String,
    pub ses_api_key: String,
    /// The SES intranet directory, for each location's address and contact details.
    /// `None` skips that part of the sync and leaves the stored directory details alone.
    pub directory: Option<DirectoryConfig>,
    pub db_prefix: String,
    pub max_retries: usize,
    pub max_updates: usize,
}

#[derive(Debug, Clone)]
pub struct DirectoryConfig {
    pub base_url: String,
    pub api_key: String,
}

#[derive(Default, Debug, Clone)]
pub struct RunStats {
    pub ses_units_seen: usize,
    pub creates: usize,
    pub name_updates: usize,
    pub details_updates: usize,
    pub accreditation_updates: usize,
    /// Units whose accreditations could not be fetched. Each is skipped, and the run
    /// fails once everything else is done.
    pub accreditation_failures: usize,
    /// Locations whose HQ code matched no directory entity, or several it could not
    /// choose between.
    pub entities_unresolved: usize,
    pub noops: usize,
}

const IMPORTED_HQ_TYPES: &[&str] = &["Unit", "StateHeadquarters", "Zone"];

fn location_name_for_unit(ses_name: &str) -> String {
    if ses_name.to_ascii_lowercase().ends_with("unit") {
        ses_name.to_string()
    } else {
        format!("{ses_name} Unit")
    }
}

fn location_name_for_hq(hq: &SesHeadquarters) -> String {
    let raw_name = hq.name.as_deref().unwrap_or("");
    match hq.headquarters_type.as_deref() {
        Some("Unit") => location_name_for_unit(raw_name),
        _ => raw_name.to_string(),
    }
}

fn non_blank(v: Option<&str>) -> Option<String> {
    v.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// `location_ids_by_hq` maps an SES headquarters id to our location for it, which is how
/// a unit's zone is resolved: zones are imported as locations too. A zone created by this
/// same run has no location yet, so its units pick it up on the next run.
fn ses_details_for(
    hq: &SesHeadquarters,
    location_ids_by_hq: &HashMap<String, String>,
) -> db::LocationSesDetails {
    let zone_location_id = match hq.headquarters_type.as_deref() {
        Some("Unit") => hq
            .zone
            .as_ref()
            .and_then(|zone| zone.id)
            .and_then(|zone_id| location_ids_by_hq.get(&zone_id.to_string()))
            .cloned(),
        _ => None,
    };
    db::LocationSesDetails {
        code: non_blank(hq.code.as_deref()),
        latitude: hq.latitude,
        longitude: hq.longitude,
        zone_location_id,
    }
}

/// Reduce a name to what survives the two systems' differences in spacing, punctuation
/// and case, and in spelling out "S&R" and "&".
fn name_key(name: &str) -> String {
    name.to_lowercase()
        .replace("s&r", "search and rescue")
        .replace('&', "and")
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// An entity's `DisplayName` without the ` - CODE` suffix the directory adds.
fn entity_stem<'a>(entity: &'a SesDirectoryEntity, code: &str) -> &'a str {
    let name = entity.display_name.as_deref().unwrap_or("").trim();
    name.strip_suffix(code)
        .and_then(|s| s.trim_end().strip_suffix('-'))
        .map(str::trim_end)
        .unwrap_or(name)
}

#[derive(Debug)]
enum EntityMatch<'a> {
    Found(&'a SesDirectoryEntity),
    Missing,
    /// Several entities share the code and nothing told them apart; their names.
    Ambiguous(Vec<String>),
}

/// The directory entity for an SES headquarters.
///
/// The join is the HQ's code, but codes are only unique among units: `CFS` is both Coffs
/// Harbour City Unit and Communications and Field Support, `SEZ` both South Eastern Zone
/// and a business support section. So among several entities sharing the code, the one
/// whose name matches the location's (ignoring spacing, punctuation, case and "S&R") wins,
/// and failing that the only one named `… Unit - CODE`. A lone entity with the code is
/// taken whatever its name: 28 units are named differently in the two systems.
fn resolve_entity<'a>(
    code: Option<&str>,
    location_name: &str,
    entities_by_code: &HashMap<String, Vec<&'a SesDirectoryEntity>>,
) -> EntityMatch<'a> {
    let Some(code) = code.map(str::trim).filter(|c| !c.is_empty()) else {
        return EntityMatch::Missing;
    };
    let candidates = match entities_by_code.get(&code.to_uppercase()) {
        None => return EntityMatch::Missing,
        Some(c) if c.is_empty() => return EntityMatch::Missing,
        Some(c) => c,
    };
    if let [only] = candidates.as_slice() {
        return EntityMatch::Found(only);
    }

    let wanted = name_key(location_name);
    let by_name: Vec<_> = candidates
        .iter()
        .filter(|e| name_key(entity_stem(e, code)) == wanted)
        .collect();
    if let [only] = by_name.as_slice() {
        return EntityMatch::Found(only);
    }

    let units: Vec<_> = candidates
        .iter()
        .filter(|e| entity_stem(e, code).ends_with(" Unit"))
        .collect();
    if let [only] = units.as_slice() {
        return EntityMatch::Found(only);
    }

    EntityMatch::Ambiguous(
        candidates
            .iter()
            .map(|e| e.display_name.clone().unwrap_or_default())
            .collect(),
    )
}

fn address_from(address: Option<&SesAddress>) -> Option<db::LocationAddress> {
    let a = address?;
    let address = db::LocationAddress {
        line1: non_blank(a.line1.as_deref()),
        line2: non_blank(a.line2.as_deref()),
        suburb: non_blank(a.suburb.as_deref()),
        state: non_blank(a.state.as_deref()),
        postcode: non_blank(a.postcode.as_deref()),
        formatted: non_blank(a.address.as_deref()),
    };
    (address != db::LocationAddress::default()).then_some(address)
}

fn directory_details_for(entity: Option<&SesDirectoryEntity>) -> db::LocationDirectoryDetails {
    let Some(entity) = entity else {
        return db::LocationDirectoryDetails::default();
    };
    db::LocationDirectoryDetails {
        entity_id: non_blank(entity.id.as_deref()),
        address: address_from(entity.address("Main address")),
        postal_address: address_from(entity.address("Postal Address")),
        email: entity.email().map(str::to_string),
        phone: entity.phone().map(str::to_string),
    }
}

#[derive(Debug, PartialEq, Serialize)]
struct StoredAccreditation {
    #[serde(rename = "type")]
    accreditation_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

/// The JSON text stored for a unit's accreditation rows: every row with a type, sorted
/// by type, so that a reordered response is not a change. `None` when there are none.
fn accreditations_json(rows: &[SesAccreditation]) -> Option<String> {
    let mut stored: Vec<StoredAccreditation> = rows
        .iter()
        .filter_map(|r| {
            Some(StoredAccreditation {
                accreditation_type: non_blank(r.accreditation_type.as_deref())?,
                status: non_blank(r.status.as_deref()),
                number: non_blank(r.accreditation_number.as_deref()),
                note: non_blank(r.note.as_deref()),
            })
        })
        .collect();
    if stored.is_empty() {
        return None;
    }
    stored.sort_by(|a, b| a.accreditation_type.cmp(&b.accreditation_type));
    Some(serde_json::to_string(&stored).expect("accreditations serialize"))
}

/// The detail fields that differ, as `field: before -> after`, for the plan output.
fn details_diff(
    before_ses: &db::LocationSesDetails,
    after_ses: &db::LocationSesDetails,
    before_dir: &db::LocationDirectoryDetails,
    after_dir: Option<&db::LocationDirectoryDetails>,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut cmp = |name: &str, changed: bool, b: String, a: String| {
        if changed {
            out.push(format!("{name}: {b} -> {a}"));
        }
    };
    let debug = |v: &dyn std::fmt::Debug| format!("{v:?}");
    cmp(
        "code",
        before_ses.code != after_ses.code,
        debug(&before_ses.code),
        debug(&after_ses.code),
    );
    cmp(
        "latitude",
        before_ses.latitude != after_ses.latitude,
        debug(&before_ses.latitude),
        debug(&after_ses.latitude),
    );
    cmp(
        "longitude",
        before_ses.longitude != after_ses.longitude,
        debug(&before_ses.longitude),
        debug(&after_ses.longitude),
    );
    cmp(
        "zone_location_id",
        before_ses.zone_location_id != after_ses.zone_location_id,
        debug(&before_ses.zone_location_id),
        debug(&after_ses.zone_location_id),
    );
    if let Some(after_dir) = after_dir {
        // An address is compared in full but shown in its one-line form.
        let formatted =
            |a: &Option<db::LocationAddress>| debug(&a.as_ref().map(|a| a.formatted.clone()));
        cmp(
            "entity_id",
            before_dir.entity_id != after_dir.entity_id,
            debug(&before_dir.entity_id),
            debug(&after_dir.entity_id),
        );
        cmp(
            "address",
            before_dir.address != after_dir.address,
            formatted(&before_dir.address),
            formatted(&after_dir.address),
        );
        cmp(
            "postal_address",
            before_dir.postal_address != after_dir.postal_address,
            formatted(&before_dir.postal_address),
            formatted(&after_dir.postal_address),
        );
        cmp(
            "email",
            before_dir.email != after_dir.email,
            debug(&before_dir.email),
            debug(&after_dir.email),
        );
        cmp(
            "phone",
            before_dir.phone != after_dir.phone,
            debug(&before_dir.phone),
            debug(&after_dir.phone),
        );
    }
    out
}

#[derive(Debug)]
enum PlannedChange {
    Create {
        ses_hq_id: String,
        name: String,
        details: db::LocationSesDetails,
        directory: Option<db::LocationDirectoryDetails>,
    },
    UpdateName {
        location_id: String,
        previous_name: String,
        name: String,
    },
    UpdateDetails {
        location_id: String,
        name: String,
        details: db::LocationSesDetails,
        directory: Option<db::LocationDirectoryDetails>,
        diff: Vec<String>,
    },
}

/// Run the location sync, recording every write it makes in the audit log as the
/// `location-sync` system actor.
pub async fn run(config: SyncConfig) -> Result<RunStats> {
    crate::audit::scope(
        crate::audit::AuditContext::system("location-sync"),
        run_inner(config),
    )
    .await
}

/// Every directory entity, or `None` (logged) when the directory is not configured or the
/// fetch failed: location details from the SES API, names and creates are still synced
/// without it. The `bool` is whether it failed.
async fn fetch_entities(
    config: Option<DirectoryConfig>,
    max_retries: usize,
) -> (Option<Vec<SesDirectoryEntity>>, bool) {
    let Some(config) = config else {
        info!("SES directory not configured; leaving location directory details alone");
        return (None, false);
    };
    let fetched = async {
        SesSearchClient::new(config.base_url, config.api_key, max_retries)?
            .list_entities()
            .await
    }
    .await;
    match fetched {
        Ok(entities) => {
            info!("SES directory returned {} entities", entities.len());
            (Some(entities), false)
        }
        Err(e) => {
            error!(
                "Fetching SES directory entities failed; leaving location directory details alone: {:#}",
                e
            );
            (None, true)
        }
    }
}

async fn run_inner(config: SyncConfig) -> Result<RunStats> {
    let ses_client = SesClient::new(
        config.ses_api_base_url,
        config.ses_api_key,
        500,
        config.max_retries,
    )
    .context("Building SES HTTP client")?;
    let db = AuditingHandler::new(dynamodb::Handler::new(&config.db_prefix, config.dry_run).await);

    let all_hqs = ses_client
        .list_headquarters()
        .await
        .context("Fetching headquarters from SES API")?;

    let units: Vec<_> = all_hqs
        .into_iter()
        .filter(|hq| {
            hq.headquarters_type
                .as_deref()
                .is_some_and(|t| IMPORTED_HQ_TYPES.contains(&t))
        })
        .collect();

    info!("SES API returned {} unit headquarters", units.len());

    let (entities, directory_failed) = fetch_entities(config.directory, config.max_retries).await;
    let mut entities_by_code: HashMap<String, Vec<&SesDirectoryEntity>> = HashMap::new();
    for entity in entities.iter().flatten() {
        if let Some(code) = non_blank(entity.code.as_deref()) {
            entities_by_code
                .entry(code.to_uppercase())
                .or_default()
                .push(entity);
        }
    }

    let all_locations = db
        .list_locations(ListLocationsFilter::All)
        .await
        .context("Listing locations from DB")?;

    // Index existing locations by their SES headquarters ID
    let existing: HashMap<String, db::Location> = all_locations
        .into_iter()
        .filter_map(|loc| {
            loc.ses_api_headquarters_id
                .clone()
                .map(|hq_id| (hq_id, loc))
        })
        .collect();
    let location_ids_by_hq: HashMap<String, String> = existing
        .iter()
        .map(|(hq_id, loc)| (hq_id.clone(), loc.id.clone()))
        .collect();

    let mut stats = RunStats {
        ses_units_seen: units.len(),
        ..Default::default()
    };

    let mut planned: Vec<PlannedChange> = Vec::new();

    for hq in &units {
        let Some(hq_id) = hq.id else { continue };
        let hq_id_str = hq_id.to_string();
        let hq_name = location_name_for_hq(hq);

        let details = ses_details_for(hq, &location_ids_by_hq);
        let directory = entities.as_ref().map(|_| {
            let entity = match resolve_entity(hq.code.as_deref(), &hq_name, &entities_by_code) {
                EntityMatch::Found(entity) => Some(entity),
                EntityMatch::Missing => {
                    warn!(hq = %hq, "no SES directory entity has this headquarters' code");
                    stats.entities_unresolved += 1;
                    None
                }
                EntityMatch::Ambiguous(names) => {
                    warn!(hq = %hq, candidates = ?names, "several SES directory entities share this headquarters' code; leaving it unmatched");
                    stats.entities_unresolved += 1;
                    None
                }
            };
            directory_details_for(entity)
        });

        let Some(loc) = existing.get(&hq_id_str) else {
            planned.push(PlannedChange::Create {
                ses_hq_id: hq_id_str,
                name: hq_name,
                details,
                directory,
            });
            continue;
        };

        let mut changed = false;
        if loc.name != hq_name {
            changed = true;
            planned.push(PlannedChange::UpdateName {
                location_id: loc.id.clone(),
                previous_name: loc.name.clone(),
                name: hq_name.clone(),
            });
        }
        let diff = details_diff(
            &loc.ses_details,
            &details,
            &loc.directory_details,
            directory.as_ref(),
        );
        if !diff.is_empty() {
            changed = true;
            planned.push(PlannedChange::UpdateDetails {
                location_id: loc.id.clone(),
                name: hq_name,
                details,
                directory,
                diff,
            });
        }
        if !changed {
            stats.noops += 1;
        }
    }

    for change in &planned {
        match change {
            PlannedChange::Create {
                ses_hq_id, name, ..
            } => {
                info!(ses_hq_id, name, "planned: create location");
            }
            PlannedChange::UpdateName {
                location_id,
                previous_name,
                name,
            } => {
                info!(location_id, previous_name, name, "planned: rename location");
            }
            PlannedChange::UpdateDetails {
                location_id,
                name,
                diff,
                ..
            } => {
                info!(location_id, name, changes = %diff.join("; "), "planned: update location details");
            }
        }
    }

    if planned.len() > config.max_updates {
        bail!(
            "{} changes planned, exceeds max_updates limit of {}; re-run with --max-updates {} to proceed",
            planned.len(),
            config.max_updates,
            planned.len(),
        );
    }

    for change in planned {
        match change {
            PlannedChange::Create {
                ref ses_hq_id,
                ref name,
                ref details,
                ref directory,
            } => {
                if !config.dry_run {
                    let location = db
                        .create_location(name, None, Some(ses_hq_id))
                        .await
                        .with_context(|| {
                            format!("Creating location for SES HQ {ses_hq_id} ({name})")
                        })?;
                    db.update_location(
                        &location.id,
                        db::LocationUpdateShape::SesDetails {
                            details,
                            directory: directory.as_ref(),
                        },
                    )
                    .await
                    .with_context(|| format!("Setting details for new location {}", location.id))?;
                }
                stats.creates += 1;
            }
            PlannedChange::UpdateName {
                ref location_id,
                ref name,
                ..
            } => {
                if !config.dry_run {
                    db.update_location(location_id, db::LocationUpdateShape::Name { name })
                        .await
                        .with_context(|| {
                            format!("Updating name for location {location_id} to {name:?}")
                        })?;
                }
                stats.name_updates += 1;
            }
            PlannedChange::UpdateDetails {
                ref location_id,
                ref details,
                ref directory,
                ..
            } => {
                if !config.dry_run {
                    db.update_location(
                        location_id,
                        db::LocationUpdateShape::SesDetails {
                            details,
                            directory: directory.as_ref(),
                        },
                    )
                    .await
                    .with_context(|| format!("Updating details for location {location_id}"))?;
                }
                stats.details_updates += 1;
            }
        }
    }

    // Accreditations come one request per unit, so they are fetched only for units that
    // already have a location (one created above gets its own on the next run), and
    // stay out of `max_updates`: a mass change to them cannot damage anything.
    for hq in units
        .iter()
        .filter(|hq| hq.headquarters_type.as_deref() == Some("Unit"))
    {
        let Some(hq_id) = hq.id.map(|id| id.to_string()) else {
            continue;
        };
        let Some(loc) = existing.get(&hq_id) else {
            continue;
        };
        let rows = match ses_client.fetch_accreditations(&hq_id).await {
            Ok(rows) => rows,
            Err(e) => {
                error!(hq = %hq, "fetching accreditations failed; skipping: {:#}", e);
                stats.accreditation_failures += 1;
                continue;
            }
        };
        let accreditations = accreditations_json(&rows);
        if accreditations == loc.accreditations {
            continue;
        }
        info!(
            location_id = loc.id,
            name = loc.name,
            before = loc.accreditations.as_deref().unwrap_or("-"),
            after = accreditations.as_deref().unwrap_or("-"),
            "planned: update accreditations"
        );
        if !config.dry_run {
            db.update_location(
                &loc.id,
                db::LocationUpdateShape::Accreditations {
                    accreditations: accreditations.as_deref(),
                },
            )
            .await
            .with_context(|| format!("Updating accreditations for location {}", loc.id))?;
        }
        stats.accreditation_updates += 1;
    }

    info!(
        "location sync complete mode={} ses_units_seen={} creates={} name_updates={} details_updates={} accreditation_updates={} accreditation_failures={} entities_unresolved={} noops={}",
        if config.dry_run { "dry-run" } else { "apply" },
        stats.ses_units_seen,
        stats.creates,
        stats.name_updates,
        stats.details_updates,
        stats.accreditation_updates,
        stats.accreditation_failures,
        stats.entities_unresolved,
        stats.noops,
    );

    // Everything that could be synced has been. Fail now so that a partial run is noticed.
    if directory_failed {
        return Err(anyhow!(
            "SES directory entities could not be fetched; location directory details were not synced"
        ));
    }
    if stats.accreditation_failures > 0 {
        return Err(anyhow!(
            "Accreditations could not be fetched for {} unit(s)",
            stats.accreditation_failures
        ));
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ses_api::SesContactDetail;

    fn entity(id: &str, display_name: &str, code: &str) -> SesDirectoryEntity {
        SesDirectoryEntity {
            id: Some(id.to_string()),
            display_name: Some(display_name.to_string()),
            code: Some(code.to_string()),
            contact_details: vec![],
            addresses: vec![],
        }
    }

    fn by_code(entities: &[SesDirectoryEntity]) -> HashMap<String, Vec<&SesDirectoryEntity>> {
        let mut map: HashMap<String, Vec<&SesDirectoryEntity>> = HashMap::new();
        for e in entities {
            map.entry(e.code.clone().unwrap().to_uppercase())
                .or_default()
                .push(e);
        }
        map
    }

    fn found_id(m: EntityMatch<'_>) -> Option<String> {
        match m {
            EntityMatch::Found(e) => e.id.clone(),
            _ => None,
        }
    }

    #[test]
    fn lone_entity_with_the_code_is_taken_whatever_its_name() {
        let entities = [entity(
            "1",
            "Darlington Point - Coleambally Unit - CLY",
            "CLY",
        )];
        let m = resolve_entity(Some("cly"), "Coleambally Unit", &by_code(&entities));
        assert_eq!(found_id(m).as_deref(), Some("1"));
    }

    #[test]
    fn shared_code_is_resolved_by_name() {
        // Both are "… Unit - COR", so only the name tells them apart.
        let entities = [
            entity("1", "Corporate Volunteering Unit - COR", "COR"),
            entity("2", "Corindi-Woolgoolga Unit - COR", "COR"),
        ];
        let m = resolve_entity(Some("COR"), "Corindi-Woolgoolga Unit", &by_code(&entities));
        assert_eq!(found_id(m).as_deref(), Some("2"));

        let entities = [
            entity("1", "Coffs Harbour City Unit - CFS", "CFS"),
            entity("2", "Communications and Field Support - CFS", "CFS"),
        ];
        let m = resolve_entity(Some("CFS"), "Coffs Harbour City Unit", &by_code(&entities));
        assert_eq!(found_id(m).as_deref(), Some("1"));
    }

    #[test]
    fn zone_codes_shared_with_sections_resolve_by_name() {
        let entities = [
            entity("1", "Business Service Support South East - SEZ", "SEZ"),
            entity("2", "South Eastern Zone - SEZ", "SEZ"),
        ];
        let m = resolve_entity(Some("SEZ"), "South Eastern Zone", &by_code(&entities));
        assert_eq!(found_id(m).as_deref(), Some("2"));
    }

    #[test]
    fn name_match_ignores_spacing_and_spells_out_search_and_rescue() {
        let entities = [
            entity("1", "Alpine Search and Rescue Unit - ASR", "ASR"),
            entity("2", "Alpine Something Else - ASR", "ASR"),
        ];
        let m = resolve_entity(Some("ASR"), "Alpine S&R Unit", &by_code(&entities));
        assert_eq!(found_id(m).as_deref(), Some("1"));

        let entities = [
            entity("1", "Armidale-Dumaresq Unit - ARM", "ARM"),
            entity("2", "Armidale Cluster - ARM", "ARM"),
        ];
        let m = resolve_entity(Some("ARM"), "Armidale - Dumaresq Unit", &by_code(&entities));
        assert_eq!(found_id(m).as_deref(), Some("1"));
    }

    #[test]
    fn falls_back_to_the_only_unit_when_no_name_matches() {
        let entities = [
            entity("1", "Some Renamed Unit - XYZ", "XYZ"),
            entity("2", "Some Section - XYZ", "XYZ"),
        ];
        let m = resolve_entity(Some("XYZ"), "Old Name Unit", &by_code(&entities));
        assert_eq!(found_id(m).as_deref(), Some("1"));
    }

    #[test]
    fn unresolvable_codes() {
        let entities = [
            entity("1", "First Unit - DUP", "DUP"),
            entity("2", "Second Unit - DUP", "DUP"),
        ];
        let map = by_code(&entities);
        assert!(matches!(
            resolve_entity(Some("DUP"), "Third Unit", &map),
            EntityMatch::Ambiguous(names) if names.len() == 2
        ));
        for code in [Some("MLM"), None, Some(" ")] {
            assert!(matches!(
                resolve_entity(code, "Moulamein Unit", &map),
                EntityMatch::Missing
            ));
        }
    }

    fn contact(detail_type: &str, detail: &str) -> SesContactDetail {
        SesContactDetail {
            detail_type: Some(detail_type.to_string()),
            detail: Some(detail.to_string()),
        }
    }

    fn address(address_type: &str, line1: &str, formatted: &str) -> SesAddress {
        SesAddress {
            address_type: Some(address_type.to_string()),
            address: Some(formatted.to_string()),
            line1: Some(line1.to_string()),
            line2: None,
            suburb: Some("EXAMPLETOWN".to_string()),
            state: Some("NSW".to_string()),
            postcode: Some("2000".to_string()),
        }
    }

    #[test]
    fn directory_details_take_mail_phone_and_both_addresses() {
        let mut e = entity("70000047", "Example Unit - EXU", "EXU");
        e.contact_details = vec![
            contact("Fax", "+61200000001"),
            contact("Internet Mail", " exu.ops@example.test "),
            contact("Phone", "+61200000000"),
        ];
        e.addresses = vec![
            address(
                "Postal Address",
                "PO Box 1",
                "PO Box 1, EXAMPLETOWN NSW 2000",
            ),
            address(
                "Main address",
                "1 Example St",
                "1 Example St, EXAMPLETOWN NSW 2000",
            ),
        ];
        let d = directory_details_for(Some(&e));
        assert_eq!(d.entity_id.as_deref(), Some("70000047"));
        assert_eq!(d.email.as_deref(), Some("exu.ops@example.test"));
        assert_eq!(d.phone.as_deref(), Some("+61200000000"));
        assert_eq!(d.address.unwrap().line1.as_deref(), Some("1 Example St"));
        assert_eq!(
            d.postal_address.unwrap().formatted.as_deref(),
            Some("PO Box 1, EXAMPLETOWN NSW 2000")
        );
    }

    #[test]
    fn directory_phone_falls_back_to_mobile_and_blanks_are_absent() {
        let mut e = entity("1", "Example Unit - EXU", "EXU");
        e.contact_details = vec![contact("Phone", "  "), contact("Mobile", "+61400000000")];
        e.addresses = vec![SesAddress {
            address_type: Some("Main address".to_string()),
            address: Some(" ".to_string()),
            line1: None,
            line2: None,
            suburb: None,
            state: None,
            postcode: None,
        }];
        let d = directory_details_for(Some(&e));
        assert_eq!(d.phone.as_deref(), Some("+61400000000"));
        assert_eq!(d.email, None);
        assert_eq!(d.address, None);
        assert_eq!(
            directory_details_for(None),
            db::LocationDirectoryDetails::default()
        );
    }

    fn accreditation(t: &str, status: &str) -> SesAccreditation {
        SesAccreditation {
            accreditation_type: Some(t.to_string()),
            status: Some(status.to_string()),
            accreditation_number: None,
            note: None,
        }
    }

    #[test]
    fn accreditations_are_sorted_and_keep_not_applicable_rows() {
        let a = accreditations_json(&[
            accreditation("Vertical Rescue", "Not Applicable"),
            accreditation("CFR", "Available"),
        ]);
        let b = accreditations_json(&[
            accreditation("CFR", "Available"),
            accreditation("Vertical Rescue", "Not Applicable"),
        ]);
        assert_eq!(a, b);
        assert_eq!(
            a.as_deref(),
            Some(
                r#"[{"type":"CFR","status":"Available"},{"type":"Vertical Rescue","status":"Not Applicable"}]"#
            )
        );
    }

    #[test]
    fn accreditation_number_and_note_are_kept_and_no_rows_is_absent() {
        let mut row = accreditation("GLR", "Accredited");
        row.accreditation_number = Some("A-1".to_string());
        row.note = Some(" ok ".to_string());
        assert_eq!(
            accreditations_json(&[row]).as_deref(),
            Some(r#"[{"type":"GLR","status":"Accredited","number":"A-1","note":"ok"}]"#)
        );
        assert_eq!(accreditations_json(&[]), None);
    }

    fn hq(id: i64, hq_type: &str, zone_id: Option<i64>) -> SesHeadquarters {
        SesHeadquarters {
            id: Some(id),
            name: Some(format!("HQ {id}")),
            code: None,
            latitude: None,
            longitude: None,
            headquarters_type: Some(hq_type.to_string()),
            status: None,
            zone: zone_id.map(|z| Box::new(hq(z, "Zone", None))),
        }
    }

    #[test]
    fn a_units_zone_resolves_to_the_zones_location() {
        let by_hq = HashMap::from([("285".to_string(), "loc-sez".to_string())]);
        let zone = |h| ses_details_for(&h, &by_hq).zone_location_id;
        assert_eq!(zone(hq(149, "Unit", Some(285))).as_deref(), Some("loc-sez"));
        // A zone not yet a location, and a unit with no zone, have none.
        assert_eq!(zone(hq(150, "Unit", Some(999))), None);
        assert_eq!(zone(hq(151, "Unit", None)), None);
        // A zone's own `zone` is State Headquarters, which is not its zone.
        let by_hq = HashMap::from([("1".to_string(), "loc-shq".to_string())]);
        assert_eq!(
            ses_details_for(&hq(285, "Zone", Some(1)), &by_hq).zone_location_id,
            None
        );
    }

    #[test]
    fn details_diff_ignores_directory_fields_without_a_directory() {
        let before_ses = db::LocationSesDetails::default();
        let after_ses = db::LocationSesDetails {
            code: Some("ABD".to_string()),
            latitude: Some(-32.17),
            longitude: Some(150.88),
            ..Default::default()
        };
        let before_dir = db::LocationDirectoryDetails {
            email: Some("old@example.test".to_string()),
            ..Default::default()
        };
        let diff = details_diff(&before_ses, &after_ses, &before_dir, None);
        assert_eq!(diff.len(), 3);
        assert!(diff[0].starts_with("code: "));

        let after_dir = db::LocationDirectoryDetails::default();
        let diff = details_diff(&after_ses, &after_ses, &before_dir, Some(&after_dir));
        assert_eq!(diff, vec![r#"email: Some("old@example.test") -> None"#]);
        assert!(details_diff(&after_ses, &after_ses, &before_dir, Some(&before_dir)).is_empty());
    }

    #[test]
    fn entity_stem_strips_only_the_code_suffix() {
        let e = entity("1", "Hume - HMC", "HMC");
        assert_eq!(entity_stem(&e, "HMC"), "Hume");
        let e = entity("1", "Armidale - Dumaresq Unit - ARM", "ARM");
        assert_eq!(entity_stem(&e, "ARM"), "Armidale - Dumaresq Unit");
        let e = entity("1", "No Suffix", "ARM");
        assert_eq!(entity_stem(&e, "ARM"), "No Suffix");
    }
}
