use std::fs;

use rand::{rngs::StdRng, RngCore, SeedableRng};
use veil_codec::object::{
    encode_object_cbor, object_signature_message_digest, ObjectV1, Signature, OBJECT_FLAG_SIGNED,
    OBJECT_V1_VERSION,
};
use veil_codec::shard::encode_shard_cbor;
use veil_core::hash::blake3_32;
use veil_core::{Epoch, Namespace};
use veil_crypto::aead::{build_veil_aad, AeadCipher, XChaCha20Poly1305Cipher};
use veil_crypto::signing::{Ed25519Signer, Ed25519Verifier, Signer};
use veil_fec::sharder::{derive_object_root, object_to_shards};
use veil_node::config::NodeRuntimeConfig;
use veil_node::receive::ReceiveEvent;
use veil_node::runtime::{pump_once_with_config, ConfigPumpParams, RuntimeStats};
use veil_node::state::NodeState;
use veil_transport::adapter::InMemoryAdapter;

fn build_signed_encrypted_object(
    payload: &[u8],
    namespace: Namespace,
    epoch: Epoch,
    tag: [u8; 32],
    key: &[u8; 32],
    nonce: [u8; 24],
    signer: &Ed25519Signer,
) -> Vec<u8> {
    let cipher = XChaCha20Poly1305Cipher;
    let aad = build_veil_aad(tag, namespace, epoch);
    let envelope = cipher
        .encrypt(key, nonce, &aad, payload)
        .expect("encryption should succeed");

    let mut object = ObjectV1 {
        version: OBJECT_V1_VERSION,
        namespace,
        epoch,
        flags: OBJECT_FLAG_SIGNED,
        tag,
        object_root: derive_object_root(payload),
        sender_pubkey: Some(signer.public_key()),
        signature: Some(Signature([0_u8; 64])),
        nonce: envelope.nonce,
        ciphertext: envelope.ciphertext,
        padding: vec![0_u8; 16],
    };
    let digest = object_signature_message_digest(&object).expect("digest should compute");
    object.signature = Some(Signature(
        signer.sign(&digest).expect("signature should succeed"),
    ));
    encode_object_cbor(&object).expect("object should encode")
}

fn read_rss_bytes_linux() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
    let kb = line.split_whitespace().nth(1)?.parse::<u64>().ok()?;
    Some(kb * 1024)
}

struct BatchCtx<'a> {
    node: &'a mut NodeState,
    adapter: &'a mut InMemoryAdapter,
    config: &'a NodeRuntimeConfig,
    key: &'a [u8; 32],
    tag: [u8; 32],
    signer: &'a Ed25519Signer,
    rng: &'a mut StdRng,
    step: &'a mut u64,
    stats: &'a mut RuntimeStats,
}

impl BatchCtx<'_> {
    fn run_batch(&mut self, objects: usize, start_idx: usize) -> usize {
        let peers = vec![
            "sender".to_string(),
            "peer-a".to_string(),
            "peer-b".to_string(),
        ];
        let mut delivered = 0usize;

        for i in 0..objects {
            let idx = start_idx + i;
            let payload_len = match idx % 3 {
                0 => 24 * 1024,
                1 => 64 * 1024,
                _ => 120 * 1024,
            };
            let payload = vec![(idx % 251) as u8; payload_len];

            let namespace = Namespace((idx % 2048) as u16);
            let epoch = Epoch(30_000 + idx as u32);
            let mut nonce = [0_u8; 24];
            self.rng.fill_bytes(&mut nonce);

            let encoded = build_signed_encrypted_object(
                &payload,
                namespace,
                epoch,
                self.tag,
                self.key,
                nonce,
                self.signer,
            );
            let wire_root = blake3_32(&encoded);
            let shards = object_to_shards(&encoded, namespace, epoch, self.tag, wire_root)
                .expect("sharding should succeed");
            let k = shards[0].header.k as usize;

            for shard in shards.iter().take(k) {
                let bytes = encode_shard_cbor(shard).expect("shard encode should succeed");
                self.adapter.enqueue_inbound("sender", bytes);
            }

            loop {
                let event = pump_once_with_config(
                    self.node,
                    self.adapter,
                    ConfigPumpParams {
                        peers: &peers,
                        now_step: *self.step,
                        decrypt_key: self.key,
                        config: self.config,
                        stats: self.stats,
                    },
                    &XChaCha20Poly1305Cipher,
                    &Ed25519Verifier,
                )
                .expect("pump should run");
                *self.step += 1;
                let Some(event) = event else {
                    break;
                };
                if matches!(event, ReceiveEvent::Delivered { .. }) {
                    delivered += 1;
                }
            }
            let _ = self.adapter.take_outbound();
        }

        delivered
    }
}

#[test]
fn e2e_memory_leak_guard_under_sustained_load() {
    let Some(rss_start) = read_rss_bytes_linux() else {
        eprintln!("skipping memory leak guard: /proc/self/status VmRSS unavailable");
        return;
    };

    const WARMUP_OBJECTS: usize = 80;
    const PHASE_OBJECTS: usize = 180;
    const MAX_PHASE2_RSS_GROWTH_BYTES: u64 = 24 * 1024 * 1024;

    let mut rng = StdRng::seed_from_u64(0x0A11_CE2E);
    let signer = Ed25519Signer::from_secret([0x44_u8; 32]);
    let key = [0xAB_u8; 32];
    let tag = [0x7E_u8; 32];

    let mut node = NodeState::default();
    node.subscriptions.insert(tag);
    let mut adapter = InMemoryAdapter::default();
    let mut stats = RuntimeStats::default();
    let mut step = 0_u64;

    let mut config = NodeRuntimeConfig::default();
    config.max_cache_shards = 512;
    config.ttl_steps = 128;

    let mut batch = BatchCtx {
        node: &mut node,
        adapter: &mut adapter,
        config: &config,
        key: &key,
        tag,
        signer: &signer,
        rng: &mut rng,
        step: &mut step,
        stats: &mut stats,
    };

    let warmup_delivered = batch.run_batch(WARMUP_OBJECTS, 0);
    assert_eq!(warmup_delivered, WARMUP_OBJECTS);
    let rss_warm = read_rss_bytes_linux().unwrap_or(rss_start);

    let phase1_delivered = batch.run_batch(PHASE_OBJECTS, WARMUP_OBJECTS);
    assert_eq!(phase1_delivered, PHASE_OBJECTS);
    let rss_after_phase1 = read_rss_bytes_linux().unwrap_or(rss_warm);

    let phase2_delivered = batch.run_batch(PHASE_OBJECTS, WARMUP_OBJECTS + PHASE_OBJECTS);
    assert_eq!(phase2_delivered, PHASE_OBJECTS);
    let rss_after_phase2 = read_rss_bytes_linux().unwrap_or(rss_after_phase1);

    let phase1_growth = rss_after_phase1.saturating_sub(rss_warm);
    let phase2_growth = rss_after_phase2.saturating_sub(rss_after_phase1);
    assert!(
        phase2_growth <= MAX_PHASE2_RSS_GROWTH_BYTES,
        "possible leak: phase2 RSS grew by {} MiB (phase1 grew by {} MiB)",
        phase2_growth / (1024 * 1024),
        phase1_growth / (1024 * 1024),
    );

    assert!(
        node.inbox.is_empty(),
        "reconstruction inbox should be drained"
    );
    assert!(
        node.pending_acks.is_empty(),
        "pending ACK state should be bounded"
    );
    assert!(
        node.cache.len() <= config.max_cache_shards,
        "cache exceeded configured bound: {} > {}",
        node.cache.len(),
        config.max_cache_shards,
    );
}
