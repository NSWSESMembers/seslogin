//! The GraphQL API server with the AWS services other than DynamoDB mocked out:
//! queue messages are recorded and logged, email is logged rather than sent.
//! Started by `make dev-local`; see DEVELOPMENT.md, "Running without AWS".
//!
//! Deliberately a separate binary from `poem.rs`, not a flag or a cargo feature
//! on it. A feature would be switched on by `--all-features`, which `make check`
//! passes to clippy — the real SQS and SES paths would then stop being linted,
//! and any `--all-features` build would quietly produce a mocked server.
//!
//! DynamoDB is *not* mocked: `DB_PREFIX` and `AWS_ENDPOINT_URL_DYNAMODB` still
//! decide which database this talks to, so it can be pointed at DynamoDB Local or
//! at a real table. That combination — a real database that provably cannot send
//! email — is the point.

use std::error::Error;

use seslogin::dynamodb;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let startup = server::init()?;
    tracing::warn!(
        "poem-local: SQS and SES are mocked. Queue messages will be logged and dropped, \
         and email will be logged instead of sent."
    );
    let db = dynamodb::Handler::new(&startup.db_prefix, !startup.cli.enable_mutations).await;
    server::run(
        startup,
        db,
        mockqueue::Handler::new(),
        mockmail::Handler::from_env(),
    )
    .await
}
