use reed_solomon_erasure::galois_8::ReedSolomon;
use thiserror::Error;
use veil_codec::error::CodecError;
use veil_codec::shard::{
    encode_shard_cbor, ShardErasureMode, ShardHeaderV1, ShardV1, SHARD_BUCKET_SIZES,
    SHARD_HEADER_LEN, SHARD_V1_VERSION,
};
use veil_core::hash::blake3_32;
use veil_core::types::{Epoch, Namespace};
use veil_core::{ObjectRoot, ShardId, Tag};

use crate::profile::{choose_profile, ErasureCodingMode, Profile};

fn mode_to_wire(mode: ErasureCodingMode) -> ShardErasureMode {
    match mode {
        ErasureCodingMode::Systematic => ShardErasureMode::Systematic,
        ErasureCodingMode::HardenedNonSystematic => ShardErasureMode::HardenedNonSystematic,
    }
}

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
    choose_profile_and_bucket_with_jitter(object_len_bytes, [0_u8; 32], 0)
}

/// Chooses `(profile, bucket)` for an object length with optional jitter over
/// larger-fitting buckets.
///
/// `bucket_jitter_extra_levels` controls how many larger-fitting bucket levels
/// may be selected beyond the minimum-fitting bucket. `0` keeps current
/// behavior (always minimum-fitting bucket).
pub fn choose_profile_and_bucket_with_jitter(
    object_len_bytes: usize,
    jitter_seed: [u8; 32],
    bucket_jitter_extra_levels: usize,
) -> Result<(Profile, usize), FecError> {
    if object_len_bytes == 0 {
        return Err(FecError::EmptyObject);
    }

    let profile = choose_profile(object_len_bytes);
    let per_shard_target = object_len_bytes.div_ceil(profile.k as usize);
    let needed = per_shard_target + SHARD_HEADER_LEN;

    let candidates = profile
        .buckets
        .iter()
        .copied()
        .filter(|b| *b >= needed)
        .collect::<Vec<_>>();
    if !candidates.is_empty() {
        let max_extra = bucket_jitter_extra_levels.min(candidates.len().saturating_sub(1));
        let idx = if max_extra == 0 {
            0
        } else {
            jitter_seed[0] as usize % (max_extra + 1)
        };
        return Ok((profile, candidates[idx]));
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
    object_to_shards_with_mode_and_padding(
        object_bytes,
        namespace,
        epoch,
        tag,
        object_root,
        ErasureCodingMode::HardenedNonSystematic,
        0,
    )
}

/// Splits encoded object bytes into `n` shards using the requested coding mode.
pub fn object_to_shards_with_mode(
    object_bytes: &[u8],
    namespace: Namespace,
    epoch: Epoch,
    tag: Tag,
    object_root: ObjectRoot,
    mode: ErasureCodingMode,
) -> Result<Vec<ShardV1>, FecError> {
    object_to_shards_with_mode_and_padding(
        object_bytes,
        namespace,
        epoch,
        tag,
        object_root,
        mode,
        0,
    )
}

/// Splits encoded object bytes into `n` shards using coding mode and optional
/// bucket-jitter padding.
pub fn object_to_shards_with_mode_and_padding(
    object_bytes: &[u8],
    namespace: Namespace,
    epoch: Epoch,
    tag: Tag,
    object_root: ObjectRoot,
    mode: ErasureCodingMode,
    bucket_jitter_extra_levels: usize,
) -> Result<Vec<ShardV1>, FecError> {
    let (profile, bucket) = choose_profile_and_bucket_with_jitter(
        object_bytes.len(),
        object_root,
        bucket_jitter_extra_levels,
    )?;
    let k = profile.k as usize;
    let n = profile.n as usize;
    let chunk_len = bucket - SHARD_HEADER_LEN;

    let total_capacity = k * chunk_len;
    if object_bytes.len() > total_capacity {
        return Err(FecError::ObjectTooLarge);
    }

    let mut padded = vec![0_u8; total_capacity];
    padded[..object_bytes.len()].copy_from_slice(object_bytes);

    let mut source_blocks: Vec<Vec<u8>> = Vec::with_capacity(k);
    for i in 0..k {
        let start = i * chunk_len;
        let end = start + chunk_len;
        source_blocks.push(padded[start..end].to_vec());
    }

    if mode == ErasureCodingMode::HardenedNonSystematic {
        source_blocks = hardened_forward_transform(source_blocks, object_root);
    }

    let mut shards_data: Vec<Vec<u8>> = Vec::with_capacity(n);
    shards_data.extend(source_blocks);
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
                profile_id: profile.id,
                erasure_mode: mode_to_wire(mode),
                bucket_size: bucket as u32,
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
    let mut out = reconstruct_object_padded_with_mode(
        shards,
        expected_root,
        ErasureCodingMode::HardenedNonSystematic,
    )?;
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
    reconstruct_object_padded_with_mode(
        shards,
        expected_root,
        ErasureCodingMode::HardenedNonSystematic,
    )
}

/// Reconstructs object bytes (truncated to `object_len`) from a shard subset.
pub fn reconstruct_object_with_mode(
    shards: &[ShardV1],
    object_len: usize,
    expected_root: ObjectRoot,
    mode: ErasureCodingMode,
) -> Result<Vec<u8>, FecError> {
    let mut out = reconstruct_object_padded_with_mode(shards, expected_root, mode)?;
    if object_len > out.len() {
        return Err(FecError::InvalidShardSet(
            "requested object length too large",
        ));
    }
    out.truncate(object_len);
    Ok(out)
}

/// Reconstructs padded object block bytes from a shard subset using coding mode.
pub fn reconstruct_object_padded_with_mode(
    shards: &[ShardV1],
    expected_root: ObjectRoot,
    mode: ErasureCodingMode,
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
            || shard.header.profile_id != first.profile_id
            || shard.header.erasure_mode != first.erasure_mode
            || shard.header.bucket_size != first.bucket_size
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

    let mut source_blocks = Vec::with_capacity(k);
    for shard in slots.into_iter().take(k) {
        let bytes = shard.ok_or(FecError::ReedSolomon)?;
        source_blocks.push(bytes);
    }
    if mode == ErasureCodingMode::HardenedNonSystematic {
        source_blocks = hardened_inverse_transform(source_blocks, expected_root);
    }

    let mut out = Vec::with_capacity(k * chunk_len);
    for block in source_blocks {
        out.extend_from_slice(&block);
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

fn hardened_rotation(root: ObjectRoot, len: usize) -> usize {
    if len <= 1 {
        return 0;
    }
    let r = (root[0] as usize) % len;
    if r == 0 {
        1
    } else {
        r
    }
}

fn xor_in_place(lhs: &mut [u8], rhs: &[u8]) {
    for (a, b) in lhs.iter_mut().zip(rhs.iter()) {
        *a ^= *b;
    }
}

fn hardened_forward_transform(mut source_blocks: Vec<Vec<u8>>, root: ObjectRoot) -> Vec<Vec<u8>> {
    let k = source_blocks.len();
    if k <= 1 {
        return source_blocks;
    }
    let rot = hardened_rotation(root, k);
    source_blocks.rotate_left(rot);

    let mut mixed = Vec::with_capacity(k);
    for i in 0..k {
        if i == 0 {
            mixed.push(source_blocks[0].clone());
            continue;
        }
        let mut out = source_blocks[i].clone();
        xor_in_place(&mut out, &source_blocks[i - 1]);
        mixed.push(out);
    }
    mixed
}

fn hardened_inverse_transform(mut transformed: Vec<Vec<u8>>, root: ObjectRoot) -> Vec<Vec<u8>> {
    let k = transformed.len();
    if k <= 1 {
        return transformed;
    }

    for i in 1..k {
        let prev = transformed[i - 1].clone();
        xor_in_place(&mut transformed[i], &prev);
    }

    let rot = hardened_rotation(root, k);
    transformed.rotate_right(rot);
    transformed
}

#[cfg(test)]
mod tests {
    use super::{
        choose_profile_and_bucket, choose_profile_and_bucket_with_jitter, derive_object_root,
        is_valid_bucket_size, object_to_shards, object_to_shards_with_mode, reconstruct_object,
        reconstruct_object_padded, reconstruct_object_with_mode, shard_id, FecError,
    };
    use crate::profile::ErasureCodingMode;
    use veil_core::types::{Epoch, Namespace};

    #[test]
    fn choose_profile_and_bucket_picks_micro_2k_for_small_object() {
        let (profile, bucket) =
            choose_profile_and_bucket(1024).expect("small object should fit profile");
        assert_eq!(profile.k, 2);
        assert_eq!(profile.n, 3);
        assert_eq!(bucket, 2 * 1024);
    }

    #[test]
    fn choose_profile_and_bucket_can_jitter_upward_within_profile() {
        let object_len = 1024;
        let (_, min_bucket) =
            choose_profile_and_bucket_with_jitter(object_len, [0x00; 32], 0).expect("fits");
        let (_, jitter_bucket) =
            choose_profile_and_bucket_with_jitter(object_len, [0x01; 32], 1).expect("fits");
        assert_eq!(min_bucket, 2 * 1024);
        assert!(jitter_bucket == 2 * 1024 || jitter_bucket == 4 * 1024);
    }

    #[test]
    fn object_to_shards_emits_n_valid_shards() {
        let object = b"hello veil";
        let root = derive_object_root(object);
        let shards = object_to_shards(object, Namespace(7), Epoch(9), [0x11; 32], root)
            .expect("object should shard");

        assert_eq!(shards.len(), shards[0].header.n as usize);
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
        let selected: Vec<_> = shards.iter().step_by(2).take(k).cloned().collect();
        assert_eq!(selected.len(), k);
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
        let k = shards[0].header.k as usize;
        let selected = shards[..k.saturating_sub(1)].to_vec();

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
        let k = shards[0].header.k as usize;
        let subset = shards[..k].to_vec();
        let recovered_padded =
            reconstruct_object_padded(&subset, root).expect("should reconstruct");
        assert_eq!(&recovered_padded[..object.len()], object.as_slice());
    }

    #[test]
    fn hardened_mode_reconstructs_from_any_k_shards() {
        let object = b"hardened mode reconstruct check".to_vec();
        let root = derive_object_root(&object);
        let shards = object_to_shards_with_mode(
            &object,
            Namespace(12),
            Epoch(3),
            [0xA5; 32],
            root,
            ErasureCodingMode::HardenedNonSystematic,
        )
        .expect("object should shard");
        let k = shards[0].header.k as usize;
        let mut selected: Vec<_> = shards.iter().skip(1).step_by(2).take(k).cloned().collect();
        if selected.len() < k {
            selected.extend(shards.iter().take(k - selected.len()).cloned());
        }
        assert_eq!(selected.len(), k);
        let recovered = reconstruct_object_with_mode(
            &selected,
            object.len(),
            root,
            ErasureCodingMode::HardenedNonSystematic,
        )
        .expect("reconstruction should work");
        assert_eq!(recovered, object);
    }

    #[test]
    fn hardened_mode_first_k_shards_are_not_plain_chunk_layout() {
        let object = b"0123456789abcdefghijklmnopqrstuvwxyz".repeat(512);
        let root = derive_object_root(&object);
        let systematic = object_to_shards_with_mode(
            &object,
            Namespace(14),
            Epoch(7),
            [0xBE; 32],
            root,
            ErasureCodingMode::Systematic,
        )
        .expect("systematic should shard");
        let hardened = object_to_shards_with_mode(
            &object,
            Namespace(14),
            Epoch(7),
            [0xBE; 32],
            root,
            ErasureCodingMode::HardenedNonSystematic,
        )
        .expect("hardened should shard");

        let k = systematic[0].header.k as usize;
        let differing = (0..k)
            .filter(|i| systematic[*i].payload != hardened[*i].payload)
            .count();
        assert!(differing > 0);
    }
}
