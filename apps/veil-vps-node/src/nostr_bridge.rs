use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use veil_core::ObjectRoot;
use veil_crypto::signing::{NostrVerifier, Verifier};
use veil_schema_feed::{BundleMeta, FeedBundle, PostBundle};

use tracing::error;

const NOSTR_EVENT_MAX_CONTENT_BYTES: usize = 16 * 1024;
const NOSTR_EVENT_MAX_FUTURE_SKEW_SECS: u64 = 15 * 60;
const NOSTR_MAX_EVENT_FRAME_BYTES: usize = 256 * 1024;
const NOSTR_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const NOSTR_SUBSCRIBE_SEND_TIMEOUT: Duration = Duration::from_secs(8);
const NOSTR_BRIDGE_QUEUE_RESERVE_TIMEOUT: Duration = Duration::from_secs(2);
const NOSTR_READ_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

type RelayWsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

#[derive(Debug)]
enum ConnectRelayError {
    Timeout,
    Connect(tokio_tungstenite::tungstenite::Error),
}

#[derive(Debug)]
enum NextFrameError {
    Timeout,
    Closed,
    Read(tokio_tungstenite::tungstenite::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BridgeEnqueueError {
    Timeout,
    Closed,
}

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
    #[serde(default = "empty_nostr_tags")]
    tags: serde_json::Value,
    content: String,
    #[serde(default)]
    sig: Option<String>,
}

fn empty_nostr_tags() -> serde_json::Value {
    serde_json::Value::Array(Vec::new())
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
        // Protect per-relay checkpointing from future-timestamp poisoning that can
        // otherwise suppress valid events after reconnect.
        let max_allowed = current_unix().saturating_add(NOSTR_EVENT_MAX_FUTURE_SKEW_SECS);
        if created_at > max_allowed {
            return false;
        }
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
        tracing::info!("nostr bridge: connecting to {relay} (attempt {connect_attempts})");
        let (mut ws, _) = match connect_with_timeout(relay.as_str(), NOSTR_CONNECT_TIMEOUT).await {
            Ok(ok) => {
                connect_attempts = 0;
                ok
            }
            Err(ConnectRelayError::Connect(err)) => {
                error!("nostr bridge connect failed {relay}: {err}");
                tokio::time::sleep(reconnect_backoff(connect_attempts, 2_000, 120_000)).await;
                continue;
            }
            Err(ConnectRelayError::Timeout) => {
                error!("nostr bridge connect timeout {relay}");
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
        let subscribe_send =
            tokio::time::timeout(NOSTR_SUBSCRIBE_SEND_TIMEOUT, ws.send(Message::Text(req))).await;
        if !matches!(subscribe_send, Ok(Ok(()))) {
            if let Ok(Err(err)) = subscribe_send {
                error!("nostr bridge subscribe send failed {relay}: {err}");
            } else {
                error!("nostr bridge subscribe send timeout {relay}");
            }
            tokio::time::sleep(reconnect_backoff(connect_attempts, 2_000, 120_000)).await;
            continue;
        }

        loop {
            let text = match next_text_frame_with_timeout(&mut ws, NOSTR_READ_IDLE_TIMEOUT).await {
                Ok(Some(text)) => text,
                Ok(None) => continue,
                Err(NextFrameError::Timeout) => {
                    error!("nostr bridge read timeout {relay}");
                    break;
                }
                Err(NextFrameError::Closed) => break,
                Err(NextFrameError::Read(err)) => {
                    error!("nostr bridge read failed {relay}: {err}");
                    break;
                }
            };
            let Some(event) = parse_nostr_event_message(&text) else {
                continue;
            };
            // Fast-path duplicate suppression before signature verification to reduce
            // CPU burn during replay storms.
            if is_event_already_seen(&state, &event) {
                if relay_checkpoint_needs_update(&state, &relay, event.created_at)
                    && verify_nostr_event_authenticity(&event)
                {
                    note_seen_duplicate_checkpoint(&state, &relay, &event);
                }
                continue;
            }
            if !verify_nostr_event_authenticity(&event) {
                continue;
            }
            let Some(payload) = map_event_to_payload(&event, &config.channel_id, config.namespace)
            else {
                continue;
            };
            match reserve_and_send_bridged_item(
                &tx,
                &state,
                &relay,
                &event,
                payload,
                NOSTR_BRIDGE_QUEUE_RESERVE_TIMEOUT,
            )
            .await
            {
                Ok(true) => {}
                Ok(false) => continue,
                Err(BridgeEnqueueError::Timeout) => {
                    tracing::warn!(
                        "nostr bridge queue saturated; dropping event relay={} event={}",
                        relay,
                        event.id
                    );
                    continue;
                }
                Err(BridgeEnqueueError::Closed) => {
                    let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
                    guard.persist_if_due(true);
                    return;
                }
            }
        }
        {
            let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
            guard.persist_if_due(true);
        }
        tokio::time::sleep(reconnect_backoff(connect_attempts, 2_000, 120_000)).await;
    }
}

fn note_bridge_event_seen(
    state: &Arc<Mutex<NostrBridgeState>>,
    relay: &str,
    event: &NostrEvent,
) -> bool {
    if event.kind != 1 {
        return false;
    }
    // Canonicalize hex IDs to avoid case-variant dedupe bypass.
    let Some(canonical_id) = canonical_hex_64(&event.id) else {
        return false;
    };
    let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
    let accepted = guard.should_accept_event(relay, &canonical_id, event.created_at);
    if accepted {
        guard.persist_if_due(false);
    }
    accepted
}

async fn connect_with_timeout(
    relay: &str,
    timeout: Duration,
) -> Result<
    (
        RelayWsStream,
        tokio_tungstenite::tungstenite::handshake::client::Response,
    ),
    ConnectRelayError,
> {
    match tokio::time::timeout(timeout, connect_async(relay)).await {
        Ok(Ok(pair)) => Ok(pair),
        Ok(Err(err)) => Err(ConnectRelayError::Connect(err)),
        Err(_) => Err(ConnectRelayError::Timeout),
    }
}

fn current_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

async fn next_text_frame_with_timeout(
    ws: &mut RelayWsStream,
    timeout: Duration,
) -> Result<Option<String>, NextFrameError> {
    match tokio::time::timeout(timeout, ws.next()).await {
        Err(_) => Err(NextFrameError::Timeout),
        Ok(None) => Err(NextFrameError::Closed),
        Ok(Some(Err(err))) => match err {
            tokio_tungstenite::tungstenite::Error::ConnectionClosed
            | tokio_tungstenite::tungstenite::Error::AlreadyClosed => Err(NextFrameError::Closed),
            other => Err(NextFrameError::Read(other)),
        },
        Ok(Some(Ok(Message::Text(text)))) => Ok(Some(text.to_string())),
        Ok(Some(Ok(Message::Close(_)))) => Err(NextFrameError::Closed),
        Ok(Some(Ok(_))) => Ok(None),
    }
}

async fn reserve_bridge_slot<'a>(
    tx: &'a tokio::sync::mpsc::Sender<BridgedItem>,
    timeout: Duration,
) -> Result<tokio::sync::mpsc::Permit<'a, BridgedItem>, BridgeEnqueueError> {
    match tokio::time::timeout(timeout, tx.reserve()).await {
        Ok(Ok(permit)) => Ok(permit),
        Ok(Err(_)) => Err(BridgeEnqueueError::Closed),
        Err(_) => Err(BridgeEnqueueError::Timeout),
    }
}

