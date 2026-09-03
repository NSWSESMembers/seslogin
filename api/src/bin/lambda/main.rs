mod errors;
mod handler;

use lambda_http::{Error, run, service_fn, tracing};
use seslogin::app;
use seslogin::auth;
use seslogin::db;
use seslogin::dynamodb;
use seslogin::graphql;
use seslogin::jwt;
use seslogin::mail;
use seslogin::queue;
use seslogin::sesmail;
use seslogin::sqs;
use std::env;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    // load JWT secret from env
    let secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");

    let key = jwt::Key::new(&secret, None, None)?;

    let queues = sqs::Queues::from_env().await?;
    let mailer = sesmail::Mailer::new().await;

    let read_only = env::var("READ_ONLY")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    let db_prefix = env::var("DB_PREFIX").expect("DB_PREFIX must be set for dynamodb backend");
    let db = dynamodb::Handler::new(&db_prefix, read_only).await;
    let app = Arc::new(app::new(db, key, 0, queues, mailer));
    let webauthn = Arc::new(app::build_webauthn().expect("WebAuthn build failed"));
    let schema = graphql::build_schema(app.clone(), webauthn);
    let handler = handler::Handler::new(app, schema);
    run(service_fn(|req| handler.handle_request(req))).await
}
