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

pub(in crate::server) fn valid_pubkey_hex(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
}

pub(in crate::server) fn valid_channel(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_CHANNEL_LEN
}

pub(in crate::server) fn hex_to_pubkey(value: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    if let Ok(bytes) = hex::decode(value) {
        if bytes.len() == 32 {
            out.copy_from_slice(&bytes);
        }
    }
    out
}

pub(in crate::server) fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
