use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinSet;

use self::contact_validation::{
    bounded_gossip_contacts as bounded_gossip_contacts_impl,
    normalize_reply_to_peer_id as normalize_reply_to_peer_id_impl,
    sanitize_contact as sanitize_contact_impl,
};
use self::http_targets::{bounded_http_gossip_concurrency, collect_http_targets};
pub use self::table::{DiscoveryStateHandle, DiscoveryTable};
use crate::api::{
    ContactBundle, DiscoveryAnnounceRequest, DiscoveryAnnounceResponse, DiscoveryGossipRequest,
    DiscoveryGossipResponse, DiscoveryLookupRequest, DiscoveryLookupResponse,
};
use crate::protocol::ProtocolEngine;
use crate::state::NodeState;
use veil_core::Namespace;

const DISCOVERY_MAX_CONTACTS: usize = 64;
const DISCOVERY_LOOKUP_DEFAULT_LIMIT: usize = 16;
const DISCOVERY_MAX_PEER_ID_LEN: usize = 128;
const DISCOVERY_MAX_BATCH_MESSAGES: usize = 64;
const DISCOVERY_MAX_PAYLOAD_BYTES: usize = 256 * 1024;
const DISCOVERY_MAX_MESSAGE_BYTES: usize = 64 * 1024;
const DISCOVERY_MAX_ENDPOINT_LEN: usize = 1024;
const DISCOVERY_MAX_LAN_ADDRS: usize = 8;
const DISCOVERY_MAX_LAN_ADDR_LEN: usize = 256;
const DISCOVERY_TAG_SEED: &[u8] = b"veil-discovery";

mod contact_validation;
mod http_targets;
mod lan;
mod lookup_keys;
mod message;
mod message_handlers;
mod table;
pub use self::message::{DiscoveryKind, DiscoveryMessage};

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub bootstrap_urls: Vec<String>,
    pub gossip_interval: Duration,
    pub max_gossip_contacts: usize,
    pub transport_enabled: bool,
    pub auth_token: Option<String>,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            bootstrap_urls: Vec::new(),
            gossip_interval: Duration::from_secs(12),
            max_gossip_contacts: 24,
            transport_enabled: true,
            auth_token: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LanDiscoveryConfig {
    pub enabled: bool,
    pub port: u16,
    pub announce_interval: Duration,
}

impl Default for LanDiscoveryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 9333,
            announce_interval: Duration::from_secs(5),
        }
    }
}

#[derive(Clone)]
pub struct DiscoveryWorker {
    state: Arc<NodeState>,
    protocol: Arc<ProtocolEngine>,
    config: DiscoveryConfig,
    http: reqwest::Client,
}

