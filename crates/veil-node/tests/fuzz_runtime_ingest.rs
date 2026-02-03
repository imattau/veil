use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::Ed25519Verifier;
use veil_node::runtime::{pump_once, PumpParams, RuntimePolicyHooks, RuntimeStats};
use veil_node::state::NodeState;
use veil_transport::adapter::InMemoryAdapter;

fn xorshift64(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn random_bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut s = seed.max(1);
    let mut out = vec![0_u8; len];
    for b in &mut out {
        *b = (xorshift64(&mut s) & 0xFF) as u8;
    }
    out
}

#[test]
fn fuzz_like_runtime_ingest_does_not_panic() {
    let mut node = NodeState::default();
    let mut adapter = InMemoryAdapter::default();
    let peers = vec![
        "sender".to_string(),
        "peer-a".to_string(),
        "peer-b".to_string(),
    ];
    let mut stats = RuntimeStats::default();
    let key = [0x11_u8; 32];

    for i in 0..1500_u64 {
        let len = ((i as usize) * 37) % 4096;
        adapter.enqueue_inbound("sender", random_bytes(0xC0DEC0DE ^ i, len));

        let _ = pump_once(
            &mut node,
            &mut adapter,
            PumpParams {
                peers: &peers,
                now_step: i,
                ttl_steps: 50,
                fanout: 2,
                policy_hooks: RuntimePolicyHooks::default(),
                decrypt_key: &key,
                stats: &mut stats,
            },
            &XChaCha20Poly1305Cipher,
            &Ed25519Verifier,
        )
        .expect("runtime ingest should not fail for arbitrary payloads");
    }
}
