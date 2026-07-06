use crate::db::{self, Handler as _};
use crate::dynamodb;
use crate::ses_api::{SesClient, SesPerson};
use anyhow::{Context, Result, anyhow};
use std::collections::{HashMap, HashSet};
use tracing::{info, warn};

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
    pub db_prefix: String,
    pub page_limit: usize,
    pub max_retries: usize,
    pub location_ids: Vec<String>,
    pub max_mutations: usize,
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
}

impl RunStats {
    pub fn total_mutations(&self) -> usize {
        self.adopts + self.creates + self.updates + self.undeletes + self.soft_deletes
    }
}

#[derive(Debug)]
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
        email: Option<String>,
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
        email: Option<String>,
        current_email: Option<String>,
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
        email: Option<String>,
        current_email: Option<String>,
    },
    SoftDelete {
        person_id: String,
        location_id: String,
        ses_api_person_id: String,
        registration_number: String,
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
            email,
        } => {
            println!(
                "[{mode}] create person location={} sesApiPersonId={} registrationNumber={} firstName='{}' lastName='{}' email='{}'",
                location_id,
                ses_api_person_id,
                registration_number,
                first_name,
                last_name,
                email.as_deref().unwrap_or("<null>")
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
            email,
            current_email,
        } => {
            println!(
                "[{mode}] update person id={} location={}=>{} sesApiPersonId={} registrationNumber={} firstName='{}'=>'{}' lastName='{}'=>'{}' email='{}'=>'{}'",
                person_id,
                current_location_id,
                location_id,
                ses_api_person_id,
                registration_number,
                current_first_name,
                first_name,
                current_last_name,
                last_name,
                current_email.as_deref().unwrap_or("<null>"),
                email.as_deref().unwrap_or("<null>")
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
            email,
            current_email,
        } => {
            println!(
                "[{mode}] undelete+update person id={} location={}=>{} sesApiPersonId={} registrationNumber={} firstName='{}'=>'{}' lastName='{}'=>'{}' email='{}'=>'{}'",
                person_id,
                current_location_id,
                location_id,
                ses_api_person_id,
                registration_number,
                current_first_name,
                first_name,
                current_last_name,
                last_name,
                current_email.as_deref().unwrap_or("<null>"),
                email.as_deref().unwrap_or("<null>")
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
                person_id, location_id, ses_api_person_id, registration_number
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
    email: Option<String>,
    is_deleted_in_ses: bool,
    unit_ses_id: Option<i64>,
}

fn normalize_email(person: &SesPerson) -> Option<String> {
    person
        .email
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
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

            if item.is_deleted_in_ses {
                if existing.deleted.is_some() {
                    stats.noops += 1;
                } else {
                    plans.push(PlannedChange::SoftDelete {
                        person_id: existing.id.clone(),
                        location_id: location_id.to_string(),
                        ses_api_person_id: item.ses_api_person_id.clone(),
                        registration_number: item.registration_number.clone(),
                    });
                }
                continue;
            }

            let needs_update = existing.deleted.is_some()
                || existing.first_name != item.first_name
                || existing.last_name != item.last_name
                || existing.registration_number.as_deref()
                    != Some(item.registration_number.as_str())
                || existing.email != item.email
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
                    email: item.email.clone(),
                    current_email: existing.email.clone(),
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
                    email: item.email.clone(),
                    current_email: existing.email.clone(),
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

        if item.is_deleted_in_ses {
            stats.noops += 1;
            continue;
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
            email: item.email.clone(),
        });
    }

    Ok(plans)
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
                email,
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
                db.update_person(
                    &person.id,
                    db::PersonUpdateShape::Email {
                        email: email.as_deref(),
                    },
                )
                .await
                .with_context(|| format!("Setting email for person id={}", person.id))?;
            }
            PlannedChange::Update {
                person_id,
                location_id,
                registration_number,
                first_name,
                last_name,
                email,
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
                db.update_person(
                    person_id,
                    db::PersonUpdateShape::Email {
                        email: email.as_deref(),
                    },
                )
                .await
                .with_context(|| format!("Updating email for person id={}", person_id))?;
            }
            PlannedChange::UndeleteAndUpdate {
                person_id,
                location_id,
                registration_number,
                first_name,
                last_name,
                email,
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
                db.update_person(
                    person_id,
                    db::PersonUpdateShape::Email {
                        email: email.as_deref(),
                    },
                )
                .await
                .with_context(|| format!("Updating email for person id={}", person_id))?;
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

        let mut ses_items_by_registration_number: HashMap<String, SesPersonWorkItem> =
            HashMap::new();
        let mut seen_ses_ids = HashSet::new();

        for ses_person in &ses_people {
            stats.ses_people_seen += 1;

            let Some(ses_id_raw) = ses_person.id else {
                warn!(
                    "Skipping SES person for location={} because id is null: {}",
                    location.id, ses_person
                );
                continue;
            };
            let ses_api_person_id = ses_id_raw.to_string();

            // skip SES people that are present in this headquarters but it is not their primary headquarters
            if let Some(primary_headquarters_id) = ses_person.headquarters_id()
                && primary_headquarters_id.to_string() != headquarters_id
            {
                info!(
                    "Skipping SES person for location={} because primary headquarters {} does not match location headquarters {}: {}",
                    location.id, primary_headquarters_id, headquarters_id, ses_person,
                );
                continue;
            }

            if !seen_ses_ids.insert(ses_api_person_id.clone()) {
                warn!(
                    "Duplicate SES person in payload for location={} with sesApiPersonId={}: {}",
                    location.id, ses_api_person_id, ses_person,
                );
                continue;
            }

            let Some(registration_number_raw) = ses_person.registration_number.as_deref() else {
                warn!(
                    "Skipping SES person for location={} because registrationNumber is null: {}",
                    location.id, ses_person,
                );
                continue;
            };

            let registration_number = registration_number_raw.trim().to_string();
            if registration_number.is_empty() {
                warn!(
                    "Skipping SES person for location={} because registrationNumber is empty: {}",
                    location.id, ses_person,
                );
                continue;
            }

            let (first_name, last_name) = normalize_names(ses_person)?;
            let is_deleted_in_ses = ses_person.deleted.unwrap_or(false);

            let new_item = SesPersonWorkItem {
                ses_api_person_id,
                registration_number: registration_number.clone(),
                first_name,
                last_name,
                email: normalize_email(ses_person),
                is_deleted_in_ses,
                unit_ses_id: ses_person.headquarters_id(),
            };

            if let Some(existing_item) =
                ses_items_by_registration_number.get_mut(&registration_number)
            {
                let existing_ses_id = existing_item
                    .ses_api_person_id
                    .parse::<i64>()
                    .with_context(|| {
                        format!(
                            "Invalid SES API person id '{}' for registrationNumber={} in location={}",
                            existing_item.ses_api_person_id, registration_number, location.id
                        )
                    })?;

                let new_ses_id = new_item.ses_api_person_id.parse::<i64>().with_context(|| {
                    format!(
                        "Invalid SES API person id '{}' for registrationNumber={} in location={}",
                        new_item.ses_api_person_id, registration_number, location.id
                    )
                })?;

                if new_ses_id < existing_ses_id {
                    warn!(
                        "Duplicate SES registrationNumber in payload for location={} registrationNumber={} keeping lower sesApiPersonId={} and discarding sesApiPersonId={} ({} {} -> {} {})",
                        location.id,
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
                        location.id,
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

        let ses_items: Vec<SesPersonWorkItem> =
            ses_items_by_registration_number.into_values().collect();

        // iterating here is not ideal but Dynamo gives us no choice
        let mut person_id_by_ses_id: HashMap<String, String> = HashMap::new();
        for item in &ses_items {
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
        for item in &ses_items {
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

        let plans = build_plans_for_location(
            &location.id,
            &ses_items,
            &people_by_id,
            &person_id_by_ses_id,
            &person_id_by_registration_number,
            config.adopt,
            &mut stats,
        )?;

        let mut adopts = 0usize;
        let mut creates = 0usize;
        let mut updates = 0usize;
        let mut undeletes = 0usize;
        let mut soft_deletes = 0usize;

        for change in &plans {
            match change {
                PlannedChange::AdoptSesApiPersonId { .. } => adopts += 1,
                PlannedChange::Create { .. } => creates += 1,
                PlannedChange::Update { .. } => updates += 1,
                PlannedChange::UndeleteAndUpdate { .. } => undeletes += 1,
                PlannedChange::SoftDelete { .. } => soft_deletes += 1,
            }
        }

        let planned_mutations = adopts + creates + updates + undeletes + soft_deletes;
        if !config.dry_run && stats.total_mutations() + planned_mutations > config.max_mutations {
            return Err(anyhow!(
                "Aborting sync: planned mutations exceed max_mutations (current_total={} planned_for_location={} max_mutations={})",
                stats.total_mutations(),
                planned_mutations,
                config.max_mutations
            ));
        }

        apply_changes(&db, &plans, config.dry_run)
            .await
            .with_context(|| format!("Applying sync changes for location={}", location.id))?;

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

        stats.adopts += adopts;
        stats.creates += creates;
        stats.updates += updates;
        stats.undeletes += undeletes;
        stats.soft_deletes += soft_deletes;
    }

    Ok(stats)
}
