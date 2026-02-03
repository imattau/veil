use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    Key, XChaCha20Poly1305, XNonce,
};
use thiserror::Error;
use veil_core::{Epoch, Namespace, Tag};

/// Encrypted payload and nonce pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AeadEnvelope {
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>,
}

/// Errors returned by AEAD helpers.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AeadError {
    #[error("invalid key length")]
    InvalidKeyLength,
    #[error("encryption failed")]
    EncryptFailed,
    #[error("decryption failed")]
    DecryptFailed,
}

/// AEAD backend trait used by object encryption/decryption flow.
pub trait AeadCipher {
    fn encrypt(
        &self,
        key: &[u8],
        nonce: [u8; 24],
        aad: &[u8],
        plaintext: &[u8],
    ) -> Result<AeadEnvelope, AeadError>;
    fn decrypt(
        &self,
        key: &[u8],
        nonce: [u8; 24],
        aad: &[u8],
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, AeadError>;
}

/// Builds VEIL AEAD associated data from `(tag, namespace, epoch)`.
pub fn build_veil_aad(tag: Tag, namespace: Namespace, epoch: Epoch) -> [u8; 38] {
    let mut aad = [0_u8; 38];
    aad[..32].copy_from_slice(&tag);
    aad[32..34].copy_from_slice(&namespace.0.to_be_bytes());
    aad[34..38].copy_from_slice(&epoch.0.to_be_bytes());
    aad
}

/// XChaCha20-Poly1305 AEAD implementation.
#[derive(Debug, Default, Clone, Copy)]
pub struct XChaCha20Poly1305Cipher;

impl XChaCha20Poly1305Cipher {
    fn init_cipher(key: &[u8]) -> Result<XChaCha20Poly1305, AeadError> {
        if key.len() != 32 {
            return Err(AeadError::InvalidKeyLength);
        }
        Ok(XChaCha20Poly1305::new(Key::from_slice(key)))
    }
}

impl AeadCipher for XChaCha20Poly1305Cipher {
    fn encrypt(
        &self,
        key: &[u8],
        nonce: [u8; 24],
        aad: &[u8],
        plaintext: &[u8],
    ) -> Result<AeadEnvelope, AeadError> {
        let cipher = Self::init_cipher(key)?;
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| AeadError::EncryptFailed)?;
        Ok(AeadEnvelope { nonce, ciphertext })
    }

    fn decrypt(
        &self,
        key: &[u8],
        nonce: [u8; 24],
        aad: &[u8],
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, AeadError> {
        let cipher = Self::init_cipher(key)?;
        cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: ciphertext,
                    aad,
                },
            )
            .map_err(|_| AeadError::DecryptFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::{build_veil_aad, AeadCipher, AeadError, XChaCha20Poly1305Cipher};
    use veil_core::{Epoch, Namespace};

    #[test]
    fn aad_layout_is_tag_namespace_epoch_big_endian() {
        let tag = [0xAB_u8; 32];
        let aad = build_veil_aad(tag, Namespace(0x1234), Epoch(0x0102_0304));
        assert_eq!(&aad[..32], &tag);
        assert_eq!(&aad[32..34], &0x1234_u16.to_be_bytes());
        assert_eq!(&aad[34..38], &0x0102_0304_u32.to_be_bytes());
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let cipher = XChaCha20Poly1305Cipher;
        let key = [0x11_u8; 32];
        let nonce = [0x22_u8; 24];
        let aad = build_veil_aad([0x33_u8; 32], Namespace(42), Epoch(123_456));
        let plaintext = b"veil payload";

        let envelope = cipher
            .encrypt(&key, nonce, &aad, plaintext)
            .expect("encryption should succeed");
        let decrypted = cipher
            .decrypt(&key, nonce, &aad, &envelope.ciphertext)
            .expect("decryption should succeed");

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decrypt_fails_with_wrong_aad() {
        let cipher = XChaCha20Poly1305Cipher;
        let key = [0x44_u8; 32];
        let nonce = [0x55_u8; 24];
        let aad = build_veil_aad([0x66_u8; 32], Namespace(7), Epoch(77));
        let wrong_aad = build_veil_aad([0x66_u8; 32], Namespace(7), Epoch(78));
        let plaintext = b"integrity-bound";

        let envelope = cipher
            .encrypt(&key, nonce, &aad, plaintext)
            .expect("encryption should succeed");
        let err = cipher
            .decrypt(&key, nonce, &wrong_aad, &envelope.ciphertext)
            .expect_err("decryption should fail with mismatched aad");

        assert_eq!(err, AeadError::DecryptFailed);
    }

    #[test]
    fn rejects_non_32_byte_key() {
        let cipher = XChaCha20Poly1305Cipher;
        let key = [0x11_u8; 31];
        let nonce = [0x22_u8; 24];
        let aad = build_veil_aad([0x33_u8; 32], Namespace(1), Epoch(1));

        let err = cipher
            .encrypt(&key, nonce, &aad, b"data")
            .expect_err("invalid key length must fail");
        assert_eq!(err, AeadError::InvalidKeyLength);
    }
}
