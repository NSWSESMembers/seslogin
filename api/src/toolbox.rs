//! Client for Toolbox, the support-ticket system behind the admin "Help" page.
//!
//! A logged-in user's feedback becomes a Toolbox ticket through Toolbox's
//! `submitVerifiedTicket` mutation, authorised by an instance-scoped integration
//! token (`mta_…`). That mutation takes the requester address on the caller's
//! word — which is the point: seslogin has already verified the user's email by
//! logging them in, so the ticket's replies go straight to their inbox with no
//! second verification step. The address is therefore always taken from the
//! user's own record on the server, never from the request.
//!
//! Configured by `TOOLBOX_GRAPHQL_URL` (Toolbox's `/graphql` endpoint) and
//! `TOOLBOX_API_TOKEN`. Either unset or blank disables feedback rather than
//! failing startup, like `ABLY_API_KEY`: `Config::from_env` returns `None` and
//! the web page says help isn't available.

use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::json;

const GRAPHQL_URL_VAR: &str = "TOOLBOX_GRAPHQL_URL";
const API_TOKEN_VAR: &str = "TOOLBOX_API_TOKEN";

/// Well inside the API Lambda's 30s timeout, so a hung Toolbox fails the
/// mutation with a useful error rather than timing out the whole request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

const SUBMIT_MUTATION: &str = r#"
mutation SubmitVerifiedTicket($subject: String!, $body: String!, $to: [String!]!) {
  submitVerifiedTicket(subject: $subject, body: $body, to: $to) {
    number
    subjectTag
  }
}
"#;

#[derive(Clone)]
pub struct Config {
    graphql_url: String,
    api_token: String,
}