async fn reserve_and_send_bridged_item(
    tx: &tokio::sync::mpsc::Sender<BridgedItem>,
    state: &Arc<Mutex<NostrBridgeState>>,
    relay: &str,
    event: &NostrEvent,
    payload: Vec<u8>,
    timeout: Duration,
) -> Result<bool, BridgeEnqueueError> {
    let permit = reserve_bridge_slot(tx, timeout).await?;
    if !note_bridge_event_seen(state, relay, event) {
        return Ok(false);
    }
    let source_event_id = canonical_hex_64(&event.id).unwrap_or_else(|| event.id.to_lowercase());
    permit.send(BridgedItem {
        payload,
        source_relay: relay.to_string(),
        source_event_id,
    });
    Ok(true)
}

fn is_event_already_seen(state: &Arc<Mutex<NostrBridgeState>>, event: &NostrEvent) -> bool {
    if event.kind != 1 {
        return true;
    }
    let Some(canonical_id) = canonical_hex_64(&event.id) else {
        return true;
    };
    let guard = state.lock().unwrap_or_else(|e| e.into_inner());
    guard.seen_set.contains(&canonical_id)
}

fn relay_checkpoint_needs_update(
    state: &Arc<Mutex<NostrBridgeState>>,
    relay: &str,
    created_at: u64,
) -> bool {
    let max_allowed = current_unix().saturating_add(NOSTR_EVENT_MAX_FUTURE_SKEW_SECS);
    if created_at > max_allowed {
        return false;
    }
    let guard = state.lock().unwrap_or_else(|e| e.into_inner());
    let current = guard.relay_last_created_at.get(relay).copied().unwrap_or(0);
    created_at > current
}

