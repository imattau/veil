use std::collections::HashSet;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::fallback_peers::{
    encode_fallback_peers, fallback_peer_supported, merge_peers, parse_fallback_peer_strings,
};
use crate::fallback_transport::FallbackPeer;
use crate::peer_store::save_peer_list;

pub(super) fn seed_discovered_peers(
    discovered_seed: &[String],
    discovered_fast: &Arc<Mutex<HashSet<String>>>,
    discovered_fallback: &Arc<Mutex<HashSet<FallbackPeer>>>,
    ws_enabled: bool,
    ws_server_enabled: bool,
    tor_enabled: bool,
    #[cfg(feature = "ble")] ble_enabled: bool,
) {
    {
        let mut guard = discovered_fast.lock().unwrap_or_else(|e| e.into_inner());
        for peer in discovered_seed.iter().filter(|p| is_fast_peer_seed(p)) {
            guard.insert(peer.to_string());
        }
    }

    let fallback_seed = parse_fallback_peer_strings(discovered_seed);
    {
        let mut guard = discovered_fallback
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for peer in fallback_seed {
            if fallback_peer_supported(
                &peer,
                ws_enabled,
                ws_server_enabled,
                tor_enabled,
                #[cfg(feature = "ble")]
                ble_enabled,
            ) {
                guard.insert(peer);
            }
        }
    }
}

pub(super) fn persist_peer_snapshot(
    peer_db: Option<&Connection>,
    peer_snapshot: &Arc<Mutex<Vec<String>>>,
    fast_seen: Vec<String>,
    fallback_seen: Vec<FallbackPeer>,
    max_peer_db_rows: usize,
) {
    let merged = merge_snapshot_strings(fast_seen, fallback_seen);
    if let Some(conn) = peer_db {
        save_peer_list(conn, &merged, max_peer_db_rows);
    }
    {
        let mut guard = peer_snapshot.lock().unwrap_or_else(|e| e.into_inner());
        *guard = merged;
    }
}

pub(super) fn compute_peer_lists<TFast, TFallback>(
    configured_fast: &[TFast],
    configured_fallback: &[TFallback],
    discovered_fast_snapshot: &[TFast],
    discovered_fallback_snapshot: &[TFallback],
    max_dynamic_peers: usize,
) -> (Vec<TFast>, Vec<TFallback>)
where
    TFast: Clone + Eq + Hash,
    TFallback: Clone + Eq + Hash,
{
    (
        merge_peers(configured_fast, discovered_fast_snapshot, max_dynamic_peers),
        merge_peers(
            configured_fallback,
            discovered_fallback_snapshot,
            max_dynamic_peers,
        ),
    )
}

fn is_fast_peer_seed(peer: &&String) -> bool {
    !peer.starts_with("ws:")
        && !peer.starts_with("wssrv:")
        && !peer.starts_with("tor:")
        && !peer.starts_with("ble:")
}

fn merge_snapshot_strings(fast_seen: Vec<String>, fallback_seen: Vec<FallbackPeer>) -> Vec<String> {
    let mut fast_snapshot = fast_seen;
    fast_snapshot.sort();
    let mut fallback_snapshot = encode_fallback_peers(&fallback_seen);
    fallback_snapshot.sort();
    let mut merged = fast_snapshot;
    merged.extend(fallback_snapshot);
    merged.sort();
    merged.dedup();
    merged
}

#[cfg(test)]
mod tests {
    use super::{compute_peer_lists, is_fast_peer_seed, merge_snapshot_strings};
    use crate::fallback_transport::FallbackPeer;

    #[test]
    fn is_fast_peer_seed_filters_transport_prefixes() {
        assert!(is_fast_peer_seed(&&"peer-a".to_string()));
        assert!(!is_fast_peer_seed(&&"ws:relay-a".to_string()));
        assert!(!is_fast_peer_seed(&&"wssrv:127.0.0.1:8080".to_string()));
        assert!(!is_fast_peer_seed(&&"tor:peer.onion:5000".to_string()));
        assert!(!is_fast_peer_seed(&&"ble:AA:BB:CC:DD:EE:FF".to_string()));
    }

    #[test]
    fn merge_snapshot_strings_sorts_and_deduplicates() {
        let merged = merge_snapshot_strings(
            vec!["b".to_string(), "a".to_string(), "a".to_string()],
            vec![
                FallbackPeer::WebSocket("relay-a".to_string()),
                FallbackPeer::Tor("peer.onion:5000".to_string()),
                FallbackPeer::WebSocket("relay-a".to_string()),
            ],
        );
        assert_eq!(
            merged,
            vec![
                "a".to_string(),
                "b".to_string(),
                "tor:peer.onion:5000".to_string(),
                "ws:relay-a".to_string(),
            ]
        );
    }

    #[test]
    fn compute_peer_lists_uses_merge_order_and_cap() {
        let (fast, fallback) = compute_peer_lists(
            &["a".to_string(), "b".to_string()],
            &["fa".to_string()],
            &["b".to_string(), "c".to_string(), "d".to_string()],
            &["fa".to_string(), "fb".to_string(), "fc".to_string()],
            3,
        );
        assert_eq!(
            fast,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert_eq!(
            fallback,
            vec!["fa".to_string(), "fb".to_string(), "fc".to_string()]
        );
    }
}
