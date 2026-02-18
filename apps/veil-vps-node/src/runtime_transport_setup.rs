use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::Connection;
use tracing::info;
#[cfg(feature = "ble-btleplug")]
use veil_transport_ble::btleplug_backend::{BtleplugLink, BtleplugLinkConfig};
#[cfg(all(feature = "ble", not(feature = "ble-btleplug")))]
use veil_transport_ble::MockBleLink;
#[cfg(feature = "ble")]
use veil_transport_ble::{BleAdapter, BleAdapterConfig};
use veil_transport_quic::{QuicAdapter, QuicAdapterConfig, QuicIdentity};
use veil_transport_tor::{TorSocksAdapter, TorSocksAdapterConfig};
use veil_transport_websocket::{
    WebSocketAdapter, WebSocketAdapterConfig, WebSocketServerAdapter, WebSocketServerAdapterConfig,
};

use crate::fallback_peers::parse_fallback_peers;
use crate::fallback_transport::{CombinedFallbackAdapter, FallbackPeer};
use crate::peer_runtime::seed_discovered_peers;
use crate::peer_store::{load_peer_list, open_peer_db};
use crate::recording_adapter::RecordingAdapter;

pub(super) struct TransportSetup {
    pub fast_adapter: RecordingAdapter<QuicAdapter>,
    pub fallback_adapter: RecordingAdapter<CombinedFallbackAdapter>,
    pub fallback_peers: Vec<FallbackPeer>,
    pub peer_db: Option<Connection>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn init_transport_setup(
    quic_bind: &str,
    identity: QuicIdentity,
    trusted: Vec<Vec<u8>>,
    ws_url: Option<String>,
    ws_listen: Option<String>,
    ws_peer_id: String,
    ws_peer: Option<String>,
    tor_socks_addr: Option<String>,
    tor_peers: Vec<String>,
    #[cfg(feature = "ble")] ble_enabled: bool,
    #[cfg(feature = "ble")] ble_peers: Vec<String>,
    #[cfg(feature = "ble")] ble_allowlist: Vec<String>,
    #[cfg(feature = "ble")] ble_mtu: usize,
    peer_db_path: &Path,
    max_dynamic_peers: usize,
) -> Result<TransportSetup, String> {
    let quic_bind_addr = quic_bind
        .parse()
        .map_err(|err| format!("fatal: invalid VEIL_VPS_QUIC_BIND: {err}"))?;

    let fast_adapter_raw = QuicAdapter::connect(QuicAdapterConfig {
        bind_addr: quic_bind_addr,
        server_name: "veil-node".to_string(),
        identity,
        trusted_peer_certs_der: trusted,
        connect_timeout: Duration::from_secs(3),
        send_timeout: Duration::from_secs(3),
        outbound_queue_capacity: 2048,
        inbound_queue_capacity: 4096,
        max_recv_bytes: 128 * 1024,
        max_payload_hint: Some(64 * 1024),
    })
    .map_err(|err| format!("fatal: quic adapter failed to start: {err}"))?;

    let ws_adapter = if let Some(url) = ws_url {
        Some(
            WebSocketAdapter::connect(WebSocketAdapterConfig {
                url,
                peer_id: ws_peer_id.clone(),
                reconnect: true,
                reconnect_initial: Duration::from_millis(250),
                reconnect_max: Duration::from_secs(10),
                outbound_queue_capacity: 1024,
                inbound_queue_capacity: 4096,
                max_payload_hint: Some(64 * 1024),
            })
            .map_err(|err| format!("fatal: websocket adapter failed to start: {err}"))?,
        )
    } else {
        None
    };

    let ws_server_adapter = if let Some(addr) = ws_listen {
        let adapter = WebSocketServerAdapter::listen(WebSocketServerAdapterConfig::new(&addr))
            .map_err(|err| format!("fatal: websocket server failed to start: {err}"))?;
        info!("websocket server listening on {addr}");
        Some(adapter)
    } else {
        None
    };

    let tor_adapter = if let Some(addr) = tor_socks_addr {
        Some(
            TorSocksAdapter::connect(TorSocksAdapterConfig {
                socks_proxy_addr: addr,
                connect_timeout: Duration::from_secs(8),
                send_timeout: Duration::from_secs(8),
                outbound_queue_capacity: 1024,
                max_payload_hint: Some(64 * 1024),
            })
            .map_err(|err| format!("fatal: tor adapter failed to start: {err}"))?,
        )
    } else {
        None
    };

    #[cfg(feature = "ble")]
    let ble_adapter = if ble_enabled {
        #[cfg(feature = "ble-btleplug")]
        let link = match BtleplugLink::spawn(BtleplugLinkConfig {
            allowlist: ble_allowlist,
            ..BtleplugLinkConfig::default()
        }) {
            Ok(link) => link,
            Err(err) => return Err(format!("ble adapter failed to start: {err:?}")),
        };
        #[cfg(all(feature = "ble", not(feature = "ble-btleplug")))]
        let link = MockBleLink::with_mtu(ble_mtu);

        Some(BleAdapter::new(
            link,
            BleAdapterConfig {
                mtu: ble_mtu,
                max_payload_hint: Some(16 * 1024),
                drop_outbound: false,
            },
        ))
    } else {
        None
    };

    let fallback_adapter = CombinedFallbackAdapter::new(
        ws_adapter,
        ws_server_adapter,
        tor_adapter,
        #[cfg(feature = "ble")]
        ble_adapter,
    );
    let ws_enabled = fallback_adapter.ws_enabled();
    let ws_server_enabled = fallback_adapter.ws_server_enabled();
    let tor_enabled = fallback_adapter.tor_enabled();
    #[cfg(feature = "ble")]
    let ble_enabled_runtime = fallback_adapter.ble_enabled();
    let fallback_peers = parse_fallback_peers(
        ws_peer,
        tor_peers,
        #[cfg(feature = "ble")]
        ble_peers,
    );

    let discovered_fast = Arc::new(Mutex::new(std::collections::HashSet::new()));
    let discovered_fallback = Arc::new(Mutex::new(std::collections::HashSet::new()));

    let fast_adapter = RecordingAdapter::new_bounded(
        fast_adapter_raw,
        Arc::clone(&discovered_fast),
        max_dynamic_peers,
    );
    let fallback_adapter = RecordingAdapter::new_bounded(
        fallback_adapter,
        Arc::clone(&discovered_fallback),
        max_dynamic_peers,
    );

    let peer_db = open_peer_db(peer_db_path);
    let discovered_seed = peer_db
        .as_ref()
        .map(|conn| load_peer_list(conn, max_dynamic_peers))
        .unwrap_or_default();
    seed_discovered_peers(
        &discovered_seed,
        &discovered_fast,
        &discovered_fallback,
        ws_enabled,
        ws_server_enabled,
        tor_enabled,
        #[cfg(feature = "ble")]
        ble_enabled_runtime,
    );

    Ok(TransportSetup {
        fast_adapter,
        fallback_adapter,
        fallback_peers,
        peer_db,
    })
}
