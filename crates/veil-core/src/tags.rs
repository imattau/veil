use crate::{
    hash::blake3_32,
    types::{Epoch, Namespace, Tag},
};

/// Normalizes app-level channel identifiers for deterministic derivation.
///
/// Empty/whitespace-only IDs map to `"general"`.
pub fn normalize_channel_id(channel_id: &str) -> String {
    let normalized = channel_id.trim().to_lowercase();
    if normalized.is_empty() {
        "general".to_string()
    } else {
        normalized
    }
}

fn fnv1a_32(bytes: &[u8]) -> u32 {
    let mut hash: u32 = 2_166_136_261;
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}

/// Derives a deterministic per-channel namespace from base namespace + channel id.
pub fn derive_channel_namespace(base_namespace: Namespace, channel_id: &str) -> Namespace {
    let channel = normalize_channel_id(channel_id);
    let channel_hash16 = (fnv1a_32(channel.as_bytes()) & 0xffff) as u16;
    Namespace(base_namespace.0.wrapping_add(channel_hash16))
}

/// Derives a stable public feed tag:
/// `H("feed" || publisher_pubkey || namespace_be)`.
pub fn derive_feed_tag(publisher_pubkey: &[u8; 32], namespace: Namespace) -> Tag {
    let mut buf = Vec::with_capacity(4 + 32 + 2);
    buf.extend_from_slice(b"feed");
    buf.extend_from_slice(publisher_pubkey);
    buf.extend_from_slice(&namespace.0.to_be_bytes());
    blake3_32(&buf)
}

/// Derives a stable public feed tag scoped by `channel_id`.
pub fn derive_channel_feed_tag(
    publisher_pubkey: &[u8; 32],
    base_namespace: Namespace,
    channel_id: &str,
) -> Tag {
    derive_feed_tag(
        publisher_pubkey,
        derive_channel_namespace(base_namespace, channel_id),
    )
}

/// Derives a rotating rendezvous tag:
/// `H("rv" || recipient_pubkey || epoch_be || namespace_be)`.
pub fn derive_rv_tag(recipient_pubkey: &[u8; 32], epoch: Epoch, namespace: Namespace) -> Tag {
    let mut buf = Vec::with_capacity(2 + 32 + 4 + 2);
    buf.extend_from_slice(b"rv");
    buf.extend_from_slice(recipient_pubkey);
    buf.extend_from_slice(&epoch.0.to_be_bytes());
    buf.extend_from_slice(&namespace.0.to_be_bytes());
    blake3_32(&buf)
}

/// Derives a rotating rendezvous tag scoped by `channel_id`.
pub fn derive_channel_rv_tag(
    recipient_pubkey: &[u8; 32],
    epoch: Epoch,
    base_namespace: Namespace,
    channel_id: &str,
) -> Tag {
    derive_rv_tag(
        recipient_pubkey,
        epoch,
        derive_channel_namespace(base_namespace, channel_id),
    )
}

/// Returns the current epoch index for `now_seconds / epoch_seconds`.
///
/// `epoch_seconds=0` is treated as `1` to avoid division-by-zero.
pub fn current_epoch(now_seconds: u64, epoch_seconds: u64) -> Epoch {
    let window = epoch_seconds.max(1);
    Epoch((now_seconds / window).min(u32::MAX as u64) as u32)
}

/// Returns true when `now_seconds` is in the overlap tail where the next
/// epoch's rendezvous tag should also be accepted.
pub fn in_next_epoch_overlap(now_seconds: u64, epoch_seconds: u64, overlap_seconds: u64) -> bool {
    if overlap_seconds == 0 {
        return false;
    }
    let window = epoch_seconds.max(1);
    let overlap = overlap_seconds.min(window);
    let offset = now_seconds % window;
    offset >= window.saturating_sub(overlap)
}

