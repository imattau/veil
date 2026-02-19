use crate::api::ContactBundle;
use crate::protocol::ProtocolEngine;
use crate::state::NodeState;
use std::net::{IpAddr, SocketAddr};

pub(super) fn join_discovery_endpoint(base: &str, path: &str) -> String {
    let base = base.trim();
    let path = path.trim_start_matches('/');
    if let Ok(parsed) = reqwest::Url::parse(base) {
        let mut with_slash = parsed;
        if !with_slash.path().ends_with('/') {
            with_slash.set_path(&format!("{}/", with_slash.path()));
        }
        if let Ok(joined) = with_slash.join(path) {
            return joined.to_string();
        }
    }
    format!("{}/{}", base.trim_end_matches('/'), path)
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
    host_is_public(host)
}

fn is_public_quic_addr(addr: &str) -> bool {
    let trimmed = addr.trim();
    let host = if let Ok(sock) = trimmed.parse::<SocketAddr>() {
        sock.ip().to_string()
    } else {
        let with_scheme = format!("quic://{trimmed}");
        let Ok(url) = reqwest::Url::parse(&with_scheme) else {
            return false;
        };
        let Some(host) = url.host_str() else {
            return false;
        };
        host.to_string()
    };

    host_is_public(&host)
}

fn host_is_public(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return false;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(v4) => {
                !(v4.is_private()
                    || v4.is_loopback()
                    || v4.is_link_local()
                    || v4.is_broadcast()
                    || v4.is_documentation()
                    || v4.is_unspecified()
                    || v4.is_multicast())
            }
            IpAddr::V6(v6) => {
                !(v6.is_loopback()
                    || v6.is_unspecified()
                    || v6.is_multicast()
                    || v6.is_unicast_link_local()
                    || v6.is_unique_local())
            }
        };
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{is_public_quic_addr, is_public_ws_url, join_discovery_endpoint};

    #[test]
    fn join_discovery_endpoint_uses_url_join_when_possible() {
        assert_eq!(
            join_discovery_endpoint("https://seed.example", "discovery/gossip"),
            "https://seed.example/discovery/gossip"
        );
        assert_eq!(
            join_discovery_endpoint("https://seed.example/api", "discovery/gossip"),
            "https://seed.example/api/discovery/gossip"
        );
        assert_eq!(
            join_discovery_endpoint("https://seed.example/api/", "/discovery/gossip"),
            "https://seed.example/api/discovery/gossip"
        );
    }

    #[test]
    fn join_discovery_endpoint_falls_back_for_invalid_base() {
        assert_eq!(
            join_discovery_endpoint("seed.example", "discovery/gossip"),
            "seed.example/discovery/gossip"
        );
    }

    #[test]
    fn ws_url_public_filter_uses_url_parser() {
        assert!(is_public_ws_url("wss://relay.example/ws"));
        assert!(is_public_ws_url("ws://1.2.3.4:8080/ws"));
        assert!(!is_public_ws_url("ws://192.168.1.10:8080/ws"));
        assert!(!is_public_ws_url("ws://127.0.0.1:7788/ws"));
        assert!(!is_public_ws_url("ws://localhost:7788/ws"));
        assert!(!is_public_ws_url("ws://0.0.0.0:7788/ws"));
        assert!(!is_public_ws_url("invalid-url"));
    }

    #[test]
    fn quic_addr_public_filter_uses_socketaddr_parser() {
        assert!(is_public_quic_addr("1.2.3.4:9443"));
        assert!(is_public_quic_addr("relay.example:9443"));
        assert!(!is_public_quic_addr("10.0.0.8:9443"));
        assert!(!is_public_quic_addr("127.0.0.1:9443"));
        assert!(!is_public_quic_addr("0.0.0.0:9443"));
        assert!(!is_public_quic_addr("[::1]:9443"));
        assert!(!is_public_quic_addr("localhost:9443"));
    }
}
