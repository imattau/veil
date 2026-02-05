use crate::state::NodeState;
use veil_core::tags::derive_rv_tag_window;
use veil_core::{Namespace, Tag};

/// Subscribes to one explicit tag. Returns true when newly added.
pub fn subscribe_tag(node: &mut NodeState, tag: Tag) -> bool {
    node.subscriptions.insert(tag)
}

/// Subscribes to rotating rendezvous tags with optional next-epoch overlap.
///
/// Returns the number of new tags inserted into `node.subscriptions`.
pub fn subscribe_rv_tag_window(
    node: &mut NodeState,
    recipient_pubkey: &[u8; 32],
    namespace: Namespace,
    now_seconds: u64,
    epoch_seconds: u64,
    overlap_seconds: u64,
) -> usize {
    let (current, next) = derive_rv_tag_window(
        recipient_pubkey,
        now_seconds,
        epoch_seconds,
        overlap_seconds,
        namespace,
    );
    let mut added = 0_usize;
    if node.subscriptions.insert(current) {
        added += 1;
    }
    if let Some(next_tag) = next {
        if node.subscriptions.insert(next_tag) {
            added += 1;
        }
    }
    added
}

#[cfg(test)]
mod tests {
    use super::{subscribe_rv_tag_window, subscribe_tag};
    use crate::state::NodeState;
    use veil_core::tags::derive_rv_tag;
    use veil_core::{Epoch, Namespace};

    #[test]
    fn subscribe_tag_inserts_once() {
        let mut node = NodeState::default();
        let tag = [0x11; 32];
        assert!(subscribe_tag(&mut node, tag));
        assert!(!subscribe_tag(&mut node, tag));
    }

    #[test]
    fn subscribe_rv_window_adds_current_and_next_in_tail() {
        let mut node = NodeState::default();
        let key = [0x22; 32];
        let ns = Namespace(7);
        let added = subscribe_rv_tag_window(&mut node, &key, ns, 86_390, 86_400, 3_600);
        assert_eq!(added, 2);
        assert!(node
            .subscriptions
            .contains(&derive_rv_tag(&key, Epoch(0), ns)));
        assert!(node
            .subscriptions
            .contains(&derive_rv_tag(&key, Epoch(1), ns)));
    }

    #[test]
    fn subscribe_rv_window_only_adds_current_outside_tail() {
        let mut node = NodeState::default();
        let key = [0x33; 32];
        let ns = Namespace(9);
        let added = subscribe_rv_tag_window(&mut node, &key, ns, 100, 86_400, 3_600);
        assert_eq!(added, 1);
        assert!(node
            .subscriptions
            .contains(&derive_rv_tag(&key, Epoch(0), ns)));
    }
}
