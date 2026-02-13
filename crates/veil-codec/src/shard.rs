use serde::{Deserialize, Serialize};
use veil_core::types::{Epoch, Namespace};
use veil_core::{ObjectRoot, Tag};

use crate::error::CodecError;

/// Shard schema version for `ShardV1`.
pub const SHARD_V1_VERSION: u16 = 2;
/// Fixed serialized shard-header length in bytes.
pub const SHARD_HEADER_LEN: usize = 2 + 2 + 4 + 32 + 32 + 2 + 1 + 4 + 2 + 2 + 2;
/// Allowed total shard bucket sizes.
pub const SHARD_BUCKET_SIZES: [usize; 6] = [
    2 * 1024,
    4 * 1024,
    8 * 1024,
    16 * 1024,
    32 * 1024,
    64 * 1024,
];

/// Erasure coding mode carried on shard headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ShardErasureMode {
    Systematic = 0,
    HardenedNonSystematic = 1,
}

/// Shard metadata header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardHeaderV1 {
    /// Wire version.
    pub version: u16,
    /// Logical namespace.
    pub namespace: Namespace,
    /// Epoch window.
    pub epoch: Epoch,
    /// Subscription tag.
    pub tag: Tag,
    /// Root grouping sibling shards for one object.
    pub object_root: ObjectRoot,
    /// FEC profile identifier.
    pub profile_id: u16,
    /// Erasure coding mode used for this shard set.
    pub erasure_mode: ShardErasureMode,
    /// Declared total shard bucket size in bytes.
    pub bucket_size: u32,
    /// Reconstruction threshold.
    pub k: u16,
    /// Total shards in set.
    pub n: u16,
    /// Shard index in `[0, n)`.
    pub index: u16,
}

/// Full shard payload unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardV1 {
    pub header: ShardHeaderV1,
    pub payload: Vec<u8>,
}

impl ShardHeaderV1 {
    /// Validates header invariants.
    pub fn validate(&self) -> Result<(), CodecError> {
        if self.version != SHARD_V1_VERSION {
            return Err(CodecError::InvalidShard("unsupported shard version"));
        }
        if self.k == 0 || self.n == 0 {
            return Err(CodecError::InvalidShard("k and n must be > 0"));
        }
        if self.k > self.n {
            return Err(CodecError::InvalidShard("k must be <= n"));
        }
        if self.index >= self.n {
            return Err(CodecError::InvalidShard("index out of range"));
        }
        if !SHARD_BUCKET_SIZES.contains(&(self.bucket_size as usize)) {
            return Err(CodecError::InvalidShard("unsupported bucket size"));
        }
        Ok(())
    }
}

impl ShardV1 {
    /// Validates header and payload bucket sizing.
    pub fn validate(&self) -> Result<(), CodecError> {
        self.header.validate()?;
        if self.payload.is_empty() {
            return Err(CodecError::InvalidShard("payload must not be empty"));
        }

        let total_len = SHARD_HEADER_LEN + self.payload.len();
        if total_len != self.header.bucket_size as usize {
            return Err(CodecError::InvalidShard(
                "payload/header length does not match declared bucket size",
            ));
        }
        if !SHARD_BUCKET_SIZES.contains(&total_len) {
            return Err(CodecError::InvalidShard(
                "shard does not match allowed bucket size",
            ));
        }
        Ok(())
    }
}

/// Encodes `ShardV1` as CBOR after validation.
pub fn encode_shard_cbor(shard: &ShardV1) -> Result<Vec<u8>, CodecError> {
    shard.validate()?;
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(shard, &mut bytes)
        .map_err(|e| CodecError::Encode(e.to_string()))?;
    Ok(bytes)
}

/// Decodes and validates a CBOR shard.
pub fn decode_shard_cbor(bytes: &[u8]) -> Result<ShardV1, CodecError> {
    let shard: ShardV1 = ciborium::de::from_reader(bytes)
        .map_err(|e| CodecError::Decode(e.to_string()))?;
    shard.validate()?;
    Ok(shard)
}

#[cfg(test)]
mod tests {
    use super::{ShardErasureMode, ShardHeaderV1, ShardV1, SHARD_HEADER_LEN, SHARD_V1_VERSION};
    use veil_core::{Epoch, Namespace};

    fn sample_shard() -> ShardV1 {
        ShardV1 {
            header: ShardHeaderV1 {
                version: SHARD_V1_VERSION,
                namespace: Namespace(1),
                epoch: Epoch(1),
                tag: [0x11_u8; 32],
                object_root: [0x22_u8; 32],
                profile_id: 1,
                erasure_mode: ShardErasureMode::HardenedNonSystematic,
                bucket_size: (16 * 1024) as u32,
                k: 6,
                n: 10,
                index: 0,
            },
            payload: vec![0x33_u8; 16 * 1024 - SHARD_HEADER_LEN],
        }
    }

    #[test]
    fn validate_rejects_k_greater_than_n() {
        let mut s = sample_shard();
        s.header.k = 11;
        s.header.n = 10;
        let err = s.validate().expect_err("k > n should fail");
        assert!(err.to_string().contains("k must be <= n"));
    }

    #[test]
    fn validate_rejects_index_out_of_range() {
        let mut s = sample_shard();
        s.header.index = s.header.n;
        let err = s.validate().expect_err("index >= n should fail");
        assert!(err.to_string().contains("index out of range"));
    }

    #[test]
    fn validate_accepts_known_bucket_sizes() {
        let s16 = sample_shard();
        assert!(s16.validate().is_ok());

        let mut s32 = sample_shard();
        s32.payload = vec![0x33_u8; 32 * 1024 - SHARD_HEADER_LEN];
        s32.header.bucket_size = (32 * 1024) as u32;
        assert!(s32.validate().is_ok());

        let mut s64 = sample_shard();
        s64.payload = vec![0x33_u8; 64 * 1024 - SHARD_HEADER_LEN];
        s64.header.bucket_size = (64 * 1024) as u32;
        assert!(s64.validate().is_ok());
    }
}
