//! The Poem HTTP server: shared by every binary that serves the GraphQL API.
//!
//! Lives in the library rather than in `bin/poem.rs` because there are two
//! servers — [`crate::bin`]`/poem.rs` against real AWS, and `poem-local.rs`
//! against the in-process mocks — and they must not drift apart. The binaries
//! differ only in which [`crate::queue::Handler`] and [`crate::mail::Handler`]
//! they construct; everything else is here.

use anyhow::Result;
use async_graphql::ServerError;
use async_graphql::{EmptySubscription, http::GraphiQLSource};
use async_graphql_poem::*;
use clap::Parser;
use poem::EndpointExt;
use poem::http::{HeaderMap, StatusCode};
use poem::middleware::Cors;
use poem::web::Data;
use poem::{IntoResponse, Route, Server, get, handler, listener::TcpListener, web::Html};
use std::env;
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::info;

use crate::app::{self, MyApp};
use crate::auth;
use crate::db;
use crate::graphql;
use crate::jwt;
use crate::mail;
use crate::queue;
use crate::request_metrics::{self, RequestMetrics};
use crate::telemetry;

type Schema<H, Q, M> = async_graphql::Schema<
    graphql::QueryRoot<MyApp<H, Q, M>>,
    graphql::MutationRoot<MyApp<H, Q, M>>,
    EmptySubscription,
>;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// JWT expiry time in seconds (default: 1h in dev)
    #[arg(short, long, default_value_t = 3600)]
    pub user_expiry_s: u64,

    /// JWT expiry time for sessions in seconds (default: 1h in dev)
    #[arg(short, long, default_value_t = 1209600)]
    pub session_expiry_s: u64,

    /// Inject lag for all responses
    #[arg(short, long, default_value_t = 0)]
    pub response_lag_ms: u64,
    /// Enable mutations (disable read-only mode)
    #[arg(long, default_value_t = false)]
    pub enable_mutations: bool,

    /// DEV ONLY: bypass auth and treat every request as this session (kiosk) record id.
    /// Mutually exclusive with --dev-auth-user.
    #[arg(long, value_name = "SESSION_ID")]
    pub dev_auth_session: Option<String>,

    /// DEV ONLY: bypass auth and treat every request as this user, given by record id
    /// or email. Mutually exclusive with --dev-auth-session.
    #[arg(long, value_name = "USER_ID_OR_EMAIL")]
    pub dev_auth_user: Option<String>,
}

/// Everything a server needs that doesn't depend on which backends it uses.
pub struct Startup {
    pub cli: Cli,
    pub key: jwt::Key,
    pub dev_auth: Option<auth::DevAuthConfig>,
    pub db_prefix: String,
}

/// Set up logging, load `.env`, parse the command line, and read the config every
/// server needs. Call this first; it must run before any backend is constructed,
/// because that's what puts `.env` into the environment.
pub fn init() -> Result<Startup, Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    crate::load_cli_env();

    let cli = Cli::parse();

    let var = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let secret = var.trim();
    let key = jwt::Key::new(secret, Some(cli.user_expiry_s), Some(cli.session_expiry_s))?;

    let dev_auth = match (&cli.dev_auth_session, &cli.dev_auth_user) {
        (Some(_), Some(_)) => {
            return Err("--dev-auth-session and --dev-auth-user are mutually exclusive".into());
        }
        (Some(id), None) => Some(auth::DevAuthConfig::Session { id: id.clone() }),
        (None, Some(id_or_email)) => Some(auth::DevAuthConfig::User {
            id_or_email: id_or_email.clone(),
        }),
        (None, None) => None,
    };
    if dev_auth.is_some() {
        tracing::warn!(
            "DEV AUTH OVERRIDE ENABLED: token verification is bypassed for all requests. \
             Never use this in a deployed environment."
        );
    }

    let db_prefix = env::var("DB_PREFIX").expect("DB_PREFIX must be set for dynamodb backend");

    Ok(Startup {
        cli,
        key,
        dev_auth,
        db_prefix,
    })
}

