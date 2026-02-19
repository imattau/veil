use super::*;

#[test]
fn dm_encrypt_decrypt_round_trip() {
    let sender_secret = [7u8; 32];
    let recipient_secret = [9u8; 32];
    let sender_pubkey = pubkey_hex_from_secret(sender_secret).expect("sender pubkey");
    let recipient_pubkey = pubkey_hex_from_secret(recipient_secret).expect("recipient pubkey");
    let payload = encrypt_direct_message_payload(
        sender_secret,
        &sender_pubkey,
        &recipient_pubkey,
        b"hello dm",
    )
    .expect("encrypt");
    let decrypted = decrypt_direct_message_payload(recipient_secret, &payload).expect("decrypt");
    assert_eq!(decrypted, b"hello dm");
}

#[test]
fn group_encrypt_decrypt_round_trip() {
    let payload =
        encrypt_group_message_payload("group-1", "k1", [5u8; 32], b"hello group").expect("encrypt");
    let decrypted = decrypt_group_message_payload(&payload, |group_id, key_id| {
        if group_id == "group-1" && key_id == "k1" {
            Some([5u8; 32])
        } else {
            None
        }
    })
    .expect("decrypt");
    assert_eq!(decrypted, b"hello group");
}

#[test]
fn group_key_share_round_trip() {
    let sender_secret = [7u8; 32];
    let recipient_secret = [9u8; 32];
    let sender_pubkey = pubkey_hex_from_secret(sender_secret).expect("sender");
    let recipient_pubkey = pubkey_hex_from_secret(recipient_secret).expect("recipient");
    let payload = encrypt_group_key_share_payload(
        sender_secret,
        &sender_pubkey,
        &recipient_pubkey,
        "group-1",
        "k1",
        [8u8; 32],
    )
    .expect("encrypt");
    let material = decrypt_group_key_share_payload(recipient_secret, &payload).expect("decrypt");
    assert_eq!(material.group_id, "group-1");
    assert_eq!(material.key_id, "k1");
    assert_eq!(material.key, [8u8; 32]);
}

#[test]
fn pubkey_from_nostr_hex_rejects_invalid_hex() {
    assert!(pubkey_from_nostr_hex("abcd").is_err());
    assert!(pubkey_from_nostr_hex(&"zz".repeat(32)).is_err());
}

#[test]
fn pubkey_from_nostr_hex_accepts_valid_pubkey() {
    let secret = [7u8; 32];
    let pubkey_hex = pubkey_hex_from_secret(secret).expect("pubkey");
    assert!(pubkey_from_nostr_hex(&pubkey_hex).is_ok());
}
