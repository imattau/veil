use std::path::Path;
use std::time::Duration;

use tracing::{info, warn};
use veil_core::tags::derive_channel_feed_tag;
use veil_core::{Namespace, Tag};
use veil_node::state::NodeState;

use crate::nostr_bridge::{start_nostr_bridge, BridgedItem, NostrBridgeConfig};

pub(super) struct BridgeSetupInputs<'a> {
    pub enabled: bool,
    pub relays: &'a [String],
    pub channel_id: &'a str,
    pub namespace: u16,
    pub since: Duration,
    pub state_path: &'a Path,
    pub max_seen_ids: usize,
    pub persist_every_updates: usize,
}

pub(super) struct BridgeRuntimeSetup {
    pub bridge_namespace: Namespace,
    pub bridge_tag: Tag,
    pub nostr_bridge_rx: Option<tokio::sync::mpsc::Receiver<BridgedItem>>,
}

pub(super) fn init_bridge_setup(
    state: &mut NodeState,
    node_pubkey: &[u8; 32],
    inputs: BridgeSetupInputs<'_>,
) -> BridgeRuntimeSetup {
    let bridge_namespace = Namespace(inputs.namespace);
    let bridge_tag = derive_channel_feed_tag(node_pubkey, bridge_namespace, inputs.channel_id);
    state.subscriptions.insert(bridge_tag);

    let nostr_bridge_rx = if inputs.enabled {
        if inputs.relays.is_empty() {
            warn!("nostr bridge enabled but VEIL_VPS_NOSTR_BRIDGE_RELAYS is empty; bridge not started");
            None
        } else {
            info!(
                "nostr bridge enabled with {} relays ({:?}), channel={}, namespace={}",
                inputs.relays.len(),
                inputs.relays,
                inputs.channel_id,
                inputs.namespace
            );
            Some(start_nostr_bridge(NostrBridgeConfig {
                relays: inputs.relays.to_vec(),
                channel_id: inputs.channel_id.to_string(),
                namespace: inputs.namespace,
                since: inputs.since,
                state_path: Some(inputs.state_path.to_path_buf()),
                max_seen_ids: inputs.max_seen_ids,
                persist_every_updates: inputs.persist_every_updates,
            }))
        }
    } else {
        None
    };

    BridgeRuntimeSetup {
        bridge_namespace,
        bridge_tag,
        nostr_bridge_rx,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Duration;

    use veil_core::tags::derive_channel_feed_tag;
    use veil_core::Namespace;
    use veil_node::state::NodeState;

    use super::{init_bridge_setup, BridgeSetupInputs};

    #[test]
    fn init_bridge_setup_adds_channel_subscription() {
        let mut state = NodeState::default();
        let node_pubkey = [0x11_u8; 32];

        let setup = init_bridge_setup(
            &mut state,
            &node_pubkey,
            BridgeSetupInputs {
                enabled: false,
                relays: &[],
                channel_id: "general",
                namespace: 2048,
                since: Duration::from_secs(60),
                state_path: Path::new("nostr-state.json"),
                max_seen_ids: 1024,
                persist_every_updates: 32,
            },
        );

        let expected = derive_channel_feed_tag(&node_pubkey, Namespace(2048), "general");
        assert_eq!(setup.bridge_tag, expected);
        assert!(state.subscriptions.contains(&expected));
        assert!(setup.nostr_bridge_rx.is_none());
    }

    #[test]
    fn init_bridge_setup_skips_bridge_when_relays_missing() {
        let mut state = NodeState::default();
        let node_pubkey = [0x22_u8; 32];

        let setup = init_bridge_setup(
            &mut state,
            &node_pubkey,
            BridgeSetupInputs {
                enabled: true,
                relays: &[],
                channel_id: "bridge",
                namespace: 7,
                since: Duration::from_secs(60),
                state_path: Path::new("nostr-state.json"),
                max_seen_ids: 64,
                persist_every_updates: 8,
            },
        );

        assert!(setup.nostr_bridge_rx.is_none());
    }
}
