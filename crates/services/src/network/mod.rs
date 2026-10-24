//! Network service for NetworkManager integration.
//!
//! This module provides a reactive subscriber for monitoring and controlling
//! network connections via NetworkManager D-Bus interface.

mod dbus;
mod nm;
mod types;

pub use types::*;

use std::time::Duration;

use anyhow::Context as _;
use futures_signals::signal::Mutable;
use futures_util::StreamExt;
use futures_util::stream::select_all;
use tracing::{debug, info};
use zbus::Connection;

use self::dbus::access_point::AccessPointProxy;
use self::dbus::statistics::StatisticsProxy;
use self::nm::NetworkManager;
use crate::lifecycle::{Lifecycle, ManagedService, RunToken};

/// How long a requested scan may take before the UI stops showing it.
const SCAN_TIMEOUT: Duration = Duration::from_secs(20);

/// Event-driven network subscriber.
///
/// This subscriber monitors network state via NetworkManager D-Bus
/// and provides reactive state updates through `futures_signals`.
#[derive(Debug, Clone, Default)]
pub struct NetworkSubscriber {
    core: crate::ServiceCore<NetworkData, NetworkCommand>,
}

impl std::ops::Deref for NetworkSubscriber {
    type Target = crate::ServiceCore<NetworkData, NetworkCommand>;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl NetworkSubscriber {
    /// Create a new network subscriber. Monitoring starts with
    /// [`ManagedService::start`].
    pub fn new() -> Self {
        Self::default()
    }

    async fn execute(&self, command: NetworkCommand, token: &RunToken) -> anyhow::Result<()> {
        let conn = crate::bus::system().await?;
        let nm = NetworkManager::new(&conn).await?;

        match command {
            NetworkCommand::SetWifiEnabled(enabled) => {
                debug!("Setting WiFi enabled: {}", enabled);
                nm.set_wireless_enabled(enabled).await?;
            }
            NetworkCommand::ToggleWifi => {
                let current = nm.wireless_enabled().await?;
                debug!("Toggling WiFi: {} -> {}", current, !current);
                nm.set_wireless_enabled(!current).await?;
            }
            NetworkCommand::RequestScan => {
                debug!("Requesting WiFi scan");
                // Cleared by the `LastScan` change that ends the scan, or by
                // the timeout when no device reports one.
                self.data.lock_mut().scanning = true;
                let data = self.data.clone();
                let token = token.clone();
                tokio::spawn(async move {
                    token
                        .run_until_cancelled(tokio::time::sleep(SCAN_TIMEOUT))
                        .await;
                    data.lock_mut().scanning = false;
                });
                let wireless_devices = nm.wireless_devices().await?;
                for device in wireless_devices {
                    let wireless = dbus::device::wireless::WirelessDeviceProxy::builder(&conn)
                        .path(&device)?
                        .build()
                        .await?;
                    let _ = wireless
                        .request_scan(std::collections::HashMap::new())
                        .await;
                }
            }
            NetworkCommand::Connect { ssid, password } => {
                debug!("Connecting to: {}", ssid);
                nm.connect_by_ssid(&ssid, password).await?;
            }
            NetworkCommand::Disconnect(ssid) => {
                debug!("Disconnecting: {}", ssid);
                nm.disconnect_by_ssid(&ssid).await?;
            }
        }

        Ok(())
    }
}

impl ManagedService for NetworkSubscriber {
    fn name(&self) -> &'static str {
        "Network"
    }

    fn lifecycle(&self) -> &Lifecycle {
        &self.lifecycle
    }

    fn memory_bytes(&self) -> usize {
        let data = self.data.lock_ref();
        size_of::<NetworkData>()
            + data
                .wireless_access_points
                .iter()
                .map(|ap| size_of::<AccessPoint>() + ap.ssid.len() + ap.path.as_str().len())
                .sum::<usize>()
            + data
                .active_connections
                .iter()
                .map(|ac| size_of::<ActiveConnectionInfo>() + ac.name().len())
                .sum::<usize>()
            + data
                .network_statistics
                .iter()
                .map(|stat| size_of::<NetworkStatistics>() + stat.device.len())
                .sum::<usize>()
    }

