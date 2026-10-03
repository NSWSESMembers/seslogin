use crate::audit::AuditingHandler;
use crate::db::{self, Handler as _, ListLocationsFilter};
use crate::dynamodb;
use crate::ses_api::SesClient;
use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use tracing::info;

#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub dry_run: bool,
    pub ses_api_base_url: String,
    pub ses_api_key: String,
    pub db_prefix: String,
    pub max_retries: usize,
    pub max_updates: usize,
}

#[derive(Default, Debug, Clone)]
pub struct RunStats {
    pub ses_units_seen: usize,
    pub creates: usize,
    pub name_updates: usize,
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

#[derive(Debug)]
enum PlannedChange {
    Create {
        ses_hq_id: String,
        name: String,
    },
    UpdateName {
        location_id: String,
        previous_name: String,
        name: String,
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

    let mut stats = RunStats {
        ses_units_seen: units.len(),
        ..Default::default()
    };

    let mut planned: Vec<PlannedChange> = Vec::new();

    for hq in &units {
        let Some(hq_id) = hq.id else { continue };
        let hq_id_str = hq_id.to_string();
        let raw_name = hq.name.as_deref().unwrap_or("");
        let hq_name = match hq.headquarters_type.as_deref() {
            Some("Unit") => location_name_for_unit(raw_name),
            _ => raw_name.to_string(),
        };

        match existing.get(&hq_id_str) {
            Some(loc) if loc.name != hq_name => {
                planned.push(PlannedChange::UpdateName {
                    location_id: loc.id.clone(),
                    previous_name: loc.name.clone(),
                    name: hq_name,
                });
            }
            Some(_) => {
                stats.noops += 1;
            }
            None => {
                planned.push(PlannedChange::Create {
                    ses_hq_id: hq_id_str,
                    name: hq_name,
                });
            }
        }
    }

    for change in &planned {
        match change {
            PlannedChange::Create { ses_hq_id, name } => {
                info!(ses_hq_id, name, "planned: create location");
            }
            PlannedChange::UpdateName {
                location_id,
                previous_name,
                name,
            } => {
                info!(location_id, previous_name, name, "planned: rename location");
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
            } => {
                if !config.dry_run {
                    db.create_location(name, None, Some(ses_hq_id))
                        .await
                        .with_context(|| {
                            format!("Creating location for SES HQ {ses_hq_id} ({name})")
                        })?;
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
        }
    }

    info!(
        "location sync complete mode={} ses_units_seen={} creates={} name_updates={} noops={}",
        if config.dry_run { "dry-run" } else { "apply" },
        stats.ses_units_seen,
        stats.creates,
        stats.name_updates,
        stats.noops,
    );

    Ok(stats)
}
