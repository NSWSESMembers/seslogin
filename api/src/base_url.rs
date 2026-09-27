//! Public base URLs the API needs to build absolute links: the web app's origin
//! (member-facing edit links, the OAuth consent page) and the API's own origin
//! (OAuth issuer/endpoint metadata).

/// Env var holding the public site origin. Shared by [`web_base_url`] and, until
/// now, duplicated inline in `period_link.rs`.
const WEB_BASE_URL_VAR: &str = "WEB_BASE_URL";

/// The public site origin used to build member- and admin-facing links into the
/// web app (period edit links, the OAuth consent page).
///
/// Falls back to the first `WEBAUTHN_RP_ORIGIN`, which is already a valid site
/// origin in every environment (`http://localhost:5173` locally). Every deployed
/// environment sets `WEB_BASE_URL` explicitly (prod/preprod → `https://seslogin.com`,
/// test → `https://test.seslogin.com`) rather than relying on the fallback, whose
/// first prod entry is the `new.` alias, not the apex a member link should use.
pub fn web_base_url() -> String {
    std::env::var(WEB_BASE_URL_VAR)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::env::var("WEBAUTHN_RP_ORIGIN")
                .ok()
                .and_then(|origins| origins.split(',').next().map(str::to_string))
        })
        .unwrap_or_else(|| "http://localhost:5173".to_string())
        .trim()
        .trim_end_matches('/')
        .to_string()
}

/// Env var holding an explicit API origin, for when the request's `Host` header
/// isn't trustworthy or convenient to derive from (e.g. behind a CDN).
const API_BASE_URL_VAR: &str = "API_BASE_URL";

/// The API's own public origin, used as the OAuth issuer and to build the
/// `token_endpoint`/`registration_endpoint` metadata URLs.
///
/// Prefers the explicit `API_BASE_URL` env var. Falling back to the request's
/// `Host` header is deliberately last-resort and derived, not configured: every
/// deployed environment's Function URL host is stable but not worth hard-coding
/// per-environment, and `poem-local`'s `localhost:8000` needs no configuration at
/// all. The scheme is `http` only for `localhost`/`127.0.0.1` hosts (local dev);
/// everything else — including a bare Function URL host — is `https`, which is
/// all that's ever fronted by API Gateway/Lambda Function URLs or `poem` behind a
/// TLS-terminating proxy.
pub fn api_base_url(host_header: Option<&str>) -> String {
    if let Ok(base) = std::env::var(API_BASE_URL_VAR) {
        let base = base.trim();
        if !base.is_empty() {
            return base.trim_end_matches('/').to_string();
        }
    }
    let host = host_header.unwrap_or("localhost:8000").trim();
    let scheme = if host.starts_with("localhost") || host.starts_with("127.0.0.1") {
        "http"
    } else {
        "https"
    };
    format!("{scheme}://{host}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Env var tests must not run concurrently with each other (or with anything
    // else touching these vars) — serialize them behind a single mutex.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env<T>(vars: &[(&str, Option<&str>)], f: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous: Vec<(String, Option<String>)> = vars
            .iter()
            .map(|(k, _)| (k.to_string(), std::env::var(k).ok()))
            .collect();
        // SAFETY: serialized by `ENV_LOCK`, and this crate spawns no other threads
        // during the test binary's setup that would read these vars concurrently.
        unsafe {
            for (k, v) in vars {
                match v {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
        }
        let result = f();
        unsafe {
            for (k, v) in previous {
                match v {
                    Some(v) => std::env::set_var(&k, v),
                    None => std::env::remove_var(&k),
                }
            }
        }
        result
    }

    #[test]
    fn web_base_url_prefers_the_explicit_var() {
        with_env(
            &[
                ("WEB_BASE_URL", Some("https://new.seslogin.com/")),
                ("WEBAUTHN_RP_ORIGIN", Some("https://seslogin.com")),
            ],
            || assert_eq!(web_base_url(), "https://new.seslogin.com"),
        );
    }

    #[test]
    fn web_base_url_falls_back_to_first_rp_origin() {
        with_env(
            &[
                ("WEB_BASE_URL", None),
                (
                    "WEBAUTHN_RP_ORIGIN",
                    Some("https://seslogin.com,https://new.seslogin.com"),
                ),
            ],
            || assert_eq!(web_base_url(), "https://seslogin.com"),
        );
    }

    #[test]
    fn web_base_url_falls_back_to_localhost() {
        with_env(
            &[("WEB_BASE_URL", None), ("WEBAUTHN_RP_ORIGIN", None)],
            || assert_eq!(web_base_url(), "http://localhost:5173"),
        );
    }

    #[test]
    fn api_base_url_prefers_the_explicit_var() {
        with_env(
            &[("API_BASE_URL", Some("https://api.seslogin.com/"))],
            || {
                assert_eq!(
                    api_base_url(Some("ignored.example.com")),
                    "https://api.seslogin.com"
                )
            },
        );
    }

    #[test]
    fn api_base_url_derives_https_from_host_by_default() {
        with_env(&[("API_BASE_URL", None)], || {
            assert_eq!(
                api_base_url(Some("abc123.lambda-url.ap-southeast-2.on.aws")),
                "https://abc123.lambda-url.ap-southeast-2.on.aws"
            )
        });
    }

    #[test]
    fn api_base_url_uses_http_for_localhost_and_loopback() {
        with_env(&[("API_BASE_URL", None)], || {
            assert_eq!(
                api_base_url(Some("localhost:8000")),
                "http://localhost:8000"
            );
            assert_eq!(
                api_base_url(Some("127.0.0.1:8000")),
                "http://127.0.0.1:8000"
            );
        });
    }

    #[test]
    fn api_base_url_falls_back_to_localhost_with_no_host() {
        with_env(&[("API_BASE_URL", None)], || {
            assert_eq!(api_base_url(None), "http://localhost:8000")
        });
    }
}
