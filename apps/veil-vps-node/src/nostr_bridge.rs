use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use veil_core::ObjectRoot;
use veil_schema_feed::{BundleMeta, FeedBundle, PostBundle};

use tracing::error;

#[derive(Debug, Clone)]
pub struct NostrBridgeConfig {
    pub relays: Vec<String>,
    pub channel_id: String,
    pub namespace: u16,
    pub since: Duration,
    pub state_path: Option<PathBuf>,
    pub max_seen_ids: usize,
    pub persist_every_updates: usize,
}

#[derive(Debug, Clone)]
pub struct BridgedItem {
    pub payload: Vec<u8>,
    pub source_relay: String,
    pub source_event_id: String,
}

#[derive(Debug, Deserialize)]
struct NostrEvent {
    id: String,
    pubkey: String,
    kind: u64,
    created_at: u64,
    content: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct NostrBridgeStateDisk {
    relay_last_created_at: HashMap<String, u64>,
    seen_event_ids: Vec<String>,
}

#[derive(Debug)]
struct NostrBridgeState {
    relay_last_created_at: HashMap<String, u64>,
    seen_set: HashSet<String>,
    seen_order: VecDeque<String>,
    state_path: Option<PathBuf>,
    max_seen_ids: usize,
    persist_every_updates: usize,
    dirty_updates: usize,
}

impl NostrBridgeState {
    fn load(
        state_path: Option<PathBuf>,
        max_seen_ids: usize,
        persist_every_updates: usize,
    ) -> Self {
        let mut out = Self {
            relay_last_created_at: HashMap::new(),
            seen_set: HashSet::new(),
            seen_order: VecDeque::new(),
            state_path,
            max_seen_ids: max_seen_ids.max(1),
            persist_every_updates: persist_every_updates.max(1),
            dirty_updates: 0,
        };
        let Some(path) = out.state_path.clone() else {
            return out;
        };
        let Ok(bytes) = fs::read(path) else {
            return out;
        };
        let Ok(disk) = serde_json::from_slice::<NostrBridgeStateDisk>(&bytes) else {
            return out;
        };
        out.relay_last_created_at = disk.relay_last_created_at;
        for id in disk.seen_event_ids {
            if out.seen_set.insert(id.clone()) {
                out.seen_order.push_back(id);
            }
        }
        while out.seen_order.len() > out.max_seen_ids {
            if let Some(old) = out.seen_order.pop_front() {
                out.seen_set.remove(&old);
            }
        }
        out
    }

    fn since_for_relay(&self, relay: &str, fallback_since_secs: u64) -> u64 {
        let baseline = current_unix().saturating_sub(fallback_since_secs);
        let checkpoint = self.relay_last_created_at.get(relay).copied().unwrap_or(0);
        baseline.max(checkpoint.saturating_sub(60))
    }

    fn should_accept_event(&mut self, relay: &str, event_id: &str, created_at: u64) -> bool {
        if let Some(last) = self.relay_last_created_at.get_mut(relay) {
            *last = (*last).max(created_at);
        } else {
            self.relay_last_created_at
                .insert(relay.to_string(), created_at);
        }
        if self.seen_set.contains(event_id) {
            return false;
        }
        self.seen_set.insert(event_id.to_string());
        self.seen_order.push_back(event_id.to_string());
        while self.seen_order.len() > self.max_seen_ids {
            if let Some(old) = self.seen_order.pop_front() {
                self.seen_set.remove(&old);
            }
        }
        self.dirty_updates = self.dirty_updates.saturating_add(1);
        true
    }

    fn persist_if_due(&mut self, force: bool) {
        if !force && self.dirty_updates < self.persist_every_updates {
            return;
        }
        let Some(path) = self.state_path.clone() else {
            self.dirty_updates = 0;
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let disk = NostrBridgeStateDisk {
            relay_last_created_at: self.relay_last_created_at.clone(),
            seen_event_ids: self.seen_order.iter().cloned().collect(),
        };
        let Ok(encoded) = serde_json::to_vec_pretty(&disk) else {
            return;
        };
        let tmp = path.with_extension("tmp");
        if fs::write(&tmp, encoded).is_ok() && fs::rename(&tmp, &path).is_ok() {
            self.dirty_updates = 0;
        }
    }
}

pub fn start_nostr_bridge(config: NostrBridgeConfig) -> tokio::sync::mpsc::Receiver<BridgedItem> {
    let (tx, rx) = tokio::sync::mpsc::channel::<BridgedItem>(512);
    if config.relays.is_empty() {
        return rx;
    }
    let _ = rustls::crypto::ring::default_provider().install_default();
    let relays = config.relays.clone();
    let state = Arc::new(Mutex::new(NostrBridgeState::load(
        config.state_path.clone(),
        config.max_seen_ids,
        config.persist_every_updates,
    )));
    for relay in relays {
        let tx = tx.clone();
        let cfg = config.clone();
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            relay_loop(relay, cfg, tx, state).await;
        });
    }
    rx
}

