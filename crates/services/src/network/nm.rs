//! NetworkManager helper for fetching network state.

use std::collections::HashMap;
use std::ops::Deref;
use std::time::Duration;

use anyhow::Result;
use futures_util::StreamExt;
use tracing::debug;
use zbus::proxy::CacheProperties;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

use super::dbus::access_point::AccessPointProxy;
use super::dbus::active_connection::ActiveConnectionProxy;
use super::dbus::device::DeviceProxy;
use super::dbus::device::wired::WiredDeviceProxy;
use super::dbus::device::wireless::WirelessDeviceProxy;
use super::dbus::nm::NetworkManagerProxy;
use super::dbus::settings::{ConnectionProxy, SettingsProxy};
use super::dbus::statistics::StatisticsProxy;
use super::types::{
    AccessPoint, ActiveConnectionInfo, DeviceState, DeviceType, NetworkStatistics, VpnProfile,
    WifiAttemptStatus,
};

/// NetworkManager wrapper for fetching network state.
#[derive(Debug)]
pub struct NetworkManager<'a>(NetworkManagerProxy<'a>);

impl<'a> Deref for NetworkManager<'a> {
    type Target = NetworkManagerProxy<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> NetworkManager<'a> {
    /// Create a new NetworkManager wrapper.
    pub async fn new(connection: &'a zbus::Connection) -> zbus::Result<NetworkManager<'a>> {
        NetworkManagerProxy::new(connection).await.map(Self)
    }

    /// Get all active connections with their details.
    pub async fn active_connections(&self) -> Result<Vec<ActiveConnectionInfo>> {
        let active_connections = self.0.active_connections().await?;
        let mut ac_proxies = Vec::with_capacity(active_connections.len());

        for active_connection in active_connections {
            let proxy = ActiveConnectionProxy::builder(self.inner().connection())
                .path(active_connection)?
                .build()
                .await?;
            ac_proxies.push(proxy);
        }

        let mut info = Vec::with_capacity(ac_proxies.len());

        for connection in ac_proxies {
            let name = connection.id().await?;
            let object_path = connection.inner().path().to_owned().into();

            if connection.vpn().await.unwrap_or_default() {
                info.push(ActiveConnectionInfo::Vpn { name, object_path });
                continue;
            }

            for device in connection.devices().await.unwrap_or_default() {
                let device_proxy = DeviceProxy::builder(self.inner().connection())
                    .path(device)?
                    .build()
                    .await?;

                match device_proxy.device_type().await.map(DeviceType::from).ok() {
                    Some(DeviceType::Ethernet) => {
                        let wired_device = WiredDeviceProxy::builder(self.inner().connection())
                            .path(device_proxy.inner().path())?
                            .build()
                            .await?;

                        info.push(ActiveConnectionInfo::Wired {
                            name: name.clone(),
                            speed: wired_device.speed().await?,
                        });
                    }
                    Some(DeviceType::Wifi) => {
                        let wireless_device =
                            WirelessDeviceProxy::builder(self.inner().connection())
                                .path(device_proxy.inner().path())?
                                .build()
                                .await?;

                        if let Ok(access_point) = wireless_device.active_access_point().await {
                            let ap_proxy = AccessPointProxy::builder(self.inner().connection())
                                .path(access_point)?
                                .build()
                                .await?;

                            info.push(ActiveConnectionInfo::WiFi {
                                id: name.clone(),
                                name: String::from_utf8_lossy(&ap_proxy.ssid().await?).into_owned(),
                                strength: ap_proxy.strength().await.unwrap_or_default(),
                                device: device_proxy.inner().path().to_string(),
                                object_path: object_path.clone(),
                            });
                        }
                    }
                    Some(DeviceType::WireGuard) => {
                        info.push(ActiveConnectionInfo::Vpn {
                            name: name.clone(),
                            object_path: object_path.clone(),
                        });
                    }
                    _ => {}
                }
            }
        }

        // Sort: VPN first, then Wired, then WiFi
        info.sort_by(|a, b| {
            let priority = |conn: &ActiveConnectionInfo| match conn {
                ActiveConnectionInfo::Vpn { .. } => 0,
                ActiveConnectionInfo::Wired { .. } => 1,
                ActiveConnectionInfo::WiFi { .. } => 2,
            };
            priority(a)
                .cmp(&priority(b))
                .then_with(|| a.name().cmp(b.name()))
        });

        Ok(info)
    }

    /// Get all wireless device paths.
    pub async fn wireless_devices(&self) -> Result<Vec<OwnedObjectPath>> {
        let devices = self.devices().await?;
        let mut wireless_devices = Vec::new();

        for device in devices {
            let device_proxy = DeviceProxy::builder(self.inner().connection())
                .path(&device)?
                .build()
                .await?;

            if matches!(
                device_proxy.device_type().await.map(DeviceType::from),
                Ok(DeviceType::Wifi)
            ) {
                wireless_devices.push(device);
            }
        }

        Ok(wireless_devices)
    }

    /// Get network statistics for WiFi devices.
    pub async fn network_statistics(&self) -> Result<Vec<NetworkStatistics>> {
        let devices = self.devices().await?;
        let mut network_statistics = Vec::new();

        for device in devices {
            let device_proxy = DeviceProxy::builder(self.inner().connection())
                .path(&device)?
                .build()
                .await?;

            if matches!(
                device_proxy.device_type().await.map(DeviceType::from),
                Ok(DeviceType::Wifi)
            ) {
                let stats_proxy = StatisticsProxy::builder(self.inner().connection())
                    .path(&device)?
                    .build()
                    .await?;

                let tx = stats_proxy.tx_bytes().await?;
                let rx = stats_proxy.rx_bytes().await?;
                let timestamp = chrono::Utc::now().timestamp();

                network_statistics.push(NetworkStatistics {
                    prev_rx: rx,
                    prev_tx: tx,
                    prev_rx_time: timestamp,
                    prev_tx_time: timestamp,
                    tx,
                    rx,
                    rx_time: timestamp,
                    tx_time: timestamp,
                    device: device_proxy.inner().path().to_string(),
                });
            }
        }

        Ok(network_statistics)
    }

    /// Read every saved profile once.
    pub async fn saved_profiles(&self) -> Result<SavedProfiles> {
        let settings_proxy = SettingsProxy::new(self.inner().connection()).await?;
        let mut profiles = SavedProfiles::default();

        for path in settings_proxy.list_connections().await? {
            let proxy = ConnectionProxy::builder(self.inner().connection())
                .path(&path)?
                .build()
                .await?;
            let Ok(settings) = proxy.get_settings().await else {
                continue;
            };
            let Some(connection) = settings.get("connection") else {
                continue;
            };
            let field =
                |value: Option<&OwnedValue>| value.and_then(|v| String::try_from(v.clone()).ok());

            match field(connection.get("type")).as_deref() {
                Some("802-11-wireless") => {
                    // Match on the SSID bytes: `connection.id` is a free-form name.
                    if let Some(ssid) = settings
                        .get("802-11-wireless")
                        .and_then(|wifi| wifi.get("ssid"))
                        .and_then(|v| <Vec<u8>>::try_from(v.clone()).ok())
                        .and_then(|bytes| String::from_utf8(bytes).ok())
                    {
                        profiles.wifi.insert(ssid, path);
                    }
                }
                Some("vpn" | "wireguard") => {
                    let name = field(connection.get("id")).unwrap_or_default();
                    profiles.vpn.push((name, path));
                }
                _ => {}
            }
        }

        Ok(profiles)
    }

    /// Active connection path for each active saved profile.
    async fn active_profiles(&self) -> Result<HashMap<OwnedObjectPath, OwnedObjectPath>> {
        let mut active = HashMap::new();
        for path in self.0.active_connections().await? {
            let proxy = ActiveConnectionProxy::builder(self.inner().connection())
                .path(&path)?
                .build()
                .await?;
            if let Ok(profile) = proxy.connection_().await {
                active.insert(profile, path);
            }
        }
        Ok(active)
    }

    /// Saved VPN profiles and whether each is active, sorted by name.
    pub async fn vpn_profiles(&self) -> Result<Vec<VpnProfile>> {
        let saved = self.saved_profiles().await?.vpn;
        let active = self.active_profiles().await?;
        let mut profiles: Vec<VpnProfile> = saved
            .into_iter()
            .map(|(name, path)| VpnProfile {
                active: active.contains_key(&path),
                name,
                path,
            })
            .collect();
        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(profiles)
    }

    pub async fn connect_vpn(&self, profile: &OwnedObjectPath) -> Result<()> {
        let root = ObjectPath::try_from("/")?;
        self.0.activate_connection(profile, &root, &root).await?;
        Ok(())
    }

    pub async fn disconnect_vpn(&self, profile: &OwnedObjectPath) -> Result<()> {
        let active = self.active_profiles().await?;
        let path = active
            .get(profile)
            .ok_or_else(|| anyhow::anyhow!("VPN {profile} is not active"))?;
        self.0.deactivate_connection(path).await?;
        Ok(())
    }

    /// Start connecting to a network by SSID; [`await_activation`] reports
    /// how it ends.
    ///
    /// A saved profile is reused, with `password` replacing its key when
    /// given. Otherwise a new profile is created from the strongest AP.
    pub async fn connect_by_ssid(
        &self,
        ssid: &str,
        password: Option<String>,
    ) -> Result<Activation> {
        let saved = self.saved_profiles().await?.wifi;
        let existing = saved.get(ssid).cloned();

        let aps = self.access_points(&saved).await?;
        let ap = aps
            .iter()
            .find(|a| a.ssid == ssid)
            .ok_or_else(|| anyhow::anyhow!("Access point '{}' not found", ssid))?;

        if let Some(profile) = existing {
            debug!("Activating known connection for {}", ssid);
            if let Some(password) = password {
                self.set_psk(&profile, password).await?;
            }
            let root = ObjectPath::try_from("/")?;
            let active = self
                .0
                .activate_connection(&profile, &ap.device_path, &root)
                .await?;
            return Ok(Activation {
                active,
                device: ap.device_path.clone(),
                created: None,
            });
        }

        debug!("Creating new connection for {}", ssid);
        let mut settings: HashMap<&str, HashMap<&str, Value<'_>>> = HashMap::from([
            (
                "802-11-wireless",
                HashMap::from([("ssid", Value::Array(ssid.as_bytes().into()))]),
            ),
            (
                "connection",
                HashMap::from([
                    ("id", Value::Str(ssid.into())),
                    ("type", Value::Str("802-11-wireless".into())),
                ]),
            ),
        ]);
        // NM fills in `key-mgmt` from the AP, which keeps WPA3/SAE working.
        if let Some(password) = password {
            settings.insert(
                "802-11-wireless-security",
                HashMap::from([("psk", Value::Str(password.into()))]),
            );
        }

