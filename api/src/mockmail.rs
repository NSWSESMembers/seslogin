//! In-process implementation of [`crate::mail::Handler`] that logs the message
//! instead of sending it.
//!
//! This is what makes the email-code login flow usable with no AWS account: the
//! code is printed in the API's own log, so you can paste it into the browser.
//! Set `MOCK_MAIL_DIR` to also drop each message into a file there — handy for
//! HTML emails, which are unreadable in a log line.

use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{Context, Result};

use crate::mail;

/// Directory to additionally write each message to, one file per message.
pub const DIR_VAR: &str = "MOCK_MAIL_DIR";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email {
    pub to: String,
    pub subject: String,
    pub body: String,
    pub html: bool,
}

#[derive(Default)]
pub struct Handler {
    sent: Mutex<Vec<Email>>,
    dir: Option<PathBuf>,
}

impl Handler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Honours [`DIR_VAR`]; falls back to logging only.
    pub fn from_env() -> Self {
        let dir = std::env::var_os(DIR_VAR)
            .map(PathBuf::from)
            .filter(|d| !d.as_os_str().is_empty());
        if let Some(dir) = &dir {
            tracing::info!("mock mail: also writing messages to {}", dir.display());
        }
        Self {
            sent: Mutex::new(Vec::new()),
            dir,
        }
    }

    /// Every message sent so far, oldest first.
    pub fn sent(&self) -> Vec<Email> {
        self.sent.lock().expect("mockmail lock").clone()
    }

    pub fn clear(&self) {
        self.sent.lock().expect("mockmail lock").clear();
    }

    fn record(&self, to: &str, subject: &str, body: &str, html: bool) -> Result<()> {
        let to = mail::resolve_recipient(to);
        tracing::info!(
            "mock mail to {to}: {subject}\n--- begin message ---\n{body}\n--- end message ---"
        );
        let email = Email {
            to,
            subject: subject.to_string(),
            body: body.to_string(),
            html,
        };
        // One lock for the whole append, so the filename index and the vector
        // position can't disagree when two sends race.
        let mut sent = self.sent.lock().expect("mockmail lock");
        if let Some(dir) = &self.dir {
            // Best-effort would hide a typo'd MOCK_MAIL_DIR, and nothing here is on a
            // path where failing to write is better than saying so.
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
            let ext = if html { "html" } else { "txt" };
            let path = dir.join(format!("{:04}-{}.{ext}", sent.len(), slug(subject)));
            std::fs::write(
                &path,
                format!(
                    "To: {}\nSubject: {}\n\n{}",
                    email.to, email.subject, email.body
                ),
            )
            .with_context(|| format!("writing {}", path.display()))?;
        }
        sent.push(email);
        Ok(())
    }
}

/// Filesystem-safe fragment of a subject line, for the per-message filename.
fn slug(subject: &str) -> String {
    let s: String = subject
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_lowercase();
    s.chars().take(40).collect()
}

impl mail::Handler for Handler {
    async fn send_plain_text(&self, to: &str, subject: &str, content: &str) -> Result<()> {
        self.record(to, subject, content, false)
    }

    async fn send_html(&self, to: &str, subject: &str, html: &str) -> Result<()> {
        self.record(to, subject, html, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::Handler as _;

    #[tokio::test]
    async fn records_what_was_sent() {
        let m = Handler::new();
        m.send_plain_text("a@example.com", "Your seslogin login code", "123456")
            .await
            .unwrap();
        assert_eq!(
            m.sent(),
            vec![Email {
                to: "a@example.com".into(),
                subject: "Your seslogin login code".into(),
                body: "123456".into(),
                html: false,
            }]
        );
    }

    #[test]
    fn slug_is_filename_safe() {
        let s = slug("SES Activity Summary — 2026-09-02");
        assert!(
            s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "unexpected characters in {s:?}"
        );
        assert!(s.starts_with("ses-activity-summary"), "{s:?}");
    }
}
