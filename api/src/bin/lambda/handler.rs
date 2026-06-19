use std::fmt::Display;
use std::sync::Arc;
use std::time::Instant;

use async_graphql::{
    Request as GraphQlRequest, Response as GraphQlResponse, ServerError as GraphQlError,
};
use http::{Method, StatusCode};
use lambda_http::{Body, Error, Request, Response};
use poem::web::headers;
use seslogin::emf::{self, RequestTelemetry};
use seslogin::request_metrics::{self, RequestMetrics};

use crate::app;
use crate::auth::{self, AuthInfo};
use crate::db;
use crate::errors::{ClientError, ServerError};
use crate::graphql;

type GraphQlSchema<H> = async_graphql::Schema<
    graphql::QueryRoot<app::MyApp<H>>,
    graphql::MutationRoot<app::MyApp<H>>,
    async_graphql::EmptySubscription,
>;

pub struct Handler<H: db::Handler + Send + Sync> {
    app: Arc<app::MyApp<H>>,
    schema: GraphQlSchema<H>,
}

impl<H: db::Handler + Send + Sync + 'static> Handler<H> {
    pub fn new(app: Arc<app::MyApp<H>>, schema: GraphQlSchema<H>) -> Self {
        Self { app, schema }
    }

    pub async fn handle_request(&self, request: Request) -> Result<Response<Body>, Error> {
        let request_start = Instant::now();
        let headers = request.headers().clone();

        let query = if request.method() == Method::POST {
            self.graphql_request_from_post(request).await
        } else if request.method() == Method::GET {
            return graphiql_for_request(request);
        } else {
            Err(ClientError::MethodNotAllowed)
        };
        let mut query = match query {
            Err(e) => return error_response(StatusCode::BAD_REQUEST, graphql_error(e)),
            Ok(q) => q,
        };

        let auth_opt = match self.try_auth(&headers).await {
            Err(auth::AuthError::Permanent(ref msg)) => {
                emit_auth_failure_telemetry(401, request_start);
                return error_response(
                    StatusCode::UNAUTHORIZED,
                    graphql_error(format!("Authentication error: {}", msg)),
                );
            }
            Err(auth::AuthError::Transient(ref msg)) => {
                tracing::error!("Transient auth error: {}", msg);
                emit_auth_failure_telemetry(503, request_start);
                return error_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    graphql_error("Service temporarily unavailable"),
                );
            }
            Ok(opt) => opt,
        };
        let (caller_type, caller_id) = auth::caller_info(auth_opt.as_ref());
        if let Some(auth) = auth_opt {
            query = query.data(auth);
        }

        query = query
            .data(self.app.clone())
            .data(graphql::get_dataloader(self.app.clone()));

        let operation_context = emf::extract_operation_context(&query);
        let metrics = Arc::new(RequestMetrics::default());
        let gql_response = request_metrics::METRICS
            .scope(metrics.clone(), self.schema.execute(query))
            .await;
        let gql_error_count = gql_response.errors.len();

        // Serialize first so the emitted telemetry carries the real final HTTP status.
        let result = serde_json::to_string(&gql_response);
        let status: u16 = if result.is_ok() { 200 } else { 500 };

        RequestTelemetry {
            status,
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
        }
        .emit();

        match result {
            Ok(response_body) => Response::builder()
                .status(200)
                .body(Body::Text(response_body))
                .map_err(ServerError::from)
                .map_err(Error::from),
            Err(e) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                graphql_error(ServerError::from(e)),
            ),
        }
    }

    async fn graphql_request_from_post(
        &self,
        request: Request,
    ) -> Result<GraphQlRequest, ClientError> {
        match request.into_body() {
            Body::Text(text) => {
                serde_json::from_str::<GraphQlRequest>(&text).map_err(ClientError::from)
            }
            Body::Binary(binary) => {
                serde_json::from_slice::<GraphQlRequest>(&binary).map_err(ClientError::from)
            }
            _ => Err(ClientError::EmptyBody),
        }
    }

    async fn try_auth(
        &self,
        headers: &headers::HeaderMap,
    ) -> Result<Option<AuthInfo>, auth::AuthError> {
        let client_version = headers
            .get(auth::CLIENT_VERSION_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let opt = if let Some(auth_header) = headers.get("Authorization")
            && let Ok(auth_str) = auth_header.to_str()
            && auth_str.starts_with("Bearer ")
        {
            let token = &auth_str[7..];
            Some(auth::verify_token(&*self.app, token, client_version).await?)
        } else {
            None
        };
        Ok(opt)
    }
}

/// Emits request telemetry for an auth failure that short-circuits before GraphQL execution.
/// `status` is 401 (permanent / unauthenticated) or 503 (transient backend error).
fn emit_auth_failure_telemetry(status: u16, request_start: Instant) {
    RequestTelemetry {
        status,
        operation_type: "unknown",
        operation_name: "unknown",
        caller_type: "unauthenticated",
        caller_id: "unknown",
        latency_ms: request_start.elapsed().as_secs_f64() * 1000.0,
        graphql_error_count: 0,
        query_failures: 0,
        mutation_failures: 0,
        rru: 0.0,
        wru: 0.0,
        ddb_calls: 0,
    }
    .emit();
}

fn graphql_error(message: impl Display) -> String {
    let message = format!("{}", message);
    let response = GraphQlResponse::from_errors(vec![GraphQlError::new(message, None)]);
    serde_json::to_string(&response).expect("Valid response should never fail to serialize")
}

fn error_response(status: StatusCode, body: String) -> Result<Response<Body>, Error> {
    Ok(Response::builder().status(status).body(Body::Text(body))?)
}

fn graphiql_for_request(_request: Request) -> Result<Response<Body>, Error> {
    let html = async_graphql::http::GraphiQLSource::build().finish();
    Response::builder()
        .status(200)
        .header("Content-Type", "text/html")
        .body(Body::Text(html))
        .map_err(Error::from)
}