/// Derives the current rendezvous tag and, during the overlap tail, also
/// derives the next-epoch rendezvous tag.
pub fn derive_rv_tag_window(
    recipient_pubkey: &[u8; 32],
    now_seconds: u64,
    epoch_seconds: u64,
    overlap_seconds: u64,
    namespace: Namespace,
) -> (Tag, Option<Tag>) {
    let cur_epoch = current_epoch(now_seconds, epoch_seconds);
    let current = derive_rv_tag(recipient_pubkey, cur_epoch, namespace);
    if in_next_epoch_overlap(now_seconds, epoch_seconds, overlap_seconds) {
        let next_epoch = Epoch(cur_epoch.0.saturating_add(1));
        (
            current,
            Some(derive_rv_tag(recipient_pubkey, next_epoch, namespace)),
        )
    } else {
        (current, None)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        current_epoch, derive_channel_feed_tag, derive_channel_namespace, derive_channel_rv_tag,
        derive_feed_tag, derive_rv_tag, derive_rv_tag_window, in_next_epoch_overlap,
        normalize_channel_id,
    };
    use crate::hash::blake3_32;
    use crate::types::{Epoch, Namespace};

    #[test]
    fn feed_tag_uses_feed_domain_separator_and_be_namespace() {
        let publisher_pubkey = [0x11_u8; 32];

        let mut expected_preimage = Vec::with_capacity(4 + 32 + 2);
        expected_preimage.extend_from_slice(b"feed");
        expected_preimage.extend_from_slice(&publisher_pubkey);
        expected_preimage.extend_from_slice(&7_u16.to_be_bytes());

        let expected = blake3_32(&expected_preimage);
        let actual = derive_feed_tag(&publisher_pubkey, Namespace(7));

        assert_eq!(actual, expected);
    }

    #[test]
    fn rv_tag_uses_rv_domain_separator_and_be_fields() {
        let recipient_pubkey = [0x22_u8; 32];

        let mut expected_preimage = Vec::with_capacity(2 + 32 + 4 + 2);
        expected_preimage.extend_from_slice(b"rv");
        expected_preimage.extend_from_slice(&recipient_pubkey);
        expected_preimage.extend_from_slice(&123_456_u32.to_be_bytes());
        expected_preimage.extend_from_slice(&7_u16.to_be_bytes());

        let expected = blake3_32(&expected_preimage);
        let actual = derive_rv_tag(&recipient_pubkey, Epoch(123_456), Namespace(7));

        assert_eq!(actual, expected);
    }

    #[test]
    fn tag_derivation_has_domain_separation() {
        let key = [0x42_u8; 32];
        let ns = Namespace(9);
        let epoch = Epoch(99);

        let feed = derive_feed_tag(&key, ns);
        let rv = derive_rv_tag(&key, epoch, ns);

        assert_ne!(feed, rv);
    }

    #[test]
    fn tag_derivation_is_deterministic() {
        let key = [0xAB_u8; 32];
        let ns = Namespace(99);
        let epoch = Epoch(1024);

        assert_eq!(derive_feed_tag(&key, ns), derive_feed_tag(&key, ns));
        assert_eq!(
            derive_rv_tag(&key, epoch, ns),
            derive_rv_tag(&key, epoch, ns)
        );
    }

    #[test]
    fn channel_id_normalization_is_stable() {
        assert_eq!(normalize_channel_id(" General "), "general");
        assert_eq!(normalize_channel_id(""), "general");
    }

    #[test]
    fn channel_namespace_derivation_matches_expected_vectors() {
        let base = Namespace(7);
        assert_eq!(derive_channel_namespace(base, "general"), Namespace(8_562));
        assert_eq!(derive_channel_namespace(base, "dev"), Namespace(38_851));
        assert_eq!(derive_channel_namespace(base, "media"), Namespace(57_098));
        assert_eq!(
            derive_channel_namespace(base, " General "),
            Namespace(8_562)
        );
        assert_eq!(derive_channel_namespace(base, ""), Namespace(8_562));
    }

    #[test]
    fn channel_scoped_tag_derivation_is_deterministic_and_separated() {
        let key = [0xCD_u8; 32];
        let base = Namespace(7);
        let epoch = Epoch(123);
        let general_feed = derive_channel_feed_tag(&key, base, "general");
        let dev_feed = derive_channel_feed_tag(&key, base, "dev");
        assert_ne!(general_feed, dev_feed);
        assert_eq!(
            general_feed,
            derive_channel_feed_tag(&key, base, " General "),
        );

        let general_rv = derive_channel_rv_tag(&key, epoch, base, "general");
        let dev_rv = derive_channel_rv_tag(&key, epoch, base, "dev");
        assert_ne!(general_rv, dev_rv);
    }

    #[test]
    fn overlap_window_derives_next_rv_tag_near_boundary() {
        let key = [0xAB; 32];
        let ns = Namespace(7);
        let epoch_seconds = 86_400;
        let overlap_seconds = 3_600;
        let now = epoch_seconds - 30;
        let (current, next) = derive_rv_tag_window(&key, now, epoch_seconds, overlap_seconds, ns);
        let expected_current = derive_rv_tag(&key, Epoch(0), ns);
        let expected_next = derive_rv_tag(&key, Epoch(1), ns);
        assert_eq!(current, expected_current);
        assert_eq!(next, Some(expected_next));
    }

    #[test]
    fn overlap_window_outside_tail_has_no_next_tag() {
        let key = [0xCD; 32];
        let ns = Namespace(9);
        let (current, next) = derive_rv_tag_window(&key, 10, 86_400, 3_600, ns);
        assert_eq!(current, derive_rv_tag(&key, Epoch(0), ns));
        assert!(next.is_none());
    }

    #[test]
    fn current_epoch_and_overlap_helpers_are_stable() {
        assert_eq!(current_epoch(172_800, 86_400), Epoch(2));
        assert!(!in_next_epoch_overlap(10, 100, 20));
        assert!(in_next_epoch_overlap(95, 100, 20));
        assert!(in_next_epoch_overlap(95, 100, 200));
    }
}
