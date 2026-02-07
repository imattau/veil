use std::fs;
use std::path::Path;

use thiserror::Error;

use crate::state::NodeState;

/// Errors returned by node state persistence helpers.
#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("failed to encode node state: {0}")]
    Encode(serde_cbor::Error),
    #[error("failed to decode node state: {0}")]
    Decode(serde_cbor::Error),
    #[error("failed to read state file: {0}")]
    Read(std::io::Error),
    #[error("failed to write state file: {0}")]
    Write(std::io::Error),
}

/// Encodes [`NodeState`] to CBOR bytes.
pub fn encode_state_cbor(state: &NodeState) -> Result<Vec<u8>, PersistenceError> {
    serde_cbor::to_vec(state).map_err(PersistenceError::Encode)
}

/// Decodes [`NodeState`] from CBOR bytes.
pub fn decode_state_cbor(bytes: &[u8]) -> Result<NodeState, PersistenceError> {
    serde_cbor::from_slice(bytes).map_err(PersistenceError::Decode)
}

/// Saves state to the given path as CBOR.
pub fn save_state_to_path(
    path: impl AsRef<Path>,
    state: &NodeState,
) -> Result<(), PersistenceError> {
    let bytes = encode_state_cbor(state)?;
    let path = path.as_ref();
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(PersistenceError::Write)?;
    fs::rename(&tmp, path).map_err(PersistenceError::Write)
}

/// Loads state from the given CBOR file path.
pub fn load_state_from_path(path: impl AsRef<Path>) -> Result<NodeState, PersistenceError> {
    let bytes = fs::read(path.as_ref()).map_err(PersistenceError::Read)?;
    decode_state_cbor(&bytes)
}

/// Loads state if the file exists; otherwise returns a default empty state.
pub fn load_state_or_default(path: impl AsRef<Path>) -> Result<NodeState, PersistenceError> {
    if !path.as_ref().exists() {
        return Ok(NodeState::default());
    }
    load_state_from_path(path)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::policy::TrustTier;
    use crate::state::{CachedShard, NodeState};

    use super::{
        decode_state_cbor, encode_state_cbor, load_state_from_path, load_state_or_default,
        save_state_to_path,
    };

    fn temp_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be monotonic enough for tests")
            .as_nanos();
        p.push(format!("veil-node-{name}-{pid}-{nanos}.cbor"));
        p
    }

    #[test]
    fn state_round_trip_cbor() {
        let mut state = NodeState::default();
        state.subscriptions.insert([0x11; 32]);
        state.cache.insert(
            [0x22; 32],
            CachedShard {
                bytes: vec![1, 2, 3],
                expiry_step: 10,
                last_seen_step: 4,
            },
        );
        state.replica_estimate.insert([0x22; 32], 3);
        state.shard_tier.insert([0x22; 32], TrustTier::Known);
        state.shard_requested.insert([0x22; 32], 7);

        let encoded = encode_state_cbor(&state).expect("state should encode");
        let decoded = decode_state_cbor(&encoded).expect("state should decode");

        assert_eq!(decoded.subscriptions, state.subscriptions);
        assert_eq!(decoded.cache.len(), 1);
        assert_eq!(decoded.replica_estimate.get(&[0x22; 32]), Some(&3));
        assert_eq!(decoded.shard_tier.get(&[0x22; 32]), Some(&TrustTier::Known));
        assert_eq!(decoded.shard_requested.get(&[0x22; 32]), Some(&7));
    }

    #[test]
    fn file_helpers_round_trip_and_default() {
        let mut state = NodeState::default();
        state.subscriptions.insert([0xAB; 32]);

        let file = temp_path("state");
        save_state_to_path(&file, &state).expect("state should be saved");
        let loaded = load_state_from_path(&file).expect("state should load");
        assert_eq!(loaded.subscriptions, state.subscriptions);

        let missing = temp_path("missing");
        let defaulted =
            load_state_or_default(&missing).expect("missing file should return default");
        assert!(defaulted.subscriptions.is_empty());

        let _ = std::fs::remove_file(&file);
    }
}