        let (profile, active) = self
            .0
            .add_and_activate_connection(settings, &ap.device_path, &ap.path)
            .await?;
        Ok(Activation {
            active,
            device: ap.device_path.clone(),
            created: Some(profile),
        })
    }

    /// Replace the stored key of a saved WiFi profile.
    async fn set_psk(&self, profile: &OwnedObjectPath, psk: String) -> Result<()> {
        let proxy = ConnectionProxy::builder(self.inner().connection())
            .path(profile)?
            .build()
            .await?;
        let current = proxy.get_settings().await?;
        let mut settings: HashMap<&str, HashMap<&str, Value<'_>>> = current
            .iter()
            .map(|(section, values)| {
                let values = values
                    .iter()
                    .map(|(key, value)| (key.as_str(), Value::from(value.clone())))
                    .collect();
                (section.as_str(), values)
            })
            .collect();
        let security = settings.entry("802-11-wireless-security").or_default();
        security.insert("psk", Value::Str(psk.into()));
        proxy.update(settings).await?;
        Ok(())
    }

    /// Delete a saved profile.
    pub async fn delete_profile(&self, profile: &OwnedObjectPath) -> Result<()> {
        ConnectionProxy::builder(self.inner().connection())
            .path(profile)?
            .build()
            .await?
            .delete()
            .await?;
        Ok(())
    }

    /// Disconnect an active WiFi connection by SSID.
    pub async fn disconnect_by_ssid(&self, ssid: &str) -> Result<()> {
        let active = self.active_connections().await?;
        let wifi_conn = active.iter().find_map(|c| {
            if let ActiveConnectionInfo::WiFi {
                name, object_path, ..
            } = c
            {
                if name == ssid {
                    Some(object_path.clone())
                } else {
                    None
                }
            } else {
                None
            }
        });

        if let Some(path) = wifi_conn {
            self.0
                .deactivate_connection(&zbus::zvariant::ObjectPath::try_from(path.as_str())?)
                .await?;
        } else {
            anyhow::bail!("No active WiFi connection for '{}'", ssid);
        }

        Ok(())
    }

    /// Get all visible wireless access points.
    pub async fn wireless_access_points(&self) -> Result<Vec<AccessPoint>> {
        let known = self.saved_profiles().await.unwrap_or_default().wifi;
        self.access_points(&known).await
    }

    /// Visible access points, `known` when `saved` has a profile for them.
    async fn access_points(
        &self,
        saved: &HashMap<String, OwnedObjectPath>,
    ) -> Result<Vec<AccessPoint>> {
        let wireless_devices = self.wireless_devices().await?;
        let mut all_access_points = Vec::new();

        for path in wireless_devices {
            let device_proxy = DeviceProxy::builder(self.inner().connection())
                .path(&path)?
                .build()
                .await?;

            let wireless_device = WirelessDeviceProxy::builder(self.inner().connection())
                .path(&path)?
                .build()
                .await?;

            let access_points = wireless_device.get_access_points().await?;
            let state = device_proxy
                .cached_state()
                .unwrap_or_default()
                .map(DeviceState::from)
                .unwrap_or(DeviceState::Unknown);

            let mut aps = HashMap::<String, AccessPoint>::new();

            for ap in access_points {
                let ap_proxy = AccessPointProxy::builder(self.inner().connection())
                    .path(ap)?
                    .build()
                    .await?;

                let ssid = String::from_utf8_lossy(&ap_proxy.ssid().await?).into_owned();
                if ssid.is_empty() {
                    continue;
                }

                let public = ap_proxy.flags().await.unwrap_or_default() == 0;
                let strength = ap_proxy.strength().await?;

                // Keep the strongest signal for each SSID
                if let Some(existing) = aps.get(&ssid)
                    && existing.strength >= strength
                {
                    continue;
                }

                let known = saved.contains_key(&ssid);

                aps.insert(
                    ssid.clone(),
                    AccessPoint {
                        ssid,
                        strength,
                        state,
                        public,
                        working: false,
                        known,
                        path: ap_proxy.inner().path().to_owned(),
                        device_path: device_proxy.inner().path().to_owned(),
                    },
                );
            }

            all_access_points.extend(aps.into_values());
        }

        // Sort by signal strength (strongest first)
        all_access_points.sort_by_key(|b| std::cmp::Reverse(b.strength));

        Ok(all_access_points)
    }
}

