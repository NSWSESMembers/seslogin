use anyhow::Context;
use anyhow::Result;
use aws_sdk_ses::Client;
use aws_sdk_ses::types::{Body, Content, Destination, Message};

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
fn resolve_recipient(to: &str) -> String {
    match std::env::var(OVERRIDE_TO_VAR) {
        Ok(override_to) if !override_to.trim().is_empty() => {
            let override_to = override_to.trim().to_string();
            tracing::warn!("{OVERRIDE_TO_VAR} is set: redirecting email for {to} to {override_to}");
            override_to
        }
        _ => to.to_string(),
    }
}

async fn ses_client() -> Result<Client> {
    let config = crate::aws_config_loader().load().await;
    Ok(Client::new(&config))
}

/// Send a plain-text email via AWS SES.
pub async fn send_plain_text(to: &str, subject: &str, content: &str) -> Result<()> {
    let to = &resolve_recipient(to);
    let client = ses_client().await?;
    let destination = Destination::builder().to_addresses(to.to_string()).build();
    let subject_content = Content::builder().data(subject).charset("UTF-8").build()?;
    let text_content = Content::builder().data(content).charset("UTF-8").build()?;
    let message = Message::builder()
        .subject(subject_content)
        .body(Body::builder().text(text_content).build())
        .build();
    client
        .send_email()
        .destination(destination)
        .message(message)
        .source(FROM)
        .reply_to_addresses(REPLY_TO.to_string())
        .send()
        .await
        .with_context(|| format!("failed to send email to {}", to))?;
    Ok(())
}

/// Send an HTML email via AWS SES.
pub async fn send_html(to: &str, subject: &str, html: &str) -> Result<()> {
    let to = &resolve_recipient(to);
    let client = ses_client().await?;
    let destination = Destination::builder().to_addresses(to.to_string()).build();
    let subject_content = Content::builder().data(subject).charset("UTF-8").build()?;
    let html_content = Content::builder().data(html).charset("UTF-8").build()?;
    let message = Message::builder()
        .subject(subject_content)
        .body(Body::builder().html(html_content).build())
        .build();
    client
        .send_email()
        .destination(destination)
        .message(message)
        .source(FROM)
        .reply_to_addresses(REPLY_TO.to_string())
        .send()
        .await
        .with_context(|| format!("failed to send email to {}", to))?;
    Ok(())
}
