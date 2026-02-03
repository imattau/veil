use veil_codec::object::{
    decode_object_cbor, encode_object_cbor, object_signature_message_digest, ObjectV1, Signature,
    OBJECT_FLAG_ACK_REQUESTED, OBJECT_FLAG_SIGNED, OBJECT_V1_VERSION,
};
use veil_codec::shard::{
    decode_shard_cbor, encode_shard_cbor, ShardHeaderV1, ShardV1, SHARD_HEADER_LEN,
    SHARD_V1_VERSION,
};
use veil_core::hash::blake3_32;
use veil_core::{Epoch, Namespace};

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn read_vector(name: &str) -> String {
    let path = format!("{}/tests/vectors/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path)
        .expect("vector file must exist")
        .trim()
        .to_string()
}

fn sample_object() -> ObjectV1 {
    ObjectV1 {
        version: OBJECT_V1_VERSION,
        namespace: Namespace(42),
        epoch: Epoch(123_456),
        flags: OBJECT_FLAG_SIGNED | OBJECT_FLAG_ACK_REQUESTED,
        tag: [0x11_u8; 32],
        object_root: [0x22_u8; 32],
        sender_pubkey: Some([0xAA_u8; 32]),
        signature: Some(Signature([0xBB_u8; 64])),
        nonce: [0x33_u8; 24],
        ciphertext: vec![0x44_u8; 48],
        padding: vec![0x55_u8; 16],
    }
}

fn sample_shard() -> ShardV1 {
    ShardV1 {
        header: ShardHeaderV1 {
            version: SHARD_V1_VERSION,
            namespace: Namespace(42),
            epoch: Epoch(123_456),
            tag: [0x11_u8; 32],
            object_root: [0x22_u8; 32],
            k: 6,
            n: 10,
            index: 2,
        },
        payload: vec![0x77_u8; 16 * 1024 - SHARD_HEADER_LEN],
    }
}

#[test]
fn golden_object_cbor_vector_matches() {
    let encoded = encode_object_cbor(&sample_object()).expect("object should encode");
    let hex = to_hex(&encoded);
    let expected = read_vector("object_v1_cbor.hex");
    assert_eq!(
        hex, expected,
        "update tests/vectors/object_v1_cbor.hex to: {hex}"
    );
}

#[test]
fn golden_shard_cbor_vector_matches() {
    let encoded = encode_shard_cbor(&sample_shard()).expect("shard should encode");
    let digest_hex = to_hex(&blake3_32(&encoded));
    let expected_len = read_vector("shard_v1_cbor.len")
        .parse::<usize>()
        .expect("length vector must parse as usize");
    let expected_digest = read_vector("shard_v1_cbor.blake3hex");
    assert_eq!(
        encoded.len(),
        expected_len,
        "update tests/vectors/shard_v1_cbor.len to: {}",
        encoded.len()
    );
    assert_eq!(
        digest_hex, expected_digest,
        "update tests/vectors/shard_v1_cbor.blake3hex to: {digest_hex}"
    );
}

#[test]
fn object_round_trip_is_lossless() {
    let obj = sample_object();
    let encoded = encode_object_cbor(&obj).expect("object should encode");
    let decoded = decode_object_cbor(&encoded).expect("object should decode");
    assert_eq!(decoded, obj);
}

#[test]
fn shard_round_trip_is_lossless() {
    let shard = sample_shard();
    let encoded = encode_shard_cbor(&shard).expect("shard should encode");
    let decoded = decode_shard_cbor(&encoded).expect("shard should decode");
    assert_eq!(decoded, shard);
}

#[test]
fn object_rejects_signed_flag_without_signature_fields() {
    let mut obj = sample_object();
    obj.sender_pubkey = None;
    obj.signature = None;

    let err = encode_object_cbor(&obj).expect_err("signed object must include signature fields");
    assert!(
        err.to_string().contains("requires sender_pubkey"),
        "unexpected error: {err}"
    );
}

#[test]
fn shard_rejects_invalid_bucket_size() {
    let mut shard = sample_shard();
    shard.payload.pop();

    let err = encode_shard_cbor(&shard).expect_err("invalid bucket size should fail");
    assert!(
        err.to_string().contains("allowed bucket size"),
        "unexpected error: {err}"
    );
}

#[test]
fn signature_message_digest_is_deterministic() {
    let obj = sample_object();
    let a = object_signature_message_digest(&obj).expect("digest should build");
    let b = object_signature_message_digest(&obj).expect("digest should build");
    assert_eq!(a, b);
}

#[test]
fn signature_message_digest_changes_when_ciphertext_changes() {
    let mut obj = sample_object();
    let original = object_signature_message_digest(&obj).expect("digest should build");
    obj.ciphertext[0] ^= 0x01;
    let updated = object_signature_message_digest(&obj).expect("digest should build");
    assert_ne!(original, updated);
}
