use veil_core::tags::{derive_feed_tag, derive_rv_tag};
use veil_core::{Epoch, Namespace};
use wasm_bindgen::prelude::*;

fn parse_32(name: &str, bytes: &[u8]) -> Result<[u8; 32], JsValue> {
    if bytes.len() != 32 {
        return Err(JsValue::from_str(&format!(
            "{name} must be 32 bytes, got {}",
            bytes.len()
        )));
    }
    let mut out = [0_u8; 32];
    out.copy_from_slice(bytes);
    Ok(out)
}

/// Derive a stable public feed tag from publisher pubkey + namespace.
#[wasm_bindgen(js_name = deriveFeedTag)]
pub fn derive_feed_tag_wasm(publisher_pubkey: &[u8], namespace: u16) -> Result<Vec<u8>, JsValue> {
    let pubkey = parse_32("publisher_pubkey", publisher_pubkey)?;
    Ok(derive_feed_tag(&pubkey, Namespace(namespace)).to_vec())
}

/// Derive a rotating private rendezvous tag from recipient pubkey + epoch + namespace.
#[wasm_bindgen(js_name = deriveRvTag)]
pub fn derive_rv_tag_wasm(
    recipient_pubkey: &[u8],
    epoch: u32,
    namespace: u16,
) -> Result<Vec<u8>, JsValue> {
    let pubkey = parse_32("recipient_pubkey", recipient_pubkey)?;
    Ok(derive_rv_tag(&pubkey, Epoch(epoch), Namespace(namespace)).to_vec())
}

/// Calculate epoch number from wall-clock seconds and epoch window.
#[wasm_bindgen(js_name = currentEpoch)]
pub fn current_epoch(now_seconds: u64, epoch_seconds: u32) -> Result<u32, JsValue> {
    if epoch_seconds == 0 {
        return Err(JsValue::from_str("epoch_seconds must be > 0"));
    }
    Ok((now_seconds / epoch_seconds as u64) as u32)
}

/// Hex encode bytes for UI/debug convenience.
#[wasm_bindgen(js_name = bytesToHex)]
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{current_epoch, derive_feed_tag_wasm, derive_rv_tag_wasm};

    #[test]
    fn derives_tags_and_epoch() {
        let key = [0x11_u8; 32];
        let feed = derive_feed_tag_wasm(&key, 7).expect("feed tag");
        assert_eq!(feed.len(), 32);

        let rv = derive_rv_tag_wasm(&key, 42, 7).expect("rv tag");
        assert_eq!(rv.len(), 32);
        assert_ne!(feed, rv);

        assert_eq!(current_epoch(86_400 * 3, 86_400).expect("epoch"), 3);
    }
}
