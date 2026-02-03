use reed_solomon_erasure::galois_8::ReedSolomon;
use thiserror::Error;
use veil_codec::error::CodecError;
use veil_codec::shard::{
    encode_shard_cbor, ShardHeaderV1, ShardV1, SHARD_BUCKET_SIZES, SHARD_HEADER_LEN,
    SHARD_V1_VERSION,
};
use veil_core::hash::blake3_32;
use veil_core::types::{Epoch, Namespace};
use veil_core::{ObjectRoot, ShardId, Tag};

use crate::profile::{choose_profile, Profile};

/// Errors returned by FEC profile/sharding/reconstruction helpers.
#[derive(Debug, Error)]
pub enum FecError {
    #[error("empty object")]
    EmptyObject,
    #[error("object too large for selected profile")]
    ObjectTooLarge,
    #[error("invalid shard set: {0}")]
    InvalidShardSet(&'static str),
    #[error("reed-solomon error")]
    ReedSolomon,
    #[error("codec error: {0}")]
    Codec(#[from] CodecError),
}

/// Chooses `(profile, bucket)` for an object length.
pub fn choose_profile_and_bucket(object_len_bytes: usize) -> Result<(Profile, usize), FecError> {
    if object_len_bytes == 0 {
        return Err(FecError::EmptyObject);
    }

    let profile = choose_profile(object_len_bytes);
    let per_shard_target = object_len_bytes.div_ceil(profile.k as usize);
    let needed = per_shard_target + SHARD_HEADER_LEN;

    if let Some(bucket) = profile.buckets.iter().copied().find(|b| *b >= needed) {
        return Ok((profile, bucket));
    }

    let largest = profile
        .buckets
        .iter()
        .copied()
        .max()
        .ok_or(FecError::ObjectTooLarge)?;
    let capacity = (largest - SHARD_HEADER_LEN) * profile.k as usize;
    if object_len_bytes > capacity {
        return Err(FecError::ObjectTooLarge);
    }
    Ok((profile, largest))
}

/// Derives object root hash.
pub fn derive_object_root(object_bytes: &[u8]) -> ObjectRoot {
    blake3_32(object_bytes)
}

/// Splits encoded object bytes into `n` shards using Reed-Solomon.
pub fn object_to_shards(
    object_bytes: &[u8],
    namespace: Namespace,
    epoch: Epoch,
    tag: Tag,
    object_root: ObjectRoot,
) -> Result<Vec<ShardV1>, FecError> {
    let (profile, bucket) = choose_profile_and_bucket(object_bytes.len())?;
    let k = profile.k as usize;
    let n = profile.n as usize;
    let chunk_len = bucket - SHARD_HEADER_LEN;

    let total_capacity = k * chunk_len;
    if object_bytes.len() > total_capacity {
        return Err(FecError::ObjectTooLarge);
    }

    let mut padded = vec![0_u8; total_capacity];
    padded[..object_bytes.len()].copy_from_slice(object_bytes);

    let mut shards_data: Vec<Vec<u8>> = Vec::with_capacity(n);
    for i in 0..k {
        let start = i * chunk_len;
        let end = start + chunk_len;
        shards_data.push(padded[start..end].to_vec());
    }
    for _ in k..n {
        shards_data.push(vec![0_u8; chunk_len]);
    }

    let rs = ReedSolomon::new(k, n - k).map_err(|_| FecError::ReedSolomon)?;
    rs.encode(&mut shards_data)
        .map_err(|_| FecError::ReedSolomon)?;

    let mut out = Vec::with_capacity(n);
    for (idx, payload) in shards_data.into_iter().enumerate() {
        out.push(ShardV1 {
            header: ShardHeaderV1 {
                version: SHARD_V1_VERSION,
                namespace,
                epoch,
                tag,
                object_root,
                k: profile.k,
                n: profile.n,
                index: idx as u16,
            },
            payload,
        });
    }
    Ok(out)
}

/// Reconstructs object bytes (truncated to `object_len`) from a shard subset.
pub fn reconstruct_object(
    shards: &[ShardV1],
    object_len: usize,
    expected_root: ObjectRoot,
) -> Result<Vec<u8>, FecError> {
    let mut out = reconstruct_object_padded(shards, expected_root)?;
    if object_len > out.len() {
        return Err(FecError::InvalidShardSet(
            "requested object length too large",
        ));
    }
    out.truncate(object_len);
    Ok(out)
}

/// Reconstructs padded object block bytes from a shard subset.
pub fn reconstruct_object_padded(
    shards: &[ShardV1],
    expected_root: ObjectRoot,
) -> Result<Vec<u8>, FecError> {
    if shards.is_empty() {
        return Err(FecError::InvalidShardSet("no shards"));
    }

    let first = &shards[0].header;
    if first.object_root != expected_root {
        return Err(FecError::InvalidShardSet("object root mismatch"));
    }
    let k = first.k as usize;
    let n = first.n as usize;
    if k == 0 || n == 0 || k > n {
        return Err(FecError::InvalidShardSet("invalid k/n in header"));
    }

    let chunk_len = shards[0].payload.len();
    let mut slots: Vec<Option<Vec<u8>>> = vec![None; n];

    for shard in shards {
        if shard.header.object_root != expected_root
            || shard.header.k != first.k
            || shard.header.n != first.n
            || shard.header.namespace != first.namespace
            || shard.header.epoch != first.epoch
            || shard.header.tag != first.tag
        {
            return Err(FecError::InvalidShardSet("mixed shard set"));
        }
        if shard.payload.len() != chunk_len {
            return Err(FecError::InvalidShardSet("payload lengths differ"));
        }
        let idx = shard.header.index as usize;
        if idx >= n {
            return Err(FecError::InvalidShardSet("index out of range"));
        }
        if slots[idx].is_none() {
            slots[idx] = Some(shard.payload.clone());
        }
    }

    let available = slots.iter().filter(|s| s.is_some()).count();
    if available < k {
        return Err(FecError::InvalidShardSet(
            "not enough shards to reconstruct",
        ));
    }

    let rs = ReedSolomon::new(k, n - k).map_err(|_| FecError::ReedSolomon)?;
    rs.reconstruct(&mut slots)
        .map_err(|_| FecError::ReedSolomon)?;

    let mut out = Vec::with_capacity(k * chunk_len);
    for shard in slots.into_iter().take(k) {
        let bytes = shard.ok_or(FecError::ReedSolomon)?;
        out.extend_from_slice(&bytes);
    }
    Ok(out)
}

/// Computes deterministic shard identifier from encoded shard bytes.
pub fn shard_id(shard: &ShardV1) -> Result<ShardId, FecError> {
    let encoded = encode_shard_cbor(shard)?;
    Ok(blake3_32(&encoded))
}

/// Returns true if payload length maps to an allowed total bucket size.
pub fn is_valid_bucket_size(payload_len: usize) -> bool {
    let total = SHARD_HEADER_LEN + payload_len;
    SHARD_BUCKET_SIZES.contains(&total)
}

#[cfg(test)]
mod tests {
    use super::{
        choose_profile_and_bucket, derive_object_root, is_valid_bucket_size, object_to_shards,
        reconstruct_object, reconstruct_object_padded, shard_id, FecError,
    };
    use veil_core::types::{Epoch, Namespace};

    #[test]
    fn choose_profile_and_bucket_picks_small_16k_for_small_object() {
        let (profile, bucket) =
            choose_profile_and_bucket(1024).expect("small object should fit profile");
        assert_eq!(profile.k, 6);
        assert_eq!(profile.n, 10);
        assert_eq!(bucket, 16 * 1024);
    }

    #[test]
    fn object_to_shards_emits_n_valid_shards() {
        let object = b"hello veil";
        let root = derive_object_root(object);
        let shards = object_to_shards(object, Namespace(7), Epoch(9), [0x11; 32], root)
            .expect("object should shard");

        assert_eq!(shards.len(), 10);
        for (i, shard) in shards.iter().enumerate() {
            assert_eq!(shard.header.index as usize, i);
            assert_eq!(shard.header.object_root, root);
            assert!(is_valid_bucket_size(shard.payload.len()));
            assert!(shard.validate().is_ok());
        }
    }

    #[test]
    fn reconstructs_from_any_k_shards() {
        let object = b"this object is reconstructed from any k shards".to_vec();
        let root = derive_object_root(&object);
        let shards = object_to_shards(&object, Namespace(1), Epoch(2), [0x22; 32], root)
            .expect("object should shard");

        let k = shards[0].header.k as usize;
        let subset = [0usize, 2, 4, 6, 8, 9];
        assert_eq!(subset.len(), k);
        let selected: Vec<_> = subset.iter().map(|i| shards[*i].clone()).collect();
        let recovered =
            reconstruct_object(&selected, object.len(), root).expect("reconstruction should work");
        assert_eq!(recovered, object);
    }

    #[test]
    fn reconstruction_fails_with_too_few_shards() {
        let object = b"insufficient shard test";
        let root = derive_object_root(object);
        let shards = object_to_shards(object, Namespace(3), Epoch(4), [0x33; 32], root)
            .expect("object should shard");
        let selected = shards[..5].to_vec(); // k=6 for small profile

        let err = reconstruct_object(&selected, object.len(), root)
            .expect_err("reconstruction must fail with too few shards");
        assert!(matches!(err, FecError::InvalidShardSet(_)));
        assert!(err.to_string().contains("not enough shards"));
    }

    #[test]
    fn shard_id_is_deterministic() {
        let object = b"stable shard id";
        let root = derive_object_root(object);
        let shards = object_to_shards(object, Namespace(8), Epoch(5), [0x44; 32], root)
            .expect("object should shard");
        let id_a = shard_id(&shards[0]).expect("id should compute");
        let id_b = shard_id(&shards[0]).expect("id should compute");
        assert_eq!(id_a, id_b);
    }

    #[test]
    fn reconstruct_padded_contains_object_prefix() {
        let object = b"prefix object".to_vec();
        let root = derive_object_root(&object);
        let shards = object_to_shards(&object, Namespace(1), Epoch(2), [0x55; 32], root)
            .expect("object should shard");
        let subset = shards[..6].to_vec();
        let recovered_padded =
            reconstruct_object_padded(&subset, root).expect("should reconstruct");
        assert_eq!(&recovered_padded[..object.len()], object.as_slice());
    }
}
