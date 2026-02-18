use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::nostr_secret::decode_nostr_secret_input;
use rand::RngCore;
use tracing::info;
use veil_core::hash::blake3_32;
use veil_crypto::signing::NostrSigner;
use veil_transport_quic::QuicIdentity;

pub(super) fn load_or_create_identity(
    cert_path: &Path,
    key_path: &Path,
) -> Result<QuicIdentity, String> {
    if cert_path.exists() && key_path.exists() {
        let cert_bytes = fs::read(cert_path)
            .map_err(|e| format!("read cert from {}: {}", cert_path.display(), e))?;
        let key_bytes = fs::read(key_path)
            .map_err(|e| format!("read key from {}: {}", key_path.display(), e))?;
        let fingerprint = blake3_32(&cert_bytes);
        info!(
            "loaded existing QUIC identity from {} (fingerprint: {})",
            cert_path.display(),
            hex::encode(fingerprint)
        );
        return Ok(QuicIdentity {
            cert_chain_der: vec![cert_bytes],
            key_der: key_bytes,
        });
    }

    let identity = QuicIdentity::generate_self_signed("veil-node")
        .map_err(|e| format!("generate identity: {e}"))?;
    let fingerprint = blake3_32(&identity.cert_chain_der[0]);
    info!(
        "generated new self-signed QUIC identity (fingerprint: {})",
        hex::encode(fingerprint)
    );
    ensure_parent(cert_path).map_err(|e| format!("create cert dir: {e}"))?;
    fs::write(cert_path, &identity.cert_chain_der[0])
        .map_err(|e| format!("write cert to {}: {}", cert_path.display(), e))?;
    fs::write(key_path, &identity.key_der)
        .map_err(|e| format!("write key to {}: {}", key_path.display(), e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(cert_path, fs::Permissions::from_mode(0o600));
        let _ = fs::set_permissions(key_path, fs::Permissions::from_mode(0o600));
    }
    Ok(identity)
}

pub(super) fn load_trusted_certs(paths: &[String]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for path in paths {
        match fs::read(path) {
            Ok(bytes) => out.push(bytes),
            Err(err) => eprintln!("failed to read trusted cert {path}: {err}"),
        }
    }
    out
}

pub(super) fn parse_required_signed_namespaces(values: &[String]) -> HashSet<u16> {
    let mut out = HashSet::new();
    for value in values {
        if let Ok(ns) = value.parse::<u16>() {
            out.insert(ns);
        }
    }
    out
}

pub(super) fn parse_core_tags(values: &[String]) -> Vec<[u8; 32]> {
    values
        .iter()
        .filter_map(|value| {
            let bytes = hex::decode(value).ok()?;
            <[u8; 32]>::try_from(bytes.as_slice()).ok()
        })
        .collect()
}

pub(super) fn pseudo_pubkey_for_peer(peer: &str) -> [u8; 32] {
    let mut preimage = Vec::with_capacity(8 + peer.len());
    preimage.extend_from_slice(b"vps-peer");
    preimage.extend_from_slice(peer.as_bytes());
    blake3_32(&preimage)
}

pub(super) fn load_or_create_node_key(path: &Path) -> Result<[u8; 32], String> {
    if path.exists() {
        let bytes = fs::read(path).map_err(|e| format!("read node key: {e}"))?;
        if bytes.len() == 32 {
            let mut out = [0_u8; 32];
            out.copy_from_slice(&bytes);
            if NostrSigner::from_secret(out).is_ok() {
                return Ok(out);
            }
        }

        if let Ok(content) = String::from_utf8(bytes) {
            if let Some(key) = decode_nostr_secret_input(&content) {
                return Ok(key);
            }
        }
    }

    let key = loop {
        let mut candidate = [0_u8; 32];
        rand::thread_rng().fill_bytes(&mut candidate);
        if NostrSigner::from_secret(candidate).is_ok() {
            break candidate;
        }
    };
    ensure_parent(path).map_err(|e| format!("create node key dir: {e}"))?;
    fs::write(path, key).map_err(|e| format!("write node key: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(key)
}

fn ensure_parent(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_core_tags, parse_required_signed_namespaces, pseudo_pubkey_for_peer};

    #[test]
    fn parse_required_signed_namespaces_filters_invalid_values() {
        let values = vec![
            "1".to_string(),
            "65535".to_string(),
            "oops".to_string(),
            "-1".to_string(),
        ];
        let parsed = parse_required_signed_namespaces(&values);
        assert!(parsed.contains(&1));
        assert!(parsed.contains(&65535));
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn parse_core_tags_keeps_only_valid_32_byte_hex() {
        let values = vec!["00".repeat(32), "ff".repeat(31), "not-hex".to_string()];
        let parsed = parse_core_tags(&values);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0], [0_u8; 32]);
    }

    #[test]
    fn pseudo_pubkey_for_peer_is_deterministic() {
        let first = pseudo_pubkey_for_peer("peer-a");
        let second = pseudo_pubkey_for_peer("peer-a");
        let other = pseudo_pubkey_for_peer("peer-b");
        assert_eq!(first, second);
        assert_ne!(first, other);
    }
}
