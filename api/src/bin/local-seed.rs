//! Populate a local DynamoDB with enough data to actually use the app.
//!
//! Two steps, deliberately separate:
//!
//!   extract  Read the *reference* data — categories and NITC groups — out of a
//!            real database (needs AWS access) and write it to a checked-in
//!            fixture. Run rarely.
//!   apply    Write that fixture, plus the hand-written records in
//!            `synthetic.json`, into the local database. Needs no AWS at all.
//!
//! Only reference data is extracted. Locations, members, users, tokens and
//! sessions are invented in `synthetic.json`, so no committed fixture describes a
//! real unit or a real person — which is what makes this safe in a public repo,
//! rather than relying on a scrub of rows copied out of production.
//!
//! The split is what keeps `make dev-local` credential-free: only `extract` ever
//! talks to a real account, and its output is committed.
//!
//! Rows are copied as **raw DynamoDB items**, not through `db::Handler`. That
//! preserves record IDs (which the trait's `create_*` methods generate) and the
//! exact attribute shape, including the project's rule that an absent optional
//! attribute is omitted rather than written as `Null`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use aws_sdk_dynamodb::Client;
use aws_sdk_dynamodb::types::AttributeValue;
use base64::Engine as _;
use clap::{Parser, Subcommand};
use serde_json::{Map, Value, json};

/// Repo root, resolved at compile time so the fixture path doesn't depend on cwd.
const REPO_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

/// Attributes that must never appear in a committed fixture. This repo is public.
const PERSONAL_ATTRIBUTES: &[&str] = &["email", "ses_api_person_id"];

#[derive(Parser)]
#[command(about = "Seed a local DynamoDB with test data")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Read reference data from a real database and write the fixture. Needs AWS access.
    Extract {
        /// Database to read from. Read-only; nothing is ever written to it.
        #[arg(long, default_value = "seslogin_prod")]
        source_prefix: String,

        /// Write the fixture even if it contains personal data. Prints what it found.
        /// Only for a database you have checked yourself — this repo is public.
        #[arg(long)]
        allow_personal_data: bool,
    },
    /// Write the fixtures into the local database. Needs no AWS access.
    Apply {
        /// Write only this table's rows, eg `user_token` to restore the seeded logins after a logout.
        #[arg(long)]
        table: Option<String>,
    },
    /// Delete every row the app itself writes — periods, ephemeral state, passkeys, the audit log —
    /// leaving the seeded fixtures alone. Needs no AWS access.
    ///
    /// `apply` cannot do this: it only ever `put_item`s the fixture rows, and no
    /// fixture describes a period, so a sign-in left behind by a script survives
    /// every reseed. That matters because an open period changes what the *next*
    /// run does — scanning a member who is already signed in signs them out
    /// instead of in — so a script that half-finished quietly breaks the one after
    /// it. `local-reset` fixes it too, by destroying and rebuilding every table.
    Clear,
    /// Write `demo.json` too: Test C Unit with months of realistic activity, for a dashboard that looks
    /// lived in. Its times are relative to now, so re-running it brings the data up to date.
    Demo,
}

// ── DynamoDB item <-> JSON ────────────────────────────────────────────────────
// The wire shape (`{"S": "x"}`) rather than anything friendlier, so a row
// round-trips through the fixture byte-for-byte.

fn av_to_json(av: &AttributeValue) -> Result<Value> {
    Ok(match av {
        AttributeValue::S(s) => json!({ "S": s }),
        AttributeValue::N(n) => json!({ "N": n }),
        AttributeValue::Bool(b) => json!({ "BOOL": b }),
        AttributeValue::Null(_) => json!({ "NULL": true }),
        AttributeValue::Ss(v) => json!({ "SS": v }),
        AttributeValue::Ns(v) => json!({ "NS": v }),
        AttributeValue::B(b) => {
            json!({ "B": base64::engine::general_purpose::STANDARD.encode(b.as_ref()) })
        }
        AttributeValue::Bs(v) => json!({ "BS": v.iter()
            .map(|b| base64::engine::general_purpose::STANDARD.encode(b.as_ref()))
            .collect::<Vec<_>>() }),
        AttributeValue::L(v) => {
            json!({ "L": v.iter().map(av_to_json).collect::<Result<Vec<_>>>()? })
        }
        AttributeValue::M(m) => json!({ "M": item_to_json(m)? }),
        other => bail!("unsupported attribute type in source data: {other:?}"),
    })
}

