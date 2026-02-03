use rand::seq::SliceRandom;
use rand::{rngs::StdRng, SeedableRng};
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

fn main() {
    let app_payload = b"VEIL e2e: encrypt -> sign -> shard -> reconstruct -> verify -> decrypt";
    let payload = app_payload.to_vec();
    let namespace = Namespace(42);
    let epoch = Epoch(123_456);
    let tag = [0x11_u8; 32];
    let payload_root = derive_object_root(&payload);

    // Encrypt payload with VEIL AAD binding.
    let cipher = XChaCha20Poly1305Cipher;
    let aead_key = [0xA5_u8; 32];
    let nonce = [0x24_u8; 24];
    let aad = build_veil_aad(tag, namespace, epoch);
    let encrypted = cipher
        .encrypt(&aead_key, nonce, &aad, &payload)
        .expect("payload encryption should succeed");

    // Build signed object (placeholder signature first to satisfy strict validation).
    let signer = Ed25519Signer::from_secret([0x42_u8; 32]);
    let mut object = ObjectV1 {
        version: OBJECT_V1_VERSION,
        namespace,
        epoch,
        flags: OBJECT_FLAG_SIGNED,
        tag,
        object_root: payload_root,
        sender_pubkey: Some(signer.public_key()),
        signature: Some(Signature([0_u8; 64])),
        nonce: encrypted.nonce,
        ciphertext: encrypted.ciphertext,
        padding: vec![0_u8; 16],
    };
    let digest = object_signature_message_digest(&object).expect("digest should compute");
    let sig = signer.sign(&digest).expect("signature should succeed");
    object.signature = Some(Signature(sig));

    let encoded_object = encode_object_cbor(&object).expect("object should encode");
    let wire_root = derive_object_root(&encoded_object);

    let shards = object_to_shards(&encoded_object, namespace, epoch, tag, wire_root)
        .expect("object should shard");
    let k = shards[0].header.k as usize;
    let n = shards[0].header.n as usize;

    // Simulate subscriber receiving exactly k shards in random order.
    let mut rng = StdRng::seed_from_u64(7);
    let mut shuffled = shards.clone();
    shuffled.shuffle(&mut rng);
    let selected = shuffled[..k].to_vec();

    let mut node = NodeState::default();
    node.subscriptions.insert(tag);
    let verifier = Ed25519Verifier;
    let mut delivered_payload: Option<Vec<u8>> = None;
    for (i, shard) in selected.iter().enumerate() {
        let event = receive_shard(
            &mut node, shard, i as u64, 300, &aead_key, &cipher, &verifier,
        )
        .expect("node receive should succeed");
        if let ReceiveEvent::Delivered { payload, .. } = event {
            delivered_payload = Some(payload);
        }
    }
    let signature_verified = delivered_payload.is_some();
    let delivered_payload = delivered_payload.expect("delivery expected after k shards");

    println!("VEIL end-to-end demo");
    println!("payload bytes: {}", payload.len());
    println!("encoded object bytes: {}", encoded_object.len());
    println!("profile: k={}, n={}", k, n);
    println!("selected shards: {}", selected.len());
    println!("signature verified: {}", signature_verified);
    println!("decryption ok: {}", delivered_payload == payload);
}
