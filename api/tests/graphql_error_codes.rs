//! End-to-end coverage of `extensions.code` classification through a real schema
//! execution — not just the `classify()` unit, but the whole path: a resolver
//! failure, through `RequestMetricsExtImpl`, into the response's `errors[]`.
//!
//! Uses `mockdb::Handler`, which fails every DB call with `Error::Infrastructure`
//! (see `api/src/mockdb.rs`) — exactly the "unclassified failure" case `classify()`
//! defaults to `INTERNAL` for.

use std::sync::Arc;

use async_graphql::{Request, Value};

use seslogin::app::{self, MyApp};
use seslogin::auth::AuthInfo;
use seslogin::graphql;
use seslogin::jwt;
use seslogin::mockdb;
use seslogin::sqs_dispatch::{SqsQueue, SqsQueues};

type TestSchema = async_graphql::Schema<
    graphql::QueryRoot<MyApp<mockdb::Handler>>,
    graphql::MutationRoot<MyApp<mockdb::Handler>>,
    async_graphql::EmptySubscription,
>;

struct Fixture {
    schema: TestSchema,
    app: Arc<MyApp<mockdb::Handler>>,
}

fn fixture() -> Fixture {
    let key = jwt::Key::new("test", None, None).expect("valid test JWT key");
    let db = mockdb::Handler::new();
    // Never sends a request — these queries don't touch SQS — so an unconfigured
    // client is fine and avoids any network-dependent config resolution.
    let sqs_client = aws_sdk_sqs::Client::from_conf(
        aws_sdk_sqs::Config::builder()
            .behavior_version(aws_sdk_sqs::config::BehaviorVersion::latest())
            .build(),
    );
    let sqs = SqsQueues {
        member_sync: SqsQueue {
            client: sqs_client.clone(),
            queue_url: String::new(),
        },
        nitc_export: SqsQueue {
            client: sqs_client.clone(),
            queue_url: String::new(),
        },
        healthcheck: SqsQueue {
            client: sqs_client,
            queue_url: String::new(),
        },
    };
    let app = Arc::new(app::new(db, key, 0, sqs));
    // No HTTP relying-party check is exercised by these queries, so a minimal fixed
    // origin is fine here — unlike `export-schema.rs`, this must not depend on
    // `WEBAUTHN_RP_ORIGIN` being set.
    let webauthn = webauthn_rs::prelude::WebauthnBuilder::new(
        "localhost",
        &url::Url::parse("http://localhost:5173").unwrap(),
    )
    .unwrap()
    .rp_name("seslogin-test")
    .build()
    .unwrap();
    let schema = graphql::build_schema(app.clone(), Arc::new(webauthn));
    Fixture { schema, app }
}

fn code_of(response: &async_graphql::Response) -> Option<String> {
    let error = response.errors.first()?;
    match error.extensions.as_ref()?.get("code")? {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

#[tokio::test]
async fn unauthenticated_query_is_classified_by_the_guard() {
    let fixture = fixture();
    let response = fixture
        .schema
        .execute(Request::new(r#"{ person(id: "abc123") { id } }"#))
        .await;

    assert!(response.is_err(), "expected a guard failure with no auth");
    assert_eq!(code_of(&response).as_deref(), Some("UNAUTHENTICATED"));
}

#[tokio::test]
async fn an_unclassified_resolver_failure_defaults_to_internal() {
    let fixture = fixture();
    let request = Request::new(r#"{ person(id: "abc123") { id } }"#)
        .data(AuthInfo::User {
            id: "user-1".to_string(),
            is_super: true,
            location_grants: vec![],
            token_id: None,
        })
        .data(fixture.app.clone())
        .data(graphql::get_dataloader(fixture.app.clone()));
    let response = fixture.schema.execute(request).await;

    // mockdb fails every DB call, so this is the resolver reaching `?` on a plain
    // db::Error — no ApiError anywhere on the chain, so `classify()` must fall back
    // rather than leaving the error unclassified.
    assert!(response.is_err(), "expected mockdb's DB error to surface");
    assert_eq!(code_of(&response).as_deref(), Some("INTERNAL"));
}
