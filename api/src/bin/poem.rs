use anyhow::Result;
use async_graphql::ServerError;
use clap::Parser;
use poem::EndpointExt;
use poem::middleware::Cors;
use seslogin::app::MyApp;
use std::env;
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::info;

use seslogin::app;
use seslogin::auth;
use seslogin::db;
use seslogin::dynamodb;
use seslogin::emf;
use seslogin::graphql;
use seslogin::jwt;
use seslogin::request_metrics::{self, RequestMetrics};
use seslogin::sqs_dispatch::{SqsQueue, SqsQueues};

use async_graphql::{EmptySubscription, http::GraphiQLSource};
use async_graphql_poem::*;
use poem::http::{HeaderMap, StatusCode};
use poem::web::Data;
use poem::{IntoResponse, Route, Server, get, handler, listener::TcpListener, web::Html};

type Schema<H> = async_graphql::Schema<
    graphql::QueryRoot<MyApp<H>>,
    graphql::MutationRoot<MyApp<H>>,
    EmptySubscription,
>;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// JWT expiry time in seconds (default: 1h in dev)
    #[arg(short, long, default_value_t = 3600)]
    user_expiry_s: u64,

    /// JWT expiry time for sessions in seconds (default: 1h in dev)
    #[arg(short, long, default_value_t = 1209600)]
    session_expiry_s: u64,

    /// Inject lag for all responses
    #[arg(short, long, default_value_t = 0)]
    response_lag_ms: u64,
    /// Enable mutations (disable read-only mode)
    #[arg(long, default_value_t = false)]
    enable_mutations: bool,

    /// DEV ONLY: bypass auth and treat every request as this session (kiosk) record id.
    /// Mutually exclusive with --dev-auth-user.
    #[arg(long, value_name = "SESSION_ID")]
    dev_auth_session: Option<String>,

    /// DEV ONLY: bypass auth and treat every request as this user, given by record id
    /// or email. Mutually exclusive with --dev-auth-session.
    #[arg(long, value_name = "USER_ID_OR_EMAIL")]
    dev_auth_user: Option<String>,
}

#[handler]
async fn index<H: db::Handler + Send + Sync + 'static>(
    schema: Data<&Schema<H>>,
    app: Data<&Arc<app::MyApp<H>>>,
    dev_auth: Data<&Arc<Option<auth::DevAuthConfig>>>,
    headers: &HeaderMap,
    body: Vec<u8>,
) -> impl IntoResponse {
    if app.response_lag > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(app.response_lag)).await;
    }

    // Read the raw body so its hash can be bound into signed-key auth, then parse it as
    // a GraphQL request. This POST-only handler always receives a JSON body from the
    // GraphQL client (GraphiQL is served separately on GET).
    let body_hash = seslogin::session_key::sha256_hex(&body);
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

    let client_version = headers
        .get(auth::CLIENT_VERSION_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut caller_type = "unauthenticated";
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
        client_version,
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
        .data(graphql::get_dataloader(app.clone()));

    let operation_context = emf::extract_operation_context(&req);
    let request_start = Instant::now();
    let metrics = Arc::new(RequestMetrics::default());
    let gql_response = request_metrics::METRICS
        .scope(metrics.clone(), schema.execute(req))
        .await;
    let gql_error_count = gql_response.errors.len();
    let response = GraphQLResponse(gql_response).into_response();

    emf::RequestTelemetry {
        status: response.status().as_u16(),
        operation_type: operation_context.operation_type,
        operation_name: operation_context
            .operation_name
            .as_deref()
            .unwrap_or("unknown"),
        caller_type,
        caller_id: &caller_id,
        latency_ms: request_start.elapsed().as_secs_f64() * 1000.0,
        graphql_error_count: gql_error_count,
        query_failures: metrics.query_failures(),
        mutation_failures: metrics.mutation_failures(),
        rru: metrics.read_units(),
        wru: metrics.write_units(),
        ddb_calls: metrics.ddb_calls(),
        auth_error: "",
    }
    .emit();
    response
}

#[handler]
async fn graphiql() -> impl IntoResponse {
    Html(GraphiQLSource::build().finish())
}

async fn run_server<H: db::Handler + Send + Sync + 'static>(
    db: H,
    key: jwt::Key,
    response_lag_ms: u64,
    sqs: SqsQueues,
    dev_auth: Option<auth::DevAuthConfig>,
) -> Result<(), Box<dyn Error>> {
    let webauthn = Arc::new(app::build_webauthn()?);
    let app = Arc::new(app::new(db, key, response_lag_ms, sqs));
    let schema = graphql::build_schema(app.clone(), webauthn);
    std::fs::write("schema.graphql", schema.sdl())?;
    let allow_cross_origin = Cors::new();
    let routes = Route::new()
        .at(
            "/",
            get(graphiql).post(index::<H> {
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    seslogin::load_cli_env();

    let cli = Cli::parse();

    let var = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let secret = var.trim();
    let key = jwt::Key::new(secret, Some(cli.user_expiry_s), Some(cli.session_expiry_s))?;

    let member_sync_queue_url =
        env::var("MEMBER_SYNC_QUEUE_URL").expect("MEMBER_SYNC_QUEUE_URL must be set");
    let nitc_export_queue_url =
        env::var("NITC_EXPORT_QUEUE_URL").expect("NITC_EXPORT_QUEUE_URL must be set");
    let healthcheck_queue_url =
        env::var("HEALTHCHECK_QUEUE_URL").expect("HEALTHCHECK_QUEUE_URL must be set");
    let aws_cfg = seslogin::aws_config_loader().load().await;
    let sqs_client = aws_sdk_sqs::Client::new(&aws_cfg);
    let sqs = SqsQueues {
        member_sync: SqsQueue {
            client: sqs_client.clone(),
            queue_url: member_sync_queue_url,
        },
        nitc_export: SqsQueue {
            client: sqs_client.clone(),
            queue_url: nitc_export_queue_url,
        },
        healthcheck: SqsQueue {
            client: sqs_client,
            queue_url: healthcheck_queue_url,
        },
    };

    let dev_auth = match (cli.dev_auth_session, cli.dev_auth_user) {
        (Some(_), Some(_)) => {
            return Err("--dev-auth-session and --dev-auth-user are mutually exclusive".into());
        }
        (Some(id), None) => Some(auth::DevAuthConfig::Session { id }),
        (None, Some(id_or_email)) => Some(auth::DevAuthConfig::User { id_or_email }),
        (None, None) => None,
    };
    if dev_auth.is_some() {
        tracing::warn!(
            "DEV AUTH OVERRIDE ENABLED: token verification is bypassed for all requests. \
             Never use this in a deployed environment."
        );
    }

    let db_prefix = env::var("DB_PREFIX").expect("DB_PREFIX must be set for dynamodb backend");
    let db = dynamodb::Handler::new(&db_prefix, !cli.enable_mutations).await;
    run_server(db, key, cli.response_lag_ms, sqs, dev_auth).await
}
