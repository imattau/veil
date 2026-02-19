use std::collections::HashSet;

use super::ingest_routes::queue_raw_object_payload;
use super::*;

pub(super) fn unique_share_recipients(
    member_pubkeys: &[String],
    sender_pubkey_hex: &str,
) -> Vec<String> {
    let sender = sender_pubkey_hex.to_ascii_lowercase();
    let mut seen = HashSet::new();
    let mut recipients = Vec::new();
    for member in member_pubkeys {
        let normalized = member.to_ascii_lowercase();
        if normalized == sender {
            continue;
        }
        if seen.insert(normalized.clone()) {
            recipients.push(normalized);
        }
    }
    recipients
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn queue_group_key_shares(
    state: &AppState,
    namespace: u16,
    group_id: &str,
    sender_pubkey_hex: &str,
    sender_secret: [u8; 32],
    key_id: &str,
    group_key: [u8; 32],
    member_pubkeys: &[String],
) -> usize {
    let mut shares = 0usize;
    for member in member_pubkeys {
        if member == sender_pubkey_hex {
            continue;
        }
        let payload = match encrypt_group_key_share_payload(
            sender_secret,
            sender_pubkey_hex,
            member,
            group_id,
            key_id,
            group_key,
        ) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if queue_raw_object_payload(state, namespace, &payload)
            .await
            .is_ok()
        {
            shares += 1;
        }
    }
    shares
}