impl Config {
    /// `None` when either variable is unset or blank: feedback is disabled.
    pub fn from_env() -> Option<Self> {
        let get = |name| {
            std::env::var(name)
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        Some(Self {
            graphql_url: get(GRAPHQL_URL_VAR)?,
            api_token: get(API_TOKEN_VAR)?,
        })
    }

    pub fn new(graphql_url: impl Into<String>, api_token: impl Into<String>) -> Self {
        Self {
            graphql_url: graphql_url.into(),
            api_token: api_token.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmittedTicket {
    pub number: i64,
    /// `[#{slug}-{number}]`, the tag Toolbox puts on every email about this ticket.
    pub subject_tag: String,
}

#[derive(Deserialize)]
struct GraphqlResponse {
    data: Option<ResponseData>,
    #[serde(default)]
    errors: Vec<GraphqlError>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResponseData {
    submit_verified_ticket: SubmittedTicketWire,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmittedTicketWire {
    number: i64,
    subject_tag: String,
}

#[derive(Deserialize)]
struct GraphqlError {
    message: String,
}

/// Open a Toolbox ticket from `requester_email`. Errors carry Toolbox's own
/// detail for the log; callers should not hand them to the user verbatim.
pub async fn submit_ticket(
    config: &Config,
    subject: &str,
    body: &str,
    requester_email: &str,
) -> Result<SubmittedTicket> {
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .context("Failed to build Toolbox HTTP client")?;
    let resp = client
        .post(&config.graphql_url)
        .bearer_auth(&config.api_token)
        .json(&json!({
            "query": SUBMIT_MUTATION,
            "variables": {
                "subject": subject,
                "body": body,
                "to": [requester_email],
            },
        }))
        .send()
        .await
        .context("Failed to reach Toolbox")?;

    let status = resp.status();
    let text = resp
        .text()
        .await
        .context("Failed to read Toolbox response")?;
    parse_response(status, &text)
}

fn parse_response(status: reqwest::StatusCode, text: &str) -> Result<SubmittedTicket> {
    // A GraphQL error is usually still HTTP 200, but a rejected token can come
    // back as a 4xx with a GraphQL body — parse first, so its message is logged.
    let parsed: GraphqlResponse = match serde_json::from_str(text) {
        Ok(parsed) => parsed,
        Err(err) if status.is_success() => {
            return Err(anyhow!(err).context("Failed to parse Toolbox response"));
        }
        Err(_) => bail!("Toolbox returned HTTP {}", status.as_u16()),
    };
    if !parsed.errors.is_empty() {
        let messages: Vec<_> = parsed.errors.into_iter().map(|e| e.message).collect();
        bail!(
            "Toolbox rejected the ticket (HTTP {}): {}",
            status.as_u16(),
            messages.join("; ")
        );
    }
    let ticket = parsed
        .data
        .ok_or_else(|| anyhow!("Toolbox returned HTTP {} with no data", status.as_u16()))?
        .submit_verified_ticket;
    Ok(SubmittedTicket {
        number: ticket.number,
        subject_tag: ticket.subject_tag,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use poem::{
        EndpointExt, Request, Route, Server, handler, listener::TcpAcceptor, post, web::Data,
    };

    #[derive(Default)]
    struct Captured {
        authorization: Option<String>,
        body: Option<serde_json::Value>,
    }

    /// Stand up a fake Toolbox on a random local port that records the request
    /// and answers with `response`.
    async fn fake_toolbox(status: u16, response: &'static str) -> (String, Arc<Mutex<Captured>>) {
        #[handler]
        async fn graphql(
            req: &Request,
            body: String,
            Data(state): Data<&(Arc<Mutex<Captured>>, u16, &'static str)>,
        ) -> poem::Response {
            let (captured, status, response) = state;
            let mut c = captured.lock().unwrap();
            c.authorization = req.header("authorization").map(str::to_string);
            c.body = serde_json::from_str(&body).ok();
            poem::Response::builder()
                .status(poem::http::StatusCode::from_u16(*status).unwrap())
                .content_type("application/json")
                .body(*response)
        }

        let captured = Arc::new(Mutex::new(Captured::default()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app =
            Route::new()
                .at("/graphql", post(graphql))
                .data((captured.clone(), status, response));
        let acceptor = TcpAcceptor::from_tokio(listener).unwrap();
        tokio::spawn(Server::new_with_acceptor(acceptor).run(app));
        (format!("http://{addr}/graphql"), captured)
    }

    #[tokio::test]
    async fn submits_with_bearer_token_and_requester_as_sole_recipient() {
        let (url, captured) = fake_toolbox(
            200,
            r#"{"data":{"submitVerifiedTicket":{"number":42,"subjectTag":"[#help-42]"}}}"#,
        )
        .await;

        let ticket = submit_ticket(
            &Config::new(url, "mta_abc.secret"),
            "Broken report",
            "It shows nothing.",
            "user@example.com",
        )
        .await
        .unwrap();

        assert_eq!(
            ticket,
            SubmittedTicket {
                number: 42,
                subject_tag: "[#help-42]".to_string()
            }
        );
        let captured = captured.lock().unwrap();
        assert_eq!(
            captured.authorization.as_deref(),
            Some("Bearer mta_abc.secret")
        );
        let body = captured.body.as_ref().unwrap();
        assert!(
            body["query"]
                .as_str()
                .unwrap()
                .contains("submitVerifiedTicket")
        );
        assert_eq!(
            body["variables"],
            json!({
                "subject": "Broken report",
                "body": "It shows nothing.",
                "to": ["user@example.com"],
            })
        );
    }

    #[tokio::test]
    async fn graphql_errors_fail_with_toolbox_message() {
        let (url, _) = fake_toolbox(
            200,
            r#"{"data":null,"errors":[{"message":"Must present a valid API token"}]}"#,
        )
        .await;
        let err = submit_ticket(&Config::new(url, "mta_bad"), "s", "b", "u@example.com")
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("Must present a valid API token"),
            "{err}"
        );
    }

    #[test]
    fn non_json_error_status_reports_the_status() {
        let err = parse_response(reqwest::StatusCode::BAD_GATEWAY, "<html>").unwrap_err();
        assert_eq!(err.to_string(), "Toolbox returned HTTP 502");
    }

    #[test]
    fn missing_data_is_an_error() {
        let err = parse_response(reqwest::StatusCode::OK, r#"{"data":null}"#).unwrap_err();
        assert!(err.to_string().contains("no data"), "{err}");
    }
}