fn item_to_json(item: &HashMap<String, AttributeValue>) -> Result<Map<String, Value>> {
    let mut out = Map::new();
    // Sorted so a re-extract produces a diffable fixture rather than random order.
    let mut keys: Vec<&String> = item.keys().collect();
    keys.sort();
    for k in keys {
        out.insert(k.clone(), av_to_json(&item[k])?);
    }
    Ok(out)
}

fn json_to_av(value: &Value) -> Result<AttributeValue> {
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow!("attribute value must be an object, got {value}"))?;
    let (tag, v) = obj
        .iter()
        .next()
        .ok_or_else(|| anyhow!("attribute value object is empty"))?;
    let str_list = |v: &Value| -> Result<Vec<String>> {
        v.as_array()
            .ok_or_else(|| anyhow!("{tag} must be an array"))?
            .iter()
            .map(|e| {
                e.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| anyhow!("{tag} entries must be strings"))
            })
            .collect()
    };
    Ok(match tag.as_str() {
        "S" => AttributeValue::S(
            v.as_str()
                .ok_or_else(|| anyhow!("S must be a string"))?
                .to_string(),
        ),
        // Numbers stay strings on the wire; that is how DynamoDB avoids float loss. A string starting with a
        // letter is a time relative to this run (`now-2h`, `monday-3w+1d+19h`).
        "N" => {
            let n = v
                .as_str()
                .ok_or_else(|| anyhow!("N must be a string, not a JSON number"))?;
            AttributeValue::N(if n.starts_with(|c: char| c.is_ascii_alphabetic()) {
                seslogin::local_dev::resolve_relative_time(n, *NOW)?.to_string()
            } else {
                n.to_string()
            })
        }
        "BOOL" => AttributeValue::Bool(v.as_bool().ok_or_else(|| anyhow!("BOOL must be a bool"))?),
        "NULL" => AttributeValue::Null(true),
        "SS" => AttributeValue::Ss(str_list(v)?),
        "NS" => AttributeValue::Ns(str_list(v)?),
        "B" => AttributeValue::B(aws_sdk_dynamodb::primitives::Blob::new(
            base64::engine::general_purpose::STANDARD
                .decode(v.as_str().ok_or_else(|| anyhow!("B must be a string"))?)?,
        )),
        "L" => AttributeValue::L(
            v.as_array()
                .ok_or_else(|| anyhow!("L must be an array"))?
                .iter()
                .map(json_to_av)
                .collect::<Result<Vec<_>>>()?,
        ),
        "M" => AttributeValue::M(json_to_item(
            v.as_object()
                .ok_or_else(|| anyhow!("M must be an object"))?,
        )?),
        other => bail!("unsupported attribute tag {other:?} in fixture"),
    })
}

/// Keys starting with `_` are notes for whoever is reading the fixture, not
/// attributes — `synthetic.json` uses them to explain what each row is for.
fn json_to_item(obj: &Map<String, Value>) -> Result<HashMap<String, AttributeValue>> {
    obj.iter()
        .filter(|(k, _)| !k.starts_with('_'))
        .map(|(k, v)| Ok((k.clone(), json_to_av(v)?)))
        .collect()
}

// ── Extract ───────────────────────────────────────────────────────────────────

async fn scan_all(client: &Client, table: &str) -> Result<Vec<Map<String, Value>>> {
    let mut out = Vec::new();
    let mut start_key = None;
    loop {
        let resp = client
            .scan()
            .table_name(table)
            .set_exclusive_start_key(start_key.clone())
            .send()
            .await
            .with_context(|| format!("scanning {table}"))?;
        for item in resp.items() {
            out.push(item_to_json(item)?);
        }
        start_key = resp.last_evaluated_key().cloned();
        if start_key.is_none() {
            break;
        }
    }
    Ok(out)
}

/// Drop soft-deleted rows.
///
/// Seed data is for working with, and a deleted record is invisible to the app
/// anyway. It also keeps the fixture honest: deleted rows are the ones most
/// likely to be leftovers nobody has looked at in years, and this repo is public.
fn drop_deleted(rows: Vec<Map<String, Value>>) -> (Vec<Map<String, Value>>, usize) {
    let before = rows.len();
    let kept: Vec<_> = rows
        .into_iter()
        .filter(|r| !r.contains_key("deleted"))
        .collect();
    let dropped = before - kept.len();
    (kept, dropped)
}

/// Report every extracted attribute that looks like personal data.
fn personal_data_findings(tables: &Map<String, Value>) -> Vec<String> {
    let mut found = Vec::new();
    for (table, rows) in tables {
        for row in rows.as_array().into_iter().flatten() {
            let Some(obj) = row.as_object() else { continue };
            let id = obj
                .get("id")
                .and_then(|v| v.get("S"))
                .and_then(Value::as_str)
                .unwrap_or("<no id>");
            for attr in PERSONAL_ATTRIBUTES {
                if let Some(v) = obj.get(*attr) {
                    found.push(format!("{table}/{id}: {attr} = {v}"));
                }
            }
        }
    }
    found
}

