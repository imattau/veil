use serde::{Deserialize, Deserializer, Serialize, Serializer};
use veil_core::hash::blake3_32;
use veil_core::types::{Epoch, Namespace};
use veil_core::{ObjectRoot, Tag};

use crate::error::CodecError;

/// Object schema version for `ObjectV1`.
pub const OBJECT_V1_VERSION: u16 = 1;
/// Object contains signature fields.
pub const OBJECT_FLAG_SIGNED: u16 = 0x0001;
/// Object is intended for public feed delivery.
pub const OBJECT_FLAG_PUBLIC: u16 = 0x0002;
/// Receiver should emit an ACK object after successful delivery.
pub const OBJECT_FLAG_ACK_REQUESTED: u16 = 0x0004;
/// Object aggregates multiple app messages/items.
pub const OBJECT_FLAG_BATCHED: u16 = 0x0008;
/// All currently valid object flag bits.
pub const OBJECT_ALLOWED_FLAGS_MASK: u16 =
    OBJECT_FLAG_SIGNED | OBJECT_FLAG_PUBLIC | OBJECT_FLAG_ACK_REQUESTED | OBJECT_FLAG_BATCHED;

/// 64-byte signature wrapper for serde byte encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature(pub [u8; 64]);

impl Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = <Vec<u8>>::deserialize(deserializer)?;
        if raw.len() != 64 {
            return Err(serde::de::Error::invalid_length(
                raw.len(),
                &"exactly 64 bytes",
            ));
        }

        let mut bytes = [0_u8; 64];
        bytes.copy_from_slice(&raw);
        Ok(Self(bytes))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectV1 {
    /// Wire version.
    pub version: u16,
    /// Logical namespace.
    pub namespace: Namespace,
    /// Epoch window.
    pub epoch: Epoch,
    /// Bitfield flags.
    pub flags: u16,
    /// Subscription tag.
    pub tag: Tag,
    /// Root of plaintext payload/object bytes (producer-defined).
    pub object_root: ObjectRoot,
    /// Optional sender pubkey (required when signed).
    pub sender_pubkey: Option<[u8; 32]>,
    /// Optional signature (required when signed).
    pub signature: Option<Signature>,
    /// XChaCha20-Poly1305 nonce.
    pub nonce: [u8; 24],
    /// AEAD ciphertext including tag.
    pub ciphertext: Vec<u8>,
    /// Opaque padding bytes.
    pub padding: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
struct SignedObjectHeaderV1 {
    version: u16,
    namespace: Namespace,
    epoch: Epoch,
    flags: u16,
    tag: Tag,
    object_root: ObjectRoot,
    sender_pubkey: Option<[u8; 32]>,
    nonce: [u8; 24],
}

impl ObjectV1 {
    /// Validates object schema and field consistency.
    pub fn validate(&self) -> Result<(), CodecError> {
        if self.version != OBJECT_V1_VERSION {
            return Err(CodecError::InvalidObject("unsupported object version"));
        }
        if self.flags & !OBJECT_ALLOWED_FLAGS_MASK != 0 {
            return Err(CodecError::InvalidObject("unknown object flags"));
        }
        if self.ciphertext.is_empty() {
            return Err(CodecError::InvalidObject("ciphertext must not be empty"));
        }

        let signed = (self.flags & OBJECT_FLAG_SIGNED) != 0;
        if signed && self.sender_pubkey.is_none() {
            return Err(CodecError::InvalidObject(
                "signed object requires sender_pubkey",
            ));
        }
        if signed && self.signature.is_none() {
            return Err(CodecError::InvalidObject(
                "signed object requires signature",
            ));
        }
        if !signed && (self.sender_pubkey.is_some() || self.signature.is_some()) {
            return Err(CodecError::InvalidObject(
                "signature fields require signed flag",
            ));
        }
        if self.sender_pubkey.is_some() != self.signature.is_some() {
            return Err(CodecError::InvalidObject(
                "sender_pubkey and signature must be set together",
            ));
        }

        Ok(())
    }
}

/// Encodes the canonical signed-header subset used in signature preimages.
pub fn canonical_object_header_cbor(object: &ObjectV1) -> Result<Vec<u8>, CodecError> {
    object.validate()?;
    let header = SignedObjectHeaderV1 {
        version: object.version,
        namespace: object.namespace,
        epoch: object.epoch,
        flags: object.flags,
        tag: object.tag,
        object_root: object.object_root,
        sender_pubkey: object.sender_pubkey,
        nonce: object.nonce,
    };
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(&header, &mut bytes)
        .map_err(|e| CodecError::Encode(e.to_string()))?;
    Ok(bytes)
}

/// Computes signature digest over canonical header and ciphertext hash.
pub fn object_signature_message_digest(object: &ObjectV1) -> Result<[u8; 32], CodecError> {
    let header_cbor = canonical_object_header_cbor(object)?;
    let ciphertext_hash = blake3_32(&object.ciphertext);
    let mut preimage = Vec::with_capacity(header_cbor.len() + ciphertext_hash.len());
    preimage.extend_from_slice(&header_cbor);
    preimage.extend_from_slice(&ciphertext_hash);
    Ok(blake3_32(&preimage))
}

/// Encodes `ObjectV1` as CBOR after validation.
pub fn encode_object_cbor(object: &ObjectV1) -> Result<Vec<u8>, CodecError> {
    object.validate()?;
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(object, &mut bytes)
        .map_err(|e| CodecError::Encode(e.to_string()))?;
    Ok(bytes)
}

/// Decodes and validates a full CBOR object.
pub fn decode_object_cbor(bytes: &[u8]) -> Result<ObjectV1, CodecError> {
    let object: ObjectV1 =
        ciborium::de::from_reader(bytes).map_err(|e| CodecError::Decode(e.to_string()))?;
    object.validate()?;
    Ok(object)
}

/// Decodes one object prefix from a byte slice, returning bytes consumed.
pub fn decode_object_cbor_prefix(bytes: &[u8]) -> Result<(ObjectV1, usize), CodecError> {
    let mut cursor = std::io::Cursor::new(bytes);
    let object: ObjectV1 =
        ciborium::de::from_reader(&mut cursor).map_err(|e| CodecError::Decode(e.to_string()))?;
    object.validate()?;
    Ok((object, cursor.position() as usize))
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_object_header_cbor, decode_object_cbor_prefix, encode_object_cbor,
        object_signature_message_digest, ObjectV1, Signature, OBJECT_FLAG_SIGNED,
        OBJECT_V1_VERSION,
    };
    use veil_core::{Epoch, Namespace};

    fn sample_object() -> ObjectV1 {
        ObjectV1 {
            version: OBJECT_V1_VERSION,
            namespace: Namespace(1),
            epoch: Epoch(1),
            flags: OBJECT_FLAG_SIGNED,
            tag: [0x11_u8; 32],
            object_root: [0x22_u8; 32],
            sender_pubkey: Some([0x33_u8; 32]),
            signature: Some(Signature([0x44_u8; 64])),
            nonce: [0x55_u8; 24],
            ciphertext: vec![0x66_u8; 8],
            padding: vec![0x77_u8; 4],
        }
    }

    #[test]
    fn validate_rejects_unknown_flag_bits() {
        let mut obj = sample_object();
        obj.flags = 0x8000;
        let err = obj.validate().expect_err("unknown flags should fail");
        assert!(err.to_string().contains("unknown object flags"));
    }

    #[test]
    fn validate_rejects_empty_ciphertext() {
        let mut obj = sample_object();
        obj.ciphertext.clear();
        let err = obj.validate().expect_err("empty ciphertext should fail");
        assert!(err.to_string().contains("ciphertext must not be empty"));
    }

    #[test]
    fn canonical_header_excludes_ciphertext_and_padding() {
        let a = sample_object();
        let mut b = sample_object();
        b.ciphertext[0] ^= 0x01;
        b.padding[0] ^= 0x01;

        let a_header = canonical_object_header_cbor(&a).expect("header should encode");
        let b_header = canonical_object_header_cbor(&b).expect("header should encode");
        assert_eq!(a_header, b_header);

        // Message digest includes ciphertext hash, so this should differ.
        let a_digest = object_signature_message_digest(&a).expect("digest should compute");
        let b_digest = object_signature_message_digest(&b).expect("digest should compute");
        assert_ne!(a_digest, b_digest);
    }

    #[test]
    fn decode_prefix_allows_trailing_zero_padding() {
        let obj = sample_object();
        let encoded = encode_object_cbor(&obj).expect("object should encode");
        let mut padded = encoded.clone();
        padded.extend_from_slice(&[0_u8; 16]);

        let (decoded, consumed) =
            decode_object_cbor_prefix(&padded).expect("prefix decode should work");
        assert_eq!(decoded, obj);
        assert_eq!(consumed, encoded.len());
    }
}
