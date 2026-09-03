//! AWS SES implementation of [`crate::mail::Handler`].
//!
//! Not to be confused with [`crate::ses_api`], which talks to the *State Emergency
//! Service* headquarters system. "SES" here is Amazon Simple Email Service.

use anyhow::{Context, Result};
use aws_sdk_ses::Client;
use aws_sdk_ses::types::{Body, Content, Destination, Message};

use crate::mail::{self, FROM, REPLY_TO};

pub struct Mailer {
    client: Client,
}

impl Mailer {
    pub async fn new() -> Self {
        let config = crate::aws_config_loader().load().await;
        Self {
            client: Client::new(&config),
        }
    }

    async fn send(&self, to: &str, subject: &str, body: Body) -> Result<()> {
        let to = mail::resolve_recipient(to);
        let destination = Destination::builder().to_addresses(to.clone()).build();
        let subject_content = Content::builder().data(subject).charset("UTF-8").build()?;
        let message = Message::builder()
            .subject(subject_content)
            .body(body)
            .build();
        self.client
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
}

impl mail::Handler for Mailer {
    async fn send_plain_text(&self, to: &str, subject: &str, content: &str) -> Result<()> {
        let text = Content::builder().data(content).charset("UTF-8").build()?;
        self.send(to, subject, Body::builder().text(text).build())
            .await
    }

    async fn send_html(&self, to: &str, subject: &str, html: &str) -> Result<()> {
        let html = Content::builder().data(html).charset("UTF-8").build()?;
        self.send(to, subject, Body::builder().html(html).build())
            .await
    }
}
