use thiserror::Error;

/// Shared lightweight error type for core primitive operations.
#[derive(Debug, Error)]
pub enum VeilError {
    /// Invalid caller input or malformed primitive value.
    #[error("invalid input: {0}")]
    InvalidInput(&'static str),
    /// Decode/parsing failure.
    #[error("decode error: {0}")]
    Decode(&'static str),
    /// Cryptographic operation failure.
    #[error("crypto error: {0}")]
    Crypto(&'static str),
}

#[cfg(test)]
mod tests {
    use super::VeilError;

    #[test]
    fn error_messages_are_stable() {
        assert_eq!(
            VeilError::InvalidInput("bad ns").to_string(),
            "invalid input: bad ns"
        );
        assert_eq!(
            VeilError::Decode("bad cbor").to_string(),
            "decode error: bad cbor"
        );
        assert_eq!(
            VeilError::Crypto("bad mac").to_string(),
            "crypto error: bad mac"
        );
    }
}
