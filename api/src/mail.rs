use anyhow::Context;
use anyhow::Result;
use aws_config;
use aws_sdk_ses::Client;
use aws_sdk_ses::types::{Body, Content, Destination, Message};

pub const FROM: &str = "no-reply@seslogin.com";
pub const REPLY_TO: &str = "support@seslogin.com";

async fn ses_client() -> Result<Client> {
    let base = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    // While this account's SES production access is pending, SES_ROLE_ARN points
    // at a role in the SES-production (old) account. Assuming it means the email
    // is sent *by that account* (out of sandbox). Empty/unset -> send normally.
    let config = match std::env::var("SES_ROLE_ARN") {
        Ok(role_arn) if !role_arn.is_empty() => {
            let provider = aws_config::sts::AssumeRoleProvider::builder(role_arn)
                .session_name("seslogin-ses")
                .configure(&base)
                .build()
                .await;
            let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
                .credentials_provider(provider);
            if let Some(region) = base.region() {
                loader = loader.region(region.clone());
            }
            loader.load().await
        }
        _ => base,
    };
    Ok(Client::new(&config))
}

/// Send a plain-text email via AWS SES.
pub async fn send_plain_text(to: &str, subject: &str, content: &str) -> Result<()> {
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
