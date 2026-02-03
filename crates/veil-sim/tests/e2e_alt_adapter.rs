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
use veil_node::receive::ReceiveEvent;
use veil_node::runtime::{pump_once, PumpParams, RuntimePolicyHooks, RuntimeStats};
use veil_node::state::NodeState;
use veil_transport::adapter::CappedInMemoryAdapter;

fn build_signed_encrypted_object(
    payload: &[u8],
    namespace: Namespace,
    epoch: Epoch,
    tag: [u8; 32],
    key: &[u8; 32],
) -> Vec<u8> {
    let nonce = [0x55_u8; 24];
    let signer = Ed25519Signer::from_secret([0x42_u8; 32]);
    let aad = build_veil_aad(tag, namespace, epoch);
    let env = XChaCha20Poly1305Cipher
        .encrypt(key, nonce, &aad, payload)
        .expect("encryption should succeed");
    let mut obj = ObjectV1 {
        version: OBJECT_V1_VERSION,
        namespace,
        epoch,
        flags: OBJECT_FLAG_SIGNED,
        tag,
        object_root: derive_object_root(payload),
        sender_pubkey: Some(signer.public_key()),
        signature: Some(Signature([0_u8; 64])),
        nonce: env.nonce,
        ciphertext: env.ciphertext,
        padding: vec![0_u8; 8],
    };
    let digest = object_signature_message_digest(&obj).expect("digest should compute");
    obj.signature = Some(Signature(
        signer.sign(&digest).expect("signature should succeed"),
    ));
    encode_object_cbor(&obj).expect("object should encode")
}

#[test]
fn e2e_alt_adapter_pipeline_delivers() {
    let mut node = NodeState::default();
    let key = [0xA5_u8; 32];
    let tag = [0x11_u8; 32];
    node.subscriptions.insert(tag);

    let payload = b"alt adapter transport path".to_vec();
    let encoded = build_signed_encrypted_object(&payload, Namespace(7), Epoch(8), tag, &key);
    let root = blake3_32(&encoded);
    let shards =
        object_to_shards(&encoded, Namespace(7), Epoch(8), tag, root).expect("sharding works");
    let k = shards[0].header.k as usize;

    let mut adapter = CappedInMemoryAdapter::with_max_send_bytes(128 * 1024);
    adapter.set_payload_hint(Some(64 * 1024));
    for shard in shards.iter().take(k) {
        adapter.enqueue_inbound(
            "sender",
            encode_shard_cbor(shard).expect("shard encode should succeed"),
        );
    }

    let peers = vec![
        "sender".to_string(),
        "peer-a".to_string(),
        "peer-b".to_string(),
    ];
    let mut stats = RuntimeStats::default();
    let mut delivered = false;
    for step in 0..k {
        let event = pump_once(
            &mut node,
            &mut adapter,
            PumpParams {
                peers: &peers,
                now_step: step as u64,
                ttl_steps: 50,
                fanout: 1,
                policy_hooks: RuntimePolicyHooks::default(),
                decrypt_key: &key,
                stats: &mut stats,
            },
            &XChaCha20Poly1305Cipher,
            &Ed25519Verifier,
        )
        .expect("pump should succeed");

        if let Some(ReceiveEvent::Delivered { payload: got, .. }) = event {
            assert_eq!(got, payload);
            delivered = true;
            break;
        }
    }

    assert!(delivered, "expected delivery through capped adapter");
    assert!(stats.parsed_shards > 0);
}
