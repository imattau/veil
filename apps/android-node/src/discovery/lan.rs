use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::api::ContactBundle;
use crate::protocol::ProtocolEngine;
use crate::state::NodeState;

use super::{sanitize_discovery_contact, LanDiscoveryConfig};

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
        let runtime_handle = tokio::runtime::Handle::try_current().ok();
        if runtime_handle.is_none() {
            tracing::warn!(
                "LAN discovery started without runtime handle; protocol lane sync is disabled"
            );
        }
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
                        if let Some(contact) = sanitize_discovery_contact(contact) {
                            state.add_contact(contact.clone());
                            if let Some(handle) = runtime_handle.as_ref() {
                                let protocol = Arc::clone(&protocol);
                                handle.spawn(async move {
                                    protocol.add_contact(&contact).await;
                                });
                            }
                        }
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
