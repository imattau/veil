/// Computes BLAKE3 and returns the first-class 32-byte digest value.
pub fn blake3_32(input: &[u8]) -> [u8; 32] {
    *blake3::hash(input).as_bytes()
}

#[cfg(test)]
mod tests {
    use super::blake3_32;

    #[test]
    fn hash_is_deterministic() {
        let input = b"veil";
        assert_eq!(blake3_32(input), blake3_32(input));
    }

    #[test]
    fn hash_changes_when_input_changes() {
        assert_ne!(blake3_32(b"veil-a"), blake3_32(b"veil-b"));
    }
}
