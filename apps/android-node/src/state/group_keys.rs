use super::identity_group_keys::generate_group_key_entry;
use super::StateInner;

pub(super) fn ensure_group_key(inner: &mut StateInner, group_id: &str) -> (String, [u8; 32]) {
    if let Some(keys) = inner.group_keys.get(group_id) {
        if let Some((key_id, key)) = keys.iter().next() {
            return (key_id.clone(), *key);
        }
    }
    rotate_group_key(inner, group_id)
}

pub(super) fn rotate_group_key(inner: &mut StateInner, group_id: &str) -> (String, [u8; 32]) {
    let (key_id, key) = generate_group_key_entry();
    inner
        .group_keys
        .entry(group_id.to_string())
        .or_default()
        .insert(key_id.clone(), key);
    (key_id, key)
}
