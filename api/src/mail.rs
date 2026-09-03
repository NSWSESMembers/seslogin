//! Outgoing-email abstraction.
//!
//! Mirrors the [`crate::db`] / [`crate::dynamodb`] / [`crate::mockdb`] split:
//! this module holds the trait, [`crate::sesmail`] sends via AWS SES, and
//! [`crate::mockmail`] logs the message instead, so the API can run — including
//! the email-code login flow — with no AWS account.

use anyhow::Result;
use std::future::Future;

pub const FROM: &str = "no-reply@seslogin.com";
pub const REPLY_TO: &str = "support@seslogin.com";

/// Redirect every outgoing email to this address instead of its real recipient.
///
/// Local dev runs against `seslogin_test`, a snapshot of production that carries
/// **real** member email addresses, so any feature that mails a member is one
/// button-press away from mailing a real volunteer from a developer's laptop.
/// Setting `MAIL_OVERRIDE_TO` in `.env` makes that impossible. Never set it in a
/// deployed environment.
const OVERRIDE_TO_VAR: &str = "MAIL_OVERRIDE_TO";

/// Resolve the address to actually send to, honouring [`OVERRIDE_TO_VAR`].
///
/// Applied by every backend, mock included, so what the mock logs is what a real
/// send would have done.
pub fn resolve_recipient(to: &str) -> String {
    match std::env::var(OVERRIDE_TO_VAR) {
        Ok(override_to) if !override_to.trim().is_empty() => {
            let override_to = override_to.trim().to_string();
            tracing::warn!("{OVERRIDE_TO_VAR} is set: redirecting email for {to} to {override_to}");
            override_to
        }
        _ => to.to_string(),
    }
}

/// `Sync` is required for the same reason as [`crate::db::Handler`]: a
/// `&impl Handler` is held across `.await` inside the `Send` futures the
/// GraphQL/Poem stack builds.
pub trait Handler: Sync {
    fn send_plain_text(
        &self,
        to: &str,
        subject: &str,
        content: &str,
    ) -> impl Future<Output = Result<()>> + Send;

    fn send_html(
        &self,
        to: &str,
        subject: &str,
        html: &str,
    ) -> impl Future<Output = Result<()>> + Send;
}
