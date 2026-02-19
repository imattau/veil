use std::collections::HashMap;

use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::NostrVerifier;
use veil_node::config::NodeRuntimeConfig;
use veil_node::receive::ReceiveEvent;
use veil_node::runtime::{
    pump_multi_lane_tick_with_config_resolvers_split, ConfigMultiLanePumpParams, RuntimeStats,
};
use veil_node::service::PublisherRuntime;

use super::ProtocolRuntime;

pub(super) fn pump_inbound_once(
    runtime: &mut ProtocolRuntime,
    cfg: &NodeRuntimeConfig,
    dynamic: &HashMap<String, [u8; 32]>,
    fast_peers: &[String],
    fallback_peers: &[String],
    now_step: u64,
    stats: &mut RuntimeStats,
    verifier: &NostrVerifier,
) -> Result<Option<ReceiveEvent>, String> {
    let PublisherRuntime {
        state,
        fast_adapter,
        fallback_adapter,
        encrypt_key,
        ..
    } = runtime;
    let resolver = |peer: &String| {
        dynamic
            .get(peer)
            .copied()
            .or_else(|| cfg.publisher_for_peer(peer))
    };
    pump_multi_lane_tick_with_config_resolvers_split(
        state,
        fast_adapter,
        fallback_adapter,
        ConfigMultiLanePumpParams {
            fast_peers,
            fallback_peers,
            now_step,
            decrypt_key: encrypt_key,
            config: cfg,
            stats,
        },
        &resolver,
        &resolver,
        &XChaCha20Poly1305Cipher,
        verifier,
    )
    .map_err(|e| e.to_string())
}
