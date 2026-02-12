use std::panic;

use veil_codec::object::{
    decode_object_cbor, decode_object_cbor_prefix, encode_object_cbor, ObjectV1, Signature,
    OBJECT_FLAG_SIGNED, OBJECT_V1_VERSION,
};
use veil_codec::shard::{
    decode_shard_cbor, encode_shard_cbor, ShardErasureMode, ShardHeaderV1, ShardV1,
    SHARD_HEADER_LEN, SHARD_V1_VERSION,
};
use veil_core::{Epoch, Namespace};

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

fn sample_object() -> ObjectV1 {
    ObjectV1 {
        version: OBJECT_V1_VERSION,
        namespace: Namespace(7),
        epoch: Epoch(9),
        flags: OBJECT_FLAG_SIGNED,
        tag: [0x11_u8; 32],
        object_root: [0x22_u8; 32],
        sender_pubkey: Some([0x33_u8; 32]),
        signature: Some(Signature([0x44_u8; 64])),
        nonce: [0x55_u8; 24],
        ciphertext: vec![0x66_u8; 64],
        padding: vec![0x77_u8; 16],
    }
}

fn sample_shard() -> ShardV1 {
    ShardV1 {
        header: ShardHeaderV1 {
            version: SHARD_V1_VERSION,
            namespace: Namespace(7),
            epoch: Epoch(9),
            tag: [0x11_u8; 32],
            object_root: [0x22_u8; 32],
            profile_id: 2,
            erasure_mode: ShardErasureMode::HardenedNonSystematic,
            bucket_size: (16 * 1024) as u32,
            k: 6,
            n: 10,
            index: 0,
        },
        payload: vec![0x88_u8; 16 * 1024 - SHARD_HEADER_LEN],
    }
}

#[test]
fn fuzz_like_random_inputs_do_not_panic_decoders() {
    for i in 0..2000_u64 {
        let len = ((i as usize) * 73) % 2048;
        let data = random_bytes(0xBAD5EED ^ i, len);

        let obj_full = panic::catch_unwind(|| decode_object_cbor(&data));
        assert!(obj_full.is_ok(), "decode_object_cbor panicked at case {i}");

        let obj_prefix = panic::catch_unwind(|| decode_object_cbor_prefix(&data));
        assert!(
            obj_prefix.is_ok(),
            "decode_object_cbor_prefix panicked at case {i}",
        );

        let shard = panic::catch_unwind(|| decode_shard_cbor(&data));
        assert!(shard.is_ok(), "decode_shard_cbor panicked at case {i}");
    }
}

#[test]
fn fuzz_like_mutations_of_valid_vectors_do_not_panic() {
    let mut obj_bytes = encode_object_cbor(&sample_object()).expect("object should encode");
    let mut shard_bytes = encode_shard_cbor(&sample_shard()).expect("shard should encode");

    for i in 0..512_usize {
        let idx_obj = i % obj_bytes.len();
        obj_bytes[idx_obj] ^= (i as u8).wrapping_mul(31).wrapping_add(1);
        let data_obj = obj_bytes.clone();

        let obj_full = panic::catch_unwind(|| decode_object_cbor(&data_obj));
        assert!(
            obj_full.is_ok(),
            "decode_object_cbor panicked for mutated object at case {i}",
        );
        let obj_prefix = panic::catch_unwind(|| decode_object_cbor_prefix(&data_obj));
        assert!(
            obj_prefix.is_ok(),
            "decode_object_cbor_prefix panicked for mutated object at case {i}",
        );

        let idx_shard = i % shard_bytes.len();
        shard_bytes[idx_shard] ^= (i as u8).wrapping_mul(17).wrapping_add(3);
        let data_shard = shard_bytes.clone();
        let shard = panic::catch_unwind(|| decode_shard_cbor(&data_shard));
        assert!(
            shard.is_ok(),
            "decode_shard_cbor panicked for mutated shard at case {i}",
        );
    }
}
