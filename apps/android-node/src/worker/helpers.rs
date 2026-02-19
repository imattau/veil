use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::api::QueueWorkerConfig;

pub(super) fn normalize_worker_config(config: QueueWorkerConfig) -> QueueWorkerConfig {
    let tick_ms = config.tick_ms.max(50);
    let max_attempts = config.max_attempts.max(1);
    let backoff_base_ms = config.backoff_base_ms.max(50);
    let backoff_max_ms = config.backoff_max_ms.max(backoff_base_ms);
    QueueWorkerConfig {
        tick_ms,
        max_attempts,
        backoff_base_ms,
        backoff_max_ms,
    }
}

fn retry_backoff_ms(attempts: u32, base_ms: u64, max_ms: u64) -> u64 {
    let exponent = attempts.saturating_sub(1).min(10);
    let factor = 2u32.saturating_pow(exponent);
    let base = Duration::from_millis(base_ms);
    let max = Duration::from_millis(max_ms);
    let scaled = base.saturating_mul(factor);
    let bounded = scaled.clamp(base, max);
    bounded.as_millis().try_into().unwrap_or(u64::MAX)
}

pub(super) fn retry_backoff_with_jitter_ms(
    attempts: u32,
    base_ms: u64,
    max_ms: u64,
    jitter_key: u128,
) -> u64 {
    let upper = retry_backoff_ms(attempts, base_ms, max_ms);
    if upper <= base_ms {
        return upper;
    }
    let span = upper - base_ms;
    let mut seed = [0_u8; 32];
    seed[..16].copy_from_slice(&jitter_key.to_le_bytes());
    seed[16..20].copy_from_slice(&attempts.to_le_bytes());
    let mut rng = StdRng::from_seed(seed);
    let jitter = rng.gen_range(0..=span);
    base_ms.saturating_add(jitter)
}

pub(super) fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(serde::Deserialize)]
struct QueuedPayloadEnvelope {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    payload_b64: Option<String>,
}

pub(super) fn queued_payload_to_bytes(payload: &str) -> Vec<u8> {
    let parsed = match serde_json::from_str::<QueuedPayloadEnvelope>(payload) {
        Ok(value) => value,
        Err(_) => return payload.as_bytes().to_vec(),
    };
    match parsed.kind.as_deref() {
        Some("raw_object_b64") | Some("raw_b64") => {
            let Some(b64) = parsed.payload_b64.as_deref() else {
                return payload.as_bytes().to_vec();
            };
            base64::engine::general_purpose::STANDARD
                .decode(b64)
                .unwrap_or_else(|_| payload.as_bytes().to_vec())
        }
        _ => payload.as_bytes().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{
        normalize_worker_config, queued_payload_to_bytes, retry_backoff_ms,
        retry_backoff_with_jitter_ms,
    };
    use crate::api::QueueWorkerConfig;

    #[test]
    fn decode_wrapped_raw_b64_payload() {
        let wrapped = serde_json::json!({
            "kind": "raw_b64",
            "payload_b64": "aGVsbG8=",
        })
        .to_string();
        let bytes = queued_payload_to_bytes(&wrapped);
        assert_eq!(bytes, b"hello");
    }

    #[test]
    fn non_wrapped_payload_falls_back_to_utf8_bytes() {
        let payload = r#"{"kind":"post","text":"hello"}"#;
        let bytes = queued_payload_to_bytes(payload);
        assert_eq!(bytes, payload.as_bytes());
    }

    #[test]
    fn wrapped_payload_without_payload_b64_falls_back_to_utf8_bytes() {
        let payload = r#"{"kind":"raw_b64"}"#;
        let bytes = queued_payload_to_bytes(payload);
        assert_eq!(bytes, payload.as_bytes());
    }

    #[test]
    fn normalize_worker_config_enforces_safe_minimums() {
        let normalized = normalize_worker_config(QueueWorkerConfig {
            tick_ms: 0,
            max_attempts: 0,
            backoff_base_ms: 0,
            backoff_max_ms: 0,
        });
        assert_eq!(normalized.tick_ms, 50);
        assert_eq!(normalized.max_attempts, 1);
        assert_eq!(normalized.backoff_base_ms, 50);
        assert_eq!(normalized.backoff_max_ms, 50);
    }

    #[test]
    fn normalize_worker_config_preserves_valid_values() {
        let normalized = normalize_worker_config(QueueWorkerConfig {
            tick_ms: 500,
            max_attempts: 3,
            backoff_base_ms: 500,
            backoff_max_ms: 20_000,
        });
        assert_eq!(normalized.tick_ms, 500);
        assert_eq!(normalized.max_attempts, 3);
        assert_eq!(normalized.backoff_base_ms, 500);
        assert_eq!(normalized.backoff_max_ms, 20_000);
    }

    #[test]
    fn retry_backoff_ms_scales_and_caps() {
        assert_eq!(retry_backoff_ms(1, 500, 20_000), 500);
        assert_eq!(retry_backoff_ms(2, 500, 20_000), 1_000);
        assert_eq!(retry_backoff_ms(3, 500, 20_000), 2_000);
        assert_eq!(retry_backoff_ms(20, 500, 20_000), 20_000);
    }

    #[test]
    fn retry_backoff_with_jitter_stays_bounded_and_varies_by_key() {
        let base = 500;
        let max = 20_000;
        let attempts = 4;
        let upper = retry_backoff_ms(attempts, base, max);
        let mut seen = HashSet::new();
        for key in 0u128..16 {
            let value = retry_backoff_with_jitter_ms(attempts, base, max, key);
            assert!(value >= base);
            assert!(value <= upper);
            seen.insert(value);
        }
        assert!(seen.len() > 1);
    }
}