fn reconnect_backoff(attempts: u32, base_ms: u64, max_ms: u64) -> Duration {
    let exponent = attempts.saturating_sub(1).min(10);
    let factor = 1u64.checked_shl(exponent).unwrap_or(u64::MAX);
    let raw = base_ms.saturating_mul(factor);
    Duration::from_millis(raw.min(max_ms).max(base_ms))
}

async fn relay_loop(
    relay: String,
    config: NostrBridgeConfig,
    tx: tokio::sync::mpsc::Sender<BridgedItem>,
    state: Arc<Mutex<NostrBridgeState>>,
) {
    let mut connect_attempts: u32 = 0;
    loop {
        connect_attempts = connect_attempts.saturating_add(1);
        let connect = connect_async(relay.as_str()).await;
        let (mut ws, _) = match connect {
            Ok(ok) => {
                connect_attempts = 0;
                ok
            }
            Err(err) => {
                error!("nostr bridge connect failed {relay}: {err}");
                tokio::time::sleep(reconnect_backoff(connect_attempts, 2_000, 120_000)).await;
                continue;
            }
        };

        let since = {
            let guard = state.lock().unwrap_or_else(|e| e.into_inner());
            guard.since_for_relay(&relay, config.since.as_secs())
        };
        let sub_id = "veil-bridge";
        let req = json!(["REQ", sub_id, { "kinds": [1], "since": since }]).to_string();
        if ws.send(Message::Text(req)).await.is_err() {
            tokio::time::sleep(reconnect_backoff(connect_attempts, 2_000, 120_000)).await;
            continue;
        }

        while let Some(frame) = ws.next().await {
            let msg = match frame {
                Ok(msg) => msg,
                Err(err) => {
                    error!("nostr bridge read failed {relay}: {err}");
                    break;
                }
            };
            let Message::Text(text) = msg else {
                continue;
            };
            let Some(event) = parse_nostr_event_message(&text) else {
                continue;
            };
            let Some(payload) = map_event_to_payload(&event, &config.channel_id, config.namespace)
            else {
                continue;
            };
            {
                let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
                if !guard.should_accept_event(&relay, &event.id, event.created_at) {
                    continue;
                }
                guard.persist_if_due(false);
            }
            if tx
                .send(BridgedItem {
                    payload,
                    source_relay: relay.clone(),
                    source_event_id: event.id,
                })
                .await
                .is_err()
            {
                let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
                guard.persist_if_due(true);
                return;
            }
        }
        {
            let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
            guard.persist_if_due(true);
        }
        tokio::time::sleep(reconnect_backoff(connect_attempts, 2_000, 120_000)).await;
    }
}

fn current_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn parse_nostr_event_message(input: &str) -> Option<NostrEvent> {
    let value: serde_json::Value = serde_json::from_str(input).ok()?;
    let array = value.as_array()?;
    if array.len() < 3 {
        return None;
    }
    if array.first()?.as_str()? != "EVENT" {
        return None;
    }
    serde_json::from_value(array[2].clone()).ok()
}

