//! The GraphQL surface of the member settings page ([`crate::member_settings`]):
//! fields that need no credentials, because the page's `slms_` token travels as an
//! argument and authorises exactly one member's settings.
//!
//! - `memberSettings(token)` reads them; an unusable token fails with
//!   `UNAUTHENTICATED`, which the page answers by going to the sign-in page.
//! - `requestMemberSettingsCode(email, turnstileToken)` emails a sign-in code, if
//!   that email signs a member in; it answers `true` either way.
//! - `signInToMemberSettings(email, code)` redeems it for a token.
//! - `updateMemberSettings(token, signedInReminders)` saves a change.
//! - `signOutOfMemberSettings(token)` ends a token.

use anyhow::Result;
use async_graphql::{Context, SimpleObject};
use tracing::{info, warn};

use super::ClientIp;
use super::error::{ApiError, ErrorCode};
use crate::app::{HasDb, HasMail};
use crate::audit::{self, Actor, AuditContext};
use crate::db::{self, Handler as _};
use crate::member_settings::{self, ResolveError};

/// A member's own settings, as their settings page shows them.
#[derive(SimpleObject, Clone, Debug)]
pub struct MemberSettings {
    pub first_name: String,
    pub last_name: String,
    /// The email SESLogin has for them (their service email, from the SES directory).
    pub email: Option<String>,
    /// Whether they get an email when they've forgotten to sign out.
    pub signed_in_reminders: bool,
}

impl From<&db::Person> for MemberSettings {
    fn from(p: &db::Person) -> Self {
        Self {
            first_name: p.first_name.clone(),
            last_name: p.last_name.clone(),
            email: p.email.clone(),
            signed_in_reminders: p.signed_in_reminders_off_at.is_none(),
        }
    }
}

/// The member a token is for, or an `UNAUTHENTICATED` error.
async fn resolve<A: HasDb>(app: &A, token: &str) -> Result<db::Person> {
    member_settings::resolve_token(app.db(), token)
        .await
        .map_err(|e| match e {
            ResolveError::Invalid => {
                ApiError::new(ErrorCode::Unauthenticated, e.to_string()).into()
            }
            ResolveError::Db(e) => anyhow::Error::new(e).context("verifying member settings link"),
        })
}

pub(super) async fn settings<A: HasDb>(app: &A, token: &str) -> Result<MemberSettings> {
    Ok(MemberSettings::from(&resolve(app, token).await?))
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
    let key = db::email_key(email);
    match crate::turnstile::verify(turnstile_token, remote_ip).await {
        Ok(true) => {}
        Ok(false) => {
            info!(email = %key, "Turnstile challenge failed for requestMemberSettingsCode");
            return true;
        }
        Err(e) => {
            warn!(email = %key, "Turnstile error in requestMemberSettingsCode: {e:#}");
            return true;
        }
    }
    if let Err(e) = member_settings::send_code(app, email).await {
        warn!(email = %key, "requestMemberSettingsCode failed: {e:#}");
    }
    true
}

pub(super) async fn sign_in<A: HasDb>(app: &A, email: &str, code: &str) -> Result<String> {
    member_settings::sign_in(app.db(), email, code)
        .await?
        .ok_or_else(|| ApiError::bad_request("That code is wrong or has expired").into())
}

pub(super) async fn update<A: HasDb>(
    app: &A,
    token: &str,
    signed_in_reminders: bool,
) -> Result<MemberSettings> {
    let person = resolve(app, token).await?;
    let off_at = if signed_in_reminders {
        None
    } else {
        // Keep the original time when it is already off.
        Some(
            person
                .signed_in_reminders_off_at
                .unwrap_or_else(crate::clock::now_sec),
        )
    };
    if off_at != person.signed_in_reminders_off_at {
        // The request carried no credentials, so it is recorded as the member the
        // token is for.
        let actor = Actor::Member {
            person_id: person.id.clone(),
            via: Some(member_settings::AUDIT_VIA.to_string()),
        };
        audit::scope(
            AuditContext::new(actor, audit::current_ip()),
            app.db().update_person(
                &person.id,
                db::PersonUpdateShape::SignedInReminders { off_at },
            ),
        )
        .await?;
        info!(
            person_id = %person.id, signed_in_reminders,
            "member settings: signed-in reminders changed"
        );
    }
    Ok(MemberSettings {
        signed_in_reminders,
        ..MemberSettings::from(&person)
    })
}

pub(super) async fn sign_out<A: HasDb>(app: &A, token: &str) -> Result<bool> {
    member_settings::revoke_token(app.db(), token).await?;
    Ok(true)
}
