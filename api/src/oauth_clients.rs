//! Which OAuth clients exist, and what each may do.
//!
//! Two kinds:
//!
//! - **Static** clients are SESLogin's own apps, listed in [`STATIC_CLIENTS`] and
//!   checked in. Adding one is a PR editing that list. Their redirect URIs are
//!   matched exactly and are *not* run through [`oauth::validate_redirect_uri`],
//!   which is how a custom scheme like `tally://` is allowed for them and nobody
//!   else. Each lists the resources it may obtain a grant for.
//! - **Dynamic** clients come from RFC 7591 registration (`POST /oauth/register`):
//!   anyone can mint one, nothing is stored, and the `client_id` is a signed blob
//!   ([`oauth::encode_client_id`]). They only ever get [`DYNAMIC_CLIENT_RESOURCES`]
//!   — the MCP endpoint.
//!
//! A static client may also list Expo Go development redirects
//! ([`StaticClient::dev_expo_redirect_paths`]). Only a user flagged as a developer
//! (`User.is_dev`) may approve one; see [`is_dev_expo_redirect`].
//!
//! A static id never contains a `.` and a dynamic one always does
//! (`<payload>.<sig>`), so the two can't collide and registration can't mint a
//! static id. [`resolve_client`] is the one lookup every caller uses.

use crate::oauth;

/// A first-party OAuth client, defined in code. See the module docs.
#[derive(Debug)]
pub struct StaticClient {
    /// Never contains a `.` (enforced by a test over [`STATIC_CLIENTS`]).
    pub client_id: &'static str,
    pub client_name: &'static str,
    /// Matched exactly. May use a custom scheme.
    pub redirect_uris: &'static [&'static str],
    /// Paths of the `exp://<private IP>:<port><path>` redirects Expo Go uses while
    /// developing the app. Only a developer account may approve one; see
    /// [`is_dev_expo_redirect`].
    pub dev_expo_redirect_paths: &'static [&'static str],
    /// Resource paths, relative to the API base URL, that this client may be
    /// granted. The first is the default when the client doesn't name one.
    pub resources: &'static [&'static str],
}

/// SESLogin's own OAuth clients.
pub const STATIC_CLIENTS: &[StaticClient] = &[StaticClient {
    client_id: "tally",
    client_name: "Tally",
    redirect_uris: &["tally://oauth/callback"],
    dev_expo_redirect_paths: &["/--/oauth/callback"],
    resources: &["/graphql"],
}];

/// The only resource a dynamically registered client may be granted.
pub const DYNAMIC_CLIENT_RESOURCES: &[&str] = &["/mcp"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKind {
    Static,
    Dynamic,
}

/// A client id resolved by [`resolve_client`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    pub kind: ClientKind,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub dev_expo_redirect_paths: &'static [&'static str],
    pub resources: &'static [&'static str],
}

impl Client {
    /// Whether this is one of SESLogin's own apps rather than a self-registered
    /// client whose name nobody vouches for.
    pub fn is_first_party(&self) -> bool {
        self.kind == ClientKind::Static
    }

    /// Exact match against the client's redirect URIs.
    pub fn allows_redirect_uri(&self, redirect_uri: &str) -> bool {
        self.redirect_uris.iter().any(|u| u == redirect_uri)
    }

    /// Whether `redirect_uri` is one of this client's Expo Go development
    /// redirects ([`is_dev_expo_redirect`]). The caller must still check that the
    /// approving user is a developer.
    pub fn allows_dev_redirect_uri(&self, redirect_uri: &str) -> bool {
        is_dev_expo_redirect(redirect_uri, self.dev_expo_redirect_paths)
    }

    /// Whether `resource` is a full URL this client may be granted, given the
    /// API's base URL (see [`crate::base_url::api_base_url`]).
    pub fn allows_resource(&self, api_base: &str, resource: &str) -> bool {
        self.resources
            .iter()
            .any(|path| resource == format!("{api_base}{path}"))
    }

    /// Whether `resource`'s path is one this client may be granted. A cheap early
    /// check for the consent step, which has no reliable API base URL; the token
    /// endpoint does the full [`Client::allows_resource`] check.
    pub fn allows_resource_path(&self, resource: &str) -> bool {
        url::Url::parse(resource)
            .is_ok_and(|u| u.query().is_none() && self.resources.contains(&u.path()))
    }

    /// The resource granted when the client doesn't ask for one.
    pub fn default_resource(&self, api_base: &str) -> String {
        format!("{api_base}{}", self.resources[0])
    }
}

