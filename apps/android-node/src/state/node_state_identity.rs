use super::{NodeIdentity, NodeState};

impl NodeState {
    pub fn status(&self) -> crate::api::StatusResponse {
        let inner = self.inner.lock().expect("state lock");
        super::status_from_inner(&inner)
    }

    pub fn identity(&self) -> NodeIdentity {
        let inner = self.inner.lock().expect("state lock");
        inner.identity.clone()
    }

    pub fn rotate_identity(&self) -> NodeIdentity {
        let mut inner = self.inner.lock().expect("state lock");
        let identity = super::generate_identity();
        inner.identity = identity.clone();
        if let Some(store) = &inner.store {
            store.persist(&super::snapshot_from_inner(&inner));
        }
        identity
    }

    pub fn get_feed(&self, limit: usize) -> Vec<crate::api::EventEnvelope> {
        let inner = self.inner.lock().expect("state lock");
        super::feed_snapshot(&inner, limit)
    }

    pub fn get_subscriptions(&self) -> Vec<String> {
        let inner = self.inner.lock().expect("state lock");
        super::subscriptions_snapshot(&inner)
    }

    pub fn export_identity(&self) -> (String, String) {
        let inner = self.inner.lock().expect("state lock");
        (
            inner.identity.public_key_hex(),
            hex::encode(inner.identity.secret_key),
        )
    }

    pub fn import_identity(&self, secret_key_hex: String) -> Result<NodeIdentity, String> {
        let secret_key = super::decode_hex_32(&secret_key_hex)
            .ok_or_else(|| "secret key must be 32 bytes".to_string())?;
        let signer = super::NostrSigner::from_secret(secret_key)
            .map_err(|_| "secret key is not a valid Nostr secp256k1 secret".to_string())?;
        let public_key = super::Signer::public_key(&signer);
        let encrypt_key = veil_crypto::keys::derive_encrypt_key(&secret_key);
        let identity = NodeIdentity {
            public_key,
            secret_key,
            encrypt_key,
        };

        let mut inner = self.inner.lock().expect("state lock");
        inner.identity = identity.clone();
        if let Some(store) = &inner.store {
            store.persist(&super::snapshot_from_inner(&inner));
        }
        Ok(identity)
    }
}
