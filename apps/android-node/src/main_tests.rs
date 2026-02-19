use super::{
    decode_hex_32, decode_non_empty_hex, is_supported_discovery_url, parse_peer_pubkey_binding,
};

#[test]
fn supports_expected_discovery_url_schemes() {
    assert!(is_supported_discovery_url("https://seed.example"));
    assert!(is_supported_discovery_url("http://seed.example"));
    assert!(is_supported_discovery_url("wss://relay.example/ws"));
    assert!(is_supported_discovery_url("ws://relay.example/ws"));
    assert!(is_supported_discovery_url("quic://node.example:9443"));
}

#[test]
fn rejects_invalid_or_unsupported_discovery_urls() {
    assert!(!is_supported_discovery_url("seed.example"));
    assert!(!is_supported_discovery_url("ftp://seed.example"));
    assert!(!is_supported_discovery_url("http://"));
    assert!(!is_supported_discovery_url("https//missing-colon.example"));
}

#[test]
fn decode_hex_32_requires_exact_32_bytes() {
    assert_eq!(decode_hex_32(&"11".repeat(32)), Some([0x11; 32]));
    assert!(decode_hex_32("11").is_none());
    assert!(decode_hex_32(&"zz".repeat(32)).is_none());
}

#[test]
fn parse_peer_pubkey_binding_validates_format_and_hex() {
    let entry = format!("peer-a={}", "22".repeat(32));
    let parsed = parse_peer_pubkey_binding(&entry).expect("valid binding");
    assert_eq!(parsed.0, "peer-a");
    assert_eq!(parsed.1, [0x22; 32]);
    assert!(parse_peer_pubkey_binding("peer-a:abcd").is_none());
    assert!(parse_peer_pubkey_binding("=").is_none());
    assert!(parse_peer_pubkey_binding("peer-a=zz").is_none());
}

#[test]
fn decode_non_empty_hex_rejects_empty_and_invalid() {
    assert_eq!(decode_non_empty_hex("00"), Some(vec![0x00]));
    assert!(decode_non_empty_hex("").is_none());
    assert!(decode_non_empty_hex("zz").is_none());
}
