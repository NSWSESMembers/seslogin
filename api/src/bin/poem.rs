//! The GraphQL API server, against real AWS: DynamoDB, SQS and SES.
//!
//! For an AWS-free local run, see `poem-local.rs` — a separate binary rather than
//! a flag on this one, so this server has no code path that can be talked into
//! mocking anything.

use std::error::Error;

use seslogin::dynamodb;
use seslogin::server;
use seslogin::sesmail;
use seslogin::sqs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let startup = server::init()?;
    let db = dynamodb::Handler::new(&startup.db_prefix, !startup.cli.enable_mutations).await;
    let queues = sqs::Queues::from_env().await?;
    let mailer = sesmail::Mailer::new().await;
    server::run(startup, db, queues, mailer).await
}
