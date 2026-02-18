use std::collections::{HashMap, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use tracing::{debug, info, warn};
use veil_node::batch::FeedBatcher;

use crate::discovery_contacts::{
    sanitize_discovery_contact, upsert_discovery_contact, DISCOVERY_GOSSIP_IMPORT_LIMIT,
    DISCOVERY_MAX_CONTACTS,
};
use crate::metrics_state::MetricsState;
use crate::nostr_bridge::BridgedItem;

const FEED_HISTORY_LIMIT: usize = 50;

pub(super) fn process_bridged_item(
    item: BridgedItem,
    metrics: &MetricsState,
    feed_history: &Arc<Mutex<VecDeque<serde_json::Value>>>,
    bridge_batcher: &mut FeedBatcher,
) {
    info!(
        "nostr bridge: relay={} event={} bytes={}",
        item.source_relay,
        item.source_event_id,
        item.payload.len()
    );
    metrics
        .nostr_bridge_events_total
        .fetch_add(1, Ordering::Relaxed);
    metrics
        .nostr_bridge_payload_bytes_total
        .fetch_add(item.payload.len() as u64, Ordering::Relaxed);

    if let Ok(bundle) = serde_json::from_slice::<veil_schema_feed::FeedBundle>(&item.payload) {
        let mut value = serde_json::to_value(bundle).unwrap_or_default();
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "source_relay".to_string(),
                serde_json::Value::String(item.source_relay.clone()),
            );
        }
        push_feed_history(feed_history, value);
        info!(
            "nostr bridge: added post to local history (relay={})",
            item.source_relay
        );
    } else {
        warn!(
            "nostr bridge: received payload that failed to parse as FeedBundle (relay={})",
            item.source_relay
        );
    }

    bridge_batcher.enqueue(item.payload);
}

pub(super) fn handle_runtime_delivered_payload(
    payload: &[u8],
    metrics: &MetricsState,
    discovery_table: &Arc<Mutex<HashMap<String, veil_android_node::ContactBundle>>>,
    feed_history: &Arc<Mutex<VecDeque<serde_json::Value>>>,
) {
    metrics.delivered.fetch_add(1, Ordering::Relaxed);
    metrics.delivered_total.fetch_add(1, Ordering::Relaxed);

    if let Ok(msg) = serde_json::from_slice::<veil_android_node::DiscoveryMessage>(payload) {
        let mut guard = discovery_table.lock().unwrap_or_else(|e| e.into_inner());
        ingest_discovery_contacts(&mut guard, msg);
    }

    if let Ok(bundle) = serde_json::from_slice::<veil_schema_feed::FeedBundle>(payload) {
        push_feed_history(
            feed_history,
            serde_json::to_value(bundle).unwrap_or_default(),
        );
        info!("runtime: delivered post added to history");
    } else if let Ok(batch) = ciborium::de::from_reader::<Vec<Vec<u8>>, _>(payload) {
        for item in batch {
            if let Ok(bundle) = serde_json::from_slice::<veil_schema_feed::FeedBundle>(&item) {
                push_feed_history(
                    feed_history,
                    serde_json::to_value(bundle).unwrap_or_default(),
                );
                info!("runtime: delivered batched post added to history");
            }
        }
    } else {
        debug!("runtime: delivered payload is not a standard FeedBundle or Batch");
    }
}

fn push_feed_history(
    feed_history: &Arc<Mutex<VecDeque<serde_json::Value>>>,
    value: serde_json::Value,
) {
    let mut guard = feed_history.lock().unwrap_or_else(|e| e.into_inner());
    if guard.len() >= FEED_HISTORY_LIMIT {
        guard.pop_front();
    }
    guard.push_back(value);
}

