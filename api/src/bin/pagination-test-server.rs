use anyhow::Result;
use async_graphql::connection::{Connection, EmptyFields};
use async_graphql::http::GraphiQLSource;
use async_graphql::{EmptyMutation, EmptySubscription, Enum, ID, Object, Schema};
use async_graphql_poem::*;
use clap::Parser;
use poem::middleware::Cors;
use poem::web::{Data, Html};
use poem::{EndpointExt, IntoResponse, Route, Server, get, handler, listener::TcpListener};
use seslogin::db;
use seslogin::dynamodb;
use seslogin::graphql::pagination::{build_connection, pagination_args};
use std::marker::PhantomData;
use std::sync::Arc;
use tracing::info;

#[derive(Parser)]
#[command(about = "Test pagination GraphQL server (no auth)")]
struct Cli {
    #[arg(short, long, default_value_t = 8001)]
    port: u16,
    /// Write schema SDL to this path on startup (signals readiness to test driver)
    #[arg(long)]
    schema_out: Option<String>,
}

// ── GraphQL types ─────────────────────────────────────────────────────────────

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
enum TestPaginationParity {
    Odd,
    Even,
}

struct TestPaginationRow(db::TestPaginationRow);

#[Object]
impl TestPaginationRow {
    async fn id(&self) -> ID {
        ID(self.0.id.clone())
    }
    async fn number(&self) -> i32 {
        self.0.number as i32
    }
    async fn name(&self) -> &str {
        &self.0.name
    }
    /// Present (value 1) when number is odd, absent otherwise.
    async fn odd(&self) -> Option<i32> {
        self.0.odd.map(|v| v as i32)
    }
    /// Present ("y") when number is even, absent otherwise.
    async fn even(&self) -> Option<&str> {
        self.0.even.as_deref()
    }
    /// number % 5
    async fn mod5(&self) -> i32 {
        self.0.mod5 as i32
    }
}

// ── Cursor helpers ────────────────────────────────────────────────────────────

const DEFAULT_PAGE_SIZE: usize = 5;
const MAX_PAGE_SIZE: usize = 100;

fn encode_cursor(row: &db::TestPaginationRow) -> String {
    format!("{}:{}", row.number, row.id)
}

fn decode_cursor(cursor: &str) -> anyhow::Result<db::TestPaginationCursor> {
    let mut parts = cursor.splitn(2, ':');
    let number = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Invalid cursor"))?
        .parse::<i64>()
        .map_err(|_| anyhow::anyhow!("Invalid cursor"))?;
    let id = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Invalid cursor"))?;
    if id.is_empty() {
        return Err(anyhow::anyhow!("Invalid cursor"));
    }
    Ok(db::TestPaginationCursor {
        number,
        id: id.to_string(),
    })
}

// ── Query root ────────────────────────────────────────────────────────────────

struct QueryRoot<H: db::Handler + Send + Sync + 'static> {
    _marker: PhantomData<H>,
}

impl<H: db::Handler + Send + Sync + 'static> Default for QueryRoot<H> {
    fn default() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[Object]
impl<H: db::Handler + Send + Sync + 'static> QueryRoot<H> {
    /// Paginated list of rows from the test_pagination fixture table, sorted
    /// ascending by number via the group_id-number GSI (group_id is always 1).
    /// Default page size is 5; maximum is 100.
    ///
    /// When parity is set, DynamoDB applies the filter after evaluating the page
    /// limit, so pages may contain fewer items than requested — this is
    /// intentional and part of what the integration test exercises.
    async fn test_pagination(
        &self,
        ctx: &async_graphql::Context<'_>,
        #[graphql(desc = "Return only odd or only even rows.")] parity: Option<
            TestPaginationParity,
        >,
        after: Option<String>,
        before: Option<String>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> anyhow::Result<Connection<String, TestPaginationRow, EmptyFields, EmptyFields>> {
        let after_cursor = after.as_deref().map(decode_cursor).transpose()?;
        let before_cursor = before.as_deref().map(decode_cursor).transpose()?;
        let has_after = after_cursor.is_some();
        let has_before = before_cursor.is_some();

        let (page_size, is_last_mode) =
            pagination_args(first, last, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE)?;
        let fetch_limit = i32::try_from(page_size.saturating_add(1))
            .map_err(|_| anyhow::anyhow!("Page size too large"))?;

        let filter = parity.map(|p| match p {
            TestPaginationParity::Odd => db::TestPaginationFilter::OddOnly,
            TestPaginationParity::Even => db::TestPaginationFilter::EvenOnly,
        });

        let db = ctx.data_unchecked::<Arc<H>>();
        let items = db
            .list_test_pagination(db::ListTestPaginationPage {
                after: after_cursor,
                before: before_cursor,
                limit: fetch_limit,
                // ascending (first) = false; descending (last) = true — scanned
                // backward then reversed below so the client always receives
                // rows in ascending-number order.
                descending: is_last_mode,
                filter,
            })
            .await?;

        Ok(build_connection(
            items,
            page_size,
            is_last_mode,
            has_after,
            has_before,
            |row| (encode_cursor(row), TestPaginationRow(row.clone())),
        ))
    }
}

// ── Server ────────────────────────────────────────────────────────────────────

type AppSchema<H> = Schema<QueryRoot<H>, EmptyMutation, EmptySubscription>;

#[handler]
async fn index<H: db::Handler + Send + Sync + 'static>(
    schema: Data<&AppSchema<H>>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.0).await.into()
}

#[handler]
async fn graphiql() -> impl IntoResponse {
    Html(GraphiQLSource::build().finish())
}

async fn run_server<H: db::Handler + Send + Sync + 'static>(
    db: H,
    port: u16,
    schema_out: Option<String>,
) -> Result<()> {
    let schema = Schema::build(QueryRoot::<H>::default(), EmptyMutation, EmptySubscription)
        .data(Arc::new(db))
        .finish();

    if let Some(path) = schema_out {
        std::fs::write(&path, schema.sdl())?;
        info!("Schema written to {}", path);
    }

    let routes = Route::new()
        .at(
            "/",
            get(graphiql).post(index::<H> {
                ..Default::default()
            }),
        )
        .with(Cors::new())
        .data(schema);

    info!("Test pagination GraphiQL: http://localhost:{}", port);
    Server::new(TcpListener::bind(format!("0.0.0.0:{}", port)))
        .run(routes)
        .await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    seslogin::load_cli_env();

    let cli = Cli::parse();
    let db_prefix = std::env::var("DB_PREFIX").expect("DB_PREFIX must be set");
    let db = dynamodb::Handler::new(&db_prefix, true).await;
    run_server(db, cli.port, cli.schema_out).await
}