async fn extract(source_prefix: &str, allow_personal: bool) -> Result<()> {
    // Read-only handle: this reads production by default, and nothing here should
    // ever write to it.
    let client = seslogin::local_dev::dynamodb_client().await;
    let t = |name: &str| format!("{source_prefix}_{name}");

    let (categories, dropped_categories) = drop_deleted(scan_all(&client, &t("category")).await?);
    let (nitc_groups, dropped_groups) = drop_deleted(scan_all(&client, &t("nitc_group")).await?);
    let dropped = dropped_categories + dropped_groups;

    let mut tables = Map::new();
    tables.insert("category".into(), json!(categories));
    tables.insert("nitc_group".into(), json!(nitc_groups));

    let findings = personal_data_findings(&tables);
    if !findings.is_empty() {
        eprintln!("Personal data in the extracted rows:");
        for f in &findings {
            eprintln!("  {f}");
        }
        if !allow_personal {
            bail!(
                "refusing to write the fixture — it is committed to a public repository. \
                 Remove these records from the source, or re-run with --allow-personal-data \
                 if you have confirmed they are synthetic."
            );
        }
        eprintln!("--allow-personal-data given: writing anyway.");
    }

    for (name, rows) in &tables {
        println!(
            "{:>12}: {} row(s)",
            name,
            rows.as_array().map_or(0, Vec::len)
        );
    }
    if dropped > 0 {
        println!("(skipped {dropped} soft-deleted row(s))");
    }

    let doc = json!({
        "_comment": "Generated by `cargo run --bin local-seed -- extract`. Do not edit by hand. \
                     Reference data only - locations, members, users, tokens and sessions are \
                     invented in synthetic.json, so nothing here describes a real person or unit.",
        "source_prefix": source_prefix,
        "tables": tables,
    });
    let path = seed_dir().join("from-prod.json");
    std::fs::create_dir_all(seed_dir())?;
    std::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&doc)?))?;
    println!("wrote {}", path.display());
    Ok(())
}

// ── Apply ─────────────────────────────────────────────────────────────────────

fn seed_dir() -> PathBuf {
    Path::new(REPO_ROOT).join("local/seed")
}

fn load_tables(path: &Path) -> Result<Map<String, Value>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let doc: Value =
        serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(doc
        .get("tables")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("{} has no `tables` object", path.display()))?
        .clone())
}

/// Tables the running app writes into that no fixture owns. Anything seeded is
/// left alone: `apply` overwrites those, so wiping them would only mean a
/// mandatory reseed. Keep this in step with `local-tables`.
const TRANSIENT_TABLES: &[&str] = &[
    "period",
    "ephemeral_state",
    "webauthn_credential",
    "audit_log",
    "task",
    "task_list",
    "note",
    "note_folder",
    "equipment",
    "equipment_log",
    "equipment_reservation",
    "on_call_role",
    "shared_contact",
    "map_pin",
    "map_layer",
    "calendar",
    "calendar_event",
    "location_share",
    "notification",
    "trophy",
    "asset",
    "asset_ref",
    "note_ref",
    "photo",
];

async fn clear() -> Result<()> {
    let endpoint = seslogin::local_dev::require_local_dynamodb_endpoint()?;
    let prefix = std::env::var("DB_PREFIX").map_err(|_| anyhow!("DB_PREFIX must be set"))?;
    let client = seslogin::local_dev::dynamodb_client().await;
    println!("clearing {prefix}_* at {endpoint}");

    let mut total = 0usize;
    for table in TRANSIENT_TABLES {
        let name = format!("{prefix}_{table}");
        // Most tables are keyed by `id` alone; `notification`, `trophy`, `asset_ref` and `note_ref` have a range key too.
        let key_attrs: Vec<String> = client
            .describe_table()
            .table_name(&name)
            .send()
            .await
            .with_context(|| format!("describing {name}"))?
            .table()
            .map(|t| {
                t.key_schema()
                    .iter()
                    .map(|k| k.attribute_name().to_string())
                    .collect()
            })
            .unwrap_or_default();
        let mut deleted = 0usize;
        let mut start_key = None;
        loop {
            let page = client
                .scan()
                .table_name(&name)
                // Only the key is needed to delete, and a period row is large.
                .projection_expression(key_attrs.join(", "))
                .set_exclusive_start_key(start_key.clone())
                .send()
                .await
                .with_context(|| format!("scanning {name}"))?;
            for item in page.items() {
                let mut delete = client.delete_item().table_name(&name);
                for attr in &key_attrs {
                    let value = item
                        .get(attr)
                        .ok_or_else(|| anyhow!("{name}: a row has no `{attr}`"))?;
                    delete = delete.key(attr, value.clone());
                }
                delete
                    .send()
                    .await
                    .with_context(|| format!("deleting from {name}"))?;
                deleted += 1;
            }
            start_key = page.last_evaluated_key().cloned();
            if start_key.is_none() {
                break;
            }
        }
        println!("    {:>20}: {} row(s) deleted", table, deleted);
        total += deleted;
    }
    println!("{total} row(s) deleted");
    Ok(())
}