fn ingest_discovery_contacts(
    table: &mut HashMap<String, veil_android_node::ContactBundle>,
    msg: veil_android_node::DiscoveryMessage,
) {
    if let Some(contact) = msg.contact.and_then(sanitize_discovery_contact) {
        upsert_discovery_contact(table, contact, DISCOVERY_MAX_CONTACTS);
    }
    for contact in msg
        .contacts
        .into_iter()
        .take(DISCOVERY_GOSSIP_IMPORT_LIMIT)
        .filter_map(sanitize_discovery_contact)
    {
        upsert_discovery_contact(table, contact, DISCOVERY_MAX_CONTACTS);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};
    use std::sync::{Arc, Mutex};

    use super::{
        ingest_discovery_contacts, push_feed_history, DISCOVERY_GOSSIP_IMPORT_LIMIT,
        DISCOVERY_MAX_CONTACTS,
    };
    use veil_android_node::{ContactBundle, DiscoveryMessage};

    fn gossip_message(
        contacts: Vec<ContactBundle>,
        contact: Option<ContactBundle>,
    ) -> DiscoveryMessage {
        serde_json::from_value(serde_json::json!({
            "kind": "gossip",
            "contact": contact,
            "contacts": contacts,
            "target_peer_id": null,
            "target_pubkey": null,
            "reply_to": null,
            "ttl": 1
        }))
        .expect("gossip message should deserialize")
    }

    #[test]
    fn push_feed_history_caps_at_limit() {
        let history = Arc::new(Mutex::new(VecDeque::new()));
        for i in 0..55 {
            push_feed_history(&history, serde_json::json!({ "i": i }));
        }

        let guard = history.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(guard.len(), 50);
        assert_eq!(
            guard.front().and_then(|v| v.get("i")),
            Some(&serde_json::json!(5))
        );
        assert_eq!(
            guard.back().and_then(|v| v.get("i")),
            Some(&serde_json::json!(54))
        );
    }

    #[test]
    fn ingest_discovery_contacts_limits_single_message_import_count() {
        let mut table = HashMap::new();
        let contacts = (0..600)
            .map(|idx| ContactBundle {
                peer_id: format!("peer-{idx}"),
                ws_url: None,
                quic_addr: None,
                pubkey_hex: format!("{:064x}", idx + 1),
                rpc_url: None,
                lan_addrs: Vec::new(),
            })
            .collect::<Vec<_>>();
        let msg = gossip_message(contacts, None);

        ingest_discovery_contacts(&mut table, msg);
        assert_eq!(table.len(), DISCOVERY_GOSSIP_IMPORT_LIMIT);
    }

    #[test]
    fn ingest_discovery_contacts_sanitizes_and_caps_table_growth() {
        let mut table = HashMap::new();
        for batch in 0..12 {
            let mut contacts = Vec::new();
            for idx in 0..300 {
                let id = batch * 300 + idx;
                contacts.push(ContactBundle {
                    peer_id: format!(" peer-{id} "),
                    ws_url: Some(" ws://example.test/ws ".to_string()),
                    quic_addr: None,
                    pubkey_hex: format!("{:064X}", id + 10_000),
                    rpc_url: None,
                    lan_addrs: vec![" 10.0.0.1:9333 ".to_string(), String::new()],
                });
            }
            contacts.push(ContactBundle {
                peer_id: format!("invalid-{batch}"),
                ws_url: None,
                quic_addr: None,
                pubkey_hex: "not-hex".to_string(),
                rpc_url: None,
                lan_addrs: Vec::new(),
            });
            let msg = gossip_message(
                contacts,
                Some(ContactBundle {
                    peer_id: format!(" root-{batch} "),
                    ws_url: None,
                    quic_addr: None,
                    pubkey_hex: format!("{:064X}", batch + 1),
                    rpc_url: None,
                    lan_addrs: Vec::new(),
                }),
            );
            ingest_discovery_contacts(&mut table, msg);
        }

        assert_eq!(table.len(), DISCOVERY_MAX_CONTACTS);
        assert!(table.values().all(|contact| {
            let trimmed_peer = contact.peer_id.trim();
            trimmed_peer == contact.peer_id
                && contact.pubkey_hex.len() == 64
                && contact.pubkey_hex.chars().all(|c| c.is_ascii_hexdigit())
                && contact.pubkey_hex.chars().all(|c| !c.is_ascii_uppercase())
        }));
        assert!(!table.keys().any(|key| key.starts_with("invalid-")));
    }
}
