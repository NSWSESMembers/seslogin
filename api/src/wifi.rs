//! The WiFi networks Tally shows its signed-in users: name, SSID and password.
//!
//! They hold passwords, so they are never committed or stored in the database: they
//! come from the `WIFI_NETWORKS` environment variable, a JSON array set from the
//! git-ignored `infra/terraform.tfvars` (`var.wifi_networks`) in deployed environments
//! and from `.env.secret` locally. Unset or blank means no networks. Only
//! `Query.wifiNetworks` reads it, and only for user callers.
//!
//! Each list has a [`version`], a hash of its content, so a client that already has the
//! current list is told so instead of being sent the passwords again, and nobody has to
//! remember to bump anything when a network changes.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::error;

const NETWORKS_VAR: &str = "WIFI_NETWORKS";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Security {
    /// WPA, WPA2 or WPA3 personal.
    #[serde(rename = "WPA")]
    Wpa,
    #[serde(rename = "WEP")]
    Wep,
    /// An open network, with no password.
    #[serde(rename = "nopass")]
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Network {
    pub title: String,
    pub ssid: String,
    /// Empty for an open network.
    pub password: String,
    pub security: Security,
    pub hidden: bool,
}

/// One entry as written in `WIFI_NETWORKS`. `title` defaults to the SSID; `security` to
/// WPA when there is a password and to an open network when there isn't.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNetwork {
    title: Option<String>,
    ssid: String,
    password: Option<String>,
    security: Option<Security>,
    #[serde(default)]
    hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Networks {
    pub version: String,
    pub networks: Vec<Network>,
}

/// Parse a `WIFI_NETWORKS` value. Errors never quote the input, so a password can't end
/// up in a log.
pub fn parse(text: &str) -> Result<Networks> {
    let raw: Vec<RawNetwork> = if text.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(text).map_err(|e| {
            anyhow!(
                "{NETWORKS_VAR} is not a JSON array of networks ({:?} error at line {}, column {})",
                e.classify(),
                e.line(),
                e.column()
            )
        })?
    };
    let networks = raw
        .into_iter()
        .enumerate()
        .map(|(i, n)| {
            let n_th = i + 1;
            let ssid = n.ssid.trim().to_string();
            if ssid.is_empty() {
                return Err(anyhow!("{NETWORKS_VAR} network {n_th} has no ssid"));
            }
            let password = n.password.unwrap_or_default();
            let security = n.security.unwrap_or(if password.is_empty() {
                Security::None
            } else {
                Security::Wpa
            });
            if security != Security::None && password.is_empty() {
                return Err(anyhow!("{NETWORKS_VAR} network {n_th} has no password"));
            }
            let title = n
                .title
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| ssid.clone());
            Ok(Network {
                title,
                ssid,
                password: if security == Security::None {
                    String::new()
                } else {
                    password
                },
                security,
                hidden: n.hidden,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Networks {
        version: version(&networks),
        networks,
    })
}

/// The first 16 hex digits of the SHA-256 of the parsed list: changes whenever anything a
/// client shows changes, including the order.
fn version(networks: &[Network]) -> String {
    let json = serde_json::to_vec(networks).expect("networks serialize");
    hex::encode(&Sha256::digest(json)[..8])
}

/// The configured networks. A malformed `WIFI_NETWORKS` is logged and refused, rather
/// than quietly showing fewer networks.
pub fn from_env() -> Result<Networks> {
    parse(&std::env::var(NETWORKS_VAR).unwrap_or_default()).inspect_err(|e| error!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_is_an_empty_list_with_a_version() {
        let parsed = parse("").unwrap();
        assert!(parsed.networks.is_empty());
        assert_eq!(parsed.version.len(), 16);
        assert_eq!(parsed, parse(" \n").unwrap());
    }

    #[test]
    fn fills_in_the_defaults() {
        let parsed = parse(
            r#"[
                {"ssid": "SES-Shed", "password": "pw"},
                {"title": " Guest ", "ssid": "Open", "password": "ignored", "security": "nopass"},
                {"title": "Old", "ssid": "Old", "password": "pw", "security": "WEP", "hidden": true}
            ]"#,
        )
        .unwrap();
        assert_eq!(
            parsed.networks,
            vec![
                Network {
                    title: "SES-Shed".into(),
                    ssid: "SES-Shed".into(),
                    password: "pw".into(),
                    security: Security::Wpa,
                    hidden: false,
                },
                Network {
                    title: "Guest".into(),
                    ssid: "Open".into(),
                    password: String::new(),
                    security: Security::None,
                    hidden: false,
                },
                Network {
                    title: "Old".into(),
                    ssid: "Old".into(),
                    password: "pw".into(),
                    security: Security::Wep,
                    hidden: true,
                },
            ]
        );
    }

    #[test]
    fn version_follows_the_content() {
        let a = parse(r#"[{"ssid": "A", "password": "one"}]"#).unwrap();
        let spaced = parse(r#"[ { "password": "one", "ssid": "A" } ]"#).unwrap();
        let changed = parse(r#"[{"ssid": "A", "password": "two"}]"#).unwrap();
        assert_eq!(a.version, spaced.version);
        assert_ne!(a.version, changed.version);
        assert_ne!(a.version, parse("").unwrap().version);
    }

    #[test]
    fn refuses_bad_entries_without_quoting_them() {
        for text in [
            r#"{"ssid": "A"}"#,
            r#"[{"ssid": " ", "password": "secret-1"}]"#,
            r#"[{"ssid": "A", "security": "WPA"}]"#,
            r#"[{"ssid": "A", "password": "secret-1", "security": "WPA4"}]"#,
            r#"[{"ssid": "A", "pasword": "secret-1"}]"#,
            r#"[{"ssid": "A", "password": "secret-1""#,
        ] {
            let err = parse(text).unwrap_err().to_string();
            assert!(!err.contains("secret-1"), "{err}");
        }
    }
}
