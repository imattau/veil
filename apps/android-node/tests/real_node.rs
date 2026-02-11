use std::net::ToSocketAddrs;
use std::time::{Duration, Instant};

use base64::Engine;
use veil_android_node::{default_protocol_config, NodeState, ProtocolEngine};

fn extract_js_value(body: &str, key: &str) -> Option<String> {
    for line in body.lines() {
        let line = line.trim();
        if !line.starts_with("window.") {
            continue;
        }
        if !line.contains(key) {
            continue;
        }
        let value = line
            .split('=')
            .nth(1)
            .map(|v| v.trim().trim_end_matches(';'))?;
        let value = value.trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            return None;
        }
        return Some(value.to_string());
    }
    None
}

fn fetch_vps_config(host: &str) -> Result<(u16, Vec<u8>), String> {
    let url = format!("https://{host}/config.js");
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(8))
        .build();
    let response = agent.get(&url).call().map_err(|err| format!("{err}"))?;
    if response.status() >= 400 {
        return Err(format!("config fetch failed: HTTP {}", response.status()));
    }
    let mut body = String::new();
    response
        .into_reader()
        .read_to_string(&mut body)
        .map_err(|err| format!("failed to read config: {err}"))?;
    let quic_port = extract_js_value(&body, "VEIL_VPS_QUIC_PORT")
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| "config.js missing QUIC port".to_string())?;
    let cert_b64 = extract_js_value(&body, "VEIL_VPS_QUIC_CERT_B64")
        .ok_or_else(|| "config.js missing QUIC cert".to_string())?;
    let cert = base64::engine::general_purpose::STANDARD
        .decode(cert_b64)
        .map_err(|err| format!("cert decode failed: {err}"))?;
    if cert.is_empty() {
        return Err("cert decode returned empty bytes".to_string());
    }
    Ok((quic_port, cert))
}

fn resolve_quic_peer(host: &str, port: u16) -> Result<String, String> {
    let addr = format!("{host}:{port}");
    let mut addrs = addr
        .to_socket_addrs()
        .map_err(|err| format!("failed to resolve QUIC peer: {err}"))?;
    let mut fallback = None;
    for resolved in addrs.by_ref() {
        if resolved.is_ipv4() {
            return Ok(resolved.to_string());
        }
        if fallback.is_none() {
            fallback = Some(resolved);
        }
    }
    let resolved = fallback.ok_or_else(|| "no QUIC peer addresses resolved".to_string())?;
    Ok(resolved.to_string())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_node_quic_send() {
    if std::env::var("VEIL_REAL_NODE").ok().as_deref() != Some("1") {
        return;
    }
    let host = std::env::var("VEIL_REAL_NODE_HOST")
        .unwrap_or_else(|_| "veilnode.3nostr.com".to_string());
    std::env::set_var("VEIL_QUIC_DEBUG", "1");
    std::env::set_var("VEIL_QUIC_CONNECT_TIMEOUT_MS", "8000");
    std::env::set_var("VEIL_QUIC_SEND_TIMEOUT_MS", "8000");
    if std::env::var("VEIL_REAL_NODE_ALPN").is_ok() {
        if let Ok(raw) = std::env::var("VEIL_REAL_NODE_ALPN") {
            std::env::set_var("VEIL_QUIC_ALPN", raw);
        }
    } else {
        std::env::set_var("VEIL_QUIC_ALPN", "veil-quic/1,veil/1,veil-node,veil,h3,hq-29");
    }
    let (quic_port, cert) = fetch_vps_config(&host).expect("config.js fetch");
    let quic_peer = resolve_quic_peer(&host, quic_port).expect("resolve quic peer");

    let node = NodeState::new("0.1-test");
    let identity = node.identity();
    let mut cfg = default_protocol_config(
        "ws://127.0.0.1:1/ws".to_string(),
        "android-node-real-test".to_string(),
        32,
        identity.public_key,
        identity.signer(),
    );
    cfg.ws_url = None;
    cfg.quic_bind_addr = "0.0.0.0:0".to_string();
    cfg.quic_server_name = Some(host.clone());
    cfg.quic_trusted_certs = vec![cert];
    cfg.fast_peers = vec![quic_peer];
    cfg.fallback_peers = Vec::new();

    let protocol = ProtocolEngine::new(cfg).expect("protocol init");
    protocol
        .publish(b"veil-real-node".to_vec(), Some(32))
        .await
        .expect("publish");

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut send_ok = 0;
    let mut send_err = 0;
    while Instant::now() < deadline {
        let details = protocol.lane_details().await;
        for detail in details {
            if detail.lane == "quic" {
                send_ok += detail.stats.outbound_send_ok;
                send_err += detail.stats.outbound_send_err;
            }
        }
        if send_ok > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert!(
        send_ok > 0,
        "expected QUIC send ok > 0 (ok={send_ok}, err={send_err}). If this fails with \"peer doesn't support any known protocol\", set VEIL_REAL_NODE_ALPN to the server's ALPN."
    );
}
