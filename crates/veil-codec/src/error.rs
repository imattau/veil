use thiserror::Error;

/// Errors returned by object/shard codec operations.
#[derive(Debug, Error)]
pub enum CodecError {
    /// CBOR serialization failure.
    #[error("encode error: {0}")]
    Encode(String),
    /// CBOR deserialization failure.
    #[error("decode error: {0}")]
    Decode(String),
    /// Object-level schema validation failure.
    #[error("invalid object: {0}")]
    InvalidObject(&'static str),
    /// Shard-level schema validation failure.
    #[error("invalid shard: {0}")]
    InvalidShard(&'static str),
}
