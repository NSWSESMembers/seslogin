//! Guards shared by the local-development binaries (`local-tables`, `local-seed`), and `poem-local`'s `/dev` routes.
//!
//! These binaries create tables and write rows unconditionally, which is only ever
//! safe against a DynamoDB running on this machine. The check lives here rather
//! than in each binary so the two cannot drift apart.

use anyhow::{Result, anyhow, bail};
use poem::endpoint::BoxEndpoint;
use poem::http::StatusCode;
use poem::web::{Data, Json, Query};
use poem::{EndpointExt, Request, Route, get, handler, post};
use std::sync::atomic::{AtomicU64, Ordering};

/// The configured DynamoDB endpoint, but only if it is unmistakably a local one.
///
/// This is the whole safety story for the local-dev binaries: with no endpoint
/// override the AWS SDK talks to real DynamoDB using whatever credentials are
/// lying around, and a stray `DB_PREFIX` would then create or overwrite rows in a
/// real account.
pub fn require_local_dynamodb_endpoint() -> Result<String> {
    let endpoint = std::env::var("AWS_ENDPOINT_URL_DYNAMODB")
        .or_else(|_| std::env::var("AWS_ENDPOINT_URL"))
        .map_err(|_| {
            anyhow!(
                "AWS_ENDPOINT_URL_DYNAMODB is not set — refusing to run in case this would \
                 write to a real AWS account. Use the `make local-*` targets, which point it \
                 at the DynamoDB Local started by local/dynamodb.sh."
            )
        })?;
    local_endpoint(endpoint)
}

/// `endpoint`, if its host is this machine. Split from the env read so it tests without mutating env.
fn local_endpoint(endpoint: String) -> Result<String> {
    let url = url::Url::parse(&endpoint)
        .map_err(|e| anyhow!("AWS_ENDPOINT_URL_DYNAMODB is not a URL: {e}"))?;
    match url.host_str() {
        Some(host) if is_local_url_host(host) => Ok(endpoint),
        other => bail!(
            "DynamoDB endpoint {:?} is not local (host {:?}) — refusing to run.",
            endpoint,
            other.unwrap_or("<none>")
        ),
    }
}

/// Extra GraphQL response delay set from the dev bar. Only `/dev/lag` writes it, so it stays 0 without `/dev`.
static EXTRA_LAG_MS: AtomicU64 = AtomicU64::new(0);

pub fn extra_lag_ms() -> u64 {
    EXTRA_LAG_MS.load(Ordering::Relaxed)
}

/// A URL host naming this machine.
fn is_local_url_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]")
}

/// A fixture time relative to when seeding runs, so seeded activity never goes stale: `now`, `today` (midnight) or
/// `monday` (this week's, midnight), Sydney time, then any `+`/`-` `<n><s|m|h|d|w>` steps, eg `monday-3w+1d+19h`.
/// `d` and `w` move by calendar days, keeping the Sydney wall time across a DST change; `h m s` are elapsed time.
pub fn resolve_relative_time(expr: &str, now: chrono::DateTime<chrono::Utc>) -> Result<i64> {
    use chrono::{Datelike, Duration, TimeZone, Utc};
    use chrono_tz::Australia::Sydney;

    // A Sydney wall time made absolute: the earlier of a repeated (autumn) hour, an hour on for a skipped one.
    let localize = |t: chrono::NaiveDateTime| {
        Sydney
            .from_local_datetime(&t)
            .earliest()
            .or_else(|| {
                Sydney
                    .from_local_datetime(&(t + Duration::hours(1)))
                    .earliest()
            })
            .expect("an hour past a DST gap exists")
            .with_timezone(&Utc)
    };
    let midnight = |d: chrono::NaiveDate| localize(d.and_hms_opt(0, 0, 0).expect("midnight"));
    let wall = |t: chrono::DateTime<Utc>| t.with_timezone(&Sydney).naive_local();
    let local = now.with_timezone(&Sydney).date_naive();
    let anchor_end = expr.find(['+', '-']).unwrap_or(expr.len());
    let mut t = match &expr[..anchor_end] {
        "now" => now,
        "today" => midnight(local),
        "monday" => midnight(local - Duration::days(local.weekday().num_days_from_monday().into())),
        other => bail!("unknown time anchor {other:?} in {expr:?}"),
    };
    let mut rest = &expr[anchor_end..];
    while let Some(sign) = rest.chars().next() {
        let step = &rest[1..];
        let digits = step
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(step.len());
        let n: i64 = step[..digits]
            .parse()
            .map_err(|_| anyhow!("{expr:?}: expected a number after {sign}"))?;
        let n = if sign == '-' { -n } else { n };
        t = match step[digits..].chars().next() {
            // Calendar days, so a 7pm training night before the change is still 7pm, not 6.
            Some('d') => localize(wall(t) + Duration::days(n)),
            Some('w') => localize(wall(t) + Duration::weeks(n)),
            Some('h') => t + Duration::hours(n),
            Some('m') => t + Duration::minutes(n),
            Some('s') => t + Duration::seconds(n),
            _ => bail!("{expr:?}: each step needs a unit, one of s m h d w"),
        };
        rest = &step[digits + 1..];
    }
    Ok(t.timestamp())
}