    fn start(&self) {
        let token = self.lifecycle.begin();
        let data = self.data.clone();

        let subscriber = self.clone();
        let jobs_token = token.clone();
        self.jobs.run(token.clone(), move |command| {
            let subscriber = subscriber.clone();
            let token = jobs_token.clone();
            async move { subscriber.execute(command, &token).await }
        });

        token.supervise(self.name(), move |token| run(data.clone(), token));
    }
}

/// One run: snapshot, then follow NetworkManager until the stream ends.
async fn run(data: Mutable<NetworkData>, token: RunToken) -> anyhow::Result<()> {
    let conn = crate::bus::system()
        .await
        .context("connecting to the system bus")?;
    let fetched = fetch_network_data(&conn)
        .await
        .context("fetching initial network state")?;
    data.set_neq(fetched);
    token.active();

    loop {
        match run_listener(data.clone(), conn.clone()).await? {
            ListenerOutcome::TopologyChanged => {
                debug!("Network topology changed, rebuilding access point streams");
            }
        }
    }
}

enum ListenerOutcome {
    TopologyChanged,
}

/// Fetch current network data from NetworkManager.
async fn fetch_network_data(conn: &Connection) -> anyhow::Result<NetworkData> {
    let nm = NetworkManager::new(conn).await?;

    let wifi_enabled = nm.wireless_enabled().await?;
    let connectivity = nm.connectivity().await?.into();
    let active_connections = nm.active_connections().await?;
    let wireless_access_points = nm.wireless_access_points().await?;
    let network_statistics = nm.network_statistics().await?;

    Ok(NetworkData {
        wifi_enabled,
        connectivity,
        active_connections,
        wireless_access_points,
        network_statistics,
        scanning: false,
    })
}

