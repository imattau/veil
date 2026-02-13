use base64::Engine;
use k256::ecdh::diffie_hellman;
use k256::{PublicKey, SecretKey};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use veil_crypto::aead::{AeadCipher, XChaCha20Poly1305Cipher};
use veil_crypto::signing::Signer;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DirectMessageCipherEnvelope {
    kind: String,
    sender_pubkey_hex: String,
    recipient_pubkey_hex: String,
    nonce_b64: String,
    ciphertext_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GroupMessageCipherEnvelope {
    kind: String,
    group_id: String,
    #[serde(default)]
    key_id: Option<String>,
    nonce_b64: String,
    ciphertext_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GroupKeyShareEnvelope {
    kind: String,
    group_id: String,
    key_id: String,
    sender_pubkey_hex: String,
    recipient_pubkey_hex: String,
    nonce_b64: String,
    ciphertext_b64: String,
}

#[derive(Debug, Clone)]
pub struct GroupKeyMaterial {
    pub group_id: String,
    pub key_id: String,
    pub key: [u8; 32],
}

const DM_KIND: &str = "dm_cipher_v1";
const GROUP_KIND_V1: &str = "group_cipher_v1";
const GROUP_KIND_V2: &str = "group_cipher_v2";
const GROUP_KEY_SHARE_KIND: &str = "group_key_share_v1";
const DM_AAD: &[u8] = b"veil-dm-v1";
const GROUP_AAD: &[u8] = b"veil-group-v2";
const GROUP_AAD_LEGACY: &[u8] = b"veil-group-v1";
const GROUP_KEY_SHARE_AAD: &[u8] = b"veil-group-key-share-v1";

pub fn encrypt_direct_message_payload(
    sender_secret: [u8; 32],
    sender_pubkey_hex: &str,
    recipient_pubkey_hex: &str,
    plaintext: &[u8],
) -> Result<Vec<u8>, String> {
    let key = derive_dm_key(sender_secret, recipient_pubkey_hex)?;
    let mut nonce = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305Cipher;
    let envelope = cipher
        .encrypt(&key, nonce, DM_AAD, plaintext)
        .map_err(|e| e.to_string())?;
    let payload = DirectMessageCipherEnvelope {
        kind: DM_KIND.to_string(),
        sender_pubkey_hex: sender_pubkey_hex.to_string(),
        recipient_pubkey_hex: recipient_pubkey_hex.to_string(),
        nonce_b64: base64::engine::general_purpose::STANDARD.encode(envelope.nonce),
        ciphertext_b64: base64::engine::general_purpose::STANDARD.encode(envelope.ciphertext),
    };
    serde_json::to_vec(&payload).map_err(|e| e.to_string())
}

pub fn decrypt_direct_message_payload(local_secret: [u8; 32], payload: &[u8]) -> Option<Vec<u8>> {
    let envelope = serde_json::from_slice::<DirectMessageCipherEnvelope>(payload).ok()?;
    if envelope.kind != DM_KIND {
        return None;
    }
    let local_pubkey_hex = pubkey_hex_from_secret(local_secret)?;
    let peer_pubkey_hex = if local_pubkey_hex == envelope.sender_pubkey_hex {
        envelope.recipient_pubkey_hex.as_str()
    } else if local_pubkey_hex == envelope.recipient_pubkey_hex {
        envelope.sender_pubkey_hex.as_str()
    } else {
        return None;
    };
    let key = derive_dm_key(local_secret, peer_pubkey_hex).ok()?;
    let nonce = base64::engine::general_purpose::STANDARD
        .decode(envelope.nonce_b64)
        .ok()?;
    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(envelope.ciphertext_b64)
        .ok()?;
    if nonce.len() != 24 {
        return None;
    }
    let mut nonce_arr = [0u8; 24];
    nonce_arr.copy_from_slice(&nonce);
    let cipher = XChaCha20Poly1305Cipher;
    cipher
        .decrypt(&key, nonce_arr, DM_AAD, &ciphertext)
        .ok()
        .map(|v| v.to_vec())
}

pub fn encrypt_group_message_payload(
    group_id: &str,
    key_id: &str,
    group_key: [u8; 32],
    plaintext: &[u8],
) -> Result<Vec<u8>, String> {
    let mut nonce = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305Cipher;
    let envelope = cipher
        .encrypt(&group_key, nonce, GROUP_AAD, plaintext)
        .map_err(|e| e.to_string())?;
    let payload = GroupMessageCipherEnvelope {
        kind: GROUP_KIND_V2.to_string(),
        group_id: group_id.to_string(),
        key_id: Some(key_id.to_string()),
        nonce_b64: base64::engine::general_purpose::STANDARD.encode(envelope.nonce),
        ciphertext_b64: base64::engine::general_purpose::STANDARD.encode(envelope.ciphertext),
    };
    serde_json::to_vec(&payload).map_err(|e| e.to_string())
}

pub fn decrypt_group_message_payload<F>(payload: &[u8], mut key_lookup: F) -> Option<Vec<u8>>
where
    F: FnMut(&str, &str) -> Option<[u8; 32]>,
{
    let envelope = serde_json::from_slice::<GroupMessageCipherEnvelope>(payload).ok()?;
    if envelope.kind != GROUP_KIND_V1 && envelope.kind != GROUP_KIND_V2 {
        return None;
    }
    let nonce = base64::engine::general_purpose::STANDARD
        .decode(envelope.nonce_b64)
        .ok()?;
    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(envelope.ciphertext_b64)
        .ok()?;
    if nonce.len() != 24 {
        return None;
    }
    let mut nonce_arr = [0u8; 24];
    nonce_arr.copy_from_slice(&nonce);
    let cipher = XChaCha20Poly1305Cipher;
    if envelope.kind == GROUP_KIND_V2 {
        let key_id = envelope.key_id.as_deref()?;
        let key = key_lookup(&envelope.group_id, key_id)?;
        return cipher
            .decrypt(&key, nonce_arr, GROUP_AAD, &ciphertext)
            .ok()
            .map(|v| v.to_vec());
    }
    let legacy_key = derive_group_key_legacy(&envelope.group_id);
    cipher
        .decrypt(&legacy_key, nonce_arr, GROUP_AAD_LEGACY, &ciphertext)
        .ok()
        .map(|v| v.to_vec())
}

pub fn encrypt_group_key_share_payload(
    sender_secret: [u8; 32],
    sender_pubkey_hex: &str,
    recipient_pubkey_hex: &str,
    group_id: &str,
    key_id: &str,
    group_key: [u8; 32],
) -> Result<Vec<u8>, String> {
    let key = derive_dm_key(sender_secret, recipient_pubkey_hex)?;
    let mut nonce = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305Cipher;
    let envelope = cipher
        .encrypt(&key, nonce, GROUP_KEY_SHARE_AAD, &group_key)
        .map_err(|e| e.to_string())?;
    let payload = GroupKeyShareEnvelope {
        kind: GROUP_KEY_SHARE_KIND.to_string(),
        group_id: group_id.to_string(),
        key_id: key_id.to_string(),
        sender_pubkey_hex: sender_pubkey_hex.to_string(),
        recipient_pubkey_hex: recipient_pubkey_hex.to_string(),
        nonce_b64: base64::engine::general_purpose::STANDARD.encode(envelope.nonce),
        ciphertext_b64: base64::engine::general_purpose::STANDARD.encode(envelope.ciphertext),
    };
    serde_json::to_vec(&payload).map_err(|e| e.to_string())
}

pub fn decrypt_group_key_share_payload(
    local_secret: [u8; 32],
    payload: &[u8],
) -> Option<GroupKeyMaterial> {
    let envelope = serde_json::from_slice::<GroupKeyShareEnvelope>(payload).ok()?;
    if envelope.kind != GROUP_KEY_SHARE_KIND {
        return None;
    }
    let local_pubkey_hex = pubkey_hex_from_secret(local_secret)?;
    let peer_pubkey_hex = if local_pubkey_hex == envelope.sender_pubkey_hex {
        envelope.recipient_pubkey_hex.as_str()
    } else if local_pubkey_hex == envelope.recipient_pubkey_hex {
        envelope.sender_pubkey_hex.as_str()
    } else {
        return None;
    };
    let key = derive_dm_key(local_secret, peer_pubkey_hex).ok()?;
    let nonce = base64::engine::general_purpose::STANDARD
        .decode(envelope.nonce_b64)
        .ok()?;
    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(envelope.ciphertext_b64)
        .ok()?;
    if nonce.len() != 24 {
        return None;
    }
    let mut nonce_arr = [0u8; 24];
    nonce_arr.copy_from_slice(&nonce);
    let cipher = XChaCha20Poly1305Cipher;
    let plaintext = cipher
        .decrypt(&key, nonce_arr, GROUP_KEY_SHARE_AAD, &ciphertext)
        .ok()?;
    if plaintext.len() != 32 {
        return None;
    }
    let mut group_key = [0u8; 32];
    group_key.copy_from_slice(&plaintext);
    Some(GroupKeyMaterial {
        group_id: envelope.group_id,
        key_id: envelope.key_id,
        key: group_key,
    })
}

fn derive_dm_key(local_secret: [u8; 32], remote_pubkey_hex: &str) -> Result<[u8; 32], String> {
    let local_secret_key = SecretKey::from_slice(&local_secret).map_err(|e| e.to_string())?;
    let remote_pubkey = pubkey_from_nostr_hex(remote_pubkey_hex)?;
    let shared_secret = diffie_hellman(
        local_secret_key.to_nonzero_scalar(),
        remote_pubkey.as_affine(),
    );
    let shared = shared_secret.raw_secret_bytes();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"veil-dm-v1:");
    hasher.update(shared.as_ref());
    Ok(*hasher.finalize().as_bytes())
}

fn derive_group_key_legacy(group_id: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"veil-group-v1:");
    hasher.update(group_id.as_bytes());
    *hasher.finalize().as_bytes()
}

