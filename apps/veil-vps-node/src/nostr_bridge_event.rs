use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use veil_core::ObjectRoot;
use veil_crypto::signing::{NostrVerifier, Verifier};
use veil_schema_feed::{BundleMeta, FeedBundle, PostBundle};

pub(crate) const NOSTR_EVENT_MAX_CONTENT_BYTES: usize = 16 * 1024;
pub(crate) const NOSTR_MAX_EVENT_FRAME_BYTES: usize = 256 * 1024;

#[derive(Debug, Deserialize)]
pub(crate) struct NostrEvent {
    pub(crate) id: String,
    pub(crate) pubkey: String,
    pub(crate) kind: u64,
    pub(crate) created_at: u64,
    #[serde(default = "empty_nostr_tags")]
    pub(crate) tags: serde_json::Value,
    pub(crate) content: String,
    #[serde(default)]
    pub(crate) sig: Option<String>,
}

fn empty_nostr_tags() -> serde_json::Value {
    serde_json::Value::Array(Vec::new())
}

pub(crate) fn parse_nostr_event_message(input: &str) -> Option<NostrEvent> {
    if input.len() > NOSTR_MAX_EVENT_FRAME_BYTES {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(input).ok()?;
    let array = value.as_array()?;
    if array.len() < 3 {
        return None;
    }
    if array.first()?.as_str()? != "EVENT" {
        return None;
    }
    serde_json::from_value(array[2].clone()).ok()
}

pub(crate) fn verify_nostr_event_authenticity(event: &NostrEvent) -> bool {
    if !valid_hex_64(&event.id) || !valid_hex_64(&event.pubkey) {
        return false;
    }
    let Some(sig_hex) = event.sig.as_deref() else {
        return false;
    };
    if !valid_hex_128(sig_hex) {
        return false;
    }
    let Some(expected_event_id) = compute_nostr_event_id_hex(event) else {
        return false;
    };
    if !event.id.eq_ignore_ascii_case(&expected_event_id) {
        return false;
    }
    let Ok(id_vec) = hex::decode(&event.id) else {
        return false;
    };
    let Ok(pubkey_vec) = hex::decode(&event.pubkey) else {
        return false;
    };
    let Ok(sig_vec) = hex::decode(sig_hex) else {
        return false;
    };
    let Ok(id_bytes) = <[u8; 32]>::try_from(id_vec.as_slice()) else {
        return false;
    };
    let Ok(pubkey) = <[u8; 32]>::try_from(pubkey_vec.as_slice()) else {
        return false;
    };
    let Ok(sig) = <[u8; 64]>::try_from(sig_vec.as_slice()) else {
        return false;
    };
    NostrVerifier
        .verify(pubkey, &id_bytes, sig)
        .unwrap_or(false)
}

fn compute_nostr_event_id_hex(event: &NostrEvent) -> Option<String> {
    if !event.tags.is_array() {
        return None;
    }
    let canonical = json!([
        0,
        event.pubkey,
        event.created_at,
        event.kind,
        event.tags,
        event.content
    ]);
    let encoded = serde_json::to_vec(&canonical).ok()?;
    let digest = Sha256::digest(encoded);
    Some(hex::encode(digest))
}

pub(crate) fn map_event_to_payload(
    event: &NostrEvent,
    channel_id: &str,
    _namespace: u16,
) -> Option<Vec<u8>> {
    if event.kind != 1 {
        return None;
    }
    if !valid_hex_64(&event.id) || !valid_hex_64(&event.pubkey) {
        return None;
    }
    let text = event.content.trim();
    if text.is_empty() {
        return None;
    }
    if text.len() > NOSTR_EVENT_MAX_CONTENT_BYTES {
        return None;
    }
    let bundle = FeedBundle::Post(PostBundle {
        meta: BundleMeta {
            version: 1,
            created_at: event.created_at,
        },
        channel_id: channel_id.to_string(),
        author_pubkey_hex: event.pubkey.clone(),
        text: text.to_string(),
        media_roots: Vec::<ObjectRoot>::new(),
        reply_to_root: None,
    });
    serde_json::to_vec(&bundle).ok()
}

fn valid_hex_64(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
}

pub(crate) fn canonical_hex_64(value: &str) -> Option<String> {
    if !valid_hex_64(value) {
        return None;
    }
    Some(value.to_ascii_lowercase())
}

fn valid_hex_128(value: &str) -> bool {
    value.len() == 128 && value.chars().all(|c| c.is_ascii_hexdigit())
}