/// Saved profiles split by what the shell does with them.
#[derive(Debug, Default)]
pub struct SavedProfiles {
    /// WiFi profile path by SSID.
    pub wifi: HashMap<String, OwnedObjectPath>,
    /// VPN and WireGuard profiles as (name, path).
    pub vpn: Vec<(String, OwnedObjectPath)>,
}

/// A WiFi activation NetworkManager accepted but has not finished.
#[derive(Debug)]
pub struct Activation {
    pub active: OwnedObjectPath,
    pub device: ObjectPath<'static>,
    /// The profile this attempt created, to drop again if it fails.
    pub created: Option<OwnedObjectPath>,
}

/// Longest an activation may take; NM's own supplicant timeouts are shorter.
const ACTIVATION_TIMEOUT: Duration = Duration::from_secs(60);

const ACTIVE_STATE_ACTIVATED: u32 = 2;
const ACTIVE_STATE_DEACTIVATED: u32 = 4;
const DEVICE_STATE_FAILED: u32 = 120;

/// Wait until `activation` is up, or say why it failed.
pub async fn await_activation(
    conn: &zbus::Connection,
    activation: &Activation,
) -> std::result::Result<(), WifiAttemptStatus> {
    let failed = |e: &dyn std::fmt::Display| WifiAttemptStatus::Failed(e.to_string());

    let active = ActiveConnectionProxy::builder(conn)
        .path(&activation.active)
        .map_err(|e| failed(&e))?
        .cache_properties(CacheProperties::No)
        .build()
        .await
        .map_err(|e| failed(&e))?;
    let device = DeviceProxy::builder(conn)
        .path(&activation.device)
        .map_err(|e| failed(&e))?
        .build()
        .await
        .map_err(|e| failed(&e))?;
    let active_changes = active
        .receive_state_transition()
        .await
        .map_err(|e| failed(&e))?;
    let device_changes = device
        .receive_state_transition()
        .await
        .map_err(|e| failed(&e))?;

    // Subscribed first, so a change from here on is not missed.
    match active.state().await {
        Ok(ACTIVE_STATE_ACTIVATED) => return Ok(()),
        Ok(ACTIVE_STATE_DEACTIVATED) | Err(_) => {
            return Err(WifiAttemptStatus::Failed("Connection failed".into()));
        }
        Ok(_) => {}
    }

    enum Change {
        Active { state: u32, reason: u32 },
        Device { state: u32, reason: u32 },
    }
    let active_changes = active_changes.filter_map(|signal| async move {
        let args = signal.args().ok()?;
        Some(Change::Active {
            state: args.state,
            reason: args.reason,
        })
    });
    let device_changes = device_changes.filter_map(|signal| async move {
        let args = signal.args().ok()?;
        Some(Change::Device {
            state: args.new_state,
            reason: args.reason,
        })
    });
    let mut changes = std::pin::pin!(futures_util::stream::select(active_changes, device_changes));

    let wait = async {
        // The device says why authentication failed; the active connection
        // often only reports that it went away.
        let mut device_reason = None;
        while let Some(change) = changes.next().await {
            match change {
                Change::Device {
                    state: DEVICE_STATE_FAILED,
                    reason,
                } => device_reason = Some(reason),
                Change::Device { .. } => {}
                Change::Active {
                    state: ACTIVE_STATE_ACTIVATED,
                    ..
                } => return Ok(()),
                Change::Active {
                    state: ACTIVE_STATE_DEACTIVATED,
                    reason,
                } => return Err(failure(reason, device_reason)),
                Change::Active { .. } => {}
            }
        }
        Err(WifiAttemptStatus::Failed("NetworkManager went away".into()))
    };

    tokio::time::timeout(ACTIVATION_TIMEOUT, wait)
        .await
        .unwrap_or_else(|_| Err(WifiAttemptStatus::Failed("Timed out".into())))
}

/// Map `NMActiveConnectionStateReason` and the device's
/// `NMDeviceStateReason` to what the UI should do next.
fn failure(active_reason: u32, device_reason: Option<u32>) -> WifiAttemptStatus {
    match (active_reason, device_reason) {
        // NO_SECRETS, LOGIN_FAILED
        (9 | 10, _) => WifiAttemptStatus::NeedsPassword,
        // NO_SECRETS, SUPPLICANT_DISCONNECT, SUPPLICANT_TIMEOUT: how a
        // rejected key shows up during the handshake.
        (_, Some(7 | 8 | 11)) => WifiAttemptStatus::NeedsPassword,
        (2, _) => WifiAttemptStatus::Failed("Cancelled".into()),
        (6, _) => WifiAttemptStatus::Failed("Timed out".into()),
        (reason, _) => WifiAttemptStatus::Failed(format!("Connection failed (reason {reason})")),
    }
}