fn pubkey_from_nostr_hex(pubkey_hex: &str) -> Result<PublicKey, String> {
    if pubkey_hex.len() != 64 {
        return Err("invalid pubkey length".to_string());
    }
    let x_bytes = hex::decode(pubkey_hex).map_err(|e| e.to_string())?;
    let mut sec1 = Vec::with_capacity(33);
    sec1.push(0x02); // BIP-340 x-only key implies even Y when compressed.
    sec1.extend_from_slice(&x_bytes);
    PublicKey::from_sec1_bytes(&sec1).map_err(|e| e.to_string())
}

fn pubkey_hex_from_secret(secret: [u8; 32]) -> Option<String> {
    let signer = veil_crypto::signing::NostrSigner::from_secret(secret).ok()?;
    Some(hex::encode(signer.public_key()))
}

#[cfg(test)]
mod tests {
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
        let decrypted =
            decrypt_direct_message_payload(recipient_secret, &payload).expect("decrypt");
        assert_eq!(decrypted, b"hello dm");
    }

    #[test]
    fn group_encrypt_decrypt_round_trip() {
        let payload = encrypt_group_message_payload("group-1", "k1", [5u8; 32], b"hello group")
            .expect("encrypt");
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
        let material =
            decrypt_group_key_share_payload(recipient_secret, &payload).expect("decrypt");
        assert_eq!(material.group_id, "group-1");
        assert_eq!(material.key_id, "k1");
        assert_eq!(material.key, [8u8; 32]);
    }
}
