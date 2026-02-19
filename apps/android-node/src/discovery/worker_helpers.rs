use crate::api::ContactBundle;
use crate::protocol::ProtocolEngine;
use crate::state::NodeState;
use std::net::{IpAddr, SocketAddr};

pub(super) fn join_discovery_endpoint(base: &str, path: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    format!("{trimmed}/{path}")
}

pub(super) fn with_auth_header(
    builder: reqwest::RequestBuilder,
    token: Option<&str>,
) -> reqwest::RequestBuilder {
    match token.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => builder.header("x-veil-token", value),
        None => builder,
    }
}

pub fn build_self_contact(node: &NodeState, protocol: &ProtocolEngine) -> ContactBundle {
    let identity = node.identity();
    let rpc_url = std::env::var("VEIL_NODE_RPC_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let ws_url = std::env::var("VEIL_NODE_WS_PUBLIC")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| protocol.ws_url().filter(|url| is_public_ws_url(url)));
    let quic_addr = std::env::var("VEIL_NODE_QUIC_PUBLIC")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            let addr = protocol.quic_bind_addr();
            if is_public_quic_addr(&addr) {
                Some(addr)
            } else {
                None
            }
        });
    ContactBundle {
        peer_id: protocol.peer_id(),
        ws_url,
        quic_addr,
        pubkey_hex: identity.public_key_hex(),
        rpc_url,
        lan_addrs: Vec::new(),
    }
}

fn is_public_ws_url(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url.trim()) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    if host.eq_ignore_ascii_case("localhost") {
        return false;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return !(ip.is_loopback() || ip.is_unspecified());
    }
    true
}

fn is_public_quic_addr(addr: &str) -> bool {
    let trimmed = addr.trim();
    if let Ok(sock) = trimmed.parse::<SocketAddr>() {
        return !(sock.ip().is_loopback() || sock.ip().is_unspecified());
    }
    let host = trimmed.split(':').next().unwrap_or(trimmed).trim();
    !host.is_empty() && !host.eq_ignore_ascii_case("localhost")
}

#[cfg(test)]
mod tests {
    use super::{is_public_quic_addr, is_public_ws_url};

    #[test]
    fn ws_url_public_filter_uses_url_parser() {
        assert!(is_public_ws_url("wss://relay.example/ws"));
        assert!(is_public_ws_url("ws://1.2.3.4:8080/ws"));
        assert!(!is_public_ws_url("ws://127.0.0.1:7788/ws"));
        assert!(!is_public_ws_url("ws://localhost:7788/ws"));
        assert!(!is_public_ws_url("ws://0.0.0.0:7788/ws"));
        assert!(!is_public_ws_url("invalid-url"));
    }

    #[test]
    fn quic_addr_public_filter_uses_socketaddr_parser() {
        assert!(is_public_quic_addr("1.2.3.4:9443"));
        assert!(!is_public_quic_addr("127.0.0.1:9443"));
        assert!(!is_public_quic_addr("0.0.0.0:9443"));
        assert!(!is_public_quic_addr("localhost:9443"));
        assert!(is_public_quic_addr("quic-peer.example:9443"));
    }
}
