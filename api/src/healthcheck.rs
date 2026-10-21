//! Pings a kiosk session's configured healthcheck URL.
//!
//! A short, timeboxed GET made by the API itself. It used to go through an SQS queue to
//! a dedicated Lambda; doing it inline saves the queue and its pollers.
//!
//! The URL is set by any unit Admin, so it is untrusted, and the request is made from
//! a process holding database access and secrets. Only public addresses are contacted:
//! resolved addresses are filtered by [`PublicOnlyResolver`], IP-literal hosts (which
//! never reach the resolver) are checked up front, redirects are not followed (a
//! redirect to an IP literal would skip that check), and no proxy is used. Without
//! this, an admin could point it at loopback, for example the Lambda runtime API on
//! `127.0.0.1:9001`, or at link-local or private addresses.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Result, anyhow};
use reqwest::dns::{Addrs, Name, Resolve, Resolving};

/// Total time allowed per ping. It is awaited inline in the request that triggered it,
/// because Lambda freezes the process once the response is sent, so a spawned task
/// could not be relied on to finish.
const TIMEOUT: Duration = Duration::from_secs(3);

/// GET `healthcheck_url` on behalf of `session_id`, logging the outcome. Never fails:
/// a healthcheck that can't be delivered must not affect the kiosk's request.
pub async fn ping(session_id: &str, healthcheck_url: &str) {
    match send(healthcheck_url).await {
        Ok(()) => tracing::info!(
            log_type = "healthcheck",
            consumer = "healthcheck",
            success = true,
            session_id = %session_id,
            "healthcheck sent",
        ),
        Err(e) => tracing::warn!(
            log_type = "healthcheck",
            consumer = "healthcheck",
            success = false,
            session_id = %session_id,
            error = %e,
            "healthcheck failed",
        ),
    }
}

async fn send(healthcheck_url: &str) -> Result<()> {
    let url = reqwest::Url::parse(healthcheck_url)?;
    check_url(&url)?;
    client()?.get(url).send().await?;
    Ok(())
}

fn client() -> Result<&'static reqwest::Client> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .dns_resolver(PublicOnlyResolver)
        .build()?;
    Ok(CLIENT.get_or_init(|| client))
}

/// Refuse a URL before sending if its scheme is wrong or its host is a non-public IP
/// literal. Hostnames are left to [`PublicOnlyResolver`].
fn check_url(url: &reqwest::Url) -> Result<()> {
    match url.scheme() {
        "http" | "https" => {}
        other => return Err(anyhow!("unsupported scheme {other:?}")),
    }
    let ip = match url.host() {
        Some(url::Host::Ipv4(ip)) => IpAddr::V4(ip),
        Some(url::Host::Ipv6(ip)) => IpAddr::V6(ip),
        Some(url::Host::Domain(_)) => return Ok(()),
        None => return Err(anyhow!("URL has no host")),
    };
    if is_public(ip) {
        Ok(())
    } else {
        Err(anyhow!("refusing non-public address {ip}"))
    }
}

/// System resolver that drops every non-public address it returns.
struct PublicOnlyResolver;

impl Resolve for PublicOnlyResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let lookup = host.clone();
            let addrs: Vec<SocketAddr> =
                tokio::task::spawn_blocking(move || (lookup.as_str(), 0).to_socket_addrs())
                    .await??
                    .filter(|addr| is_public(addr.ip()))
                    .collect();
            if addrs.is_empty() {
                return Err(format!("{host} has no public address").into());
            }
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}

fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_v4(ip),
        IpAddr::V6(ip) => match ip.to_ipv4_mapped() {
            Some(v4) => is_public_v4(v4),
            None => is_public_v6(ip),
        },
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || a == 0 // "this network"
        || (a == 100 && (64..128).contains(&b)) // shared address space (CGNAT)
        || (a == 192 && b == 0 && ip.octets()[2] == 0) // IETF protocol assignments
        || (a == 198 && (18..20).contains(&b)) // benchmarking
        || a >= 240) // reserved
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || (first & 0xfe00) == 0xfc00 // unique local
        || (first & 0xffc0) == 0xfe80 // link-local
        || (first == 0x2001 && ip.segments()[1] == 0x0db8) // documentation
        || (first == 0x0064 && ip.segments()[1] == 0xff9b)) // NAT64, could reach any v4
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn classifies_addresses() {
        for s in [
            "127.0.0.1",
            "0.0.0.0",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "255.255.255.255",
            "224.0.0.1",
            "240.0.0.1",
            "::",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "64:ff9b::7f00:1",
        ] {
            assert!(!is_public(ip(s)), "{s} should not be public");
        }
        for s in ["1.1.1.1", "8.8.8.8", "100.128.0.1", "2606:4700::1111"] {
            assert!(is_public(ip(s)), "{s} should be public");
        }
    }

    #[test]
    fn refuses_non_public_ip_literals() {
        for s in [
            "http://127.0.0.1:9001/2018-06-01/runtime/invocation/next",
            "http://[::1]/",
            "http://169.254.169.254/latest/meta-data/",
            "https://10.0.0.1/ping",
            "http://[::ffff:7f00:1]/",
            "ftp://example.com/",
        ] {
            let url = reqwest::Url::parse(s).unwrap();
            assert!(check_url(&url).is_err(), "{s} should be refused");
        }
    }

    #[test]
    fn allows_hostnames_and_public_literals() {
        for s in ["https://hc-ping.com/abc", "http://1.1.1.1/"] {
            let url = reqwest::Url::parse(s).unwrap();
            assert!(check_url(&url).is_ok(), "{s} should be allowed");
        }
    }

    #[tokio::test]
    async fn resolver_drops_loopback() {
        let err = PublicOnlyResolver
            .resolve("localhost".parse().unwrap())
            .await
            .err()
            .expect("localhost must not resolve to anything usable");
        assert!(err.to_string().contains("no public address"), "{err}");
    }

    #[tokio::test]
    async fn ping_never_contacts_loopback() {
        // Bind a listener and point the ping at it, by hostname and by literal; neither
        // may connect.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        for url in [
            format!("http://localhost:{port}/"),
            format!("http://127.0.0.1:{port}/"),
        ] {
            assert!(send(&url).await.is_err(), "{url} should fail");
        }
        let accepted = tokio::time::timeout(Duration::from_millis(100), listener.accept()).await;
        assert!(
            accepted.is_err(),
            "a connection reached the loopback listener"
        );
    }
}
