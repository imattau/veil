#![no_main]

use libfuzzer_sys::fuzz_target;
use veil_codec::object::{decode_object_cbor, decode_object_cbor_prefix};
use veil_codec::shard::decode_shard_cbor;

fuzz_target!(|data: &[u8]| {
    let _ = decode_object_cbor(data);
    let _ = decode_object_cbor_prefix(data);
    let _ = decode_shard_cbor(data);
});
