use std::collections::{HashMap, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use tracing::{debug, info, warn};
use veil_node::batch::FeedBatcher;

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
        if let Some(contact) = msg.contact {
            let mut guard = discovery_table.lock().unwrap_or_else(|e| e.into_inner());
            guard.insert(contact.peer_id.clone(), contact);
        }
        for contact in msg.contacts {
            let mut guard = discovery_table.lock().unwrap_or_else(|e| e.into_inner());
            guard.insert(contact.peer_id.clone(), contact);
        }
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

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use super::push_feed_history;

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
}
