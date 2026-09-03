//! Create the DynamoDB tables in a local DynamoDB (`docker compose -f local/docker-compose.yml up`).
//!
//! The schema here is transcribed from `infra/dynamodb_test.tf` — that file, not this
//! one, is the source of truth for the deployed tables. `--check` diffs a live local
//! DynamoDB against this list so drift shows up as a failing command rather than a
//! confusing `ValidationException` mid-session.
//!
//! Refuses to run unless the DynamoDB endpoint points at localhost, so a stray
//! `DB_PREFIX` can never create tables in a real AWS account.

use anyhow::{Result, anyhow, bail};
use aws_sdk_dynamodb::Client;
use aws_sdk_dynamodb::types::{
    AttributeDefinition, BillingMode, GlobalSecondaryIndex, KeySchemaElement, KeyType, Projection,
    ProjectionType, ScalarAttributeType, TimeToLiveSpecification,
};
use clap::Parser;

/// A key attribute and its DynamoDB scalar type.
#[derive(Clone, Copy)]
struct Attr(&'static str, ScalarKind);

#[derive(Clone, Copy, PartialEq)]
enum ScalarKind {
    S,
    N,
}

impl From<ScalarKind> for ScalarAttributeType {
    fn from(k: ScalarKind) -> Self {
        match k {
            ScalarKind::S => ScalarAttributeType::S,
            ScalarKind::N => ScalarAttributeType::N,
        }
    }
}

#[derive(Clone, Copy)]
struct Gsi {
    name: &'static str,
    hash: &'static str,
    range: Option<&'static str>,
    /// KEYS_ONLY when true, ALL when false. No table uses INCLUDE.
    keys_only: bool,
}

#[derive(Clone, Copy)]
struct Table {
    /// Suffix after `${DB_PREFIX}_`.
    name: &'static str,
    hash: &'static str,
    /// Every attribute used as a table or index key, with its type.
    attrs: &'static [Attr],
    gsis: &'static [Gsi],
    /// TTL attribute, if the table has TTL enabled.
    ttl: Option<&'static str>,
}

use ScalarKind::{N, S};

const fn all(name: &'static str, hash: &'static str, range: Option<&'static str>) -> Gsi {
    Gsi {
        name,
        hash,
        range,
        keys_only: false,
    }
}

const fn keys_only(name: &'static str, hash: &'static str) -> Gsi {
    Gsi {
        name,
        hash,
        range: None,
        keys_only: true,
    }
}

const TABLES: &[Table] = &[
    Table {
        name: "user",
        hash: "id",
        attrs: &[Attr("id", S), Attr("email", S)],
        gsis: &[keys_only("email-index", "email")],
        ttl: None,
    },
    Table {
        name: "category",
        hash: "id",
        attrs: &[Attr("id", S), Attr("nitc_group_id", S)],
        gsis: &[all("nitc_group_id-index", "nitc_group_id", None)],
        ttl: None,
    },
    Table {
        name: "location",
        hash: "id",
        attrs: &[Attr("id", S)],
        gsis: &[],
        ttl: None,
    },
    Table {
        name: "period",
        hash: "id",
        attrs: &[
            Attr("id", S),
            Attr("person_id", S),
            Attr("start_time", N),
            Attr("nitc_event_id", S),
            Attr("location_open", S),
            Attr("location_live", S),
        ],
        gsis: &[
            all(
                "person_id-start_time-index",
                "person_id",
                Some("start_time"),
            ),
            all("nitc_event_id-index", "nitc_event_id", None),
            all(
                "location_open-start_time-index",
                "location_open",
                Some("start_time"),
            ),
            all(
                "location_live-start_time-index",
                "location_live",
                Some("start_time"),
            ),
        ],
        ttl: None,
    },
    Table {
        name: "person",
        hash: "id",
        attrs: &[
            Attr("id", S),
            Attr("location_id", S),
            Attr("registration_number", S),
            Attr("ses_api_person_id", S),
        ],
        gsis: &[
            all("location_id-index", "location_id", None),
            keys_only("registration_number-index", "registration_number"),
            keys_only("ses_api_person_id-index", "ses_api_person_id"),
        ],
        ttl: None,
    },
    Table {
        name: "session",
        hash: "id",
        attrs: &[
            Attr("id", S),
            Attr("code", S),
            Attr("location_id", S),
            Attr("active", N),
            Attr("key_fingerprint", S),
        ],
        gsis: &[
            keys_only("code-index", "code"),
            all("active-location_id-index", "active", Some("location_id")),
            keys_only("key_fingerprint-index", "key_fingerprint"),
        ],
        ttl: None,
    },
    Table {
        name: "api_token",
        hash: "id",
        attrs: &[Attr("id", S), Attr("token_hash", S), Attr("active", N)],
        gsis: &[
            keys_only("token_hash-index", "token_hash"),
            all("active-index", "active", None),
        ],
        ttl: None,
    },
    Table {
        name: "nitc_group",
        hash: "id",
        attrs: &[Attr("id", S)],
        gsis: &[],
        ttl: None,
    },
    Table {
        name: "nitc_tag",
        hash: "id",
        attrs: &[Attr("id", S)],
        gsis: &[],
        ttl: None,
    },
    Table {
        name: "nitc_event",
        hash: "id",
        attrs: &[Attr("id", S), Attr("location_id", S), Attr("topic_date", S)],
        gsis: &[all(
            "location_id-topic_date-index",
            "location_id",
            Some("topic_date"),
        )],
        ttl: None,
    },
    Table {
        name: "login_code",
        hash: "email",
        attrs: &[Attr("email", S)],
        gsis: &[],
        ttl: Some("expires_at"),
    },
    Table {
        name: "user_token",
        hash: "id",
        attrs: &[Attr("id", S), Attr("token_hash", S)],
        gsis: &[keys_only("token_hash-index", "token_hash")],
        ttl: None,
    },
    Table {
        name: "webauthn_credential",
        hash: "id",
        attrs: &[Attr("id", S), Attr("user_id", S)],
        gsis: &[all("user_id-index", "user_id", None)],
        ttl: None,
    },
    Table {
        name: "ephemeral_state",
        hash: "id",
        attrs: &[Attr("id", S)],
        gsis: &[],
        ttl: Some("expires_at"),
    },
    Table {
        name: "test_pagination",
        hash: "id",
        attrs: &[Attr("id", S), Attr("group_id", N), Attr("number", N)],
        gsis: &[all("group_id-number-index", "group_id", Some("number"))],
        ttl: None,
    },
];

#[derive(Parser)]
#[command(about = "Create the seslogin tables in a local DynamoDB")]
struct Cli {
    /// Delete and recreate every table, discarding all local data.
    #[arg(long)]
    recreate: bool,

    /// Don't create anything; report which tables are missing and exit non-zero if any are.
    #[arg(long)]
    check: bool,
}

/// The endpoint the SDK will use, and whether it is unmistakably a local one.
///
/// This is the whole safety story for this binary: without an endpoint override the
/// SDK would happily talk to real DynamoDB with whatever credentials are lying around.
fn local_endpoint() -> Result<String> {
    let endpoint = std::env::var("AWS_ENDPOINT_URL_DYNAMODB")
        .or_else(|_| std::env::var("AWS_ENDPOINT_URL"))
        .map_err(|_| {
            anyhow!(
                "AWS_ENDPOINT_URL_DYNAMODB is not set — refusing to run in case this would \
                 create tables in a real AWS account. Use `make local-tables`, which points \
                 it at the container in local/docker-compose.yml."
            )
        })?;
    let url = url::Url::parse(&endpoint)
        .map_err(|e| anyhow!("AWS_ENDPOINT_URL_DYNAMODB is not a URL: {e}"))?;
    match url.host_str() {
        Some("localhost" | "127.0.0.1" | "::1" | "[::1]") => Ok(endpoint),
        other => bail!(
            "DynamoDB endpoint {:?} is not local (host {:?}) — refusing to run.",
            endpoint,
            other.unwrap_or("<none>")
        ),
    }
}

async fn create(client: &Client, prefix: &str, table: &Table) -> Result<()> {
    let full_name = format!("{prefix}_{}", table.name);
    let mut req = client
        .create_table()
        .table_name(&full_name)
        .billing_mode(BillingMode::PayPerRequest)
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name(table.hash)
                .key_type(KeyType::Hash)
                .build()?,
        );
    for Attr(name, kind) in table.attrs {
        req = req.attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name(*name)
                .attribute_type(ScalarAttributeType::from(*kind))
                .build()?,
        );
    }
    for gsi in table.gsis {
        let mut index = GlobalSecondaryIndex::builder()
            .index_name(gsi.name)
            .key_schema(
                KeySchemaElement::builder()
                    .attribute_name(gsi.hash)
                    .key_type(KeyType::Hash)
                    .build()?,
            )
            .projection(
                Projection::builder()
                    .projection_type(if gsi.keys_only {
                        ProjectionType::KeysOnly
                    } else {
                        ProjectionType::All
                    })
                    .build(),
            );
        if let Some(range) = gsi.range {
            index = index.key_schema(
                KeySchemaElement::builder()
                    .attribute_name(range)
                    .key_type(KeyType::Range)
                    .build()?,
            );
        }
        req = req.global_secondary_indexes(index.build()?);
    }
    req.send().await?;

    if let Some(ttl_attr) = table.ttl {
        client
            .update_time_to_live()
            .table_name(&full_name)
            .time_to_live_specification(
                TimeToLiveSpecification::builder()
                    .attribute_name(ttl_attr)
                    .enabled(true)
                    .build()?,
            )
            .send()
            .await?;
    }
    println!("created {full_name}");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    seslogin::load_cli_env();
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    let endpoint = local_endpoint()?;
    let prefix = std::env::var("DB_PREFIX").map_err(|_| anyhow!("DB_PREFIX must be set"))?;

    let config = seslogin::aws_config_loader()
        .region(aws_config::Region::new(
            std::env::var("AWS_REGION").unwrap_or_else(|_| "ap-southeast-2".to_string()),
        ))
        .load()
        .await;
    let client = Client::new(&config);

    let existing: Vec<String> = client
        .list_tables()
        .send()
        .await?
        .table_names
        .unwrap_or_default();
    println!(
        "{endpoint} has {} table(s), prefix {prefix}_",
        existing.len()
    );

    if cli.check {
        let missing: Vec<String> = TABLES
            .iter()
            .map(|t| format!("{prefix}_{}", t.name))
            .filter(|n| !existing.contains(n))
            .collect();
        if missing.is_empty() {
            println!("all {} tables present", TABLES.len());
            return Ok(());
        }
        bail!("missing {} table(s): {}", missing.len(), missing.join(", "));
    }

    for table in TABLES {
        let full_name = format!("{prefix}_{}", table.name);
        if existing.contains(&full_name) {
            if !cli.recreate {
                println!("exists  {full_name}");
                continue;
            }
            client.delete_table().table_name(&full_name).send().await?;
            println!("dropped {full_name}");
        }
        create(&client, &prefix, table).await?;
    }
    Ok(())
}
