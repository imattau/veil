use veil_core::hash::blake3_32;

/// Derives a deterministic 32-byte symmetric encryption key from a secret key.
/// 
/// This ensures that nodes sharing the same identity (e.g. VPS and mobile)
/// derive the same encryption key for their protocol runtimes.
pub fn derive_encrypt_key(secret_key: &[u8; 32]) -> [u8; 32] {
    let mut preimage = Vec::with_capacity(24 + 32);
    preimage.extend_from_slice(b"veil/encrypt-key/v1");
    preimage.extend_from_slice(secret_key);
    blake3_32(&preimage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivation_is_deterministic() {
        let secret = [0x42; 32];
        let k1 = derive_encrypt_key(&secret);
        let k2 = derive_encrypt_key(&secret);
        assert_eq!(k1, k2);
        assert_ne!(k1, secret);
    }

    #[test]
    fn derivation_differs_by_secret() {
        let k1 = derive_encrypt_key(&[0x01; 32]);
        let k2 = derive_encrypt_key(&[0x02; 32]);
        assert_ne!(k1, k2);
    }
}
