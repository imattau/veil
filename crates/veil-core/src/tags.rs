use crate::{
    hash::blake3_32,
    types::{Epoch, Namespace, Tag},
};

/// Derives a stable public feed tag:
/// `H("feed" || publisher_pubkey || namespace_be)`.
pub fn derive_feed_tag(publisher_pubkey: &[u8; 32], namespace: Namespace) -> Tag {
    let mut buf = Vec::with_capacity(4 + 32 + 2);
    buf.extend_from_slice(b"feed");
    buf.extend_from_slice(publisher_pubkey);
    buf.extend_from_slice(&namespace.0.to_be_bytes());
    blake3_32(&buf)
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

#[cfg(test)]
mod tests {
    use super::{derive_feed_tag, derive_rv_tag};
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
}