fn map_event_to_payload(event: &NostrEvent, channel_id: &str, _namespace: u16) -> Option<Vec<u8>> {
    if event.kind != 1 {
        return None;
    }
    let text = event.content.trim();
    if text.is_empty() {
        return None;
    }
    let bundle = FeedBundle::Post(PostBundle {
        meta: BundleMeta {
            version: 1,
            created_at: event.created_at,
        },
        channel_id: channel_id.to_string(),
        author_pubkey_hex: event.pubkey.clone(),
        text: text.to_string(),
        media_roots: Vec::<ObjectRoot>::new(),
        reply_to_root: None,
    });
    serde_json::to_vec(&bundle).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        map_event_to_payload, parse_nostr_event_message, reconnect_backoff, NostrBridgeState,
    };
    use veil_schema_feed::FeedBundle;

    #[test]
    fn reconnect_backoff_increases_exponentially_and_caps() {
        use std::time::Duration;
        assert_eq!(
            reconnect_backoff(1, 2_000, 120_000),
            Duration::from_millis(2_000)
        );
        assert_eq!(
            reconnect_backoff(2, 2_000, 120_000),
            Duration::from_millis(4_000)
        );
        assert_eq!(
            reconnect_backoff(3, 2_000, 120_000),
            Duration::from_millis(8_000)
        );
        assert_eq!(
            reconnect_backoff(4, 2_000, 120_000),
            Duration::from_millis(16_000)
        );
        assert_eq!(
            reconnect_backoff(10, 2_000, 120_000),
            Duration::from_millis(120_000)
        );
        assert_eq!(
            reconnect_backoff(20, 2_000, 120_000),
            Duration::from_millis(120_000)
        );
    }

    #[test]
    fn parses_event_message() {
        let msg = r#"["EVENT","sub-a",{"id":"abc","pubkey":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","kind":1,"created_at":1700000000,"content":"hello veil"}]"#;
        let ev = parse_nostr_event_message(msg).expect("event should parse");
        assert_eq!(ev.id, "abc");
        assert_eq!(ev.kind, 1);
    }

    #[test]
    fn maps_kind1_to_feed_post_payload() {
        let msg = r#"["EVENT","sub-a",{"id":"abc","pubkey":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","kind":1,"created_at":1700000000,"content":"hello veil"}]"#;
        let ev = parse_nostr_event_message(msg).expect("event should parse");
        let payload =
            map_event_to_payload(&ev, "nostr", 32).expect("payload should map for kind1 event");
        let bundle: FeedBundle = serde_json::from_slice(&payload).expect("bundle should decode");
        match bundle {
            FeedBundle::Post(post) => {
                assert_eq!(post.text, "hello veil");
                assert_eq!(post.channel_id, "nostr");
            }
            _ => panic!("expected post bundle"),
        }
    }

    #[test]
    fn state_dedup_and_capacity_trim_work() {
        let mut state = NostrBridgeState::load(None, 2, 1);
        assert!(state.should_accept_event("r", "a", 10));
        assert!(state.should_accept_event("r", "b", 11));
        assert!(!state.should_accept_event("r", "a", 12));
        assert!(state.should_accept_event("r", "c", 13));
        assert_eq!(state.seen_order.len(), 2);
        assert!(!state.seen_set.contains("a"));
        assert_eq!(state.relay_last_created_at.get("r").copied(), Some(13));
    }

    #[tokio::test]
    #[ignore = "live network test; run explicitly"]
    async fn live_relays_emit_bridge_item() {
        let relays = vec![
            "wss://relay.damus.io".to_string(),
            "wss://nos.lol".to_string(),
            "wss://relay.snort.social".to_string(),
        ];
        let mut rx = super::start_nostr_bridge(super::NostrBridgeConfig {
            relays: relays.clone(),
            channel_id: "nostr-bridge".to_string(),
            namespace: 32,
            since: std::time::Duration::from_secs(3_600),
            state_path: None,
            max_seen_ids: 1_000,
            persist_every_updates: 1,
        });
        let item = tokio::time::timeout(std::time::Duration::from_secs(30), rx.recv())
            .await
            .expect("timeout waiting for bridge item")
            .expect("expected at least one bridged nostr event from live relays");
        assert!(
            relays.contains(&item.source_relay),
            "unexpected relay source: {}",
            item.source_relay
        );
        assert_eq!(item.source_event_id.len(), 64);
        let bundle: FeedBundle =
            serde_json::from_slice(&item.payload).expect("bridge payload should decode");
        match bundle {
            FeedBundle::Post(post) => {
                assert!(!post.text.trim().is_empty());
                assert_eq!(post.channel_id, "nostr-bridge");
            }
            _ => panic!("expected post bundle"),
        }
    }

    #[tokio::test]
    #[ignore = "live network test; run explicitly"]
    async fn live_relays_restart_uses_persisted_dedupe_state() {
        let relays = vec![
            "wss://relay.damus.io".to_string(),
            "wss://nos.lol".to_string(),
            "wss://relay.snort.social".to_string(),
        ];
        let temp = tempfile::tempdir().expect("tempdir");
        let state_path = temp.path().join("nostr-bridge-state.json");

        let first_id = {
            let mut rx = super::start_nostr_bridge(super::NostrBridgeConfig {
                relays: relays.clone(),
                channel_id: "nostr-bridge".to_string(),
                namespace: 32,
                since: std::time::Duration::from_secs(3_600),
                state_path: Some(state_path.clone()),
                max_seen_ids: 2_000,
                persist_every_updates: 1,
            });
            let item = tokio::time::timeout(std::time::Duration::from_secs(30), rx.recv())
                .await
                .expect("timeout")
                .expect("expected bridged event on first run");
            item.source_event_id
        };

        let mut persisted_contains_first = false;
        for _ in 0..20 {
            if let Ok(bytes) = std::fs::read(&state_path) {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    let ids = value
                        .get("seen_event_ids")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    if ids.iter().any(|v| v.as_str() == Some(first_id.as_str())) {
                        persisted_contains_first = true;
                        break;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
        assert!(
            persisted_contains_first,
            "expected first bridged event id to be persisted"
        );

        let mut rx2 = super::start_nostr_bridge(super::NostrBridgeConfig {
            relays: relays.clone(),
            channel_id: "nostr-bridge".to_string(),
            namespace: 32,
            since: std::time::Duration::from_secs(3_600),
            state_path: Some(state_path.clone()),
            max_seen_ids: 2_000,
            persist_every_updates: 1,
        });
        let item2 = tokio::time::timeout(std::time::Duration::from_secs(30), rx2.recv())
            .await
            .expect("timeout")
            .expect("expected bridged event on second run");
        assert_ne!(
            item2.source_event_id, first_id,
            "persisted dedupe should prevent replaying same first event id on restart"
        );
    }
}
