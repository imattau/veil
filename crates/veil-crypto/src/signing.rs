use ed25519_dalek::{
    Signature as DalekSignature, Signer as DalekSignerTrait, SigningKey,
    Verifier as DalekVerifierTrait, VerifyingKey,
};
use thiserror::Error;

/// Errors returned by signing/verification helpers.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SigningError {
    /// Pubkey bytes are not a valid Ed25519 verifying key.
    #[error("invalid public key bytes")]
    InvalidPublicKey,
}

/// Trait for message signing backends.
pub trait Signer {
    /// Signs `msg` and returns a 64-byte signature.
    fn sign(&self, msg: &[u8]) -> Result<[u8; 64], SigningError>;
    /// Returns the signer's raw 32-byte public key.
    fn public_key(&self) -> [u8; 32];
}

/// Trait for signature verification backends.
pub trait Verifier {
    /// Verifies a signature against `(pubkey, msg)`.
    fn verify(&self, pubkey: [u8; 32], msg: &[u8], sig: [u8; 64]) -> Result<bool, SigningError>;
}

/// Ed25519 signing implementation backed by `ed25519-dalek`.
#[derive(Debug, Clone)]
pub struct Ed25519Signer {
    signing_key: SigningKey,
}

impl Ed25519Signer {
    /// Creates a signer from a 32-byte secret key.
    pub fn from_secret(secret: [u8; 32]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(&secret),
        }
    }
}

impl Signer for Ed25519Signer {
    fn sign(&self, msg: &[u8]) -> Result<[u8; 64], SigningError> {
        let signature = self.signing_key.sign(msg);
        Ok(signature.to_bytes())
    }

    fn public_key(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }
}

/// Stateless Ed25519 verifier.
#[derive(Debug, Default, Clone, Copy)]
pub struct Ed25519Verifier;

impl Verifier for Ed25519Verifier {
    fn verify(&self, pubkey: [u8; 32], msg: &[u8], sig: [u8; 64]) -> Result<bool, SigningError> {
        let verifying_key =
            VerifyingKey::from_bytes(&pubkey).map_err(|_| SigningError::InvalidPublicKey)?;
        let signature = DalekSignature::from_bytes(&sig);
        Ok(verifying_key.verify(msg, &signature).is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::{Ed25519Signer, Ed25519Verifier, Signer, Verifier};

    #[test]
    fn sign_and_verify_round_trip() {
        let signer = Ed25519Signer::from_secret([0x42_u8; 32]);
        let verifier = Ed25519Verifier;
        let msg = b"veil signed payload";

        let signature = signer.sign(msg).expect("sign should succeed");
        let ok = verifier
            .verify(signer.public_key(), msg, signature)
            .expect("verify should succeed");
        assert!(ok);
    }

    #[test]
    fn verify_fails_when_message_changes() {
        let signer = Ed25519Signer::from_secret([0x10_u8; 32]);
        let verifier = Ed25519Verifier;

        let signature = signer.sign(b"original").expect("sign should succeed");
        let ok = verifier
            .verify(signer.public_key(), b"tampered", signature)
            .expect("verify should run");
        assert!(!ok);
    }

    #[test]
    fn verify_fails_when_signature_changes() {
        let signer = Ed25519Signer::from_secret([0xAA_u8; 32]);
        let verifier = Ed25519Verifier;
        let msg = b"message";

        let mut signature = signer.sign(msg).expect("sign should succeed");
        signature[0] ^= 0x01;
        let ok = verifier
            .verify(signer.public_key(), msg, signature)
            .expect("verify should run");
        assert!(!ok);
    }
}
