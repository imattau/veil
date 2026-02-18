use std::collections::HashSet;
use std::hash::Hash;

#[cfg(feature = "ble")]
use veil_transport_ble::BlePeer;

use crate::fallback_transport::FallbackPeer;

fn strip_ascii_prefix_ci<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let head = value.get(..prefix.len())?;
    if head.eq_ignore_ascii_case(prefix) {
        Some(&value[prefix.len()..])
    } else {
        None
    }
}

pub(super) fn parse_fallback_peers(
    ws_peer: Option<String>,
    tor_peers: Vec<String>,
    #[cfg(feature = "ble")] ble_peers: Vec<String>,
) -> Vec<FallbackPeer> {
    let mut peers = Vec::new();
    if let Some(ws_peer) = ws_peer {
        let ws_peer = ws_peer.trim();
        if !ws_peer.is_empty() {
            peers.push(FallbackPeer::WebSocket(ws_peer.to_string()));
        }
    }
    for peer in tor_peers {
        let peer = peer.trim();
        if !peer.is_empty() {
            peers.push(FallbackPeer::Tor(peer.to_string()));
        }
    }
    #[cfg(feature = "ble")]
    for peer in ble_peers {
        let peer = peer.trim();
        if !peer.is_empty() {
            peers.push(FallbackPeer::Ble(BlePeer::new(peer.to_string())));
        }
    }
    peers
}

pub(super) fn parse_fallback_peer_strings(values: &[String]) -> Vec<FallbackPeer> {
    values
        .iter()
        .filter_map(|value| {
            let value = value.trim();
            if let Some(rest) = strip_ascii_prefix_ci(value, "ws:") {
                let url = rest.trim();
                if url.is_empty() {
                    None
                } else {
                    Some(FallbackPeer::WebSocket(url.to_string()))
                }
            } else if let Some(rest) = strip_ascii_prefix_ci(value, "wssrv:") {
                let addr = rest.trim();
                if addr.is_empty() {
                    None
                } else {
                    Some(FallbackPeer::WebSocketServer(addr.to_string()))
                }
            } else if let Some(rest) = strip_ascii_prefix_ci(value, "tor:") {
                let addr = rest.trim();
                if addr.is_empty() {
                    None
                } else {
                    Some(FallbackPeer::Tor(addr.to_string()))
                }
            } else if let Some(_rest) = strip_ascii_prefix_ci(value, "ble:") {
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

#[cfg(test)]
mod tests {
    use super::{parse_fallback_peer_strings, parse_fallback_peers};
    use crate::fallback_transport::FallbackPeer;

    #[test]
    fn parse_fallback_peers_trims_and_filters_empty_entries() {
        let peers = parse_fallback_peers(
            Some("  ws-peer  ".to_string()),
            vec![
                "  peer-a.onion:5000  ".to_string(),
                "".to_string(),
                "   ".to_string(),
            ],
            #[cfg(feature = "ble")]
            vec![],
        );
        assert_eq!(
            peers,
            vec![
                FallbackPeer::WebSocket("ws-peer".to_string()),
                FallbackPeer::Tor("peer-a.onion:5000".to_string()),
            ]
        );
    }

    #[test]
    fn parse_fallback_peer_strings_accepts_case_insensitive_prefixes() {
        let parsed = parse_fallback_peer_strings(&[
            "WS:relay-a".to_string(),
            "WSSRV:127.0.0.1:7000".to_string(),
            "Tor:peer.onion:5000".to_string(),
        ]);
        assert!(parsed.contains(&FallbackPeer::WebSocket("relay-a".to_string())));
        assert!(parsed.contains(&FallbackPeer::WebSocketServer("127.0.0.1:7000".to_string())));
        assert!(parsed.contains(&FallbackPeer::Tor("peer.onion:5000".to_string())));
    }

    #[test]
    fn parse_fallback_peer_strings_trims_outer_whitespace() {
        let parsed = parse_fallback_peer_strings(&[
            "  ws:relay-a  ".to_string(),
            "\ttor:peer.onion:5000\t".to_string(),
        ]);
        assert!(parsed.contains(&FallbackPeer::WebSocket("relay-a".to_string())));
        assert!(parsed.contains(&FallbackPeer::Tor("peer.onion:5000".to_string())));
    }
}
