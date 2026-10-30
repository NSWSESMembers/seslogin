//! `Query.wifiNetworks`: the WiFi networks Tally shows its signed-in members. See
//! `crate::wifi` for where they come from.

use anyhow::{Result, anyhow};
use async_graphql::{Enum, SimpleObject};

use crate::wifi;

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum WifiSecurity {
    /// WPA, WPA2 or WPA3 personal.
    Wpa,
    Wep,
    /// An open network, with no password.
    None,
}

impl From<wifi::Security> for WifiSecurity {
    fn from(security: wifi::Security) -> Self {
        match security {
            wifi::Security::Wpa => Self::Wpa,
            wifi::Security::Wep => Self::Wep,
            wifi::Security::None => Self::None,
        }
    }
}

#[derive(SimpleObject)]
pub struct WifiNetwork {
    /// What the list shows, such as "Shed".
    pub title: String,
    pub ssid: String,
    /// Empty for an open network.
    pub password: String,
    pub security: WifiSecurity,
    pub hidden: bool,
}

#[derive(SimpleObject)]
pub struct WifiNetworks {
    /// Changes whenever any network does. Pass it back as `knownVersion` next time.
    pub version: String,
    /// Null when `knownVersion` is already the current version, so a client that has
    /// the list isn't sent the passwords again.
    pub networks: Option<Vec<WifiNetwork>>,
}

pub(super) fn wifi_networks(known_version: Option<&str>) -> Result<WifiNetworks> {
    let current =
        wifi::from_env().map_err(|_| anyhow!("WiFi networks are misconfigured on this server"))?;
    let networks = (known_version != Some(current.version.as_str())).then(|| {
        current
            .networks
            .into_iter()
            .map(|n| WifiNetwork {
                title: n.title,
                ssid: n.ssid,
                password: n.password,
                security: n.security.into(),
                hidden: n.hidden,
            })
            .collect()
    });
    Ok(WifiNetworks {
        version: current.version,
        networks,
    })
}
