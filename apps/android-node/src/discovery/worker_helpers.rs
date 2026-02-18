use crate::api::ContactBundle;
use crate::protocol::ProtocolEngine;
use crate::state::NodeState;

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
        .or_else(|| {
            protocol.ws_url().filter(|url| {
                !url.contains("127.0.0.1") && !url.contains("localhost") && !url.contains("0.0.0.0")
            })
        });
    let quic_addr = std::env::var("VEIL_NODE_QUIC_PUBLIC")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            let addr = protocol.quic_bind_addr();
            if addr.starts_with("0.0.0.0")
                || addr.starts_with("127.0.0.1")
                || addr.contains("localhost")
            {
                None
            } else {
                Some(addr)
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
