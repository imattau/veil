use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use veil_crypto::aead::{AeadCipher, XChaCha20Poly1305Cipher};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: Uuid,
    pub namespace: u16,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoreSnapshot {
    pub queue: Vec<QueueItem>,
    pub identity: Option<IdentityRecord>,
    #[serde(default)]
    pub policy_json: Option<String>,
    #[serde(default)]
    pub contacts: Vec<crate::api::ContactBundle>,
    #[serde(default)]
    pub feed_history: Vec<crate::api::EventEnvelope>,
    #[serde(default)]
    pub subscriptions: Vec<String>,
    #[serde(default)]
    pub group_keys: Vec<GroupKeyRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityRecord {
    pub public_key_hex: String,
    #[serde(default)]
    pub secret_key_hex: String,
    #[serde(default)]
    pub secret_key_enc_nonce_b64: Option<String>,
    #[serde(default)]
    pub secret_key_enc_b64: Option<String>,
    #[serde(default)]
    pub encrypt_key_hex: String,
    #[serde(default)]
    pub encrypt_key_enc_nonce_b64: Option<String>,
    #[serde(default)]
    pub encrypt_key_enc_b64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupKeyRecord {
    pub group_id: String,
    pub key_id: String,
    #[serde(default)]
    pub key_hex: String,
    #[serde(default)]
    pub key_enc_nonce_b64: Option<String>,
    #[serde(default)]
    pub key_enc_b64: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StateStore {
    path: PathBuf,
    state_key: Option<[u8; 32]>,
}

impl StateStore {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let state_key = std::env::var("VEIL_NODE_STATE_KEY_HEX")
            .ok()
            .and_then(|hex| decode_state_key_hex(&hex));
        Self::new_with_state_key(path, state_key)
    }

    pub fn new_with_state_key(path: impl AsRef<Path>, state_key: Option<[u8; 32]>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            state_key,
        }
    }

    pub fn load(&self) -> StoreSnapshot {
        let data = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(_) => return StoreSnapshot::default(),
        };
        let mut snapshot = serde_json::from_slice::<StoreSnapshot>(&data).unwrap_or_default();
        if let (Some(key), Some(identity)) = (self.state_key, snapshot.identity.as_mut()) {
            if identity.secret_key_hex.is_empty() {
                if let Some(value) = decrypt_secret_hex(
                    key,
                    identity.secret_key_enc_nonce_b64.as_deref(),
                    identity.secret_key_enc_b64.as_deref(),
                ) {
                    identity.secret_key_hex = value;
                }
            }
            if identity.encrypt_key_hex.is_empty() {
                if let Some(value) = decrypt_secret_hex(
                    key,
                    identity.encrypt_key_enc_nonce_b64.as_deref(),
                    identity.encrypt_key_enc_b64.as_deref(),
                ) {
                    identity.encrypt_key_hex = value;
                }
            }
        }
        if let Some(key) = self.state_key {
            for record in &mut snapshot.group_keys {
                if record.key_hex.is_empty() {
                    if let Some(value) = decrypt_secret_hex(
                        key,
                        record.key_enc_nonce_b64.as_deref(),
                        record.key_enc_b64.as_deref(),
                    ) {
                        record.key_hex = value;
                    }
                }
            }
        }
        snapshot
    }

    pub fn persist(&self, snapshot: &StoreSnapshot) {
        let mut to_store = snapshot.clone();
        if let (Some(key), Some(identity)) = (self.state_key, to_store.identity.as_mut()) {
            if identity.secret_key_hex.len() == 64 {
                if let Some((nonce_b64, ciphertext_b64)) =
                    encrypt_secret_hex(key, identity.secret_key_hex.as_bytes())
                {
                    identity.secret_key_enc_nonce_b64 = Some(nonce_b64);
                    identity.secret_key_enc_b64 = Some(ciphertext_b64);
                    identity.secret_key_hex.clear();
                }
            }
            if identity.encrypt_key_hex.len() == 64 {
                if let Some((nonce_b64, ciphertext_b64)) =
                    encrypt_secret_hex(key, identity.encrypt_key_hex.as_bytes())
                {
                    identity.encrypt_key_enc_nonce_b64 = Some(nonce_b64);
                    identity.encrypt_key_enc_b64 = Some(ciphertext_b64);
                    identity.encrypt_key_hex.clear();
                }
            }
        }
        if let Some(key) = self.state_key {
            for record in &mut to_store.group_keys {
                if record.key_hex.len() == 64 {
                    if let Some((nonce_b64, ciphertext_b64)) =
                        encrypt_secret_hex(key, record.key_hex.as_bytes())
                    {
                        record.key_enc_nonce_b64 = Some(nonce_b64);
                        record.key_enc_b64 = Some(ciphertext_b64);
                        record.key_hex.clear();
                    }
                }
            }
        }
        if let Ok(data) = serde_json::to_vec(&to_store) {
            let _ = fs::write(&self.path, data);
        }
    }
}

