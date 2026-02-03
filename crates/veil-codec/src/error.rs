use thiserror::Error;

/// Errors returned by object/shard codec operations.
#[derive(Debug, Error)]
pub enum CodecError {
    /// CBOR serialization/deserialization failure.
    #[error("decode error: {0}")]
    Decode(#[from] serde_cbor::Error),
    /// Object-level schema validation failure.
    #[error("invalid object: {0}")]
    InvalidObject(&'static str),
    /// Shard-level schema validation failure.
    #[error("invalid shard: {0}")]
    InvalidShard(&'static str),
}
