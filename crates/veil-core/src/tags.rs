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

#[cfg(test)]
mod tests {
    use super::{
        derive_channel_feed_tag, derive_channel_namespace, derive_channel_rv_tag, derive_feed_tag,
        derive_rv_tag, normalize_channel_id,
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
}
