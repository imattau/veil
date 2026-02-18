use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[cfg(test)]
use crate::nostr_bridge_event::NOSTR_MAX_EVENT_FRAME_BYTES;
use crate::nostr_bridge_event::{
    canonical_hex_64, map_event_to_payload, parse_nostr_event_message,
    verify_nostr_event_authenticity, NostrEvent,
};
use crate::nostr_bridge_state::{current_unix, NostrBridgeState, NOSTR_EVENT_MAX_FUTURE_SKEW_SECS};
use tracing::error;

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

#[cfg(test)]
#[path = "nostr_bridge_tests.rs"]
mod nostr_bridge_tests;
