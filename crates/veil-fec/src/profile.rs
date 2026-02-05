/// Erasure coding profile parameters.
#[derive(Debug, Clone, Copy)]
pub struct Profile {
    /// Reconstruction threshold.
    pub k: u16,
    /// Total shard count.
    pub n: u16,
    /// Allowed shard buckets for this profile.
    pub buckets: &'static [usize],
}

/// Erasure coding mode used by shard split/reconstruct routines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErasureCodingMode {
    /// Classic systematic RS encoding (first `k` shards are data blocks).
    Systematic,
    /// Hardened mode: deterministic non-systematic pre-transform before RS.
    HardenedNonSystematic,
}

/// Default profile for smaller objects.
pub const PROFILE_SMALL: Profile = Profile {
    k: 6,
    n: 10,
    buckets: &[16 * 1024, 32 * 1024],
};

/// Default profile for larger objects.
pub const PROFILE_LARGE: Profile = Profile {
    k: 10,
    n: 16,
    buckets: &[32 * 1024, 64 * 1024],
};

/// Chooses profile by object length threshold.
pub fn choose_profile(object_len: usize) -> Profile {
    if object_len <= 128 * 1024 {
        PROFILE_SMALL
    } else {
        PROFILE_LARGE
    }
}

#[cfg(test)]
mod tests {
    use super::{choose_profile, PROFILE_LARGE, PROFILE_SMALL};

    #[test]
    fn chooses_small_at_and_below_boundary() {
        let p_low = choose_profile(0);
        let p_edge = choose_profile(128 * 1024);
        assert_eq!(p_low.k, PROFILE_SMALL.k);
        assert_eq!(p_edge.n, PROFILE_SMALL.n);
    }

    #[test]
    fn chooses_large_above_boundary() {
        let p = choose_profile(128 * 1024 + 1);
        assert_eq!(p.k, PROFILE_LARGE.k);
        assert_eq!(p.n, PROFILE_LARGE.n);
        assert_eq!(p.buckets, PROFILE_LARGE.buckets);
    }
}
