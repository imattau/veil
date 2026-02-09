use std::io::{self, Write};
use std::net::UdpSocket;
use std::time::{Duration, Instant};

use veil_transport::adapter::TransportAdapter;
use veil_transport_quic::{QuicAdapter, QuicAdapterConfig, QuicIdentity};

fn free_udp_addr() -> std::net::SocketAddr {
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind should work");
    sock.local_addr().expect("local addr should resolve")
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn main() {
    let mut timeout_ms = 10_000u64;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--timeout-ms" {
            if let Some(value) = args.next() {
                if let Ok(parsed) = value.parse::<u64>() {
                    timeout_ms = parsed;
                }
            }
        }
    }

    let identity =
        QuicIdentity::generate_self_signed("127.0.0.1").expect("identity should be generated");
    let bind_addr = free_udp_addr();
    let mut adapter = QuicAdapter::connect(QuicAdapterConfig::new(
        bind_addr,
        "localhost",
        identity.clone(),
    ))
    .expect("adapter should start");

    println!(
        "READY {} {}",
        bind_addr,
        hex_encode(&identity.cert_der)
    );
    let _ = io::stdout().flush();

    let echo = std::env::var_os("VEIL_QUIC_ECHO").is_some();
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    while Instant::now() < deadline {
        if let Some((peer, bytes)) = adapter.recv() {
            println!("RECV {}", hex_encode(&bytes));
            let _ = io::stdout().flush();
            if echo {
                let _ = adapter.send(&peer, &bytes);
            }
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }

    println!("TIMEOUT");
    let _ = io::stdout().flush();
    std::process::exit(2);
}
