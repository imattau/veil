use std::net::SocketAddr;

use veil_transport::adapter::{TransportAdapter, TransportHealthSnapshot};
use veil_transport_quic::{QuicAdapter, QuicAdapterConfig, QuicAdapterError, QuicIdentity};
use veil_transport_tor::{TorSocksAdapter, TorSocksAdapterConfig, TorSocksAdapterError};
use veil_transport_websocket::{WebSocketAdapter, WebSocketAdapterConfig, WebSocketAdapterError};

pub enum LaneAdapter {
    Quic(QuicAdapter),
    WebSocket(WebSocketAdapter),
    Tor(TorSocksAdapter),
    InMemory(veil_transport::adapter::InMemoryAdapter),
}

pub enum MultiLaneError {
    Quic(QuicAdapterError),
    WebSocket(WebSocketAdapterError),
    Tor(TorSocksAdapterError),
    InMemory,
    AllFailed,
}

pub struct MultiLaneAdapter {
    lanes: Vec<LaneAdapter>,
    recv_cursor: usize,
}

pub type FastAdapter = MultiLaneAdapter;
pub type FallbackAdapter = MultiLaneAdapter;

impl MultiLaneAdapter {
    pub fn new(lanes: Vec<LaneAdapter>) -> Self {
        Self { lanes, recv_cursor: 0 }
    }

    pub fn labels(&self) -> Vec<&'static str> {
        self.lanes
            .iter()
            .map(|lane| match lane {
                LaneAdapter::Quic(_) => "quic",
                LaneAdapter::WebSocket(_) => "websocket",
                LaneAdapter::Tor(_) => "tor",
                LaneAdapter::InMemory(_) => "none",
            })
            .collect()
    }

    pub fn lane_snapshots(&self) -> Vec<LaneSnapshot> {
        self.lanes
            .iter()
            .map(|lane| match lane {
                LaneAdapter::Quic(adapter) => LaneSnapshot {
                    label: "quic",
                    connected: adapter.can_send(),
                    health: adapter.health_snapshot(),
                },
                LaneAdapter::WebSocket(adapter) => LaneSnapshot {
                    label: "websocket",
                    connected: adapter.can_send(),
                    health: adapter.health_snapshot(),
                },
                LaneAdapter::Tor(adapter) => LaneSnapshot {
                    label: "tor",
                    connected: adapter.can_send(),
                    health: adapter.health_snapshot(),
                },
                LaneAdapter::InMemory(adapter) => LaneSnapshot {
                    label: "none",
                    connected: adapter.can_send(),
                    health: adapter.health_snapshot(),
                },
            })
            .collect()
    }

    fn aggregate_health(&self) -> TransportHealthSnapshot {
        let mut snapshot = TransportHealthSnapshot::default();
        for lane in &self.lanes {
            let lane_snapshot = match lane {
                LaneAdapter::Quic(adapter) => adapter.health_snapshot(),
                LaneAdapter::WebSocket(adapter) => adapter.health_snapshot(),
                LaneAdapter::Tor(adapter) => adapter.health_snapshot(),
                LaneAdapter::InMemory(adapter) => adapter.health_snapshot(),
            };
            snapshot.outbound_queued = snapshot.outbound_queued.saturating_add(lane_snapshot.outbound_queued);
            snapshot.outbound_send_ok = snapshot.outbound_send_ok.saturating_add(lane_snapshot.outbound_send_ok);
            snapshot.outbound_send_err = snapshot.outbound_send_err.saturating_add(lane_snapshot.outbound_send_err);
            snapshot.inbound_received = snapshot.inbound_received.saturating_add(lane_snapshot.inbound_received);
            snapshot.inbound_dropped = snapshot.inbound_dropped.saturating_add(lane_snapshot.inbound_dropped);
            snapshot.reconnect_attempts = snapshot.reconnect_attempts.saturating_add(lane_snapshot.reconnect_attempts);
        }
        snapshot
    }
}

pub struct LaneSnapshot {
    pub label: &'static str,
    pub connected: bool,
    pub health: TransportHealthSnapshot,
}

