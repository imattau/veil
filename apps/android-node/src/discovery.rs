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

mod api_handlers;
mod contact_validation;
mod http_targets;
mod lan;
mod lookup_keys;
mod message;
mod message_handlers;
mod table;
mod worker_helpers;
pub use self::message::{DiscoveryKind, DiscoveryMessage};
pub use self::worker_helpers::build_self_contact;
use self::worker_helpers::{join_discovery_endpoint, with_auth_header};

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
        let local_pubkey = self.state.identity().public_key;

        let target_count = http_targets.len();
        let contact_count = contacts.len();

        if target_count > 0 {
            let payload = DiscoveryGossipRequest {
                contacts: self.state.discovery_sample(gossip_limit),
            };
            let local_peer_id = self.protocol.peer_id();
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
                let local_peer_id = local_peer_id.clone();
                let local_pubkey = local_pubkey;
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
                                    if is_local_identity_contact(
                                        &contact,
                                        &local_peer_id,
                                        local_pubkey,
                                    ) {
                                        continue;
                                    }
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
    api_handlers::handle_discovery_announce(state, request, max_neighbors, sanitize_contact)
}

pub fn handle_discovery_lookup(
    state: &NodeState,
    request: DiscoveryLookupRequest,
) -> DiscoveryLookupResponse {
    api_handlers::handle_discovery_lookup(
        state,
        request,
        DISCOVERY_LOOKUP_DEFAULT_LIMIT,
        DISCOVERY_MAX_CONTACTS,
    )
}

pub fn handle_discovery_gossip(
    state: &NodeState,
    request: DiscoveryGossipRequest,
    max_contacts: usize,
) -> DiscoveryGossipResponse {
    api_handlers::handle_discovery_gossip(
        state,
        request,
        DISCOVERY_MAX_CONTACTS,
        sanitize_contact,
        |value| bounded_gossip_contacts(value.min(max_contacts)),
    )
}

pub fn discovery_tag(namespace: Namespace) -> [u8; 32] {
    let mut input = Vec::with_capacity(DISCOVERY_TAG_SEED.len() + 2);
    input.extend_from_slice(DISCOVERY_TAG_SEED);
    input.extend_from_slice(&namespace.0.to_be_bytes());
    blake3::hash(&input).into()
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

pub(crate) fn is_local_identity_contact(
    contact: &ContactBundle,
    local_peer_id: &str,
    local_pubkey: [u8; 32],
) -> bool {
    if contact.peer_id == local_peer_id {
        return true;
    }
    let mut pubkey = [0u8; 32];
    hex::decode_to_slice(&contact.pubkey_hex, &mut pubkey)
        .map(|_| pubkey == local_pubkey)
        .unwrap_or(false)
}

pub fn sanitize_discovery_contact(contact: ContactBundle) -> Option<ContactBundle> {
    sanitize_contact(contact)
}

#[cfg(test)]
mod tests;
