use crate::api::ContactBundle;
use crate::discovery::discovery_tag;
use veil_core::tags::derive_channel_feed_tag;
use veil_core::Namespace;

use super::config_helpers::decode_hex_32;

pub(super) fn build_subscription_tags(
    channels: &[String],
    contacts: &[ContactBundle],
    my_pubkey: [u8; 32],
    namespace: Namespace,
    discovery_namespace: Namespace,
) -> Vec<[u8; 32]> {
    let mut tags = Vec::new();

    // Always include discovery namespace subscription.
    tags.push(discovery_tag(discovery_namespace));

    // For each channel add direct tag (if hex) or derive self/contact channel tags.
    for channel in channels {
        if let Some(tag) = decode_hex_32(channel) {
            tags.push(tag);
            continue;
        }

        tags.push(derive_channel_feed_tag(&my_pubkey, namespace, channel));
        for contact in contacts {
            if let Some(pubkey) = decode_hex_32(&contact.pubkey_hex) {
                tags.push(derive_channel_feed_tag(&pubkey, namespace, channel));
            }
        }
    }

    tags
}

#[cfg(test)]
mod tests {
    use super::build_subscription_tags;
    use crate::api::ContactBundle;
    use crate::discovery::discovery_tag;
    use veil_core::tags::derive_channel_feed_tag;
    use veil_core::Namespace;

    fn contact(pubkey_hex: &str) -> ContactBundle {
        ContactBundle {
            peer_id: "peer".to_string(),
            ws_url: None,
            quic_addr: None,
            pubkey_hex: pubkey_hex.to_string(),
            rpc_url: None,
            lan_addrs: Vec::new(),
        }
    }

    #[test]
    fn includes_discovery_and_channel_tags_for_self_and_contacts() {
        let channels = vec!["general".to_string()];
        let contacts = vec![contact(&"22".repeat(32))];
        let my_pubkey = [0x11; 32];
        let namespace = Namespace(32);
        let discovery_namespace = Namespace(4096);

        let tags = build_subscription_tags(
            &channels,
            &contacts,
            my_pubkey,
            namespace,
            discovery_namespace,
        );

        assert_eq!(tags[0], discovery_tag(discovery_namespace));
        assert!(tags.contains(&derive_channel_feed_tag(&my_pubkey, namespace, "general")));
        assert!(tags.contains(&derive_channel_feed_tag(&[0x22; 32], namespace, "general")));
    }

    #[test]
    fn hex_channel_is_added_directly_without_contact_expansion() {
        let direct_tag = [0xAB; 32];
        let channels = vec![hex::encode(direct_tag)];
        let contacts = vec![contact(&"22".repeat(32))];

        let tags = build_subscription_tags(
            &channels,
            &contacts,
            [0x11; 32],
            Namespace(32),
            Namespace(4096),
        );

        assert!(tags.contains(&direct_tag));
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn invalid_contact_pubkey_is_ignored() {
        let channels = vec!["general".to_string()];
        let contacts = vec![contact("invalid")];
        let namespace = Namespace(32);

        let tags =
            build_subscription_tags(&channels, &contacts, [0x11; 32], namespace, Namespace(4096));

        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&derive_channel_feed_tag(&[0x11; 32], namespace, "general")));
    }
}
