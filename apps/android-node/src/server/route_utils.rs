use std::time::{SystemTime, UNIX_EPOCH};

use subtle::ConstantTimeEq;

use super::*;

pub(in crate::server) fn authorized(headers: &HeaderMap, token: &str) -> bool {
    if token.trim().is_empty() {
        return false;
    }
    let provided = headers
        .get("x-veil-token")
        .and_then(|value| value.to_str().ok())
        .map(str::as_bytes);
    let expected = token.as_bytes();
    match provided {
        Some(value) if value.len() == expected.len() => value.ct_eq(expected).into(),
        _ => false,
    }
}

pub(in crate::server) fn bad_request(code: &str, message: &str) -> Response {
    let payload = ErrorResponse {
        code: code.to_string(),
        message: message.to_string(),
    };
    (StatusCode::BAD_REQUEST, Json(payload)).into_response()
}

pub(in crate::server) fn serialize_feed_bundle_payload(
    feed_bundle: &FeedBundle,
    max_bytes: Option<usize>,
) -> Result<String, Response> {
    let payload = match serde_json::to_string(feed_bundle) {
        Ok(value) => value,
        Err(_) => return Err(bad_request("invalid_bundle", "bundle serialization failed")),
    };
    if let Some(limit) = max_bytes {
        if payload.len() > limit {
            return Err(bad_request("bundle_too_large", "bundle exceeds max size"));
        }
    }
    Ok(payload)
}

pub(in crate::server) fn enqueue_and_inject_feed_bundle(
    state: &AppState,
    namespace: u16,
    payload: String,
    feed_bundle: &FeedBundle,
) -> uuid::Uuid {
    let bundle_value = serde_json::to_value(feed_bundle).unwrap_or_default();
    let message_id = state
        .node
        .enqueue_publish(PublishRequest { namespace, payload });
    state
        .node
        .inject_local_feed_bundle(bundle_value, blake3::hash(message_id.as_bytes()).into());
    message_id
}

pub(in crate::server) fn valid_pubkey_hex(value: &str) -> bool {
    parse_hex_32(value).is_some()
}

pub(in crate::server) fn valid_channel(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_CHANNEL_LEN
}

pub(in crate::server) fn hex_to_pubkey(value: &str) -> [u8; 32] {
    parse_hex_32(value).unwrap_or([0u8; 32])
}

pub(in crate::server) fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn parse_hex_32(value: &str) -> Option<[u8; 32]> {
    <[u8; 32] as hex::FromHex>::from_hex(value).ok()
}

#[cfg(test)]
mod tests {
    use super::{hex_to_pubkey, valid_pubkey_hex};

    #[test]
    fn valid_pubkey_hex_requires_exact_32_bytes() {
        assert!(valid_pubkey_hex(&"11".repeat(32)));
        assert!(!valid_pubkey_hex("11"));
        assert!(!valid_pubkey_hex(&"zz".repeat(32)));
    }

    #[test]
    fn hex_to_pubkey_returns_zeroes_for_invalid_input() {
        assert_eq!(hex_to_pubkey("zz"), [0u8; 32]);
    }
}