fn note_seen_duplicate_checkpoint(
    state: &Arc<Mutex<NostrBridgeState>>,
    relay: &str,
    event: &NostrEvent,
) {
    if event.kind != 1 {
        return;
    }
    let max_allowed = current_unix().saturating_add(NOSTR_EVENT_MAX_FUTURE_SKEW_SECS);
    if event.created_at > max_allowed {
        return;
    }
    let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(last) = guard.relay_last_created_at.get_mut(relay) {
        *last = (*last).max(event.created_at);
    } else {
        guard
            .relay_last_created_at
            .insert(relay.to_string(), event.created_at);
    }
}

fn parse_nostr_event_message(input: &str) -> Option<NostrEvent> {
    if input.len() > NOSTR_MAX_EVENT_FRAME_BYTES {
        return None;
    }
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

fn verify_nostr_event_authenticity(event: &NostrEvent) -> bool {
    if !valid_hex_64(&event.id) || !valid_hex_64(&event.pubkey) {
        return false;
    }
    let Some(sig_hex) = event.sig.as_deref() else {
        return false;
    };
    if !valid_hex_128(sig_hex) {
        return false;
    }
    let Some(expected_event_id) = compute_nostr_event_id_hex(event) else {
        return false;
    };
    if !event.id.eq_ignore_ascii_case(&expected_event_id) {
        return false;
    }
    let Ok(id_vec) = hex::decode(&event.id) else {
        return false;
    };
    let Ok(pubkey_vec) = hex::decode(&event.pubkey) else {
        return false;
    };
    let Ok(sig_vec) = hex::decode(sig_hex) else {
        return false;
    };
    let Ok(id_bytes) = <[u8; 32]>::try_from(id_vec.as_slice()) else {
        return false;
    };
    let Ok(pubkey) = <[u8; 32]>::try_from(pubkey_vec.as_slice()) else {
        return false;
    };
    let Ok(sig) = <[u8; 64]>::try_from(sig_vec.as_slice()) else {
        return false;
    };
    NostrVerifier
        .verify(pubkey, &id_bytes, sig)
        .unwrap_or(false)
}

fn compute_nostr_event_id_hex(event: &NostrEvent) -> Option<String> {
    if !event.tags.is_array() {
        return None;
    }
    let canonical = json!([
        0,
        event.pubkey,
        event.created_at,
        event.kind,
        event.tags,
        event.content
    ]);
    let encoded = serde_json::to_vec(&canonical).ok()?;
    let digest = Sha256::digest(encoded);
    Some(hex::encode(digest))
}

fn map_event_to_payload(event: &NostrEvent, channel_id: &str, _namespace: u16) -> Option<Vec<u8>> {
    if event.kind != 1 {
        return None;
    }
    if !valid_hex_64(&event.id) || !valid_hex_64(&event.pubkey) {
        return None;
    }
    let text = event.content.trim();
    if text.is_empty() {
        return None;
    }
    if text.len() > NOSTR_EVENT_MAX_CONTENT_BYTES {
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

fn valid_hex_64(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
}

fn canonical_hex_64(value: &str) -> Option<String> {
    if !valid_hex_64(value) {
        return None;
    }
    Some(value.to_ascii_lowercase())
}

fn valid_hex_128(value: &str) -> bool {
    value.len() == 128 && value.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::{
        connect_with_timeout, current_unix, is_event_already_seen, map_event_to_payload,
        note_seen_duplicate_checkpoint, parse_nostr_event_message, reconnect_backoff,
        reserve_and_send_bridged_item, reserve_bridge_slot, verify_nostr_event_authenticity,
        BridgeEnqueueError, ConnectRelayError, NextFrameError, NostrBridgeState, NostrEvent,
        NOSTR_EVENT_MAX_FUTURE_SKEW_SECS,
    };
    use sha2::{Digest, Sha256};
    use veil_crypto::signing::{NostrSigner, Signer};
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
    fn parse_nostr_event_message_rejects_oversized_frame() {
        let oversized_content = "a".repeat(super::NOSTR_MAX_EVENT_FRAME_BYTES + 1);
        let frame = format!(
            "[\"EVENT\",\"sub-a\",{{\"id\":\"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"pubkey\":\"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"kind\":1,\"created_at\":1700000000,\"content\":\"{}\"}}]",
            oversized_content
        );
        assert!(parse_nostr_event_message(&frame).is_none());
    }

    #[test]
    fn maps_kind1_to_feed_post_payload() {
        let msg = r#"["EVENT","sub-a",{"id":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","pubkey":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","kind":1,"created_at":1700000000,"content":"hello veil"}]"#;
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

    #[test]
    fn state_rejects_future_timestamp_without_advancing_checkpoint() {
        let mut state = NostrBridgeState::load(None, 8, 1);
        let future = current_unix().saturating_add(NOSTR_EVENT_MAX_FUTURE_SKEW_SECS + 1);
        assert!(!state.should_accept_event("relay-a", "future-id", future));
        assert!(state.relay_last_created_at.get("relay-a").is_none());
        assert!(state.seen_set.is_empty());
        assert!(state.seen_order.is_empty());
        assert!(state.should_accept_event("relay-a", "ok-id", current_unix()));
    }

    #[test]
    fn map_event_to_payload_rejects_invalid_pubkey_and_oversized_content() {
        let invalid_pubkey_msg = r#"["EVENT","sub-a",{"id":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","pubkey":"not-hex","kind":1,"created_at":1700000000,"content":"hello"}]"#;
        let invalid_pubkey_event =
            parse_nostr_event_message(invalid_pubkey_msg).expect("event should parse");
        assert!(map_event_to_payload(&invalid_pubkey_event, "nostr", 32).is_none());

        let oversized = "a".repeat(16 * 1024 + 1);
        let oversized_msg = format!(
            "[\"EVENT\",\"sub-a\",{{\"id\":\"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"pubkey\":\"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"kind\":1,\"created_at\":1700000000,\"content\":\"{}\"}}]",
            oversized
        );
        let oversized_event =
            parse_nostr_event_message(&oversized_msg).expect("event should parse");
        assert!(map_event_to_payload(&oversized_event, "nostr", 32).is_none());
    }

    #[test]
    fn verify_nostr_event_authenticity_accepts_valid_signed_event() {
        let msg = signed_event_frame([0x33; 32], 1_700_000_123, "hello signed");
        let event = parse_nostr_event_message(&msg).expect("event should parse");
        assert!(verify_nostr_event_authenticity(&event));
    }

    #[test]
    fn verify_nostr_event_authenticity_rejects_tampered_event() {
        let msg = signed_event_frame([0x34; 32], 1_700_000_123, "hello signed");
        let mut event = parse_nostr_event_message(&msg).expect("event should parse");
        event.content = "tampered".to_string();
        assert!(!verify_nostr_event_authenticity(&event));
    }

    #[test]
    fn note_bridge_event_seen_accepts_once_and_tracks_checkpoint() {
        let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
        let event = NostrEvent {
            id: "11".repeat(32),
            pubkey: "not-a-hex-pubkey".to_string(),
            kind: 1,
            created_at: 1_700_000_123,
            tags: serde_json::json!([]),
            content: "hello".to_string(),
            sig: None,
        };

        assert!(super::note_bridge_event_seen(&state, "relay-a", &event));
        assert!(!super::note_bridge_event_seen(&state, "relay-a", &event));
        let guard = state.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(
            guard.relay_last_created_at.get("relay-a").copied(),
            Some(event.created_at)
        );
    }

    #[test]
    fn note_bridge_event_seen_rejects_invalid_event_id_without_state_change() {
        let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
        let event = NostrEvent {
            id: "bad-id".to_string(),
            pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            kind: 1,
            created_at: 1_700_000_123,
            tags: serde_json::json!([]),
            content: "hello".to_string(),
            sig: None,
        };

        assert!(!super::note_bridge_event_seen(&state, "relay-a", &event));
        let guard = state.lock().unwrap_or_else(|e| e.into_inner());
        assert!(guard.relay_last_created_at.get("relay-a").is_none());
        assert!(guard.seen_set.is_empty());
    }

    #[test]
    fn note_bridge_event_seen_rejects_non_kind1_event_without_state_change() {
        let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
        let event = NostrEvent {
            id: "22".repeat(32),
            pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            kind: 42,
            created_at: 1_700_000_123,
            tags: serde_json::json!([]),
            content: "not kind1".to_string(),
            sig: None,
        };

        assert!(!super::note_bridge_event_seen(&state, "relay-a", &event));
        let guard = state.lock().unwrap_or_else(|e| e.into_inner());
        assert!(guard.relay_last_created_at.get("relay-a").is_none());
        assert!(guard.seen_set.is_empty());
        assert!(guard.seen_order.is_empty());
    }

    #[test]
    fn note_bridge_event_seen_dedupes_case_variant_event_ids() {
        let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
        let lower_id = "ab".repeat(32);
        let upper_id = lower_id.to_ascii_uppercase();
        let base_event = NostrEvent {
            id: lower_id,
            pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            kind: 1,
            created_at: 1_700_000_123,
            tags: serde_json::json!([]),
            content: "hello".to_string(),
            sig: None,
        };
        let upper_event = NostrEvent {
            id: upper_id,
            pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            kind: 1,
            created_at: 1_700_000_124,
            tags: serde_json::json!([]),
            content: "hello".to_string(),
            sig: None,
        };

        assert!(super::note_bridge_event_seen(
            &state,
            "relay-a",
            &base_event
        ));
        assert!(!super::note_bridge_event_seen(
            &state,
            "relay-a",
            &upper_event
        ));
        let guard = state.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(guard.seen_set.len(), 1);
        assert!(guard.seen_set.contains(&"ab".repeat(32)));
    }

    #[test]
    fn is_event_already_seen_detects_case_variant_ids() {
        let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
        let base_event = NostrEvent {
            id: "cd".repeat(32),
            pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            kind: 1,
            created_at: 1_700_000_123,
            tags: serde_json::json!([]),
            content: "hello".to_string(),
            sig: None,
        };
        assert!(!is_event_already_seen(&state, &base_event));
        assert!(super::note_bridge_event_seen(
            &state,
            "relay-a",
            &base_event
        ));

        let mut variant = base_event;
        variant.id = "CD".repeat(32);
        assert!(is_event_already_seen(&state, &variant));
    }

    #[test]
    fn note_seen_duplicate_checkpoint_updates_checkpoint_for_other_relays() {
        let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
        let event = NostrEvent {
            id: "ef".repeat(32),
            pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            kind: 1,
            created_at: 1_700_000_123,
            tags: serde_json::json!([]),
            content: "hello".to_string(),
            sig: None,
        };
        assert!(super::note_bridge_event_seen(&state, "relay-a", &event));
        note_seen_duplicate_checkpoint(&state, "relay-b", &event);
        let guard = state.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(
            guard.relay_last_created_at.get("relay-b").copied(),
            Some(event.created_at)
        );
    }

    #[test]
    fn note_seen_duplicate_checkpoint_does_not_checkpoint_future_timestamp() {
        let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
        let future = current_unix().saturating_add(NOSTR_EVENT_MAX_FUTURE_SKEW_SECS + 1);
        let event = NostrEvent {
            id: "aa".repeat(32),
            pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            kind: 1,
            created_at: current_unix(),
            tags: serde_json::json!([]),
            content: "hello".to_string(),
            sig: None,
        };
        assert!(super::note_bridge_event_seen(&state, "relay-a", &event));

        let future_variant = NostrEvent {
            id: "AA".repeat(32),
            created_at: future,
            ..event
        };
        note_seen_duplicate_checkpoint(&state, "relay-b", &future_variant);
        let guard = state.lock().unwrap_or_else(|e| e.into_inner());
        assert!(guard.relay_last_created_at.get("relay-b").is_none());
    }

    #[tokio::test]
    async fn connect_with_timeout_times_out_on_stalled_handshake() {
        use std::time::Duration;
        use tokio::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind should succeed");
        let addr = listener.local_addr().expect("local addr");

        let server = tokio::spawn(async move {
            let (_sock, _) = listener.accept().await.expect("accept should succeed");
            tokio::time::sleep(Duration::from_millis(250)).await;
        });

        let relay = format!("ws://{addr}");
        let result = connect_with_timeout(&relay, Duration::from_millis(50)).await;
        assert!(matches!(result, Err(ConnectRelayError::Timeout)));
        let _ = server.await;
    }

    #[tokio::test]
    async fn reserve_bridge_slot_times_out_when_channel_is_full() {
        use std::time::Duration;
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        tx.send(super::BridgedItem {
            payload: vec![1, 2, 3],
            source_relay: "relay-a".to_string(),
            source_event_id: "11".repeat(32),
        })
        .await
        .expect("seed queue");
        let result = reserve_bridge_slot(&tx, Duration::from_millis(30)).await;
        assert!(matches!(result, Err(BridgeEnqueueError::Timeout)));
        let _ = rx.recv().await;
    }

    #[tokio::test]
    async fn reserve_bridge_slot_reports_closed_when_receiver_is_gone() {
        use std::time::Duration;
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        drop(rx);
        let result = reserve_bridge_slot(&tx, Duration::from_millis(30)).await;
        assert!(matches!(result, Err(BridgeEnqueueError::Closed)));
    }

    #[tokio::test]
    async fn reserve_and_send_does_not_checkpoint_when_queue_is_full() {
        use std::time::Duration;
        let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        tx.send(super::BridgedItem {
            payload: vec![9, 9, 9],
            source_relay: "relay-a".to_string(),
            source_event_id: "11".repeat(32),
        })
        .await
        .expect("seed full queue");

        let event = NostrEvent {
            id: "22".repeat(32),
            pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            kind: 1,
            created_at: 1_700_000_123,
            tags: serde_json::json!([]),
            content: "hello".to_string(),
            sig: None,
        };
        let result = reserve_and_send_bridged_item(
            &tx,
            &state,
            "relay-a",
            &event,
            vec![1, 2, 3],
            Duration::from_millis(30),
        )
        .await;
        assert!(matches!(result, Err(BridgeEnqueueError::Timeout)));
        let guard = state.lock().unwrap_or_else(|e| e.into_inner());
        assert!(guard.relay_last_created_at.get("relay-a").is_none());
        assert!(guard.seen_set.is_empty());
        assert!(guard.seen_order.is_empty());
        drop(guard);
        let _ = rx.recv().await;
    }

    #[tokio::test]
    async fn reserve_and_send_emits_canonical_source_event_id() {
        use std::time::Duration;
        let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let event = NostrEvent {
            id: "EF".repeat(32),
            pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            kind: 1,
            created_at: 1_700_000_123,
            tags: serde_json::json!([]),
            content: "hello".to_string(),
            sig: None,
        };
        let result = reserve_and_send_bridged_item(
            &tx,
            &state,
            "relay-a",
            &event,
            vec![7, 8, 9],
            Duration::from_millis(30),
        )
        .await;
        assert!(matches!(result, Ok(true)));

        let item = rx.recv().await.expect("expected bridged item");
        assert_eq!(item.source_event_id, "ef".repeat(32));
        let guard = state.lock().unwrap_or_else(|e| e.into_inner());
        assert!(guard.seen_set.contains(&"ef".repeat(32)));
    }

    #[test]
    fn relay_checkpoint_needs_update_tracks_created_at_watermark() {
        let state = std::sync::Arc::new(std::sync::Mutex::new(NostrBridgeState::load(None, 8, 1)));
        let ts = 1_700_000_123;
        assert!(super::relay_checkpoint_needs_update(&state, "relay-a", ts));
        let event = NostrEvent {
            id: "11".repeat(32),
            pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            kind: 1,
            created_at: ts,
            tags: serde_json::json!([]),
            content: "hello".to_string(),
            sig: None,
        };
        assert!(super::note_bridge_event_seen(&state, "relay-a", &event));
        assert!(!super::relay_checkpoint_needs_update(&state, "relay-a", ts));
        assert!(!super::relay_checkpoint_needs_update(
            &state,
            "relay-a",
            ts.saturating_sub(1)
        ));
        assert!(super::relay_checkpoint_needs_update(
            &state,
            "relay-a",
            ts.saturating_add(1)
        ));

        let future = current_unix().saturating_add(NOSTR_EVENT_MAX_FUTURE_SKEW_SECS + 1);
        assert!(!super::relay_checkpoint_needs_update(
            &state, "relay-a", future
        ));
    }

    #[tokio::test]
    async fn next_text_frame_with_timeout_detects_stalled_relay() {
        use std::time::Duration;
        use tokio::net::TcpListener;
        use tokio_tungstenite::accept_async;

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind should succeed");
        let addr = listener.local_addr().expect("local addr");

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept should succeed");
            let _ws = accept_async(stream)
                .await
                .expect("ws accept should succeed");
            tokio::time::sleep(Duration::from_millis(250)).await;
        });

        let relay = format!("ws://{addr}");
        let (mut ws, _) = connect_with_timeout(&relay, Duration::from_secs(1))
            .await
            .expect("connect should succeed");
        let result = super::next_text_frame_with_timeout(&mut ws, Duration::from_millis(50)).await;
        assert!(matches!(result, Err(NextFrameError::Timeout)));
        let _ = server.await;
    }

    #[tokio::test]
    async fn next_text_frame_with_timeout_detects_closed_relay() {
        use futures_util::SinkExt;
        use std::time::Duration;
        use tokio::net::TcpListener;
        use tokio_tungstenite::accept_async;
        use tokio_tungstenite::tungstenite::Message;

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind should succeed");
        let addr = listener.local_addr().expect("local addr");

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept should succeed");
            let mut ws = accept_async(stream)
                .await
                .expect("ws accept should succeed");
            ws.send(Message::Close(None))
                .await
                .expect("close frame should send");
        });

        let relay = format!("ws://{addr}");
        let (mut ws, _) = connect_with_timeout(&relay, Duration::from_secs(1))
            .await
            .expect("connect should succeed");
        let result = super::next_text_frame_with_timeout(&mut ws, Duration::from_secs(1)).await;
        assert!(matches!(result, Err(NextFrameError::Closed)));
        let _ = server.await;
    }

    fn signed_event_frame(secret: [u8; 32], created_at: u64, content: &str) -> String {
        let signer = NostrSigner::from_secret(secret).expect("valid signer");
        let pubkey_hex = hex::encode(signer.public_key());
        let tags = serde_json::json!([]);
        let canonical = serde_json::json!([0, pubkey_hex, created_at, 1, tags, content]);
        let encoded = serde_json::to_vec(&canonical).expect("canonical event bytes");
        let id_bytes: [u8; 32] = Sha256::digest(encoded).into();
        let id_hex = hex::encode(id_bytes);
        let sig_hex = hex::encode(signer.sign(&id_bytes).expect("sign event id"));
        serde_json::json!([
            "EVENT",
            "sub-a",
            {
                "id": id_hex,
                "pubkey": pubkey_hex,
                "kind": 1,
                "created_at": created_at,
                "tags": [],
                "content": content,
                "sig": sig_hex
            }
        ])
        .to_string()
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
