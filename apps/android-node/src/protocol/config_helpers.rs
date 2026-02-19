use std::time::{SystemTime, UNIX_EPOCH};

use veil_core::{Epoch, Namespace};
use veil_crypto::signing::NostrSigner;
use veil_node::config::{BloomExchangeConfig, NodeRuntimeConfig, ProbabilisticForwardingConfig};

use super::ProtocolConfig;

pub(super) fn build_default_protocol_config(
    ws_url: String,
    peer_id: String,
    namespace: u16,
    identity_pubkey: [u8; 32],
    encrypt_key: [u8; 32],
    signer: NostrSigner,
) -> ProtocolConfig {
    let mut cfg = NodeRuntimeConfig::default();
    cfg.probabilistic_forwarding = ProbabilisticForwardingConfig {
        enabled: true,
        min_probability: 0.20,
        replica_divisor: 8,
    };
    cfg.bloom_exchange = BloomExchangeConfig {
        enabled: true,
        interval_steps: 192,
        false_positive_rate: 0.05,
    };
    ProtocolConfig {
        ws_url: Some(ws_url),
        quic_bind_addr: "0.0.0.0:0".to_string(),
        quic_server_name: None,
        quic_trusted_certs: Vec::new(),
        tor_socks: None,
        peer_id,
        namespace: Namespace(namespace),
        discovery_namespace: Namespace(4096),
        encrypt_key,
        identity_pubkey,
        signer,
        fast_peers: Vec::new(),
        fallback_peers: Vec::new(),
        runtime_config: cfg,
        cache_state_path: None,
    }
}

pub(super) fn current_epoch() -> Epoch {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Epoch((now / 86_400) as u32)
}

pub(super) fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    <[u8; 32] as hex::FromHex>::from_hex(value).ok()
}
