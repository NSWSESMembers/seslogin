//! The GraphQL surface of Tally sign-in ([`crate::tally_sign_in`]): three
//! unauthenticated fields the sign-in page (`web/src/tally/`) calls.
//!
//! - `tallySignInRequest(clientId, redirectUri)` checks the link the app opened the
//!   page with, before anyone types an email.
//! - `requestTallySignInCode(email, turnstileToken)` emails a code, if that email
//!   can sign in; it answers `true` either way.
//! - `completeTallySignIn(...)` redeems the code and returns the URL to send the
//!   browser back to the app with an authorization code.

use anyhow::Result;
use async_graphql::{Context, SimpleObject};
use tracing::{info, warn};

use super::ClientIp;
use super::error::ApiError;
use crate::app::{App, HasDb, HasMail};
use crate::oauth_clients::Client;
use crate::tally_sign_in::{self, AuthorizationRequest};

/// The consent screen's answer for a member sign-in client: it signs in elsewhere.
pub(crate) fn member_sign_in_client(client: &Client) -> anyhow::Error {
    ApiError::bad_request(format!(
        "{} has its own sign-in and can't be connected here",
        client.client_name
    ))
    .into()
}

/// What the sign-in page shows about the app that opened it.
#[derive(SimpleObject, Clone, Debug)]
pub struct TallySignInRequest {
    pub client_name: String,
    /// A development build, redirecting to the Expo server on a developer's machine.
    /// Only a developer account can finish signing in to one.
    pub is_development_build: bool,
}

pub(super) fn sign_in_request<A: App>(
    app: &A,
    client_id: &str,
    redirect_uri: &str,
) -> Result<TallySignInRequest> {
    let checked = tally_sign_in::check_client(app, client_id, redirect_uri)?;
    Ok(TallySignInRequest {
        client_name: checked.client.client_name,
        is_development_build: checked.is_development_build,
    })
}

/// Email a code unless Turnstile fails. Always `true`, so the answer says nothing
/// about the email.
pub(super) async fn request_code<A: HasDb + HasMail>(
    ctx: &Context<'_>,
    app: &A,
    email: &str,
    turnstile_token: &str,
) -> bool {
    let remote_ip = ctx.data_opt::<ClientIp>().and_then(|ip| ip.0.as_deref());
    match crate::turnstile::verify(turnstile_token, remote_ip).await {
        Ok(true) => {}
        Ok(false) => {
            info!("Turnstile challenge failed for requestTallySignInCode");
            return true;
        }
        Err(e) => {
            warn!("Turnstile error in requestTallySignInCode: {e:#}");
            return true;
        }
    }
    if let Err(e) = tally_sign_in::send_code(app, email).await {
        warn!("requestTallySignInCode failed: {e:#}");
    }
    true
}

pub(super) async fn complete<A: App + HasDb>(
    app: &A,
    email: &str,
    code: &str,
    request: AuthorizationRequest,
) -> Result<String> {
    tally_sign_in::complete(app, email, code, request).await
}