/// Whether `uri` is an Expo Go development redirect,
/// `exp://<host>:<port><path>` with `path` one of `paths`.
///
/// Expo Go serves the app from the developer's machine, so the redirect names
/// that machine's LAN address and can't be listed ahead of time. It is still
/// dangerous: whoever runs the Expo server at that address receives the code, and
/// an attacker who started the flow holds its PKCE verifier. So it is fenced in:
///
/// - **Developer accounts only.** `oauthAuthorizationRequest` and
///   `approveOauthAuthorization` accept it only when the signed-in user has
///   `is_dev` (set by a super user). The code is only ever issued there, so a
///   phishing link with an `exp://` redirect gets nowhere with anyone else.
/// - **Private addresses only**: loopback, RFC 1918 or link-local IPv4, or
///   `localhost`. A public host or an Expo tunnel (`*.exp.direct`) is refused, so
///   an attacker has to be on the developer's own network.
/// - **Exact shape**: the `exp` scheme, an explicit port, a listed path, and no
///   userinfo, query or fragment.
/// - **Visible**: the consent page warns that it is a development build and
///   names the address, and the grant shows that address under Connected apps
///   rather than as SESLogin's own app.
pub fn is_dev_expo_redirect(uri: &str, paths: &[&str]) -> bool {
    let Ok(url) = url::Url::parse(uri) else {
        return false;
    };
    if url.scheme() != "exp"
        || url.port().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !paths.contains(&url.path())
    {
        return false;
    }
    // `exp` isn't a special scheme, so `url` keeps the host as an opaque string
    // rather than parsing an IP out of it.
    match url.host_str() {
        Some("localhost") => true,
        Some(host) => host
            .parse::<std::net::Ipv4Addr>()
            .is_ok_and(|ip| ip.is_private() || ip.is_loopback() || ip.is_link_local()),
        None => false,
    }
}

/// The static client with this id, if any.
pub fn static_client(client_id: &str) -> Option<&'static StaticClient> {
    STATIC_CLIENTS.iter().find(|c| c.client_id == client_id)
}