/// One moment for the whole run, so every relative time in it resolves against the same now.
static NOW: std::sync::LazyLock<chrono::DateTime<chrono::Utc>> =
    std::sync::LazyLock::new(chrono::Utc::now);

/// Write one fixture's rows, or only `only`'s table; returns how many were written.
async fn write_fixture(
    client: &Client,
    prefix: &str,
    file: &str,
    only: Option<&str>,
) -> Result<usize> {
    let tables = load_tables(&seed_dir().join(file))?;
    println!("  {file}");
    let now = NOW.timestamp();
    let in_future = |n: Option<&AttributeValue>| matches!(n, Some(AttributeValue::N(n)) if n.parse::<i64>().is_ok_and(|t| t > now));
    let mut total = 0;
    for (table, rows) in &tables {
        if only.is_some_and(|t| t != table) {
            continue;
        }
        let rows = rows
            .as_array()
            .ok_or_else(|| anyhow!("{file}: {table} must be an array"))?;
        let (mut written, mut skipped) = (0, 0);
        for row in rows {
            let obj = row
                .as_object()
                .ok_or_else(|| anyhow!("{file}: {table} rows must be objects"))?;
            let item = json_to_item(obj)?;
            // A relative time can land after now, eg this week's training night seeded on a Monday: not yet.
            // Delete the copy an earlier run wrote under this id too: it holds that run's dates, so left in place
            // it would sit a week early, on top of the previous week's own row.
            if table == "period"
                && (in_future(item.get("start_time")) || in_future(item.get("end_time")))
            {
                client
                    .delete_item()
                    .table_name(format!("{prefix}_{table}"))
                    .set_key(Some(item.into_iter().filter(|(k, _)| k == "id").collect()))
                    .send()
                    .await
                    .with_context(|| format!("removing a not-yet-due {table} row from {file}"))?;
                skipped += 1;
                continue;
            }
            client
                .put_item()
                .table_name(format!("{prefix}_{table}"))
                .set_item(Some(item))
                .send()
                .await
                .with_context(|| format!("writing a {table} row from {file}"))?;
            written += 1;
        }
        if skipped > 0 {
            println!("    {table:>12}: {written} row(s), {skipped} not due yet");
        } else {
            println!("    {table:>12}: {written} row(s)");
        }
        total += written;
    }
    Ok(total)
}

async fn apply(only: Option<&str>) -> Result<()> {
    let endpoint = seslogin::local_dev::require_local_dynamodb_endpoint()?;
    let prefix = std::env::var("DB_PREFIX").map_err(|_| anyhow!("DB_PREFIX must be set"))?;
    let client = seslogin::local_dev::dynamodb_client().await;
    println!("seeding {prefix}_* at {endpoint}");
    let mut total = 0;
    for file in ["from-prod.json", "synthetic.json"] {
        total += write_fixture(&client, &prefix, file, only).await?;
    }
    if let Some(t) = only
        && total == 0
    {
        bail!("no fixture rows for table `{t}`");
    }
    println!("{total} row(s) written");
    Ok(())
}

async fn demo() -> Result<()> {
    let endpoint = seslogin::local_dev::require_local_dynamodb_endpoint()?;
    let prefix = std::env::var("DB_PREFIX").map_err(|_| anyhow!("DB_PREFIX must be set"))?;
    let client = seslogin::local_dev::dynamodb_client().await;
    println!("seeding demo data into {prefix}_* at {endpoint}");
    let total = write_fixture(&client, &prefix, "demo.json", None).await?;
    println!("{total} row(s) written");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    seslogin::load_cli_env();
    tracing_subscriber::fmt::init();
    match Cli::parse().command {
        Command::Extract {
            source_prefix,
            allow_personal_data,
        } => extract(&source_prefix, allow_personal_data).await,
        Command::Apply { table } => apply(table.as_deref()).await,
        Command::Clear => clear().await,
        Command::Demo => demo().await,
    }
}