impl DiscoveryWorker {
    pub fn new(
        state: Arc<NodeState>,
        protocol: Arc<ProtocolEngine>,
        config: DiscoveryConfig,
    ) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            state,
            protocol,
            config,
            http,
        }
    }

    pub async fn run(self) {
        let mut worker = self;
        let mut next_gossip = tokio::time::Instant::now();

        loop {
            tokio::time::sleep_until(next_gossip).await;
            worker.gossip_once().await;

            let contact_count = worker.state.contacts().len();
            let interval = if contact_count == 0 {
                // Gossip more frequently when bootstrapping
                Duration::from_secs(2)
            } else {
                worker.config.gossip_interval
            };

            next_gossip = tokio::time::Instant::now() + interval;
        }
    }

    async fn gossip_once(&mut self) {
        let gossip_limit = bounded_gossip_contacts(self.config.max_gossip_contacts);
        let contacts = self.state.contacts();
        let http_targets = collect_http_targets(&self.config.bootstrap_urls, &contacts);

        let target_count = http_targets.len();
        let contact_count = contacts.len();

        if target_count > 0 {
            let payload = DiscoveryGossipRequest {
                contacts: self.state.discovery_sample(gossip_limit),
            };
            tracing::info!(
                "Starting HTTP discovery gossip with {} targets (known contacts: {})",
                target_count,
                contact_count
            );

            let mut handles = JoinSet::new();
            let max_in_flight = bounded_http_gossip_concurrency(target_count);
            for target in http_targets {
                let payload = payload.clone();
                let state = Arc::clone(&self.state);
                let protocol = Arc::clone(&self.protocol);
                let http = self.http.clone();
                let auth_token = self.config.auth_token.clone();
                handles.spawn(async move {
                    let url = join_discovery_endpoint(&target, "discovery/gossip");
                    let request =
                        with_auth_header(http.post(&url).json(&payload), auth_token.as_deref());
                    match request.send().await {
                        Ok(resp) => {
                            if let Ok(parsed) = resp.json::<DiscoveryGossipResponse>().await {
                                let mut new_contacts = 0;
                                for contact in parsed
                                    .contacts
                                    .into_iter()
                                    .filter_map(sanitize_contact)
                                    .take(DISCOVERY_MAX_CONTACTS)
                                {
                                    state.add_contact(contact.clone());
                                    let _ = protocol.add_contact(&contact).await;
                                    new_contacts += 1;
                                }
                                if new_contacts > 0 {
                                    tracing::info!(
                                        "HTTP gossip with {} yielded {} contacts",
                                        target,
                                        new_contacts
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("HTTP gossip failed with {}: {}", target, e);
                        }
                    }
                });
                while handles.len() >= max_in_flight {
                    let _ = handles.join_next().await;
                }
            }
            while handles.join_next().await.is_some() {}
        } else {
            tracing::warn!(
                "No discovery targets available (bootstrap_urls empty and no known RPC peers)"
            );
        }

        if self.config.transport_enabled {
            let self_contact = build_self_contact(&self.state, &self.protocol);
            let announce = DiscoveryMessage::announce(self_contact);
            let gossip = DiscoveryMessage::gossip(self.state.discovery_sample(gossip_limit));

            if let Err(e) = self.protocol.publish_discovery(announce).await {
                tracing::debug!("Transport discovery announce failed: {}", e);
            }
            if let Err(e) = self.protocol.publish_discovery(gossip).await {
                tracing::debug!("Transport discovery gossip failed: {}", e);
            }
        }
    }
}

pub use self::lan::LanDiscoveryWorker;

pub async fn handle_discovery_payload(
    state: &NodeState,
    protocol: &ProtocolEngine,
    payload: &[u8],
) -> Option<()> {
    if payload.len() > DISCOVERY_MAX_PAYLOAD_BYTES {
        tracing::debug!(
            payload_len = payload.len(),
            max_payload = DISCOVERY_MAX_PAYLOAD_BYTES,
            "dropping oversized discovery payload"
        );
        return None;
    }
    if payload.len() <= DISCOVERY_MAX_MESSAGE_BYTES {
        if let Ok(msg) = serde_json::from_slice::<DiscoveryMessage>(payload) {
            return message_handlers::handle_discovery_message(
                state,
                protocol,
                msg,
                DISCOVERY_MAX_CONTACTS,
                normalize_reply_to_peer_id,
                sanitize_contact,
            )
            .await;
        }
    }
    if let Ok(items) = ciborium::de::from_reader::<Vec<Vec<u8>>, _>(payload) {
        let mut handled = false;
        for item in items.into_iter().take(DISCOVERY_MAX_BATCH_MESSAGES) {
            if item.len() > DISCOVERY_MAX_MESSAGE_BYTES {
                continue;
            }
            if let Ok(msg) = serde_json::from_slice::<DiscoveryMessage>(&item) {
                let _ = message_handlers::handle_discovery_message(
                    state,
                    protocol,
                    msg,
                    DISCOVERY_MAX_CONTACTS,
                    normalize_reply_to_peer_id,
                    sanitize_contact,
                )
                .await;
                handled = true;
            }
        }
        if handled {
            return Some(());
        }
    }
    None
}

pub fn handle_discovery_announce(
    state: &NodeState,
    request: DiscoveryAnnounceRequest,
    max_neighbors: usize,
) -> DiscoveryAnnounceResponse {
    let Some(contact) = sanitize_contact(request.contact) else {
        return DiscoveryAnnounceResponse {
            accepted: false,
            neighbors: Vec::new(),
        };
    };
    state.add_contact(contact.clone());
    let neighbors = state.discovery_lookup_contact(&contact, max_neighbors);
    DiscoveryAnnounceResponse {
        accepted: true,
        neighbors,
    }
}

pub fn handle_discovery_lookup(
    state: &NodeState,
    request: DiscoveryLookupRequest,
) -> DiscoveryLookupResponse {
    let limit = request
        .limit
        .unwrap_or(DISCOVERY_LOOKUP_DEFAULT_LIMIT)
        .min(DISCOVERY_MAX_CONTACTS);
    let mut result = Vec::new();
    if let Some(peer_id) = request.peer_id.as_deref() {
        result = state.discovery_lookup_peer(peer_id, limit);
    } else if let Some(pubkey_hex) = request.pubkey_hex.as_deref() {
        result = state.discovery_lookup_pubkey(pubkey_hex, limit);
    }
    DiscoveryLookupResponse { contacts: result }
}

pub fn handle_discovery_gossip(
    state: &NodeState,
    request: DiscoveryGossipRequest,
    max_contacts: usize,
) -> DiscoveryGossipResponse {
    for contact in request
        .contacts
        .into_iter()
        .filter_map(sanitize_contact)
        .take(DISCOVERY_MAX_CONTACTS)
    {
        state.add_contact(contact);
    }
    DiscoveryGossipResponse {
        contacts: state.discovery_sample(bounded_gossip_contacts(max_contacts)),
    }
}

pub fn discovery_tag(namespace: Namespace) -> [u8; 32] {
    let mut input = Vec::with_capacity(DISCOVERY_TAG_SEED.len() + 2);
    input.extend_from_slice(DISCOVERY_TAG_SEED);
    input.extend_from_slice(&namespace.0.to_be_bytes());
    blake3::hash(&input).into()
}

pub fn build_self_contact(node: &NodeState, protocol: &ProtocolEngine) -> ContactBundle {
    let identity = node.identity();
    let rpc_url = std::env::var("VEIL_NODE_RPC_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let ws_url = std::env::var("VEIL_NODE_WS_PUBLIC")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            protocol.ws_url().filter(|url| {
                !url.contains("127.0.0.1") && !url.contains("localhost") && !url.contains("0.0.0.0")
            })
        });
    let quic_addr = std::env::var("VEIL_NODE_QUIC_PUBLIC")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            let addr = protocol.quic_bind_addr();
            if addr.starts_with("0.0.0.0")
                || addr.starts_with("127.0.0.1")
                || addr.contains("localhost")
            {
                None
            } else {
                Some(addr)
            }
        });
    ContactBundle {
        peer_id: protocol.peer_id(),
        ws_url,
        quic_addr,
        pubkey_hex: identity.public_key_hex(),
        rpc_url,
        lan_addrs: Vec::new(),
    }
}

