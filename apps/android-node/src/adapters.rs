use std::net::SocketAddr;

use veil_transport::adapter::{TransportAdapter, TransportHealthSnapshot};
use veil_transport_quic::{QuicAdapter, QuicAdapterConfig, QuicAdapterError, QuicIdentity};
use veil_transport_tor::{TorSocksAdapter, TorSocksAdapterConfig, TorSocksAdapterError};
use veil_transport_websocket::{
    WebSocketAdapter, WebSocketAdapterConfig, WebSocketAdapterError,
};

pub enum FastAdapter {
    Quic(QuicAdapter),
    WebSocket(WebSocketAdapter),
}

pub enum FallbackAdapter {
    WebSocket(WebSocketAdapter),
    Tor(TorSocksAdapter),
    InMemory(veil_transport::adapter::InMemoryAdapter),
}

pub enum FastAdapterError {
    Quic(QuicAdapterError),
    WebSocket(WebSocketAdapterError),
}

pub enum FallbackAdapterError {
    WebSocket(WebSocketAdapterError),
    Tor(TorSocksAdapterError),
    InMemory,
}

impl TransportAdapter for FastAdapter {
    type Peer = String;
    type Error = FastAdapterError;

    fn send(&mut self, peer: &Self::Peer, bytes: &[u8]) -> Result<(), Self::Error> {
        match self {
            FastAdapter::Quic(adapter) => adapter.send(peer, bytes).map_err(FastAdapterError::Quic),
            FastAdapter::WebSocket(adapter) => adapter
                .send(peer, bytes)
                .map_err(FastAdapterError::WebSocket),
        }
    }

    fn recv(&mut self) -> Option<(Self::Peer, Vec<u8>)> {
        match self {
            FastAdapter::Quic(adapter) => adapter.recv(),
            FastAdapter::WebSocket(adapter) => adapter.recv(),
        }
    }

    fn max_payload_hint(&self) -> Option<usize> {
        match self {
            FastAdapter::Quic(adapter) => adapter.max_payload_hint(),
            FastAdapter::WebSocket(adapter) => adapter.max_payload_hint(),
        }
    }

    fn health_snapshot(&self) -> TransportHealthSnapshot {
        match self {
            FastAdapter::Quic(adapter) => adapter.health_snapshot(),
            FastAdapter::WebSocket(adapter) => adapter.health_snapshot(),
        }
    }
}

impl TransportAdapter for FallbackAdapter {
    type Peer = String;
    type Error = FallbackAdapterError;

    fn send(&mut self, peer: &Self::Peer, bytes: &[u8]) -> Result<(), Self::Error> {
        match self {
            FallbackAdapter::WebSocket(adapter) => adapter
                .send(peer, bytes)
                .map_err(FallbackAdapterError::WebSocket),
            FallbackAdapter::Tor(adapter) => adapter
                .send(peer, bytes)
                .map_err(FallbackAdapterError::Tor),
            FallbackAdapter::InMemory(adapter) => adapter
                .send(peer, bytes)
                .map_err(|_| FallbackAdapterError::InMemory),
        }
    }

    fn recv(&mut self) -> Option<(Self::Peer, Vec<u8>)> {
        match self {
            FallbackAdapter::WebSocket(adapter) => adapter.recv(),
            FallbackAdapter::Tor(adapter) => adapter.recv(),
            FallbackAdapter::InMemory(adapter) => adapter.recv(),
        }
    }

    fn max_payload_hint(&self) -> Option<usize> {
        match self {
            FallbackAdapter::WebSocket(adapter) => adapter.max_payload_hint(),
            FallbackAdapter::Tor(adapter) => adapter.max_payload_hint(),
            FallbackAdapter::InMemory(adapter) => adapter.max_payload_hint(),
        }
    }

    fn health_snapshot(&self) -> TransportHealthSnapshot {
        match self {
            FallbackAdapter::WebSocket(adapter) => adapter.health_snapshot(),
            FallbackAdapter::Tor(adapter) => adapter.health_snapshot(),
            FallbackAdapter::InMemory(adapter) => adapter.health_snapshot(),
        }
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
    QuicAdapter::connect(cfg)
}

pub fn build_ws_adapter(url: String, peer_id: String) -> Result<WebSocketAdapter, WebSocketAdapterError> {
    WebSocketAdapter::connect(WebSocketAdapterConfig::new(url, peer_id))
}

pub fn build_tor_adapter(socks: String) -> Result<TorSocksAdapter, TorSocksAdapterError> {
    TorSocksAdapter::connect(TorSocksAdapterConfig::new(socks))
}