/// Resolve a `client_id`: the static list first, then a dynamic registration
/// signed with `key` ([`crate::jwt::Key::oauth_client_id_key`]). `None` for an
/// unknown or tampered id.
pub fn resolve_client(key: &[u8], client_id: &str) -> Option<Client> {
    if let Some(c) = static_client(client_id) {
        return Some(Client {
            kind: ClientKind::Static,
            client_name: c.client_name.to_string(),
            redirect_uris: c.redirect_uris.iter().map(|u| u.to_string()).collect(),
            dev_expo_redirect_paths: c.dev_expo_redirect_paths,
            resources: c.resources,
        });
    }
    let registration = oauth::decode_client_id(key, client_id)?;
    Some(Client {
        kind: ClientKind::Dynamic,
        client_name: registration.client_name,
        redirect_uris: registration.redirect_uris,
        dev_expo_redirect_paths: &[],
        resources: DYNAMIC_CLIENT_RESOURCES,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [7u8; 32];

    fn dynamic_id(name: &str) -> String {
        oauth::encode_client_id(
            &KEY,
            &oauth::ClientRegistration {
                client_name: name.to_string(),
                redirect_uris: vec!["https://claude.ai/callback".to_string()],
                iat: 1_000_000,
            },
        )
    }

    #[test]
    fn static_clients_are_well_formed() {
        let mut ids = std::collections::HashSet::new();
        for c in STATIC_CLIENTS {
            assert!(!c.client_id.is_empty());
            assert!(
                !c.client_id.contains('.'),
                "{} could collide with a dynamic id",
                c.client_id
            );
            assert!(ids.insert(c.client_id), "duplicate id {}", c.client_id);
            assert!(!c.redirect_uris.is_empty(), "{}", c.client_id);
            for uri in c.redirect_uris {
                let parsed = url::Url::parse(uri).expect("redirect URI parses");
                assert!(parsed.fragment().is_none(), "{uri}");
            }
            for path in c.dev_expo_redirect_paths {
                assert!(path.starts_with('/'), "{path}");
            }
            assert!(!c.resources.is_empty(), "{}", c.client_id);
            for path in c.resources {
                assert!(path.starts_with('/'), "{path}");
            }
            // A static id must never be readable as a dynamic registration.
            assert_eq!(oauth::decode_client_id(&KEY, c.client_id), None);
        }
    }

    #[test]
    fn resolves_tally_as_static() {
        let client = resolve_client(&KEY, "tally").expect("tally is listed");
        assert_eq!(client.kind, ClientKind::Static);
        assert!(client.is_first_party());
        assert_eq!(client.client_name, "Tally");
        assert_eq!(client.redirect_uris, vec!["tally://oauth/callback"]);
        assert_eq!(client.resources, &["/graphql"]);
    }

    #[test]
    fn unknown_ids_do_not_resolve() {
        assert_eq!(resolve_client(&KEY, "nope"), None);
        assert_eq!(resolve_client(&KEY, "Tally"), None);
        assert_eq!(resolve_client(&KEY, ""), None);
        assert_eq!(resolve_client(&KEY, "tally.sig"), None);
    }

    #[test]
    fn dynamic_ids_resolve_as_dynamic_and_mcp_only() {
        // Even a registration that calls itself "tally" stays dynamic.
        let id = dynamic_id("tally");
        assert!(id.contains('.'));
        assert!(static_client(&id).is_none());
        let client = resolve_client(&KEY, &id).expect("signed id resolves");
        assert_eq!(client.kind, ClientKind::Dynamic);
        assert!(!client.is_first_party());
        assert_eq!(client.resources, DYNAMIC_CLIENT_RESOURCES);
        assert_eq!(resolve_client(&[9u8; 32], &id), None);
    }

    #[test]
    fn redirect_uris_match_exactly() {
        let client = resolve_client(&KEY, "tally").unwrap();
        assert!(client.allows_redirect_uri("tally://oauth/callback"));
        for uri in [
            "tally://oauth/callback/",
            "tally://oauth/callback?x=1",
            "tally://oauth/other",
            "TALLY://oauth/callback",
            "https://oauth/callback",
        ] {
            assert!(!client.allows_redirect_uri(uri), "{uri}");
        }
    }

    #[test]
    fn resources_are_limited_to_the_clients_own_paths() {
        let base = "https://seslogin.com";
        let tally = resolve_client(&KEY, "tally").unwrap();
        assert_eq!(tally.default_resource(base), "https://seslogin.com/graphql");
        assert!(tally.allows_resource(base, "https://seslogin.com/graphql"));
        assert!(!tally.allows_resource(base, "https://seslogin.com/mcp"));
        assert!(!tally.allows_resource(base, "https://evil.example/graphql"));
        assert!(!tally.allows_resource(base, "https://seslogin.com/graphql/"));

        let dynamic = resolve_client(&KEY, &dynamic_id("Claude")).unwrap();
        assert_eq!(dynamic.default_resource(base), "https://seslogin.com/mcp");
        assert!(dynamic.allows_resource(base, "https://seslogin.com/mcp"));
        assert!(!dynamic.allows_resource(base, "https://seslogin.com/graphql"));
    }

    #[test]
    fn resource_path_check() {
        let tally = resolve_client(&KEY, "tally").unwrap();
        assert!(tally.allows_resource_path("http://localhost:8000/graphql"));
        assert!(!tally.allows_resource_path("http://localhost:8000/mcp"));
        assert!(!tally.allows_resource_path("http://localhost:8000/graphql?x"));
        assert!(!tally.allows_resource_path("not a url"));
        let dynamic = resolve_client(&KEY, &dynamic_id("Claude")).unwrap();
        assert!(dynamic.allows_resource_path("https://seslogin.com/mcp"));
        assert!(!dynamic.allows_resource_path("https://seslogin.com/graphql"));
    }

    #[test]
    fn dev_expo_redirects_need_a_private_host_port_and_listed_path() {
        let paths = &["/--/oauth/callback"];
        for uri in [
            "exp://192.168.1.20:8081/--/oauth/callback",
            "exp://10.0.0.5:8081/--/oauth/callback",
            "exp://172.20.1.1:19000/--/oauth/callback",
            "exp://127.0.0.1:8081/--/oauth/callback",
            "exp://169.254.3.4:8081/--/oauth/callback",
            "exp://localhost:8081/--/oauth/callback",
        ] {
            assert!(is_dev_expo_redirect(uri, paths), "{uri}");
        }
        for uri in [
            "exp://8.8.8.8:8081/--/oauth/callback",
            "exp://172.32.0.1:8081/--/oauth/callback",
            "exp://abc-anon-8081.exp.direct/--/oauth/callback",
            "exp://evil.example:8081/--/oauth/callback",
            "exp://192.168.1.20/--/oauth/callback",
            "exp://192.168.1.20:8081/--/oauth/other",
            "exp://192.168.1.20:8081/--/oauth/callback/",
            "exp://192.168.1.20:8081/--/oauth/callback?x=1",
            "exp://192.168.1.20:8081/--/oauth/callback#x",
            "exp://u:p@192.168.1.20:8081/--/oauth/callback",
            "exps://192.168.1.20:8081/--/oauth/callback",
            "https://192.168.1.20:8081/--/oauth/callback",
            "not a url",
        ] {
            assert!(!is_dev_expo_redirect(uri, paths), "{uri}");
        }
        assert!(!is_dev_expo_redirect(
            "exp://192.168.1.20:8081/--/oauth/callback",
            &[]
        ));
    }

    #[test]
    fn only_static_clients_listing_dev_paths_get_dev_redirects() {
        let dev = "exp://192.168.1.20:8081/--/oauth/callback";
        let tally = resolve_client(&KEY, "tally").unwrap();
        assert!(tally.allows_dev_redirect_uri(dev));
        assert!(!tally.allows_redirect_uri(dev));
        let dynamic = resolve_client(&KEY, &dynamic_id("Claude")).unwrap();
        assert!(!dynamic.allows_dev_redirect_uri(dev));
    }
}
