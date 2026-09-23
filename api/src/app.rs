use crate::db;
use crate::jwt;
use crate::mail;
use crate::queue;
use crate::realtime;
use webauthn_rs::prelude::{Webauthn, WebauthnBuilder};

pub trait App {
    fn jwt(&self) -> &jwt::Key;
    fn response_lag(&self) -> u64;
}

/// Build the WebAuthn relying-party instance from environment configuration.
///
/// `WEBAUTHN_RP_ID` is the relying-party ID (defaults to `localhost`).
/// `WEBAUTHN_RP_ORIGIN` is a comma-separated list of allowed origins; the first
/// is the primary (defaults to `http://localhost:5173`). Multiple origins let a
/// single deployment serve both e.g. `https://seslogin.com` and
/// `https://new.seslogin.com`.
pub fn build_webauthn() -> anyhow::Result<Webauthn> {
    let rp_id = std::env::var("WEBAUTHN_RP_ID").unwrap_or_else(|_| "localhost".to_string());
    let origins_raw =
        std::env::var("WEBAUTHN_RP_ORIGIN").unwrap_or_else(|_| "http://localhost:5173".to_string());
    let mut origins = origins_raw
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(url::Url::parse)
        .collect::<Result<Vec<_>, _>>()?;
    if origins.is_empty() {
        return Err(anyhow::anyhow!(
            "WEBAUTHN_RP_ORIGIN must contain at least one origin"
        ));
    }
    let primary = origins.remove(0);
    let mut builder = WebauthnBuilder::new(&rp_id, &primary)?.rp_name("seslogin");
    for extra in &origins {
        builder = builder.append_allowed_origin(extra);
    }
    Ok(builder.build()?)
}

pub trait HasDb {
    fn db(&self) -> &impl db::Handler;
}

pub trait HasQueues {
    fn queues(&self) -> &impl queue::Handler;
}

pub trait HasMail {
    fn mail(&self) -> &impl mail::Handler;
}

pub trait HasRealtime {
    fn realtime(&self) -> &impl realtime::Handler;
}

/// struct for holding our global singletons
///
/// Every backend is a type parameter, so each binary compiles exactly the
/// implementations it uses: `bin/poem.rs` gets DynamoDB + SQS + SES + Ably, and
/// `bin/poem-local.rs` gets DynamoDB + the mocks (queue, mail, and realtime all
/// record/log instead of dispatching). There is deliberately no runtime switch
/// — a server that could be talked into mocking its own email by an
/// environment variable is a worse thing to deploy than two binaries.
pub struct MyApp<DBH: db::Handler, Q: queue::Handler, M: mail::Handler, R: realtime::Handler> {
    pub db: DBH,
    pub jwt: jwt::Key,
    pub response_lag: u64,
    pub queues: Q,
    pub mail: M,
    pub realtime: R,
}

pub fn new<DBH: db::Handler, Q: queue::Handler, M: mail::Handler, R: realtime::Handler>(
    db: DBH,
    jwt: jwt::Key,
    response_lag: u64,
    queues: Q,
    mail: M,
    realtime: R,
) -> MyApp<DBH, Q, M, R> {
    MyApp {
        db,
        jwt,
        response_lag,
        queues,
        mail,
        realtime,
    }
}

impl<DBH: db::Handler, Q: queue::Handler, M: mail::Handler, R: realtime::Handler> App
    for MyApp<DBH, Q, M, R>
{
    fn jwt(&self) -> &jwt::Key {
        &self.jwt
    }
    fn response_lag(&self) -> u64 {
        self.response_lag
    }
}

impl<DBH: db::Handler, Q: queue::Handler, M: mail::Handler, R: realtime::Handler> HasDb
    for MyApp<DBH, Q, M, R>
{
    fn db(&self) -> &impl db::Handler {
        &self.db
    }
}

impl<DBH: db::Handler, Q: queue::Handler, M: mail::Handler, R: realtime::Handler> HasQueues
    for MyApp<DBH, Q, M, R>
{
    fn queues(&self) -> &impl queue::Handler {
        &self.queues
    }
}

impl<DBH: db::Handler, Q: queue::Handler, M: mail::Handler, R: realtime::Handler> HasMail
    for MyApp<DBH, Q, M, R>
{
    fn mail(&self) -> &impl mail::Handler {
        &self.mail
    }
}

impl<DBH: db::Handler, Q: queue::Handler, M: mail::Handler, R: realtime::Handler> HasRealtime
    for MyApp<DBH, Q, M, R>
{
    fn realtime(&self) -> &impl realtime::Handler {
        &self.realtime
    }
}
