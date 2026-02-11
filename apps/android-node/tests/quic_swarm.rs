use std::thread;
use std::time::{Duration, Instant};

use veil_transport::adapter::TransportAdapter;
use veil_transport_quic::{QuicAdapter, QuicAdapterConfig, QuicIdentity};

#[test]
fn quic_swarm_inbound_receives_payloads() {
    run_swarm(5, 3, Duration::from_secs(12));
}

#[test]
fn quic_swarm_inbound_receives_payloads_stress() {
    if std::env::var_os("VEIL_QUIC_SWARM_STRESS").is_none() {
        return;
    }
    run_swarm(8, 6, Duration::from_secs(20));
}

fn run_swarm(peer_count: usize, min_success: usize, timeout: Duration) {
    std::env::set_var("VEIL_QUIC_INSECURE", "1");
    std::env::set_var("VEIL_QUIC_DEBUG", "1");
    let server_name = "localhost";

    // Pre-generate identities so every adapter trusts all peers.
    let mut identities = Vec::new();
    let mut certs = Vec::new();
    for _ in 0..peer_count {
        let id =
            QuicIdentity::generate_self_signed("localhost").expect("identity");
        certs.push(id.cert_der.clone());
        identities.push(id);
    }

    let mut adapters = Vec::new();
    let mut addrs = Vec::new();
    for identity in identities {
        let port = reserve_port();
        let addr = format!("127.0.0.1:{port}");
        let mut cfg =
            QuicAdapterConfig::new(addr.parse().unwrap(), server_name, identity);
        cfg.trusted_peer_certs_der = certs.clone();
        adapters.push(QuicAdapter::connect(cfg).expect("adapter"));
        addrs.push(addr);
    }
    thread::sleep(Duration::from_millis(300));

    // Poll for inbound messages while periodically sending.
    let deadline = Instant::now() + timeout;
    let mut received = vec![0usize; adapters.len()];
    while Instant::now() < deadline {
        for idx in 0..adapters.len() {
            let sender = idx;
            let target = if idx == 0 { adapters.len() - 1 } else { idx - 1 };
            let payload = format!("ping-{sender}-to-{target}").into_bytes();
            let _ = adapters[sender].send(&addrs[target], &payload);
        }
        for (idx, adapter) in adapters.iter_mut().enumerate() {
            if adapter.recv().is_some() {
                received[idx] += 1;
            }
        }
        if received.iter().all(|count| *count > 0) {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    let success_count = received.iter().filter(|count| **count > 0).count();
    assert!(
        success_count >= min_success,
        "expected at least {min_success} peers to receive inbound quic payload (counts={received:?})"
    );
}

fn reserve_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("local_addr")
        .port()
}
