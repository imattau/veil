mod frb_generated; /* AUTO INJECTED BY flutter_rust_bridge. This line may not be accurate, and you can change it according to your needs. */
#![allow(unexpected_cfgs)]

use flutter_rust_bridge::frb;

#[frb]
pub mod api {
    use flutter_rust_bridge::frb;
    use veil_codec::object::{
        decode_object_cbor, OBJECT_FLAG_ACK_REQUESTED, OBJECT_FLAG_BATCHED, OBJECT_FLAG_PUBLIC,
        OBJECT_FLAG_SIGNED,
    };
    use veil_codec::shard::decode_shard_cbor;
    use veil_crypto::aead::{build_veil_aad, AeadCipher, XChaCha20Poly1305Cipher};
    use veil_core::hash::blake3_32;
    use veil_core::tags::{current_epoch, derive_feed_tag, derive_rv_tag};
    use veil_core::types::{Epoch, Namespace};
    use veil_core::{ObjectRoot, Tag};
    use veil_fec::sharder::reconstruct_object_padded;

    #[derive(Clone, Debug)]
    #[frb]
    pub struct ShardMeta {
        pub version: u16,
        pub namespace: u16,
        pub epoch: u32,
        pub tag_hex: String,
        pub object_root_hex: String,
        pub k: u16,
        pub n: u16,
        pub index: u16,
        pub payload_len: usize,
    }

    #[derive(Clone, Debug)]
    #[frb]
    pub struct ObjectMeta {
        pub version: u16,
        pub namespace: u16,
        pub epoch: u32,
        pub flags: u16,
        pub signed: bool,
        pub public: bool,
        pub ack_requested: bool,
        pub batched: bool,
        pub tag_hex: String,
        pub object_root_hex: String,
        pub sender_pubkey_hex: Option<String>,
        pub nonce_hex: String,
        pub ciphertext_len: usize,
        pub padding_len: usize,
    }

    fn hex_encode(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            out.push_str(&format!("{:02x}", b));
        }
        out
    }

    fn hex_decode_32(input: &str) -> Result<[u8; 32], String> {
        if input.len() != 64 {
            return Err("expected 64 hex chars".to_string());
        }
        let mut out = [0_u8; 32];
        for (i, chunk) in input.as_bytes().chunks_exact(2).enumerate() {
            let s = std::str::from_utf8(chunk).map_err(|_| "invalid hex")?;
            out[i] = u8::from_str_radix(s, 16).map_err(|_| "invalid hex")?;
        }
        Ok(out)
    }

    #[frb]
    pub fn derive_feed_tag_hex(
        publisher_pubkey_hex: String,
        namespace: u16,
    ) -> Result<String, String> {
        let key = hex_decode_32(&publisher_pubkey_hex)?;
        let tag: Tag = derive_feed_tag(&key, Namespace(namespace));
        Ok(hex_encode(&tag))
    }

    #[frb]
    pub fn derive_rv_tag_hex(
        recipient_pubkey_hex: String,
        epoch: u32,
        namespace: u16,
    ) -> Result<String, String> {
        let key = hex_decode_32(&recipient_pubkey_hex)?;
        let tag: Tag = derive_rv_tag(&key, Epoch(epoch), Namespace(namespace));
        Ok(hex_encode(&tag))
    }

    #[frb]
    pub fn current_epoch_seconds(now: u64, epoch_seconds: u64) -> u64 {
        current_epoch(now, epoch_seconds).0 as u64
    }

    #[frb]
    pub fn decode_shard_meta(bytes: Vec<u8>) -> Result<ShardMeta, String> {
        let shard = decode_shard_cbor(&bytes).map_err(|e| e.to_string())?;
        Ok(ShardMeta {
            version: shard.header.version,
            namespace: shard.header.namespace.0,
            epoch: shard.header.epoch.0,
            tag_hex: hex_encode(&shard.header.tag),
            object_root_hex: hex_encode(&shard.header.object_root),
            k: shard.header.k,
            n: shard.header.n,
            index: shard.header.index,
            payload_len: shard.payload.len(),
        })
    }

    #[frb]
    pub fn decode_object_meta(bytes: Vec<u8>) -> Result<ObjectMeta, String> {
        let obj = decode_object_cbor(&bytes).map_err(|e| e.to_string())?;
        let flags = obj.flags;
        Ok(ObjectMeta {
            version: obj.version,
            namespace: obj.namespace.0,
            epoch: obj.epoch.0,
            flags,
            signed: flags & OBJECT_FLAG_SIGNED != 0,
            public: flags & OBJECT_FLAG_PUBLIC != 0,
            ack_requested: flags & OBJECT_FLAG_ACK_REQUESTED != 0,
            batched: flags & OBJECT_FLAG_BATCHED != 0,
            tag_hex: hex_encode(&obj.tag),
            object_root_hex: hex_encode(&obj.object_root),
            sender_pubkey_hex: obj.sender_pubkey.map(|p| hex_encode(&p)),
            nonce_hex: hex_encode(&obj.nonce),
            ciphertext_len: obj.ciphertext.len(),
            padding_len: obj.padding.len(),
        })
    }

    #[frb]
    pub fn derive_object_root_hex(object_bytes: Vec<u8>) -> String {
        let root: ObjectRoot = blake3_32(&object_bytes);
        hex_encode(&root)
    }

    #[frb]
    pub fn reconstruct_object_padded_from_shards(
        shard_bytes: Vec<Vec<u8>>,
        expected_root_hex: String,
    ) -> Result<Vec<u8>, String> {
        let expected_root = hex_decode_32(&expected_root_hex)?;
        let mut shards = Vec::with_capacity(shard_bytes.len());
        for bytes in shard_bytes {
            let shard = decode_shard_cbor(&bytes).map_err(|e| e.to_string())?;
            shards.push(shard);
        }
        reconstruct_object_padded(&shards, expected_root).map_err(|e| e.to_string())
    }

    #[frb]
    pub fn decrypt_object_payload(
        object_bytes: Vec<u8>,
        key_bytes: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        if key_bytes.len() != 32 {
            return Err("decrypt key must be 32 bytes".to_string());
        }
        let obj = decode_object_cbor(&object_bytes).map_err(|e| e.to_string())?;
        let aad = build_veil_aad(obj.tag, obj.namespace, obj.epoch);
        let cipher = XChaCha20Poly1305Cipher;
        cipher
            .decrypt(&key_bytes, obj.nonce, &aad, &obj.ciphertext)
            .map_err(|e| e.to_string())
    }
}
