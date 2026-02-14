use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};

use crate::api::{
    ContactBundle, DiscoveryAnnounceRequest, DiscoveryAnnounceResponse, DiscoveryGossipRequest,
    DiscoveryGossipResponse, DiscoveryLookupRequest, DiscoveryLookupResponse,
};
use crate::protocol::ProtocolEngine;
use crate::state::NodeState;
use veil_core::Namespace;

const DISCOVERY_MAX_CONTACTS: usize = 64;
const DISCOVERY_TAG_SEED: &[u8] = b"veil-discovery";

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub bootstrap_urls: Vec<String>,
    pub gossip_interval: Duration,
    pub max_gossip_contacts: usize,
    pub transport_enabled: bool,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            bootstrap_urls: Vec::new(),
            gossip_interval: Duration::from_secs(12),
            max_gossip_contacts: 24,
            transport_enabled: true,
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

#[derive(Debug, Default)]
pub struct DiscoveryTable {
    contacts: HashMap<String, ContactBundle>,
}

impl DiscoveryTable {
    pub fn upsert(&mut self, contact: ContactBundle) {
        self.contacts.insert(contact.peer_id.clone(), contact);
    }

    pub fn lookup(&self, key: &[u8; 32], limit: usize) -> Vec<ContactBundle> {
        let mut contacts: Vec<_> = self
            .contacts
            .values()
            .cloned()
            .map(|contact| {
                let ckey = contact_key(&contact);
                let distance = xor_distance(key, &ckey);
                (distance, contact)
            })
            .collect();
        contacts.sort_by(|a, b| a.0.cmp(&b.0));
        contacts
            .into_iter()
            .take(limit)
            .map(|(_, contact)| contact)
            .collect()
    }

