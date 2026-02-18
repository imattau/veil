use std::collections::HashSet;
use std::hash::Hash;

#[cfg(feature = "ble")]
use veil_transport_ble::BlePeer;

use crate::fallback_transport::FallbackPeer;

pub(super) fn parse_fallback_peers(
    ws_peer: Option<String>,
    tor_peers: Vec<String>,
    #[cfg(feature = "ble")] ble_peers: Vec<String>,
) -> Vec<FallbackPeer> {
    let mut peers = Vec::new();
    if let Some(ws_peer) = ws_peer {
        peers.push(FallbackPeer::WebSocket(ws_peer));
    }
    for peer in tor_peers {
        peers.push(FallbackPeer::Tor(peer));
    }
    #[cfg(feature = "ble")]
    for peer in ble_peers {
        peers.push(FallbackPeer::Ble(BlePeer::new(peer)));
    }
    peers
}

pub(super) fn parse_fallback_peer_strings(values: &[String]) -> Vec<FallbackPeer> {
    values
        .iter()
        .filter_map(|value| {
            if let Some(rest) = value.strip_prefix("ws:") {
                let url = rest.trim();
                if url.is_empty() {
                    None
                } else {
                    Some(FallbackPeer::WebSocket(url.to_string()))
                }
            } else if let Some(rest) = value.strip_prefix("wssrv:") {
                let addr = rest.trim();
                if addr.is_empty() {
                    None
                } else {
                    Some(FallbackPeer::WebSocketServer(addr.to_string()))
                }
            } else if let Some(rest) = value.strip_prefix("tor:") {
                let addr = rest.trim();
                if addr.is_empty() {
                    None
                } else {
                    Some(FallbackPeer::Tor(addr.to_string()))
                }
            } else if let Some(_rest) = value.strip_prefix("ble:") {
                #[cfg(feature = "ble")]
                {
                    let addr = _rest.trim();
                    if addr.is_empty() {
                        None
                    } else {
                        Some(FallbackPeer::Ble(BlePeer::new(addr.to_string())))
                    }
                }
                #[cfg(not(feature = "ble"))]
                {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn fallback_peer_supported(
    peer: &FallbackPeer,
    ws_enabled: bool,
    ws_server_enabled: bool,
    tor_enabled: bool,
    #[cfg(feature = "ble")] ble_enabled: bool,
) -> bool {
    match peer {
        FallbackPeer::WebSocket(_) => ws_enabled,
        FallbackPeer::WebSocketServer(_) => ws_server_enabled,
        FallbackPeer::Tor(_) => tor_enabled,
        #[cfg(feature = "ble")]
        FallbackPeer::Ble(_) => ble_enabled,
    }
}

pub(super) fn encode_fallback_peers(peers: &[FallbackPeer]) -> Vec<String> {
    peers.iter().map(|peer| peer.to_string()).collect()
}

pub(super) fn merge_peers<T: Clone + Eq + Hash>(
    configured: &[T],
    discovered: &[T],
    max_total: usize,
) -> Vec<T> {
    let mut seen = HashSet::with_capacity(configured.len() + discovered.len());
    let mut out = Vec::new();
    for peer in configured {
        if seen.insert(peer) {
            out.push(peer.clone());
        }
    }
    for peer in discovered {
        if out.len() >= max_total {
            break;
        }
        if seen.insert(peer) {
            out.push(peer.clone());
        }
    }
    out
}