/// A DynamoDB client for the given endpoint-and-region environment.
pub async fn dynamodb_client() -> aws_sdk_dynamodb::Client {
    let config = crate::aws_config_loader()
        .region(aws_config::Region::new(
            std::env::var("AWS_REGION").unwrap_or_else(|_| "ap-southeast-2".to_string()),
        ))
        .load()
        .await;
    aws_sdk_dynamodb::Client::new(&config)
}

/// `/dev/*` shortcuts for the web dev bar (web/src/dev/DevBar.tsx), mounted by `poem-local` only.
///
/// The shortcuts are off unless the server is on `seslogin_local` at a local endpoint; every request must come from this machine.
/// Data writes run the `local-seed`, `cli` and `badge-nightly` binaries, so each shortcut is the command it replaces.
///
/// `/dev/storage/upload` is mounted regardless, and is not limited to this machine: the upload form that
/// [`crate::mockstorage`] hands out names it, and a phone on the LAN running Tally has to reach it. What
/// protects it is the form's signed policy, not the caller's address (see [`storage_upload`]).
pub fn dev_routes(
    db_prefix: &str,
    mail: crate::mockmail::Handler,
    storage: crate::mockstorage::Handler,
) -> BoxEndpoint<'static> {
    let mut route = Route::new().at("/storage/upload", post(storage_upload).data(storage));
    if dev_routes_allowed(db_prefix, require_local_dynamodb_endpoint()) {
        tracing::warn!("dev routes on at /dev, localhost callers only");
        route = route.nest("/", routes(mail));
    } else {
        tracing::info!("dev routes off: they need DB_PREFIX=seslogin_local on a local DynamoDB");
    }
    route.boxed()
}

/// The mock storage upload endpoint: what an S3 presigned POST is for the real thing.
///
/// Takes the form [`crate::mockstorage::Handler::presign_post`] described, as multipart, with the file part
/// last. Answers 204 when the key, content type and exact size match the policy it was signed with and it
/// has not expired, else 400 with a short message. Never 403 or 404, which CloudFront would rewrite to the web
/// app's `index.html` in any deployed setup (`/dev` never reaches one, but the convention is kept).
#[handler]
async fn storage_upload(
    storage: Data<&crate::mockstorage::Handler>,
    multipart: Option<poem::web::Multipart>,
) -> poem::Response {
    let bad = |message: String| {
        poem::Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .content_type("text/plain; charset=utf-8")
            .body(message)
    };
    let Some(mut multipart) = multipart else {
        return bad("expected a multipart/form-data body".to_string());
    };
    let mut fields = std::collections::HashMap::new();
    let mut file = None;
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(e) => return bad(format!("bad form: {e}")),
        };
        let name = field.name().unwrap_or_default().to_string();
        if name == "file" {
            match field.bytes().await {
                Ok(bytes) => file = Some(bytes),
                Err(e) => return bad(format!("bad file part: {e}")),
            }
            // Like S3, ignore anything after the file.
            break;
        }
        match field.text().await {
            Ok(text) => {
                fields.insert(name, text);
            }
            Err(e) => return bad(format!("bad form field {name}: {e}")),
        }
    }
    let Some(file) = file else {
        return bad("missing file part".to_string());
    };
    match storage
        .accept_upload(&fields, file, crate::clock::now_sec())
        .await
    {
        Ok(()) => poem::Response::builder()
            .status(StatusCode::NO_CONTENT)
            .finish(),
        Err(message) => bad(message),
    }
}

