use std::path::Path;

use tracing::info;
use veil_crypto::signing::{NostrSigner, Signer};
use veil_transport_quic::QuicIdentity;

use crate::node_bootstrap::{load_or_create_identity, load_or_create_node_key, load_trusted_certs};
use crate::nostr_secret::{decode_nostr_secret_input, encode_nostr_nsec};

pub(super) struct RuntimeIdentityInputs<'a> {
    pub node_key_input: Option<&'a str>,
    pub node_key_path: &'a Path,
    pub quic_cert_path: &'a Path,
    pub quic_key_path: &'a Path,
    pub quic_trusted_certs: &'a [String],
}

pub(super) struct RuntimeIdentitySetup {
    pub decrypt_key: [u8; 32],
    pub node_signer: NostrSigner,
    pub node_pubkey: [u8; 32],
    pub node_secret_hex: String,
    pub node_secret_nsec: String,
    pub node_pubkey_hex: String,
    pub identity: QuicIdentity,
    pub trusted: Vec<Vec<u8>>,
}

pub(super) fn init_runtime_identity(
    inputs: RuntimeIdentityInputs<'_>,
) -> Result<RuntimeIdentitySetup, String> {
    let node_key = if let Some(key_input) = inputs.node_key_input {
        match decode_nostr_secret_input(key_input) {
            Some(key) => {
                info!("using node key from configuration/environment");
                key
            }
            None => {
                return Err("fatal: invalid node_key provided in configuration/environment".into());
            }
        }
    } else {
        load_or_create_node_key(inputs.node_key_path).map_err(|err| format!("fatal: {err}"))?
    };

    let node_signer = NostrSigner::from_secret(node_key).expect("node key validated");
    let decrypt_key = veil_crypto::keys::derive_encrypt_key(&node_key);
    let node_pubkey = node_signer.public_key();
    let node_secret_hex = hex::encode(node_key);
    let node_secret_nsec = encode_nostr_nsec(node_key).unwrap_or_default();
    let node_pubkey_hex = hex::encode(node_pubkey);
    info!("node identity (nostr x-only pubkey): {node_pubkey_hex}");

    let identity = load_or_create_identity(inputs.quic_cert_path, inputs.quic_key_path)
        .map_err(|err| format!("fatal: {err}"))?;

    let mut trusted = load_trusted_certs(inputs.quic_trusted_certs);
    if trusted.is_empty() {
        trusted.push(identity.cert_chain_der[0].clone());
    }

    Ok(RuntimeIdentitySetup {
        decrypt_key,
        node_signer,
        node_pubkey,
        node_secret_hex,
        node_secret_nsec,
        node_pubkey_hex,
        identity,
        trusted,
    })
}