/// Run the network listener event loop.
///
/// Runs under [`RunToken::run_until_cancelled`], so a stop drops it - and
/// every pending write in its streams - at the next await.
async fn run_listener(
    data: Mutable<NetworkData>,
    conn: Connection,
) -> anyhow::Result<ListenerOutcome> {
    info!("Network subscriber started");

    let nm = NetworkManager::new(&conn).await?;

    // Stream for wireless enabled changes
    let data_wifi = data.clone();
    let wireless_enabled = nm
        .receive_wireless_enabled_changed()
        .await
        .then(move |v| {
            let data = data_wifi.clone();
            async move {
                if let Ok(value) = v.get().await {
                    data.lock_mut().wifi_enabled = value;
                    debug!("WiFi enabled changed: {}", value);
                }
                false
            }
        })
        .boxed();

    // Stream for connectivity changes
    let data_conn = data.clone();
    let connectivity_changed = nm
        .receive_connectivity_changed()
        .await
        .then(move |val| {
            let data = data_conn.clone();
            async move {
                if let Ok(value) = val.get().await {
                    data.lock_mut().connectivity = value.into();
                    debug!("Connectivity changed: {:?}", value);
                }
                false
            }
        })
        .boxed();

    // Stream for active connections changes — also refresh AP list
    let data_ac = data.clone();
    let conn_ac = conn.clone();
    let active_connections = nm
        .receive_active_connections_changed()
        .await
        .then(move |_| {
            let data = data_ac.clone();
            let conn = conn_ac.clone();
            async move {
                if let Ok(nm) = NetworkManager::new(&conn).await {
                    if let Ok(connections) = nm.active_connections().await {
                        data.lock_mut().active_connections = connections;
                        debug!("Active connections changed");
                    }
                    // Also refresh access points so connected/disconnected state is current
                    if let Ok(aps) = nm.wireless_access_points().await {
                        data.lock_mut().wireless_access_points = aps;
                    }
                }
                false
            }
        })
        .boxed();

    // Set up streams for scan completion on wireless devices
    // When a scan completes, refresh the entire AP list
    let wireless_devices = nm.wireless_devices().await?;
    let mut scan_changes = Vec::new();

    for device_path in &wireless_devices {
        if let Ok(wireless) = dbus::device::wireless::WirelessDeviceProxy::builder(&conn)
            .path(device_path.clone())?
            .build()
            .await
        {
            let data_scan = data.clone();
            let conn_scan = conn.clone();
            scan_changes.push(
                wireless
                    .receive_last_scan_changed()
                    .await
                    .then(move |_| {
                        let data = data_scan.clone();
                        let conn = conn_scan.clone();
                        async move {
                            let mut topology_changed = false;
                            if let Ok(nm) = NetworkManager::new(&conn).await
                                && let Ok(aps) = nm.wireless_access_points().await
                            {
                                let mut current = data.lock_mut();
                                topology_changed = current.wireless_access_points.len()
                                    != aps.len()
                                    || current
                                        .wireless_access_points
                                        .iter()
                                        .any(|old| !aps.iter().any(|new| new.path == old.path));
                                current.wireless_access_points = aps;
                                current.scanning = false;
                                debug!("Access points refreshed after scan");
                            }
                            topology_changed
                        }
                    })
                    .boxed(),
            );
        }
    }

    // Set up streams for access point strength changes
    let wireless_aps = nm.wireless_access_points().await?;
    let mut strength_changes = Vec::with_capacity(wireless_aps.len());

    for ap in &wireless_aps {
        let ssid = ap.ssid.clone();
        let data_ap = data.clone();

        if let Ok(ap_proxy) = AccessPointProxy::builder(&conn)
            .path(ap.path.clone())?
            .build()
            .await
        {
            strength_changes.push(
                ap_proxy
                    .receive_strength_changed()
                    .await
                    .then(move |val| {
                        let ssid = ssid.clone();
                        let data = data_ap.clone();
                        async move {
                            if let Ok(value) = val.get().await {
                                let mut guard = data.lock_mut();

                                // Update in access points
                                if let Some(ap) = guard
                                    .wireless_access_points
                                    .iter_mut()
                                    .find(|ap| ap.ssid == ssid)
                                {
                                    ap.strength = value;
                                }

                                // Update in active connections
                                if let Some(ActiveConnectionInfo::WiFi { strength, .. }) = guard
                                    .active_connections
                                    .iter_mut()
                                    .find(|ac| ac.name() == ssid)
                                {
                                    *strength = value;
                                }
                            }
                            false
                        }
                    })
                    .boxed(),
            );
        }
    }

    // Set up streams for network statistics
    let devices = nm.devices().await?;
    let mut statistics_changes = Vec::new();

    for device in devices {
        let device_string = device.to_string();

        if let Ok(stats_proxy) = StatisticsProxy::builder(&conn)
            .path(device.clone())?
            .build()
            .await
        {
            // Set refresh rate
            let _ = stats_proxy.set_refresh_rate_ms(1000).await;

            // RX bytes stream
            let device_rx = device_string.clone();
            let data_rx = data.clone();
            statistics_changes.push(
                stats_proxy
                    .receive_rx_bytes_changed()
                    .await
                    .then(move |val| {
                        let device_str = device_rx.clone();
                        let data = data_rx.clone();
                        async move {
                            if let Ok(value) = val.get().await {
                                let mut guard = data.lock_mut();
                                for stat in guard.network_statistics.iter_mut() {
                                    if stat.device == device_str {
                                        stat.prev_rx = stat.rx;
                                        stat.prev_rx_time = stat.rx_time;
                                        stat.rx = value;
                                        stat.rx_time = chrono::Utc::now().timestamp();
                                    }
                                }
                            }
                            false
                        }
                    })
                    .boxed(),
            );

            // TX bytes stream
            let device_tx = device_string.clone();
            let data_tx = data.clone();
            statistics_changes.push(
                stats_proxy
                    .receive_tx_bytes_changed()
                    .await
                    .then(move |val| {
                        let device_str = device_tx.clone();
                        let data = data_tx.clone();
                        async move {
                            if let Ok(value) = val.get().await {
                                let mut guard = data.lock_mut();
                                for stat in guard.network_statistics.iter_mut() {
                                    if stat.device == device_str {
                                        stat.prev_tx = stat.tx;
                                        stat.prev_tx_time = stat.tx_time;
                                        stat.tx = value;
                                        stat.tx_time = chrono::Utc::now().timestamp();
                                    }
                                }
                            }
                            false
                        }
                    })
                    .boxed(),
            );
        }
    }

    // Combine all streams
    let mut events = select_all(vec![
        wireless_enabled,
        connectivity_changed,
        active_connections,
    ]);

    for stream in scan_changes {
        events.push(stream);
    }

    for stream in strength_changes {
        events.push(stream);
    }

    for stream in statistics_changes {
        events.push(stream);
    }

    while let Some(topology_changed) = events.next().await {
        if topology_changed {
            return Ok(ListenerOutcome::TopologyChanged);
        }
    }

    anyhow::bail!("network event streams ended")
}
