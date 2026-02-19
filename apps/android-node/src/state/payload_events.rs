use base64::Engine;

use crate::secure_message::{
    decrypt_direct_message_payload, decrypt_group_key_share_payload, decrypt_group_message_payload,
};

use super::payload_parsing::parse_feed_bundles;
use super::{emit_event_locked, StateInner};

pub(super) fn emit_payload_events(
    inner: &mut StateInner,
    object_root: &[u8; 32],
    payload: &[u8],
    namespace: u16,
    epoch: u32,
    tag: &[u8; 32],
    flags: u16,
) -> bool {
    let mut group_key_updated = false;
    if let Some(material) = decrypt_group_key_share_payload(inner.identity.secret_key, payload) {
        inner
            .group_keys
            .entry(material.group_id.clone())
            .or_default()
            .insert(material.key_id.clone(), material.key);
        emit_event_locked(
            inner,
            "group_key_updated",
            serde_json::json!({
                "group_id": material.group_id,
                "key_id": material.key_id,
            }),
        );
        group_key_updated = true;
    }

    let payload_for_event = decrypt_direct_message_payload(inner.identity.secret_key, payload)
        .or_else(|| {
            decrypt_group_message_payload(payload, |group_id, key_id| {
                inner
                    .group_keys
                    .get(group_id)
                    .and_then(|keys| keys.get(key_id))
                    .copied()
            })
        })
        .unwrap_or_else(|| payload.to_vec());
    emit_event_locked(
        inner,
        "payload",
        serde_json::json!({
            "object_root": hex::encode(object_root),
            "payload_b64": base64::engine::general_purpose::STANDARD.encode(payload_for_event),
            "namespace": namespace,
            "epoch": epoch,
            "tag": hex::encode(tag),
            "flags": flags,
        }),
    );

    let object_root_hex = hex::encode(object_root);
    for bundle in parse_feed_bundles(payload) {
        let mut value = serde_json::to_value(bundle).unwrap_or_default();
        attach_object_root(&mut value, &object_root_hex);
        emit_event_locked(inner, "feed_bundle", value);
    }

    group_key_updated
}

pub(super) fn emit_local_feed_bundle_event(
    inner: &mut StateInner,
    mut bundle: serde_json::Value,
    object_root: [u8; 32],
) {
    attach_object_root(&mut bundle, &hex::encode(object_root));
    emit_event_locked(inner, "feed_bundle", bundle);
}

fn attach_object_root(value: &mut serde_json::Value, object_root_hex: &str) {
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "object_root".to_string(),
            serde_json::json!(object_root_hex),
        );
    }
}
