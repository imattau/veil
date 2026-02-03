use std::collections::HashMap;
use std::time::Instant;

use rand::seq::SliceRandom;
use rand::{rngs::StdRng, Rng, RngCore, SeedableRng};
use veil_codec::object::{
    encode_object_cbor, object_signature_message_digest, ObjectV1, Signature, OBJECT_FLAG_SIGNED,
    OBJECT_V1_VERSION,
};
use veil_codec::shard::encode_shard_cbor;
use veil_core::hash::blake3_32;
use veil_core::{Epoch, Namespace, ObjectRoot};
use veil_crypto::aead::{build_veil_aad, AeadCipher, XChaCha20Poly1305Cipher};
use veil_crypto::signing::{Ed25519Signer, Ed25519Verifier, Signer};
use veil_fec::sharder::{derive_object_root, object_to_shards};
use veil_node::receive::ReceiveEvent;
use veil_node::runtime::{pump_once, PumpParams, RuntimePolicyHooks, RuntimeStats};
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

struct PumpHarness<'a> {
    node: &'a mut NodeState,
    adapter: &'a mut InMemoryAdapter,
    decrypt_key: &'a [u8; 32],
    stats: &'a mut RuntimeStats,
    expected: &'a mut HashMap<ObjectRoot, Vec<u8>>,
    delivered_count: &'a mut usize,
    delivered_bytes: &'a mut usize,
    step: &'a mut u64,
}