/// Every `/dev` route, behind [`local_only`].
fn routes(mail: crate::mockmail::Handler) -> BoxEndpoint<'static> {
    Route::new()
        .at("/seed", post(seed))
        .at("/clear", post(clear))
        .at("/tokens", post(tokens))
        .at("/demo", post(demo))
        .at("/summary", post(summary))
        .at("/changes", post(changes))
        .at("/badges", post(badges))
        .at("/badges/digest", post(badge_digest))
        .at("/badges/clear", post(clear_badges))
        .at("/signin", post(signin))
        .at("/reactivate", post(reactivate))
        .at("/period-link", post(period_link))
        .at("/lag", post(set_lag))
        .at("/errors", post(set_errors))
        .at("/status", get(status))
        .before(local_only)
        .data(mail)
        .boxed()
}

/// Refuse other machines (the API listens on every interface) and other sites' pages, which could post here cross-site.
async fn local_only(req: Request) -> poem::Result<Request> {
    if caller_allowed(req.remote_addr().as_socket_addr(), req.header("origin")) {
        Ok(req)
    } else {
        Err(poem::Error::from_status(StatusCode::FORBIDDEN))
    }
}

/// Mount `/dev` only for the local tables on a local DynamoDB endpoint.
fn dev_routes_allowed(db_prefix: &str, endpoint: Result<String>) -> bool {
    db_prefix == "seslogin_local" && endpoint.is_ok()
}

/// A loopback caller, with no `Origin` (curl) or a local one (the dev server's pages).
fn caller_allowed(remote: Option<&std::net::SocketAddr>, origin: Option<&str>) -> bool {
    remote.is_some_and(|a| a.ip().is_loopback())
        && origin.is_none_or(|o| {
            url::Url::parse(o)
                .ok()
                .and_then(|u| u.host_str().map(is_local_url_host))
                .unwrap_or(false)
        })
}