impl TransportAdapter for MultiLaneAdapter {
    type Peer = String;
    type Error = MultiLaneError;

    fn send(&mut self, peer: &Self::Peer, bytes: &[u8]) -> Result<(), Self::Error> {
        let mut last_error: Option<MultiLaneError> = None;
        let mut ok = false;
        for lane in &mut self.lanes {
            let result = match lane {
                LaneAdapter::Quic(adapter) => adapter.send(peer, bytes).map_err(MultiLaneError::Quic),
                LaneAdapter::WebSocket(adapter) => adapter.send(peer, bytes).map_err(MultiLaneError::WebSocket),
                LaneAdapter::Tor(adapter) => adapter.send(peer, bytes).map_err(MultiLaneError::Tor),
                LaneAdapter::InMemory(adapter) => adapter.send(peer, bytes).map_err(|_| MultiLaneError::InMemory),
            };
            match result {
                Ok(()) => ok = true,
                Err(err) => last_error = Some(err),
            }
        }
        if ok {
            Ok(())
        } else {
            Err(last_error.unwrap_or(MultiLaneError::AllFailed))
        }
    }

    fn recv(&mut self) -> Option<(Self::Peer, Vec<u8>)> {
        if self.lanes.is_empty() {
            return None;
        }
        let start = self.recv_cursor % self.lanes.len();
        for offset in 0..self.lanes.len() {
            let idx = (start + offset) % self.lanes.len();
            let result = match &mut self.lanes[idx] {
                LaneAdapter::Quic(adapter) => adapter.recv(),
                LaneAdapter::WebSocket(adapter) => adapter.recv(),
                LaneAdapter::Tor(adapter) => adapter.recv(),
                LaneAdapter::InMemory(adapter) => adapter.recv(),
            };
            if result.is_some() {
                self.recv_cursor = idx + 1;
                return result;
            }
        }
        None
    }

    fn max_payload_hint(&self) -> Option<usize> {
        let mut hint: Option<usize> = None;
        for lane in &self.lanes {
            let lane_hint = match lane {
                LaneAdapter::Quic(adapter) => adapter.max_payload_hint(),
                LaneAdapter::WebSocket(adapter) => adapter.max_payload_hint(),
                LaneAdapter::Tor(adapter) => adapter.max_payload_hint(),
                LaneAdapter::InMemory(adapter) => adapter.max_payload_hint(),
            };
            if let Some(value) = lane_hint {
                hint = Some(hint.map(|current| current.min(value)).unwrap_or(value));
            }
        }
        hint
    }

    fn health_snapshot(&self) -> TransportHealthSnapshot {
        self.aggregate_health()
    }
}

pub fn build_quic_adapter(
    bind_addr: SocketAddr,
    server_name: String,
    trusted_certs: Vec<Vec<u8>>,
) -> Result<QuicAdapter, QuicAdapterError> {
    let identity = QuicIdentity::generate_self_signed("veil-android-node")
        .map_err(|_| QuicAdapterError::InvalidIdentity)?;
    let mut cfg = QuicAdapterConfig::new(bind_addr, server_name, identity);
    cfg.trusted_peer_certs_der = trusted_certs;
    if let Ok(raw) = std::env::var("VEIL_QUIC_CONNECT_TIMEOUT_MS") {
        if let Ok(ms) = raw.parse::<u64>() {
            cfg.connect_timeout = std::time::Duration::from_millis(ms);
        }
    }
    if let Ok(raw) = std::env::var("VEIL_QUIC_SEND_TIMEOUT_MS") {
        if let Ok(ms) = raw.parse::<u64>() {
            cfg.send_timeout = std::time::Duration::from_millis(ms);
        }
    }
    QuicAdapter::connect(cfg)
}

pub fn build_ws_adapter(url: String, peer_id: String) -> Result<WebSocketAdapter, WebSocketAdapterError> {
    WebSocketAdapter::connect(WebSocketAdapterConfig::new(url, peer_id))
}

pub fn build_tor_adapter(socks: String) -> Result<TorSocksAdapter, TorSocksAdapterError> {
    TorSocksAdapter::connect(TorSocksAdapterConfig::new(socks))
}