    pub fn sample(&self, max: usize) -> Vec<ContactBundle> {
        let mut entries: Vec<_> = self.contacts.values().cloned().collect();
        if entries.len() <= max {
            return entries;
        }
        entries.shuffle(&mut thread_rng());
        entries.truncate(max);
        entries
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
        let mut all_targets = self.config.bootstrap_urls.clone();
        let contacts = self.state.contacts();
        for contact in &contacts {
            if let Some(rpc_url) = &contact.rpc_url {
                all_targets.push(rpc_url.clone());
            }
        }
        all_targets.sort();
        all_targets.dedup();

        // Only use http/https targets for reqwest
        let mut http_targets = all_targets.clone();
        http_targets.retain(|url| url.starts_with("http://") || url.starts_with("https://"));

        let target_count = http_targets.len();
        let contact_count = contacts.len();

        if target_count > 0 {
            let payload = DiscoveryGossipRequest {
                contacts: self.state.discovery_sample(self.config.max_gossip_contacts),
            };
            tracing::info!(
                "Starting HTTP discovery gossip with {} targets (known contacts: {})",
                target_count,
                contact_count
            );

            let mut handles = Vec::new();
            for target in http_targets {
                let payload = payload.clone();
                let state = Arc::clone(&self.state);
                let protocol = Arc::clone(&self.protocol);
                let http = self.http.clone();
                handles.push(tokio::spawn(async move {
                    let url = join_discovery_endpoint(&target, "discovery/gossip");
                    match http.post(&url).json(&payload).send().await {
                        Ok(resp) => {
                            if let Ok(parsed) = resp.json::<DiscoveryGossipResponse>().await {
                                let mut new_contacts = 0;
                                for contact in parsed.contacts {
                                    state.add_contact(contact.clone());
                                    let _ = protocol.add_contact(&contact).await;
                                    new_contacts += 1;
                                }
                                if new_contacts > 0 {
                                    tracing::info!("HTTP gossip with {} yielded {} contacts", target, new_contacts);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("HTTP gossip failed with {}: {}", target, e);
                        }
                    }
                }));
            }
            for handle in handles {
                let _ = handle.await;
            }
        } else {
            tracing::warn!("No discovery targets available (bootstrap_urls empty and no known RPC peers)");
        }

        if self.config.transport_enabled {
            let self_contact = build_self_contact(&self.state, &self.protocol);
            let announce = DiscoveryMessage::announce(self_contact);
            let gossip = DiscoveryMessage::gossip(
                self.state.discovery_sample(self.config.max_gossip_contacts),
            );
            
            if let Err(e) = self.protocol.publish_discovery(announce).await {
                tracing::debug!("Transport discovery announce failed: {}", e);
            }
            if let Err(e) = self.protocol.publish_discovery(gossip).await {
                tracing::debug!("Transport discovery gossip failed: {}", e);
            }
        }
    }
}

#[derive(Clone)]
pub struct LanDiscoveryWorker {
    state: Arc<NodeState>,
    protocol: Arc<ProtocolEngine>,
    config: LanDiscoveryConfig,
}

impl LanDiscoveryWorker {
    pub fn new(
        state: Arc<NodeState>,
        protocol: Arc<ProtocolEngine>,
        config: LanDiscoveryConfig,
    ) -> Self {
        Self {
            state,
            protocol,
            config,
        }
    }

    pub fn start(self, self_contact: ContactBundle) {
        if !self.config.enabled {
            return;
        }
        let config = self.config.clone();
        let state = Arc::clone(&self.state);
        let protocol = Arc::clone(&self.protocol);
        thread::spawn(move || {
            let socket = match UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], config.port))) {
                Ok(sock) => sock,
                Err(_) => return,
            };
            let _ = socket.set_broadcast(true);
            let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));
            let mut last_announce = Instant::now() - config.announce_interval;
            loop {
                if last_announce.elapsed() >= config.announce_interval {
                    let announce = LanAnnounce::from_contact(&self_contact);
                    if let Ok(bytes) = serde_json::to_vec(&announce) {
                        let _ = socket.send_to(
                            &bytes,
                            SocketAddr::from(([255, 255, 255, 255], config.port)),
                        );
                    }
                    last_announce = Instant::now();
                }
                let mut buf = vec![0u8; 2048];
                if let Ok((len, addr)) = socket.recv_from(&mut buf) {
                    if len == 0 {
                        continue;
                    }
                    if let Ok(announce) = serde_json::from_slice::<LanAnnounce>(&buf[..len]) {
                        if announce.peer_id == self_contact.peer_id {
                            continue;
                        }
                        let mut contact = announce.into_contact();
                        contact.lan_addrs.push(addr.to_string());
                        state.add_contact(contact.clone());
                        drop(protocol.add_contact(&contact));
                    }
                }
            }
        });
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LanAnnounce {
    peer_id: String,
    quic_addr: Option<String>,
    ws_url: Option<String>,
    pubkey_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryKind {
    Announce,
    Lookup,
    Response,
    Gossip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryMessage {
    pub kind: DiscoveryKind,
    #[serde(default)]
    pub contact: Option<ContactBundle>,
    #[serde(default)]
    pub contacts: Vec<ContactBundle>,
    #[serde(default)]
    pub target_peer_id: Option<String>,
    #[serde(default)]
    pub target_pubkey: Option<String>,
    #[serde(default)]
    pub reply_to: Option<String>,
    #[serde(default)]
    pub ttl: u8,
}

impl DiscoveryMessage {
    pub fn announce(contact: ContactBundle) -> Self {
        Self {
            kind: DiscoveryKind::Announce,
            contact: Some(contact),
            contacts: Vec::new(),
            target_peer_id: None,
            target_pubkey: None,
            reply_to: None,
            ttl: 1,
        }
    }

    pub fn gossip(mut contacts: Vec<ContactBundle>) -> Self {
        contacts.truncate(DISCOVERY_MAX_CONTACTS);
        Self {
            kind: DiscoveryKind::Gossip,
            contact: None,
            contacts,
            target_peer_id: None,
            target_pubkey: None,
            reply_to: None,
            ttl: 1,
        }
    }

    pub fn lookup(peer_id: Option<String>, pubkey_hex: Option<String>, reply_to: String) -> Self {
        Self {
            kind: DiscoveryKind::Lookup,
            contact: None,
            contacts: Vec::new(),
            target_peer_id: peer_id,
            target_pubkey: pubkey_hex,
            reply_to: Some(reply_to),
            ttl: 1,
        }
    }

    pub fn response(contacts: Vec<ContactBundle>, target_peer_id: Option<String>) -> Self {
        Self {
            kind: DiscoveryKind::Response,
            contact: None,
            contacts,
            target_peer_id,
            target_pubkey: None,
            reply_to: None,
            ttl: 1,
        }
    }
}

pub async fn handle_discovery_payload(
    state: &NodeState,
    protocol: &ProtocolEngine,
    payload: &[u8],
) -> Option<()> {
    if let Ok(msg) = serde_json::from_slice::<DiscoveryMessage>(payload) {
        return handle_discovery_message(state, protocol, msg).await;
    }
    if let Ok(items) = ciborium::de::from_reader::<Vec<Vec<u8>>, _>(payload) {
        let mut handled = false;
        for item in items {
            if let Ok(msg) = serde_json::from_slice::<DiscoveryMessage>(&item) {
                let _ = handle_discovery_message(state, protocol, msg).await;
                handled = true;
            }
        }
        if handled {
            return Some(());
        }
    }
    None
}

async fn handle_discovery_message(
    state: &NodeState,
    protocol: &ProtocolEngine,
    msg: DiscoveryMessage,
) -> Option<()> {
    match msg.kind {
        DiscoveryKind::Announce => {
            let contact = msg.contact?;
            if contact.peer_id == protocol.peer_id() {
                return None;
            }
            state.add_contact(contact.clone());
            protocol.add_contact(&contact).await;
            let neighbors = state.discovery_lookup_contact(&contact, 12);
            let response = DiscoveryMessage::response(neighbors, Some(contact.peer_id));
            let _ = protocol.publish_discovery(response).await;
        }
        DiscoveryKind::Lookup => {
            let limit = 16;
            let reply_to = msg.reply_to.clone().unwrap_or_default();
            let contacts = if let Some(peer_id) = msg.target_peer_id.as_deref() {
                state.discovery_lookup_peer(peer_id, limit)
            } else if let Some(pubkey_hex) = msg.target_pubkey.as_deref() {
                state.discovery_lookup_pubkey(pubkey_hex, limit)
            } else {
                Vec::new()
            };
            if !contacts.is_empty() {
                let response = DiscoveryMessage::response(contacts, Some(reply_to));
                let _ = protocol.publish_discovery(response).await;
            }
        }
        DiscoveryKind::Response => {
            if let Some(target_peer_id) = msg.target_peer_id.as_deref() {
                if target_peer_id != protocol.peer_id() {
                    return None;
                }
            }
            for contact in msg.contacts {
                state.add_contact(contact.clone());
                protocol.add_contact(&contact).await;
            }
        }
        DiscoveryKind::Gossip => {
            for contact in msg.contacts {
                state.add_contact(contact.clone());
                protocol.add_contact(&contact).await;
            }
        }
    }
    Some(())
}

impl LanAnnounce {
    fn from_contact(contact: &ContactBundle) -> Self {
        Self {
            peer_id: contact.peer_id.clone(),
            quic_addr: contact.quic_addr.clone(),
            ws_url: contact.ws_url.clone(),
            pubkey_hex: contact.pubkey_hex.clone(),
        }
    }

    fn into_contact(self) -> ContactBundle {
        ContactBundle {
            peer_id: self.peer_id,
            ws_url: self.ws_url,
            quic_addr: self.quic_addr,
            pubkey_hex: self.pubkey_hex,
            rpc_url: None,
            lan_addrs: Vec::new(),
        }
    }
}

pub fn handle_discovery_announce(
    state: &NodeState,
    request: DiscoveryAnnounceRequest,
    max_neighbors: usize,
) -> DiscoveryAnnounceResponse {
    state.add_contact(request.contact.clone());
    let neighbors = state.discovery_lookup_contact(&request.contact, max_neighbors);
    DiscoveryAnnounceResponse {
        accepted: true,
        neighbors,
    }
}

pub fn handle_discovery_lookup(
    state: &NodeState,
    request: DiscoveryLookupRequest,
) -> DiscoveryLookupResponse {
    let limit = request.limit.unwrap_or(16);
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
    for contact in request.contacts {
        state.add_contact(contact);
    }
    DiscoveryGossipResponse {
        contacts: state.discovery_sample(max_contacts),
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
            if addr.starts_with("0.0.0.0") || addr.starts_with("127.0.0.1") || addr.contains("localhost") {
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

fn contact_key(contact: &ContactBundle) -> [u8; 32] {
    if contact.pubkey_hex.len() == 64 {
        if let Ok(bytes) = hex::decode(&contact.pubkey_hex) {
            if bytes.len() == 32 {
                let mut out = [0u8; 32];
                out.copy_from_slice(&bytes);
                return out;
            }
        }
    }
    blake3::hash(contact.peer_id.as_bytes()).into()
}

fn key_for_peer(peer_id: &str) -> [u8; 32] {
    blake3::hash(peer_id.as_bytes()).into()
}

fn key_for_pubkey(pubkey_hex: &str) -> Option<[u8; 32]> {
    if pubkey_hex.len() != 64 {
        return None;
    }
    hex::decode(pubkey_hex).ok().and_then(|bytes| {
        if bytes.len() == 32 {
            let mut out = [0u8; 32];
            out.copy_from_slice(&bytes);
            Some(out)
        } else {
            None
        }
    })
}

fn xor_distance(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = a[i] ^ b[i];
    }
    out
}

#[derive(Debug)]
pub struct DiscoveryStateHandle {
    table: Arc<Mutex<DiscoveryTable>>,
}

impl DiscoveryStateHandle {
    pub fn new(table: Arc<Mutex<DiscoveryTable>>) -> Self {
        Self { table }
    }

    pub fn upsert(&self, contact: ContactBundle) {
        let mut table = self.table.lock().expect("discovery lock");
        table.upsert(contact);
    }

    pub fn lookup_peer(&self, peer_id: &str, limit: usize) -> Vec<ContactBundle> {
        let key = key_for_peer(peer_id);
        let table = self.table.lock().expect("discovery lock");
        table.lookup(&key, limit)
    }

    pub fn lookup_pubkey(&self, pubkey_hex: &str, limit: usize) -> Vec<ContactBundle> {
        let key = match key_for_pubkey(pubkey_hex) {
            Some(key) => key,
            None => return Vec::new(),
        };
        let table = self.table.lock().expect("discovery lock");
        table.lookup(&key, limit)
    }

    pub fn lookup_contact(&self, contact: &ContactBundle, limit: usize) -> Vec<ContactBundle> {
        let key = contact_key(contact);
        let table = self.table.lock().expect("discovery lock");
        table.lookup(&key, limit)
    }

    pub fn sample(&self, max: usize) -> Vec<ContactBundle> {
        let table = self.table.lock().expect("discovery lock");
        table.sample(max)
    }
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
    fn discovery_lookup_orders_by_distance() {
        let mut table = DiscoveryTable::default();
        table.upsert(make_contact(
            "alpha",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ));
        table.upsert(make_contact(
            "beta",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        ));
        table.upsert(make_contact(
            "gamma",
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        ));
        let key = key_for_peer("alpha");
        let results = table.lookup(&key, 2);
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|c| c.peer_id == "alpha"));
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
}
