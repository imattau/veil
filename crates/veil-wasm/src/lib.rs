use serde::Serialize;
use veil_codec::object::{
    decode_object_cbor, OBJECT_FLAG_ACK_REQUESTED, OBJECT_FLAG_BATCHED, OBJECT_FLAG_PUBLIC,
    OBJECT_FLAG_SIGNED,
};
use veil_codec::shard::decode_shard_cbor;
use veil_core::tags::{derive_feed_tag, derive_rv_tag};
use veil_core::{Epoch, Namespace};
use wasm_bindgen::prelude::*;

fn parse_32(name: &str, bytes: &[u8]) -> Result<[u8; 32], JsValue> {
    if bytes.len() != 32 {
        return Err(JsValue::from_str(&format!(
            "{name} must be 32 bytes, got {}",
            bytes.len()
        )));
    }
    let mut out = [0_u8; 32];
    out.copy_from_slice(bytes);
    Ok(out)
}

fn bytes_to_hex_inner(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShardMeta {
    version: u16,
    namespace: u16,
    epoch: u32,
    tag_hex: String,
    object_root_hex: String,
    k: u16,
    n: u16,
    index: u16,
    payload_len: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ObjectMeta {
    version: u16,
    namespace: u16,
    epoch: u32,
    flags: u16,
    signed: bool,
    public: bool,
    ack_requested: bool,
    batched: bool,
    tag_hex: String,
    object_root_hex: String,
    sender_pubkey_hex: Option<String>,
    nonce_hex: String,
    ciphertext_len: usize,
    padding_len: usize,
}

/// Derive a stable public feed tag from publisher pubkey + namespace.
#[wasm_bindgen(js_name = deriveFeedTag)]
pub fn derive_feed_tag_wasm(publisher_pubkey: &[u8], namespace: u16) -> Result<Vec<u8>, JsValue> {
    let pubkey = parse_32("publisher_pubkey", publisher_pubkey)?;
    Ok(derive_feed_tag(&pubkey, Namespace(namespace)).to_vec())
}

/// Derive a rotating private rendezvous tag from recipient pubkey + epoch + namespace.
#[wasm_bindgen(js_name = deriveRvTag)]
pub fn derive_rv_tag_wasm(
    recipient_pubkey: &[u8],
    epoch: u32,
    namespace: u16,
) -> Result<Vec<u8>, JsValue> {
    let pubkey = parse_32("recipient_pubkey", recipient_pubkey)?;
    Ok(derive_rv_tag(&pubkey, Epoch(epoch), Namespace(namespace)).to_vec())
}

/// Decode `ShardV1` CBOR and return metadata as a JS object.
#[wasm_bindgen(js_name = decodeShardMeta)]
pub fn decode_shard_meta_wasm(bytes: &[u8]) -> Result<JsValue, JsValue> {
    let shard = decode_shard_cbor(bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let header = shard.header;
    let meta = ShardMeta {
        version: header.version,
        namespace: header.namespace.0,
        epoch: header.epoch.0,
        tag_hex: bytes_to_hex_inner(&header.tag),
        object_root_hex: bytes_to_hex_inner(&header.object_root),
        k: header.k,
        n: header.n,
        index: header.index,
        payload_len: shard.payload.len(),
    };
    serde_wasm_bindgen::to_value(&meta).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Decode `ObjectV1` CBOR and return metadata as a JS object.
#[wasm_bindgen(js_name = decodeObjectMeta)]
pub fn decode_object_meta_wasm(bytes: &[u8]) -> Result<JsValue, JsValue> {
    let object = decode_object_cbor(bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let flags = object.flags;
    let meta = ObjectMeta {
        version: object.version,
        namespace: object.namespace.0,
        epoch: object.epoch.0,
        flags,
        signed: (flags & OBJECT_FLAG_SIGNED) != 0,
        public: (flags & OBJECT_FLAG_PUBLIC) != 0,
        ack_requested: (flags & OBJECT_FLAG_ACK_REQUESTED) != 0,
        batched: (flags & OBJECT_FLAG_BATCHED) != 0,
        tag_hex: bytes_to_hex_inner(&object.tag),
        object_root_hex: bytes_to_hex_inner(&object.object_root),
        sender_pubkey_hex: object.sender_pubkey.map(|k| bytes_to_hex_inner(&k)),
        nonce_hex: bytes_to_hex_inner(&object.nonce),
        ciphertext_len: object.ciphertext.len(),
        padding_len: object.padding.len(),
    };
    serde_wasm_bindgen::to_value(&meta).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Validate shard CBOR bytes.
#[wasm_bindgen(js_name = validateShardCbor)]
pub fn validate_shard_cbor(bytes: &[u8]) -> Result<bool, JsValue> {
    decode_shard_cbor(bytes)
        .map(|_| true)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Validate object CBOR bytes.
#[wasm_bindgen(js_name = validateObjectCbor)]
pub fn validate_object_cbor(bytes: &[u8]) -> Result<bool, JsValue> {
    decode_object_cbor(bytes)
        .map(|_| true)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Calculate epoch number from wall-clock seconds and epoch window.
#[wasm_bindgen(js_name = currentEpoch)]
pub fn current_epoch(now_seconds: u64, epoch_seconds: u32) -> Result<u32, JsValue> {
    if epoch_seconds == 0 {
        return Err(JsValue::from_str("epoch_seconds must be > 0"));
    }
    Ok((now_seconds / epoch_seconds as u64) as u32)
}

/// Hex encode bytes for UI/debug convenience.
#[wasm_bindgen(js_name = bytesToHex)]
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes_to_hex_inner(bytes)
}

#[cfg(test)]
mod tests {
    use super::{
        current_epoch, derive_feed_tag_wasm, derive_rv_tag_wasm, validate_object_cbor,
        validate_shard_cbor,
    };
    use veil_codec::object::{encode_object_cbor, ObjectV1, OBJECT_V1_VERSION};
    use veil_codec::shard::{
        encode_shard_cbor, ShardErasureMode, ShardHeaderV1, ShardV1, SHARD_HEADER_LEN,
        SHARD_V1_VERSION,
    };
    use veil_core::{Epoch, Namespace};

    #[test]
    fn derives_tags_and_epoch() {
        let key = [0x11_u8; 32];
        let feed = derive_feed_tag_wasm(&key, 7).expect("feed tag");
        assert_eq!(feed.len(), 32);

        let rv = derive_rv_tag_wasm(&key, 42, 7).expect("rv tag");
        assert_eq!(rv.len(), 32);
        assert_ne!(feed, rv);

        assert_eq!(current_epoch(86_400 * 3, 86_400).expect("epoch"), 3);
    }

    #[test]
    fn validates_shard_and_object_cbor() {
        let shard = ShardV1 {
            header: ShardHeaderV1 {
                version: SHARD_V1_VERSION,
                namespace: Namespace(1),
                epoch: Epoch(2),
                tag: [0x11; 32],
                object_root: [0x22; 32],
                profile_id: 2,
                erasure_mode: ShardErasureMode::HardenedNonSystematic,
                bucket_size: (16 * 1024) as u32,
                k: 6,
                n: 10,
                index: 1,
            },
            payload: vec![0x33; 16 * 1024 - SHARD_HEADER_LEN],
        };
        let shard_bytes = encode_shard_cbor(&shard).expect("encode shard");
        let shard_valid = validate_shard_cbor(&shard_bytes).expect("validate shard cbor");
        assert!(shard_valid);

        let object = ObjectV1 {
            version: OBJECT_V1_VERSION,
            namespace: Namespace(1),
            epoch: Epoch(2),
            flags: 0,
            tag: [0x11; 32],
            object_root: [0x22; 32],
            sender_pubkey: None,
            signature: None,
            nonce: [0x44; 24],
            ciphertext: vec![0x55; 16],
            padding: vec![0x66; 8],
        };
        let object_bytes = encode_object_cbor(&object).expect("encode object");
        let object_valid = validate_object_cbor(&object_bytes).expect("validate object cbor");
        assert!(object_valid);
    }
}