/// Run a sibling binary via `cargo run`; returns its combined output.
async fn run_bin<S: AsRef<std::ffi::OsStr>>(
    bin: &str,
    args: impl IntoIterator<Item = S>,
) -> poem::Result<String> {
    let out = tokio::process::Command::new("cargo")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["run", "--quiet", "--bin", bin, "--"])
        // Plain logs: the output lands in the dev bar, not a terminal.
        .env("NO_COLOR", "1")
        .args(args)
        .output()
        .await
        .map_err(poem::error::InternalServerError)?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if out.status.success() {
        Ok(text)
    } else {
        Err(poem::Error::from_string(
            text,
            StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}

/// `make local-seed`
#[handler]
async fn seed() -> poem::Result<String> {
    run_bin("local-seed", ["apply"]).await
}

/// `make local-clear`
#[handler]
async fn clear() -> poem::Result<String> {
    run_bin("local-seed", ["clear"]).await
}

/// `local-seed apply --table user_token`: the seeded logins again, eg after Log out deleted one.
#[handler]
async fn tokens() -> poem::Result<String> {
    run_bin("local-seed", ["apply", "--table=user_token"]).await
}

/// `local-seed demo`: Test C Unit and its activity, relative to now.
#[handler]
async fn demo() -> poem::Result<String> {
    run_bin("local-seed", ["demo"]).await
}

/// Dry run of an email job: the emails it would send, printed, else why there are none.
async fn preview(bin: &str, args: &[&str], none: &str) -> poem::Result<String> {
    let out = run_bin(bin, args).await?;
    Ok(if out.contains("--- DRY RUN") {
        out
    } else {
        format!("{out}\nNo email to send: {none}")
    })
}

/// `activity-summary` dry run
#[handler]
async fn summary() -> poem::Result<String> {
    preview(
        "activity-summary",
        &[],
        "needs a user subscribed to daily summaries (admin Settings) and activity yesterday at their unit (Seed: Demo has both).",
    )
    .await
}

/// `change-digest` dry run for today, not yesterday, so a member edit shows straight away.
#[handler]
async fn changes() -> poem::Result<String> {
    let today = chrono::Utc::now()
        .with_timezone(&chrono_tz::Australia::Sydney)
        .date_naive();
    preview(
        "change-digest",
        &[&format!("--date={today}")],
        "needs a user subscribed to daily membership changes (admin Settings) and a member changed today at their unit.",
    )
    .await
}

/// `badge-nightly --apply`: the nightly badge awards, now.
#[handler]
async fn badges() -> poem::Result<String> {
    run_bin("badge-nightly", ["--apply"]).await
}

/// `badge-digest` dry run
#[handler]
async fn badge_digest() -> poem::Result<String> {
    preview(
        "badge-digest",
        &[],
        "needs a badge awarded this week at a unit whose weekly digest a user subscribed to (admin Settings).",
    )
    .await
}

/// `cli person clear-badges`, so badges can be earned again.
#[handler]
async fn clear_badges() -> poem::Result<String> {
    run_bin("cli", ["person", "clear-badges"]).await
}

#[derive(serde::Deserialize)]
struct SigninParams {
    person: String,
    location: String,
    session: Option<String>,
    hours_ago: Option<f64>,
}

/// `cli period create-signin ... --dry-run false`; `--flag=value` so a value can't become a flag.
#[handler]
async fn signin(Query(p): Query<SigninParams>) -> poem::Result<String> {
    let mut args = vec![
        "period".to_string(),
        "create-signin".into(),
        format!("--person={}", p.person),
        format!("--location={}", p.location),
        "--dry-run=false".into(),
    ];
    args.extend(p.session.map(|s| format!("--session={s}")));
    args.extend(p.hours_ago.map(|h| format!("--hours-ago={h}")));
    run_bin("cli", args).await
}

#[derive(serde::Deserialize)]
struct SessionParams {
    session: String,
}

/// `cli session edit --require-reactivation --dry-run false`
#[handler]
async fn reactivate(Query(p): Query<SessionParams>) -> poem::Result<String> {
    let args = [
        "session",
        "edit",
        "--require-reactivation",
        "--dry-run=false",
        "--",
        &p.session,
    ];
    run_bin("cli", args).await
}

#[derive(serde::Deserialize)]
struct PeriodParams {
    period: String,
}

/// `cli period-link issue`: a member's edit link for the period, against this site.
#[handler]
async fn period_link(Query(p): Query<PeriodParams>) -> poem::Result<String> {
    let base = format!("--base-url={}", crate::base_url::web_base_url());
    run_bin("cli", ["period-link", "issue", &base, "--", &p.period]).await
}

#[derive(serde::Deserialize)]
struct LagParams {
    ms: u64,
}

/// `--response-lag-ms`, without the restart.
#[handler]
async fn set_lag(Query(p): Query<LagParams>) -> String {
    EXTRA_LAG_MS.store(p.ms, Ordering::Relaxed);
    format!("Every GraphQL response now waits an extra {}ms", p.ms)
}

#[derive(serde::Deserialize)]
struct ErrorParams {
    #[serde(default)]
    targets: String,
}

/// `SESLOGIN_FORCE_FIELD_ERRORS`, without the restart; empty clears. Dev builds only, like the injector.
#[handler]
async fn set_errors(Query(p): Query<ErrorParams>) -> poem::Result<String> {
    #[cfg(debug_assertions)]
    return Ok(
        match crate::graphql::error_injection::set_runtime_targets(&p.targets) {
            0 if p.targets.trim().is_empty() => "Error injection off".to_string(),
            0 => {
                return Err(poem::Error::from_string(
                    format!("No valid targets in `{}`", p.targets),
                    StatusCode::BAD_REQUEST,
                ));
            }
            n => format!(
                "Failing {n} target(s): {}",
                crate::graphql::error_injection::runtime_targets()
            ),
        },
    );
    #[cfg(not(debug_assertions))]
    Err(poem::Error::from_string(
        format!(
            "error injection is compiled out of release builds (asked for `{}`)",
            p.targets
        ),
        StatusCode::NOT_IMPLEMENTED,
    ))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DevStatus {
    lag_ms: u64,
    error_targets: String,
    /// Newest message the mock mailer "sent", eg a login code.
    last_mail: Option<String>,
}

/// Runtime toggles and the newest mail, polled by the dev bar.
#[handler]
async fn status(mail: Data<&crate::mockmail::Handler>) -> Json<DevStatus> {
    #[cfg(debug_assertions)]
    let error_targets = crate::graphql::error_injection::runtime_targets();
    #[cfg(not(debug_assertions))]
    let error_targets = String::new();
    Json(DevStatus {
        lag_ms: extra_lag_ms(),
        error_targets,
        last_mail: (mail.sent().last())
            .map(|m| format!("To: {}\nSubject: {}\n\n{}", m.to, m.subject, m.body)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCAL: &str = "http://localhost:8100";

    #[test]
    fn dev_routes_only_for_local_tables_on_local_dynamodb() {
        let local = || local_endpoint(LOCAL.into());
        assert!(dev_routes_allowed("seslogin_local", local()));
        for prefix in ["seslogin_prod", "seslogin_test", "", "seslogin_local_old"] {
            assert!(
                !dev_routes_allowed(prefix, local()),
                "{prefix} must not mount /dev"
            );
        }
        for endpoint in [
            "https://dynamodb.ap-southeast-2.amazonaws.com",
            "http://192.0.2.10:8100",
            "not a url",
        ] {
            assert!(
                !dev_routes_allowed("seslogin_local", local_endpoint(endpoint.into())),
                "{endpoint} must not mount /dev"
            );
        }
        assert!(!dev_routes_allowed("seslogin_local", Err(anyhow!("unset"))));
    }

    #[test]
    fn dev_callers_must_be_this_machine_and_a_local_page() {
        let v4 = "127.0.0.1:50000".parse().unwrap();
        let v6 = "[::1]:50000".parse().unwrap();
        let lan = "192.0.2.10:50000".parse().unwrap();
        assert!(caller_allowed(Some(&v4), None));
        assert!(caller_allowed(Some(&v6), Some("http://localhost:5173")));
        assert!(caller_allowed(Some(&v4), Some("http://127.0.0.1:5173")));
        assert!(!caller_allowed(Some(&lan), None), "another machine");
        assert!(!caller_allowed(None, None), "unknown address");
        for origin in [
            "https://example.com",
            "http://localhost.example.com",
            "null",
        ] {
            assert!(
                !caller_allowed(Some(&v4), Some(origin)),
                "{origin} is another site"
            );
        }
    }

    /// `local_only` wraps every route: a request with no loopback address is refused.
    #[tokio::test]
    async fn dev_routes_refuse_unknown_callers() {
        use poem::Endpoint;
        for path in ["/seed", "/clear", "/status"] {
            let req = Request::builder()
                .method(poem::http::Method::POST)
                .uri(path.parse().unwrap())
                .finish();
            let code = routes(Default::default()).get_response(req).await.status();
            assert_eq!(code, StatusCode::FORBIDDEN, "{path}");
        }
    }

    /// The upload form `mockstorage` presigns, posted to the real route as a client would.
    async fn post_upload(
        storage: &crate::mockstorage::Handler,
        change: impl Fn(&str, String) -> Option<String>,
        file: Vec<u8>,
    ) -> StatusCode {
        use crate::storage::Handler as _;
        use poem::test::{TestClient, TestForm};
        let post = storage
            .presign_post(
                "pending/abc",
                "image/jpeg",
                4,
                std::time::Duration::from_secs(900),
            )
            .await
            .unwrap();
        assert_eq!(post.url, "/dev/storage/upload");
        let mut form = TestForm::new();
        for (name, value) in post.fields {
            if let Some(value) = change(&name, value) {
                form = form.text(name, value);
            }
        }
        let route = Route::new().nest(
            "/dev",
            dev_routes("not_local", Default::default(), storage.clone()),
        );
        let resp = TestClient::new(route)
            .post("/dev/storage/upload")
            .multipart(form.bytes("file", file))
            .send()
            .await;
        resp.0.status()
    }

    #[tokio::test]
    async fn storage_upload_stores_a_matching_form() {
        use crate::storage::Handler as _;
        let storage = crate::mockstorage::Handler::in_memory();
        let code = post_upload(&storage, |_, v| Some(v), vec![1, 2, 3, 4]).await;
        assert_eq!(code, StatusCode::NO_CONTENT);
        assert_eq!(
            storage.get("pending/abc").await.unwrap().unwrap(),
            [1, 2, 3, 4]
        );
    }

    #[tokio::test]
    async fn storage_upload_refuses_a_form_that_does_not_match() {
        use crate::storage::Handler as _;
        let storage = crate::mockstorage::Handler::in_memory();
        let same = |_: &str, v: String| Some(v);
        let cases: Vec<(&str, StatusCode)> = vec![
            (
                "wrong size",
                post_upload(&storage, same, vec![1, 2, 3]).await,
            ),
            (
                "wrong key",
                post_upload(
                    &storage,
                    |n, v| {
                        Some(if n == "key" {
                            "pending/other".into()
                        } else {
                            v
                        })
                    },
                    vec![1, 2, 3, 4],
                )
                .await,
            ),
            (
                "wrong type",
                post_upload(
                    &storage,
                    |n, v| {
                        Some(if n == "Content-Type" {
                            "image/webp".into()
                        } else {
                            v
                        })
                    },
                    vec![1, 2, 3, 4],
                )
                .await,
            ),
            (
                "no policy",
                post_upload(
                    &storage,
                    |n, v| (n != "policy").then_some(v),
                    vec![1, 2, 3, 4],
                )
                .await,
            ),
            (
                "forged policy",
                post_upload(
                    &storage,
                    |n, v| Some(if n == "policy" { format!("x{v}") } else { v }),
                    vec![1, 2, 3, 4],
                )
                .await,
            ),
        ];
        for (what, code) in cases {
            assert_eq!(code, StatusCode::BAD_REQUEST, "{what}");
        }
        assert_eq!(storage.head("pending/abc").await.unwrap(), None);
    }

    #[tokio::test]
    async fn storage_upload_refuses_an_expired_form() {
        use crate::storage::Handler as _;
        use poem::test::{TestClient, TestForm};
        let storage = crate::mockstorage::Handler::in_memory();
        // A form already past its expiry when it is presigned.
        let post = storage
            .presign_post(
                "pending/abc",
                "image/jpeg",
                4,
                std::time::Duration::from_secs(0),
            )
            .await
            .unwrap();
        let mut form = TestForm::new();
        for (name, value) in post.fields {
            form = form.text(name, value);
        }
        let route = Route::new().nest(
            "/dev",
            dev_routes("not_local", Default::default(), storage.clone()),
        );
        let resp = TestClient::new(route)
            .post("/dev/storage/upload")
            .multipart(form.bytes("file", vec![1, 2, 3, 4]))
            .send()
            .await;
        assert_eq!(resp.0.status(), StatusCode::BAD_REQUEST);
        assert_eq!(storage.head("pending/abc").await.unwrap(), None);
    }

    /// The other `/dev` routes stay local-only (and absent off the local tables), the upload route does not.
    #[tokio::test]
    async fn storage_upload_is_mounted_even_when_the_other_dev_routes_are_off() {
        use poem::test::TestClient;
        let route = Route::new().nest(
            "/dev",
            dev_routes(
                "seslogin_test",
                Default::default(),
                crate::mockstorage::Handler::in_memory(),
            ),
        );
        let client = TestClient::new(route);
        let resp = client.post("/dev/seed").send().await;
        assert_ne!(resp.0.status(), StatusCode::NO_CONTENT);
        let resp = client.post("/dev/storage/upload").send().await;
        assert_eq!(resp.0.status(), StatusCode::BAD_REQUEST);
    }

    /// With the other routes on, nesting them at `/` beside the upload route reaches both.
    #[tokio::test]
    async fn upload_route_coexists_with_the_local_only_routes() {
        use poem::test::TestClient;
        let route = Route::new()
            .at(
                "/storage/upload",
                post(storage_upload).data(crate::mockstorage::Handler::in_memory()),
            )
            .nest("/", routes(Default::default()));
        let client = TestClient::new(route);
        // No socket address in a test client, so the local-only guard refuses: the route exists.
        assert_eq!(
            client.post("/seed").send().await.0.status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            client.post("/storage/upload").send().await.0.status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn relative_times_resolve_from_the_seeding_moment_in_sydney() {
        // Sunday 25 Oct 2026, 20:00 Sydney (AEDT, UTC+11).
        let now = "2026-10-25T09:00:00Z".parse().unwrap();
        let at = |expr| resolve_relative_time(expr, now).unwrap();
        let utc = |s: &str| {
            s.parse::<chrono::DateTime<chrono::Utc>>()
                .unwrap()
                .timestamp()
        };
        assert_eq!(at("now"), now.timestamp());
        assert_eq!(at("now-2h-30m"), now.timestamp() - 9000);
        assert_eq!(at("today"), utc("2026-10-24T13:00:00Z"));
        assert_eq!(at("monday"), utc("2026-10-18T13:00:00Z"));
        // Tuesday 19:00, two weeks back.
        assert_eq!(at("monday-2w+1d+19h"), utc("2026-10-06T08:00:00Z"));
        // Across the 4 Oct DST start: still Tuesday 19:00 local, now AEST (UTC+10).
        assert_eq!(at("monday-4w+1d+19h"), utc("2026-09-22T09:00:00Z"));
        // Elapsed time stays elapsed: ten real hours back, across the change.
        let just_after_change = "2026-10-04T00:00:00Z".parse().unwrap(); // 11:00 AEDT, 9h after the change
        assert_eq!(
            resolve_relative_time("now-10h", just_after_change).unwrap(),
            just_after_change.timestamp() - 36_000
        );
        for bad in ["", "yesterday", "now-2", "now+h", "now-2y", "now-x1h"] {
            assert!(
                resolve_relative_time(bad, now).is_err(),
                "{bad:?} should not parse"
            );
        }
    }

    /// Only `poem-local` may mount `/dev`: the real server and the Lambda must never reference it.
    #[test]
    fn deployed_servers_never_mount_dev_routes() {
        let src = |rel: &str| {
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
                .unwrap_or_else(|e| panic!("reading {rel}: {e}"))
        };
        let poem = src("src/bin/poem.rs");
        assert!(!poem.contains("dev_routes") && !poem.contains("local_dev"));
        assert!(
            poem.contains("storage, None).await"),
            "poem.rs must pass no dev routes"
        );
        let lambda_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bin/lambda");
        for entry in std::fs::read_dir(&lambda_dir).expect("reading src/bin/lambda") {
            let path = entry.expect("dir entry").path();
            let text = std::fs::read_to_string(&path).expect("reading a lambda source");
            assert!(
                !text.contains("local_dev"),
                "{} references local_dev",
                path.display()
            );
        }
    }
}