#[handler]
async fn index<H, Q, M>(
    schema: Data<&Schema<H, Q, M>>,
    app: Data<&Arc<MyApp<H, Q, M>>>,
    dev_auth: Data<&Arc<Option<auth::DevAuthConfig>>>,
    headers: &HeaderMap,
    body: Vec<u8>,
) -> impl IntoResponse
where
    H: db::Handler + Send + Sync + 'static,
    Q: queue::Handler + Send + Sync + 'static,
    M: mail::Handler + Send + Sync + 'static,
{
    if app.response_lag > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(app.response_lag)).await;
    }

    // Read the raw body so its hash can be bound into signed-key auth, then parse it as
    // a GraphQL request. This POST-only handler always receives a JSON body from the
    // GraphQL client (GraphiQL is served separately on GET).
    let body_hash = crate::session_key::sha256_hex(&body);
    let mut req: async_graphql::Request = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            info!("Malformed GraphQL request body: {}", e);
            let response = GraphQLResponse(async_graphql::Response::from_errors(vec![
                ServerError::new("Malformed request body", None),
            ]));
            return response
                .with_status(StatusCode::BAD_REQUEST)
                .into_response();
        }
    };

    let header = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
    let client = crate::client_info::ClientReport::from_headers(
        header(auth::CLIENT_VERSION_HEADER),
        header(crate::client_info::CLIENT_INFO_HEADER),
        header("User-Agent"),
        crate::clock::now_ms(),
    );
    let mut caller_type = auth::CallerType::Unauthenticated;
    let mut caller_id = String::from("unknown");
    if let Some(cfg) = (***dev_auth).as_ref() {
        // Dev-only override: skip token verification and act as the configured caller.
        match auth::resolve_dev_auth(&***app, cfg).await {
            Ok(auth_info) => {
                (caller_type, caller_id) = auth::caller_info(Some(&auth_info));
                req = req.data(auth_info);
            }
            Err(e) => {
                tracing::error!("Dev auth override failed: {}", e);
                let response = GraphQLResponse(async_graphql::Response::from_errors(vec![
                    ServerError::new("Dev auth override failed", None),
                ]));
                return response
                    .with_status(StatusCode::UNAUTHORIZED)
                    .into_response();
            }
        }
    } else if let Some(res) = auth::verify_authorization_header(
        &***app,
        headers
            .get("Authorization")
            .and_then(|value| value.to_str().ok()),
        &body_hash,
        &client,
    )
    .await
    {
        let auth_info = match res {
            Err(auth::AuthError::Permanent(ref msg)) => {
                info!("Auth permanent failure: {}", msg);
                let response = GraphQLResponse(async_graphql::Response::from_errors(vec![
                    ServerError::new("Authentication failed", None),
                ]));
                return response
                    .with_status(StatusCode::UNAUTHORIZED)
                    .into_response();
            }
            Err(auth::AuthError::Transient(ref msg)) => {
                tracing::error!("Transient auth error: {}", msg);
                let response = GraphQLResponse(async_graphql::Response::from_errors(vec![
                    ServerError::new("Service temporarily unavailable", None),
                ]));
                return response
                    .with_status(StatusCode::SERVICE_UNAVAILABLE)
                    .into_response();
            }
            Ok(v) => v,
        };
        (caller_type, caller_id) = auth::caller_info(Some(&auth_info));
        req = req.data(auth_info);
    }
    req = req
        .data(app.clone())
        .data(graphql::ClientIp::from_forwarded_for(
            headers
                .get("x-forwarded-for")
                .and_then(|value| value.to_str().ok()),
        ))
        .data(graphql::get_dataloader(app.clone()));

    let operation_context = telemetry::extract_operation_context(&mut req);
    let request_start = Instant::now();
    let metrics = Arc::new(RequestMetrics::default());
    let gql_response = request_metrics::METRICS
        .scope(metrics.clone(), schema.execute(req))
        .await;
    let gql_error_count = gql_response.errors.len();
    let response = GraphQLResponse(gql_response).into_response();

    telemetry::RequestTelemetry {
        status: response.status().as_u16(),
        operation_type: operation_context.operation_type,
        operation_name: operation_context.operation_name(),
        caller_type,
        caller_id: &caller_id,
        latency_ms: request_start.elapsed().as_secs_f64() * 1000.0,
        graphql_error_count: gql_error_count,
        ..Default::default()
    }
    .with_metrics(&metrics)
    .emit();
    response
}

#[handler]
async fn graphiql() -> impl IntoResponse {
    Html(GraphiQLSource::build().finish())
}

/// Serve the GraphQL API on :8000 until interrupted.
pub async fn run<H, Q, M>(
    startup: Startup,
    db: H,
    queues: Q,
    mailer: M,
) -> Result<(), Box<dyn Error>>
where
    H: db::Handler + Send + Sync + 'static,
    Q: queue::Handler + Send + Sync + 'static,
    M: mail::Handler + Send + Sync + 'static,
{
    let Startup {
        cli, key, dev_auth, ..
    } = startup;
    let webauthn = Arc::new(app::build_webauthn()?);
    let app = Arc::new(app::new(db, key, cli.response_lag_ms, queues, mailer));
    let schema = graphql::build_schema(app.clone(), webauthn);
    std::fs::write("schema.graphql", schema.sdl())?;
    let allow_cross_origin = Cors::new();
    let routes = Route::new()
        .at(
            "/",
            get(graphiql).post(index::<H, Q, M> {
                ..Default::default()
            }),
        )
        .with(allow_cross_origin)
        .data(schema)
        .data(app)
        .data(Arc::new(dev_auth));
    info!("GraphiQL: http://localhost:8000");
    Server::new(TcpListener::bind("0.0.0.0:8000"))
        .idle_timeout(Duration::from_secs(60))
        .run(routes)
        .await?;
    Ok(())
}
