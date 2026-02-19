use std::collections::HashSet;

use crate::adapters::{FallbackAdapter, FastAdapter, LaneAdapter, LaneSnapshot, MultiLaneAdapter};
use crate::api::{LaneDetail, LaneStats};

use super::ProtocolConfig;

pub(super) fn derive_server_name(peer: &str) -> Option<String> {
    let trimmed = peer.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(addr) = trimmed.parse::<std::net::SocketAddr>() {
        return Some(addr.ip().to_string());
    }
    if let Ok(url) = reqwest::Url::parse(trimmed) {
        return url.host_str().map(ToString::to_string);
    }
    let with_scheme = format!("quic://{trimmed}");
    reqwest::Url::parse(&with_scheme)
        .ok()
        .and_then(|url| url.host_str().map(ToString::to_string))
}

pub(super) fn is_ws_url(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value.trim()) else {
        return false;
    };
    matches!(url.scheme(), "ws" | "wss")
}

pub(super) fn build_lane_details(role: &str, snapshots: Vec<LaneSnapshot>) -> Vec<LaneDetail> {
    snapshots
        .into_iter()
        .map(|snapshot| LaneDetail {
            role: role.to_string(),
            lane: snapshot.label.to_string(),
            connected: snapshot.connected,
            last_error: snapshot.health.last_error,
            last_error_code: snapshot.health.last_error_code,
            stats: LaneStats {
                outbound_queued: snapshot.health.outbound_queued,
                outbound_send_ok: snapshot.health.outbound_send_ok,
                outbound_send_err: snapshot.health.outbound_send_err,
                inbound_received: snapshot.health.inbound_received,
                inbound_dropped: snapshot.health.inbound_dropped,
                reconnect_attempts: snapshot.health.reconnect_attempts,
            },
        })
        .collect()
}

pub(super) fn build_fast_adapter(config: &ProtocolConfig) -> Result<FastAdapter, String> {
    let mut lanes: Vec<LaneAdapter> = Vec::new();

    let server_name = config.quic_server_name.clone().or_else(|| {
        config
            .fast_peers
            .first()
            .and_then(|peer| derive_server_name(peer))
    });
    if let Some(name) = server_name {
        let bind_addr = config
            .quic_bind_addr
            .parse()
            .map_err(|_| "invalid QUIC bind addr")?;
        let quic =
            crate::adapters::build_quic_adapter(bind_addr, name, config.quic_trusted_certs.clone())
                .map_err(|e| e.to_string())?;
        lanes.push(LaneAdapter::Quic(quic));
    }

    // Fast lane only uses WebSocket if QUIC is not available.
    if lanes.is_empty() && config.ws_url.is_some() {
        lanes.push(build_ws_fast(config)?);
    }

    if lanes.is_empty() {
        return Err("no fast lanes available".to_string());
    }

    Ok(MultiLaneAdapter::new(lanes))
}

pub(super) fn build_fallback_adapter(config: &ProtocolConfig) -> Result<FallbackAdapter, String> {
    let mut lanes: Vec<LaneAdapter> = Vec::new();
    let mut seen_ws: HashSet<String> = HashSet::new();

    // Fallback only uses WebSocket if it's NOT already in the fast lane.
    let has_ws_in_fast = config.quic_server_name.is_none()
        && config.fast_peers.is_empty()
        && config.ws_url.is_some();

    if !has_ws_in_fast {
        // 1. Add explicitly configured primary ws_url.
        if let Some(ws_url) = &config.ws_url {
            let ws_url = ws_url.trim();
            if !ws_url.is_empty() && is_ws_url(ws_url) {
                let ws =
                    crate::adapters::build_ws_adapter(ws_url.to_string(), config.peer_id.clone())
                        .map_err(|e| e.to_string())?;
                lanes.push(LaneAdapter::WebSocket(ws));
                seen_ws.insert(ws_url.to_string());
            }
        }

        // 2. Add WebSocket URLs from fallback peers.
        for peer in &config.fallback_peers {
            let peer = peer.trim();
            if is_ws_url(peer) && seen_ws.insert(peer.to_string()) {
                let ws =
                    crate::adapters::build_ws_adapter(peer.to_string(), config.peer_id.clone())
                        .map_err(|e| e.to_string())?;
                lanes.push(LaneAdapter::WebSocket(ws));
            }
        }
    }

    if let Some(socks) = &config.tor_socks {
        let tor = crate::adapters::build_tor_adapter(socks.clone()).map_err(|e| e.to_string())?;
        lanes.push(LaneAdapter::Tor(tor));
    }

    if lanes.is_empty() {
        lanes.push(LaneAdapter::InMemory(
            veil_transport::adapter::InMemoryAdapter::default(),
        ));
    }

    Ok(MultiLaneAdapter::new(lanes))
}

fn build_ws_fast(config: &ProtocolConfig) -> Result<LaneAdapter, String> {
    let ws_url = config
        .ws_url
        .clone()
        .ok_or_else(|| "missing WS url".to_string())?;
    let ws = crate::adapters::build_ws_adapter(ws_url, config.peer_id.clone())
        .map_err(|e| e.to_string())?;
    Ok(LaneAdapter::WebSocket(ws))
}
