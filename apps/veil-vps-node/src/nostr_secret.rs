use bech32::{Bech32, Hrp};

pub(crate) fn decode_nostr_secret_input(value: &str) -> Option<[u8; 32]> {
    let trimmed = value.trim();
    if let Ok(bytes) = hex::decode(trimmed) {
        if let Ok(key) = <[u8; 32]>::try_from(bytes.as_slice()) {
            return Some(key);
        }
    }
    let (decoded_hrp, data) = bech32::decode(trimmed).ok()?;
    if decoded_hrp.as_str() != "nsec" {
        return None;
    }
    if let Ok(key) = <[u8; 32]>::try_from(data.as_slice()) {
        return Some(key);
    }
    None
}

pub(super) fn encode_nostr_nsec(secret: [u8; 32]) -> Option<String> {
    let hrp = Hrp::parse("nsec").ok()?;
    bech32::encode::<Bech32>(hrp, &secret).ok()
}