fn decode_state_key_hex(value: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(value.trim()).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Some(out)
}

fn encrypt_secret_hex(key: [u8; 32], plaintext: &[u8]) -> Option<(String, String)> {
    let mut nonce = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305Cipher;
    let envelope = cipher
        .encrypt(&key, nonce, b"veil-android-state-identity-v1", plaintext)
        .ok()?;
    Some((
        base64::engine::general_purpose::STANDARD.encode(envelope.nonce),
        base64::engine::general_purpose::STANDARD.encode(envelope.ciphertext),
    ))
}

fn decrypt_secret_hex(
    key: [u8; 32],
    nonce_b64: Option<&str>,
    ciphertext_b64: Option<&str>,
) -> Option<String> {
    let nonce = base64::engine::general_purpose::STANDARD
        .decode(nonce_b64?)
        .ok()?;
    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(ciphertext_b64?)
        .ok()?;
    if nonce.len() != 24 {
        return None;
    }
    let mut nonce_arr = [0u8; 24];
    nonce_arr.copy_from_slice(&nonce);
    let cipher = XChaCha20Poly1305Cipher;
    let plaintext = cipher
        .decrypt(
            &key,
            nonce_arr,
            b"veil-android-state-identity-v1",
            &ciphertext,
        )
        .ok()?;
    String::from_utf8(plaintext).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn persists_identity_secret_encrypted_when_state_key_present() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("state.json");
        let store = StateStore::new_with_state_key(&path, Some([1u8; 32]));
        let snapshot = StoreSnapshot {
            identity: Some(IdentityRecord {
                public_key_hex: "aa".repeat(32),
                secret_key_hex: "bb".repeat(32),
                secret_key_enc_nonce_b64: None,
                secret_key_enc_b64: None,
            }),
            ..Default::default()
        };
        store.persist(&snapshot);
        let raw = fs::read_to_string(&path).expect("state file");
        assert!(!raw.contains(&"bb".repeat(32)));
        let loaded = store.load();
        assert_eq!(
            loaded
                .identity
                .as_ref()
                .map(|i| i.secret_key_hex.clone())
                .unwrap_or_default(),
            "bb".repeat(32)
        );
    }

    #[test]
    fn persists_group_key_records_encrypted_when_state_key_present() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("state.json");
        let store = StateStore::new_with_state_key(&path, Some([1u8; 32]));
        let mut snapshot = StoreSnapshot::default();
        snapshot.group_keys.push(GroupKeyRecord {
            group_id: "g".to_string(),
            key_id: "k1".to_string(),
            key_hex: "cc".repeat(32),
            key_enc_nonce_b64: None,
            key_enc_b64: None,
        });
        store.persist(&snapshot);
        let raw = fs::read_to_string(&path).expect("state file");
        assert!(!raw.contains(&"cc".repeat(32)));
        let loaded = store.load();
        assert_eq!(loaded.group_keys.len(), 1);
        assert_eq!(loaded.group_keys[0].key_hex, "cc".repeat(32));
    }
}
