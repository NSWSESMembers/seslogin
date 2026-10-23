//! Backfill the kiosk display metadata (group name, short name, icon) onto category
//! rows from `assets/category-display.json`.
//!
//! The file mirrors the kiosk's hard-coded tree in `web/src/lib/categories.ts` (a
//! web test fails when the two drift). Categories not listed in it are left alone,
//! and so are listed ids that are missing or soft-deleted (they are reported).
//! Dry run unless `--apply`.

use anyhow::{Result, anyhow};
use clap::Parser;
use serde::Deserialize;
use seslogin::audit::{self, AuditContext, AuditingHandler};
use seslogin::db::{self, Handler};
use seslogin::dynamodb;
use std::collections::HashMap;

const DISPLAY_JSON: &str = include_str!("../../assets/category-display.json");

#[derive(Parser, Debug)]
#[command(
    about = "Backfill category display metadata (group, short name, icon) from the embedded file"
)]
struct Cli {
    /// DynamoDB table prefix (e.g. "seslogin_prod"). Falls back to DB_PREFIX env var.
    #[arg(long)]
    db_prefix: Option<String>,

    /// Write the changes to DynamoDB. Without it this is a dry run, which prints what
    /// would change without writing.
    #[arg(long, default_value_t = false)]
    apply: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    id: String,
    group_name: String,
    short_name: String,
    icon: String,
}

impl Entry {
    fn display(&self) -> db::CategoryDisplay {
        db::CategoryDisplay {
            group_name: Some(self.group_name.clone()),
            short_name: Some(self.short_name.clone()),
            icon: Some(self.icon.clone()),
        }
    }
}

#[derive(Debug, PartialEq)]
struct Change {
    id: String,
    name: String,
    before: db::CategoryDisplay,
    after: db::CategoryDisplay,
}

#[derive(Debug, Default, PartialEq)]
struct Plan {
    changes: Vec<Change>,
    /// Listed ids already carrying exactly this metadata.
    unchanged: usize,
    /// Listed ids with no live category.
    missing: Vec<String>,
}

/// Compare the file against the stored categories. Pure, so it can be tested without
/// a database. A soft-deleted category (`deleted` rows are not returned by
/// `list_categories`) counts as missing.
fn plan(entries: &[Entry], categories: &[db::Category]) -> Plan {
    let by_id: HashMap<&str, &db::Category> =
        categories.iter().map(|c| (c.id.as_str(), c)).collect();
    let mut plan = Plan::default();
    for entry in entries {
        let Some(category) = by_id.get(entry.id.as_str()) else {
            plan.missing.push(entry.id.clone());
            continue;
        };
        let after = entry.display();
        let before = category.display();
        if before == after {
            plan.unchanged += 1;
        } else {
            plan.changes.push(Change {
                id: category.id.clone(),
                name: category.name.clone(),
                before,
                after,
            });
        }
    }
    plan
}

fn show(d: &db::CategoryDisplay) -> String {
    format!(
        "group={:?} short={:?} icon={:?}",
        d.group_name.as_deref().unwrap_or("-"),
        d.short_name.as_deref().unwrap_or("-"),
        d.icon.as_deref().unwrap_or("-"),
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    seslogin::load_cli_env();

    let cli = Cli::parse();
    let db_prefix = cli
        .db_prefix
        .or_else(|| std::env::var("DB_PREFIX").ok())
        .ok_or_else(|| anyhow!("DB_PREFIX is required (flag or env var)"))?;

    let entries: Vec<Entry> = serde_json::from_str(DISPLAY_JSON)?;
    // Read-only handler for a dry run, so it cannot write even if a bug tried to.
    let db = AuditingHandler::new(dynamodb::Handler::new(&db_prefix, !cli.apply).await);
    let categories = db.list_categories().await?;
    let plan = plan(&entries, &categories);

    for id in &plan.missing {
        eprintln!("  skipped {id}: no live category with that id");
    }
    for c in &plan.changes {
        println!(
            "  {}{} {:?}: {} -> {}",
            if cli.apply { "" } else { "[dry-run] " },
            c.id,
            c.name,
            show(&c.before),
            show(&c.after),
        );
    }

    let mut applied = 0;
    if cli.apply {
        // Everything else about the category is written back as stored.
        let current: HashMap<&str, &db::Category> =
            categories.iter().map(|c| (c.id.as_str(), c)).collect();
        applied = audit::scope(AuditContext::system("load-category-display"), async {
            let mut applied = 0;
            for c in &plan.changes {
                let cat = current[c.id.as_str()];
                db.update_category(
                    &cat.id,
                    &cat.name,
                    cat.enabled,
                    cat.is_virtual,
                    cat.nitc_group_id.as_deref(),
                    cat.nitc_participant_type.as_deref(),
                    &c.after,
                )
                .await?;
                applied += 1;
            }
            Ok::<usize, anyhow::Error>(applied)
        })
        .await?;
    }

    eprintln!(
        "{}: {} planned, {applied} applied, {} unchanged, {} skipped (not found)",
        if cli.apply { "Done" } else { "Dry run" },
        plan.changes.len(),
        plan.unchanged,
        plan.missing.len(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, group: &str, short: &str, icon: &str) -> Entry {
        Entry {
            id: id.into(),
            group_name: group.into(),
            short_name: short.into(),
            icon: icon.into(),
        }
    }

    fn category(id: &str, display: db::CategoryDisplay) -> db::Category {
        db::Category {
            id: id.into(),
            name: format!("Name {id}"),
            enabled: true,
            is_virtual: false,
            nitc_participant_type: None,
            nitc_group_id: None,
            group_name: display.group_name,
            short_name: display.short_name,
            icon: display.icon,
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn embedded_file_parses_and_has_unique_ids() {
        let entries: Vec<Entry> = serde_json::from_str(DISPLAY_JSON).unwrap();
        assert!(!entries.is_empty());
        let mut ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), entries.len());
    }

    #[test]
    fn plans_only_differences() {
        let entries = vec![
            entry("a", "Training", "AIIMS", "aiims"),
            entry("b", "Training", "Job Ready", "job_ready"),
            entry("c", "Training", "Gone", "gone"),
            entry("d", "Other", "Partial", "partial"),
        ];
        let categories = vec![
            category("a", entries[0].display()),
            category("b", db::CategoryDisplay::default()),
            category(
                "d",
                db::CategoryDisplay {
                    group_name: Some("Other".into()),
                    short_name: Some("Old".into()),
                    icon: Some("partial".into()),
                },
            ),
            // Not in the file: untouched.
            category("z", db::CategoryDisplay::default()),
        ];
        let plan = plan(&entries, &categories);
        assert_eq!(plan.unchanged, 1);
        assert_eq!(plan.missing, vec!["c".to_string()]);
        let ids: Vec<&str> = plan.changes.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "d"]);
        assert_eq!(plan.changes[0].after, entries[1].display());
    }
}
