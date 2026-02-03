use rand::seq::SliceRandom;
use rand::{rngs::StdRng, Rng, RngCore, SeedableRng};
use veil_codec::object::{
    encode_object_cbor, object_signature_message_digest, ObjectV1, Signature, OBJECT_FLAG_SIGNED,
    OBJECT_V1_VERSION,
};
use veil_core::{Epoch, Namespace};
use veil_crypto::aead::{build_veil_aad, AeadCipher, XChaCha20Poly1305Cipher};
use veil_crypto::signing::{Ed25519Signer, Ed25519Verifier, Signer};
use veil_fec::sharder::{derive_object_root, object_to_shards};
use veil_node::receive::{receive_shard, ReceiveEvent};
use veil_node::state::NodeState;

fn build_signed_encrypted_object(
    payload: &[u8],
    namespace: Namespace,
    epoch: Epoch,
    tag: [u8; 32],
    aead_key: &[u8; 32],
    nonce: [u8; 24],
    signer: &Ed25519Signer,
) -> Vec<u8> {
    let cipher = XChaCha20Poly1305Cipher;
    let aad = build_veil_aad(tag, namespace, epoch);
    let envelope = cipher
        .encrypt(aead_key, nonce, &aad, payload)
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

#[test]
fn stress_e2e_delivers_with_random_loss_when_at_least_k_shards_arrive() {
    const CASES: usize = 24;

    let mut rng = StdRng::seed_from_u64(0x5EED_CAFE);
    let signer = Ed25519Signer::from_secret([0x42_u8; 32]);
    let verifier = Ed25519Verifier;
    let cipher = XChaCha20Poly1305Cipher;
    let key = [0xA5_u8; 32];

    let mut saw_small_profile = false;
    let mut saw_large_profile = false;

    for case_idx in 0..CASES {
        let payload_len = if case_idx % 2 == 0 {
            rng.gen_range(64..=110_000)
        } else {
            rng.gen_range(130_000..=220_000)
        };

        let mut payload = vec![0_u8; payload_len];
        rng.fill_bytes(&mut payload);

        let namespace = Namespace((case_idx % 1024) as u16);
        let epoch = Epoch(1_000 + case_idx as u32);
        let mut tag = [0_u8; 32];
        rng.fill_bytes(&mut tag);
        let mut nonce = [0_u8; 24];
        rng.fill_bytes(&mut nonce);

        let encoded_object =
            build_signed_encrypted_object(&payload, namespace, epoch, tag, &key, nonce, &signer);
        let wire_root = derive_object_root(&encoded_object);
        let mut shards = object_to_shards(&encoded_object, namespace, epoch, tag, wire_root)
            .expect("sharding should succeed");

        let k = shards[0].header.k as usize;
        let n = shards[0].header.n as usize;
        if n == 10 {
            saw_small_profile = true;
        }
        if n == 16 {
            saw_large_profile = true;
        }

        shards.shuffle(&mut rng);
        let recv_count = rng.gen_range(k..=n);
        let selected = &shards[..recv_count];

        let mut node = NodeState::default();
        node.subscriptions.insert(tag);

        let mut delivered: Option<Vec<u8>> = None;
        for (step, shard) in selected.iter().enumerate() {
            let event = receive_shard(
                &mut node,
                shard,
                step as u64,
                1_000,
                &key,
                &cipher,
                &verifier,
            )
            .expect("valid flow should not error");
            if let ReceiveEvent::Delivered { payload, .. } = event {
                delivered = Some(payload);
                break;
            }
        }

        assert_eq!(
            delivered.as_deref(),
            Some(payload.as_slice()),
            "case {case_idx}: expected delivery with >=k shards",
        );
    }

    assert!(saw_small_profile, "expected at least one small-profile run");
    assert!(saw_large_profile, "expected at least one large-profile run");
}

#[test]
fn stress_e2e_rejects_tampered_shards() {
    const CASES: usize = 12;

    let mut rng = StdRng::seed_from_u64(0xBAD5_1A9E);
    let signer = Ed25519Signer::from_secret([0x99_u8; 32]);
    let verifier = Ed25519Verifier;
    let cipher = XChaCha20Poly1305Cipher;
    let key = [0xC3_u8; 32];

    for case_idx in 0..CASES {
        let payload_len = rng.gen_range(1_024..=32_768);
        let mut payload = vec![0_u8; payload_len];
        rng.fill_bytes(&mut payload);

        let namespace = Namespace((2000 + case_idx) as u16);
        let epoch = Epoch(9_000 + case_idx as u32);
        let mut tag = [0_u8; 32];
        rng.fill_bytes(&mut tag);
        let mut nonce = [0_u8; 24];
        rng.fill_bytes(&mut nonce);

        let encoded_object =
            build_signed_encrypted_object(&payload, namespace, epoch, tag, &key, nonce, &signer);
        let wire_root = derive_object_root(&encoded_object);
        let shards = object_to_shards(&encoded_object, namespace, epoch, tag, wire_root)
            .expect("sharding should succeed");

        let k = shards[0].header.k as usize;
        // Use systematic data shards to ensure tampering impacts reconstructed bytes.
        let mut selected = shards[..k].to_vec();

        let chunk_len = selected[0].payload.len();
        let global_offset = rng.gen_range(0..encoded_object.len());
        let victim = global_offset / chunk_len;
        let payload_idx = global_offset % chunk_len;
        selected[victim].payload[payload_idx] ^= 0x01;

        let mut node = NodeState::default();
        node.subscriptions.insert(tag);

        let mut errored = false;
        let mut delivered = false;
        for (step, shard) in selected.iter().enumerate() {
            match receive_shard(
                &mut node,
                shard,
                step as u64,
                1_000,
                &key,
                &cipher,
                &verifier,
            ) {
                Ok(ReceiveEvent::Delivered { .. }) => {
                    delivered = true;
                    break;
                }
                Ok(_) => {}
                Err(_) => {
                    errored = true;
                    break;
                }
            }
        }

        assert!(
            errored && !delivered,
            "case {case_idx}: tampered shard must not be delivered",
        );
    }
}
