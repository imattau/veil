#![no_main]

use libfuzzer_sys::fuzz_target;
use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::Ed25519Verifier;
use veil_node::runtime::{pump_once, PumpParams, RuntimePolicyHooks, RuntimeStats};
use veil_node::state::NodeState;
use veil_transport::adapter::InMemoryAdapter;

fuzz_target!(|data: &[u8]| {
    let mut node = NodeState::default();
    let mut adapter = InMemoryAdapter::default();
    adapter.enqueue_inbound("sender", data.to_vec());

    let peers = vec![
        "sender".to_string(),
        "peer-a".to_string(),
        "peer-b".to_string(),
    ];
    let mut stats = RuntimeStats::default();
    let key = [0x11_u8; 32];

    let _ = pump_once(
        &mut node,
        &mut adapter,
        PumpParams {
            peers: &peers,
            now_step: 0,
            ttl_steps: 10,
            fanout: 2,
            policy_hooks: RuntimePolicyHooks::default(),
            decrypt_key: &key,
            stats: &mut stats,
        },
        &XChaCha20Poly1305Cipher,
        &Ed25519Verifier,
    );
});