impl PumpHarness<'_> {
    fn run_until_empty(&mut self) {
        let peers = vec![
            "sender".to_string(),
            "peer-a".to_string(),
            "peer-b".to_string(),
        ];
        loop {
            let event = pump_once(
                self.node,
                self.adapter,
                PumpParams {
                    peers: &peers,
                    now_step: *self.step,
                    ttl_steps: 10_000,
                    fanout: 1,
                    policy_hooks: RuntimePolicyHooks::default(),
                    decrypt_key: self.decrypt_key,
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
            if let ReceiveEvent::Delivered {
                object_root,
                payload,
                ..
            } = event
            {
                let expected_payload = self
                    .expected
                    .remove(&object_root)
                    .expect("delivered root should be expected");
                assert_eq!(payload, expected_payload);
                *self.delivered_count += 1;
                *self.delivered_bytes += payload.len();
            }
        }
    }
}

#[test]
fn e2e_performance_smoke() {
    const OBJECTS: usize = 50;

    let mut rng = StdRng::seed_from_u64(0xD15EA5E);
    let signer = Ed25519Signer::from_secret([0x42_u8; 32]);
    let key = [0xA5_u8; 32];
    let tag = [0x11_u8; 32];

    let mut node = NodeState::default();
    node.subscriptions.insert(tag);
    let mut adapter = InMemoryAdapter::default();
    let mut stats = RuntimeStats::default();
    let mut expected = HashMap::<ObjectRoot, Vec<u8>>::new();
    let mut delivered_count = 0usize;
    let mut delivered_bytes = 0usize;
    let mut step = 0u64;

    let start = Instant::now();
    for idx in 0..OBJECTS {
        let payload_len = rng.gen_range(8 * 1024..=96 * 1024);
        let mut payload = vec![0_u8; payload_len];
        rng.fill_bytes(&mut payload);

        let namespace = Namespace((idx % 1024) as u16);
        let epoch = Epoch(10_000 + idx as u32);
        let mut nonce = [0_u8; 24];
        rng.fill_bytes(&mut nonce);

        let encoded =
            build_signed_encrypted_object(&payload, namespace, epoch, tag, &key, nonce, &signer);
        let wire_root = blake3_32(&encoded);
        expected.insert(wire_root, payload);

        let mut shards = object_to_shards(&encoded, namespace, epoch, tag, wire_root)
            .expect("sharding should succeed");
        let k = shards[0].header.k as usize;
        shards.shuffle(&mut rng);

        for shard in shards.iter().take(k) {
            let bytes = encode_shard_cbor(shard).expect("shard encode should succeed");
            adapter.enqueue_inbound("sender", bytes);
        }
        PumpHarness {
            node: &mut node,
            adapter: &mut adapter,
            decrypt_key: &key,
            stats: &mut stats,
            expected: &mut expected,
            delivered_count: &mut delivered_count,
            delivered_bytes: &mut delivered_bytes,
            step: &mut step,
        }
        .run_until_empty();
        let _ = adapter.take_outbound();
    }
    let elapsed = start.elapsed();
    let secs = elapsed.as_secs_f64();
    let throughput_mib_s = (delivered_bytes as f64 / (1024.0 * 1024.0)) / secs.max(1e-9);

    assert_eq!(delivered_count, OBJECTS, "all objects should be delivered");
    assert!(
        expected.is_empty(),
        "all expected deliveries should be consumed"
    );
    assert!(
        throughput_mib_s >= 0.3,
        "throughput too low: {throughput_mib_s:.2} MiB/s over {secs:.2}s",
    );
}

#[test]
fn e2e_load_with_noise_and_duplicates() {
    const OBJECTS: usize = 120;

    let mut rng = StdRng::seed_from_u64(0x10AD_F00D);
    let signer = Ed25519Signer::from_secret([0x99_u8; 32]);
    let key = [0xC3_u8; 32];
    let tag = [0x33_u8; 32];

    let mut node = NodeState::default();
    node.subscriptions.insert(tag);
    let mut adapter = InMemoryAdapter::default();
    let mut stats = RuntimeStats::default();
    let mut expected = HashMap::<ObjectRoot, Vec<u8>>::new();
    let mut delivered_count = 0usize;
    let mut delivered_bytes = 0usize;
    let mut step = 0u64;

    let start = Instant::now();
    for idx in 0..OBJECTS {
        let payload_len = if idx % 3 == 0 {
            rng.gen_range(2 * 1024..=24 * 1024)
        } else if idx % 3 == 1 {
            rng.gen_range(24 * 1024..=110 * 1024)
        } else {
            rng.gen_range(130 * 1024..=210 * 1024)
        };
        let mut payload = vec![0_u8; payload_len];
        rng.fill_bytes(&mut payload);

        let namespace = Namespace((2000 + idx) as u16);
        let epoch = Epoch(20_000 + idx as u32);
        let mut nonce = [0_u8; 24];
        rng.fill_bytes(&mut nonce);

        let encoded =
            build_signed_encrypted_object(&payload, namespace, epoch, tag, &key, nonce, &signer);
        let wire_root = blake3_32(&encoded);
        expected.insert(wire_root, payload);

        let mut shards = object_to_shards(&encoded, namespace, epoch, tag, wire_root)
            .expect("sharding should succeed");
        let k = shards[0].header.k as usize;
        let n = shards[0].header.n as usize;
        shards.shuffle(&mut rng);

        let recv_count = rng.gen_range(k..=n);
        for shard in shards.iter().take(recv_count) {
            let bytes = encode_shard_cbor(shard).expect("shard encode should succeed");
            adapter.enqueue_inbound("sender", bytes.clone());
            if rng.gen_bool(0.12) {
                adapter.enqueue_inbound("sender", bytes);
            }
        }

        if idx % 5 == 0 {
            adapter.enqueue_inbound("sender", vec![0xDE, 0xAD, 0xBE, 0xEF]);
        }

        PumpHarness {
            node: &mut node,
            adapter: &mut adapter,
            decrypt_key: &key,
            stats: &mut stats,
            expected: &mut expected,
            delivered_count: &mut delivered_count,
            delivered_bytes: &mut delivered_bytes,
            step: &mut step,
        }
        .run_until_empty();
        let _ = adapter.take_outbound();
    }

    let elapsed = start.elapsed();
    let secs = elapsed.as_secs_f64();
    let throughput_mib_s = (delivered_bytes as f64 / (1024.0 * 1024.0)) / secs.max(1e-9);

    assert_eq!(delivered_count, OBJECTS, "all objects should be delivered");
    assert!(
        expected.is_empty(),
        "all expected deliveries should be consumed"
    );
    assert!(
        stats.ignored_messages > 0,
        "expected noise/duplicates to produce ignored messages",
    );
    assert!(
        throughput_mib_s >= 0.2,
        "load throughput too low: {throughput_mib_s:.2} MiB/s over {secs:.2}s",
    );
}
