#[cfg(test)]
use std::collections::HashMap;

use crate::api::ContactBundle;

use super::dynamic_peers::DynamicPeerStore;
use super::peer_lists::merge_unique_peers;

pub(super) fn add_dynamic_contact(dynamic: &mut DynamicPeerStore, contact: &ContactBundle) {
    dynamic.add_contact(contact);
}

pub(super) fn replace_dynamic_contacts(dynamic: &mut DynamicPeerStore, contacts: &[ContactBundle]) {
    dynamic.replace_from_contacts(contacts);
}

pub(super) fn merged_dynamic_publish_peers(
    configured_fast: &[String],
    configured_fallback: &[String],
    dynamic: &DynamicPeerStore,
) -> (Vec<String>, Vec<String>) {
    let fast = merge_unique_peers(configured_fast, dynamic.fast_peers());
    let fallback = merge_unique_peers(configured_fallback, dynamic.fallback_peers());
    (fast, fallback)
}

#[cfg(test)]
pub(super) fn dynamic_peer_snapshot_from_store(
    dynamic: &DynamicPeerStore,
) -> (Vec<String>, Vec<String>) {
    let (fast, fallback, _) = dynamic.snapshots();
    (fast, fallback)
}

#[cfg(test)]
pub(super) fn dynamic_peer_map_snapshot_from_store(
    dynamic: &DynamicPeerStore,
) -> HashMap<String, [u8; 32]> {
    let (_, _, peer_map) = dynamic.snapshots();
    peer_map
}