fn join_discovery_endpoint(base: &str, path: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    format!("{trimmed}/{path}")
}

fn with_auth_header(
    builder: reqwest::RequestBuilder,
    token: Option<&str>,
) -> reqwest::RequestBuilder {
    match token.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => builder.header("x-veil-token", value),
        None => builder,
    }
}

fn bounded_gossip_contacts(requested: usize) -> usize {
    bounded_gossip_contacts_impl(requested, DISCOVERY_MAX_CONTACTS)
}

fn normalize_reply_to_peer_id(value: &str) -> Option<String> {
    normalize_reply_to_peer_id_impl(value, DISCOVERY_MAX_PEER_ID_LEN)
}

fn sanitize_contact(contact: ContactBundle) -> Option<ContactBundle> {
    sanitize_contact_impl(
        contact,
        DISCOVERY_MAX_PEER_ID_LEN,
        DISCOVERY_MAX_ENDPOINT_LEN,
        DISCOVERY_MAX_LAN_ADDR_LEN,
        DISCOVERY_MAX_LAN_ADDRS,
    )
}

pub fn sanitize_discovery_contact(contact: ContactBundle) -> Option<ContactBundle> {
    sanitize_contact(contact)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_contact(peer_id: &str, pubkey_hex: &str) -> ContactBundle {
        ContactBundle {
            peer_id: peer_id.to_string(),
            ws_url: None,
            quic_addr: None,
            pubkey_hex: pubkey_hex.to_string(),
            rpc_url: None,
            lan_addrs: Vec::new(),
        }
    }

    #[test]
    fn discovery_message_roundtrip() {
        let contact = make_contact(
            "peer-x",
            "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        );
        let msg = DiscoveryMessage::announce(contact);
        let encoded = serde_json::to_vec(&msg).unwrap();
        let decoded: DiscoveryMessage = serde_json::from_slice(&encoded).unwrap();
        assert!(matches!(decoded.kind, DiscoveryKind::Announce));
        assert!(decoded.contact.is_some());
    }

    #[test]
    fn sanitize_contact_rejects_invalid_pubkey() {
        let contact = make_contact("peer-x", "zzzz");
        assert!(sanitize_contact(contact).is_none());
    }

    #[test]
    fn discovery_announce_rejects_invalid_contact() {
        let state = NodeState::new("test");
        let request = DiscoveryAnnounceRequest {
            contact: ContactBundle {
                peer_id: "peer-x".to_string(),
                ws_url: None,
                quic_addr: None,
                pubkey_hex: "abcd".to_string(),
                rpc_url: None,
                lan_addrs: Vec::new(),
            },
        };
        let response = handle_discovery_announce(&state, request, 16);
        assert!(!response.accepted);
        assert!(response.neighbors.is_empty());
        assert!(state.contacts().is_empty());
    }

    #[test]
    fn discovery_gossip_limits_and_filters_contacts() {
        let state = NodeState::new("test");
        let mut contacts = vec![ContactBundle {
            peer_id: String::new(),
            ws_url: Some("ws://invalid".to_string()),
            quic_addr: None,
            pubkey_hex: "ff".repeat(32),
            rpc_url: None,
            lan_addrs: vec!["".to_string()],
        }];
        for i in 0..(DISCOVERY_MAX_CONTACTS + 20) {
            contacts.push(ContactBundle {
                peer_id: format!("peer-{i}"),
                ws_url: Some(" ws://example.com/ws ".to_string()),
                quic_addr: Some(" 127.0.0.1:9444 ".to_string()),
                pubkey_hex: format!("{:064x}", i + 1),
                rpc_url: Some(" https://example.com/rpc ".to_string()),
                lan_addrs: vec![" ".to_string(), "10.0.0.1:9333".to_string()],
            });
        }
        let response = handle_discovery_gossip(&state, DiscoveryGossipRequest { contacts }, 256);
        assert_eq!(state.contacts().len(), DISCOVERY_MAX_CONTACTS);
        assert_eq!(response.contacts.len(), DISCOVERY_MAX_CONTACTS);
        assert!(state
            .contacts()
            .iter()
            .all(|contact| contact.pubkey_hex.len() == 64
                && contact.pubkey_hex.chars().all(|c| c.is_ascii_hexdigit())));
    }

    #[test]
    fn discovery_lookup_caps_requested_limit() {
        let state = NodeState::new("test");
        for i in 0..(DISCOVERY_MAX_CONTACTS + 40) {
            state.add_contact(ContactBundle {
                peer_id: format!("peer-{i}"),
                ws_url: None,
                quic_addr: None,
                pubkey_hex: format!("{:064x}", i + 1),
                rpc_url: None,
                lan_addrs: Vec::new(),
            });
        }
        let response = handle_discovery_lookup(
            &state,
            DiscoveryLookupRequest {
                peer_id: Some("peer-z".to_string()),
                pubkey_hex: None,
                limit: Some(usize::MAX),
            },
        );
        assert_eq!(response.contacts.len(), DISCOVERY_MAX_CONTACTS);
    }

    #[test]
    fn with_auth_header_sets_token_header() {
        let client = reqwest::Client::new();
        let request = with_auth_header(client.post("http://example.com"), Some("secret-token"))
            .build()
            .expect("request");
        let header = request.headers().get("x-veil-token").expect("header");
        assert_eq!(header, "secret-token");
    }

    #[test]
    fn with_auth_header_skips_empty_token() {
        let client = reqwest::Client::new();
        let request = with_auth_header(client.post("http://example.com"), Some("  "))
            .build()
            .expect("request");
        assert!(request.headers().get("x-veil-token").is_none());
    }

    #[test]
    fn bounded_gossip_contacts_is_capped_and_non_zero() {
        assert_eq!(bounded_gossip_contacts(0), 1);
        assert_eq!(bounded_gossip_contacts(1), 1);
        assert_eq!(bounded_gossip_contacts(24), 24);
        assert_eq!(
            bounded_gossip_contacts(DISCOVERY_MAX_CONTACTS + 999),
            DISCOVERY_MAX_CONTACTS
        );
    }

    #[test]
    fn discovery_gossip_response_caps_requested_max_contacts() {
        let state = NodeState::new("test");
        for i in 0..(DISCOVERY_MAX_CONTACTS + 40) {
            state.add_contact(ContactBundle {
                peer_id: format!("peer-{i}"),
                ws_url: None,
                quic_addr: None,
                pubkey_hex: format!("{:064x}", i + 1),
                rpc_url: None,
                lan_addrs: Vec::new(),
            });
        }
        let response = handle_discovery_gossip(
            &state,
            DiscoveryGossipRequest {
                contacts: Vec::new(),
            },
            usize::MAX,
        );
        assert_eq!(response.contacts.len(), DISCOVERY_MAX_CONTACTS);
    }

    #[tokio::test]
    async fn discovery_payload_batch_caps_message_count() {
        let state = NodeState::new("test");
        let identity = state.identity();
        let protocol = ProtocolEngine::new(crate::default_protocol_config(
            "ws://127.0.0.1:9/ws".to_string(),
            "node-a".to_string(),
            32,
            identity.public_key,
            identity.encrypt_key,
            identity.signer(),
        ))
        .expect("protocol init");

        let mut items = Vec::new();
        for i in 0..(DISCOVERY_MAX_BATCH_MESSAGES + 20) {
            let msg = DiscoveryMessage::gossip(vec![ContactBundle {
                peer_id: format!("peer-{i}"),
                ws_url: None,
                quic_addr: Some(format!("127.0.0.1:{}", 9100 + i)),
                pubkey_hex: format!("{:064x}", i + 1),
                rpc_url: None,
                lan_addrs: Vec::new(),
            }]);
            items.push(serde_json::to_vec(&msg).expect("encode message"));
        }
        let mut payload = Vec::new();
        ciborium::ser::into_writer(&items, &mut payload).expect("encode payload");

        let handled = handle_discovery_payload(&state, &protocol, &payload).await;
        assert_eq!(handled, Some(()));
        assert_eq!(state.contacts().len(), DISCOVERY_MAX_BATCH_MESSAGES);
    }

    #[tokio::test]
    async fn discovery_lookup_requires_reply_target() {
        let state = NodeState::new("test");
        state.add_contact(ContactBundle {
            peer_id: "peer-target".to_string(),
            ws_url: None,
            quic_addr: Some("127.0.0.1:9200".to_string()),
            pubkey_hex: "aa".repeat(32),
            rpc_url: None,
            lan_addrs: Vec::new(),
        });
        let identity = state.identity();
        let protocol = ProtocolEngine::new(crate::default_protocol_config(
            "ws://127.0.0.1:9/ws".to_string(),
            "node-a".to_string(),
            32,
            identity.public_key,
            identity.encrypt_key,
            identity.signer(),
        ))
        .expect("protocol init");
        let lookup =
            DiscoveryMessage::lookup(Some("peer-target".to_string()), None, "   ".to_string());
        let payload = serde_json::to_vec(&lookup).expect("encode lookup");

        let handled = handle_discovery_payload(&state, &protocol, &payload).await;
        assert_eq!(handled, None);
    }

    #[tokio::test]
    async fn discovery_payload_rejects_oversized_input() {
        let state = NodeState::new("test");
        let identity = state.identity();
        let protocol = ProtocolEngine::new(crate::default_protocol_config(
            "ws://127.0.0.1:9/ws".to_string(),
            "node-a".to_string(),
            32,
            identity.public_key,
            identity.encrypt_key,
            identity.signer(),
        ))
        .expect("protocol init");
        let oversized = vec![0x42; DISCOVERY_MAX_PAYLOAD_BYTES + 1];

        let handled = handle_discovery_payload(&state, &protocol, &oversized).await;
        assert_eq!(handled, None);
        assert!(state.contacts().is_empty());
    }

    #[tokio::test]
    async fn discovery_payload_batch_skips_oversized_message_items() {
        let state = NodeState::new("test");
        let identity = state.identity();
        let protocol = ProtocolEngine::new(crate::default_protocol_config(
            "ws://127.0.0.1:9/ws".to_string(),
            "node-a".to_string(),
            32,
            identity.public_key,
            identity.encrypt_key,
            identity.signer(),
        ))
        .expect("protocol init");

        let valid = DiscoveryMessage::gossip(vec![ContactBundle {
            peer_id: "peer-ok".to_string(),
            ws_url: None,
            quic_addr: Some("127.0.0.1:9300".to_string()),
            pubkey_hex: "bb".repeat(32),
            rpc_url: None,
            lan_addrs: Vec::new(),
        }]);
        let mut items = Vec::new();
        items.push(vec![0x55; DISCOVERY_MAX_MESSAGE_BYTES + 1]);
        items.push(serde_json::to_vec(&valid).expect("encode valid"));

        let mut payload = Vec::new();
        ciborium::ser::into_writer(&items, &mut payload).expect("encode payload");

        let handled = handle_discovery_payload(&state, &protocol, &payload).await;
        assert_eq!(handled, Some(()));
        assert_eq!(state.contacts().len(), 1);
        assert!(state.contacts().iter().any(|c| c.peer_id == "peer-ok"));
    }
}
