use std::ffi::{CStr, CString};
use std::net::UdpSocket;
use std::thread;
use std::time::{Duration, Instant};

use veil_sdk_bridge::{
    veil_quic_fetch_peer_cert, veil_quic_free_string, veil_quic_metrics, veil_quic_send,
    veil_quic_start, veil_quic_stop, veil_quic_test_ffi_call, QuicMetrics,
};
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

fn cstring(input: &str) -> CString {
    CString::new(input).expect("cstring should be valid")
}

#[test]
fn ffi_fetches_peer_cert_from_quic_server() {
    let identity =
        QuicIdentity::generate_self_signed("localhost").expect("identity should be generated");
    let bind_addr = free_udp_addr();

    let _server = QuicAdapter::connect(QuicAdapterConfig::new(
        bind_addr,
        "localhost",
        identity.clone(),
    ))
    .expect("server adapter should start");

    let endpoint = cstring(&bind_addr.to_string());
    let server_name = cstring("localhost");
    let cert_ptr = veil_quic_fetch_peer_cert(endpoint.as_ptr(), server_name.as_ptr());
    assert!(!cert_ptr.is_null(), "cert pointer should not be null");

    let cert_hex = unsafe { CStr::from_ptr(cert_ptr) }
        .to_str()
        .expect("cert hex should be utf8")
        .to_string();
    unsafe { veil_quic_free_string(cert_ptr) };

    assert_eq!(cert_hex, hex_encode(&identity.cert_der));
}

#[test]
fn ffi_can_send_payload_to_quic_server() {
    std::env::set_var("VEIL_QUIC_DEBUG", "1");
    veil_quic_test_ffi_call();

    let server_identity =
        QuicIdentity::generate_self_signed("localhost").expect("identity should be generated");
    let server_bind = free_udp_addr();
    let mut server = QuicAdapter::connect(QuicAdapterConfig::new(
        server_bind,
        "localhost",
        server_identity.clone(),
    ))
    .expect("server adapter should start");

    let endpoint = cstring(&server_bind.to_string());
    let server_name = cstring("localhost");
    let cert_ptr = veil_quic_fetch_peer_cert(endpoint.as_ptr(), server_name.as_ptr());
    assert!(!cert_ptr.is_null(), "cert pointer should not be null");
    let cert_hex = unsafe { CStr::from_ptr(cert_ptr) }
        .to_str()
        .expect("cert hex should be utf8")
        .to_string();
    unsafe { veil_quic_free_string(cert_ptr) };

    let client_handle = veil_quic_start(
        cstring("127.0.0.1:0").as_ptr(),
        cstring("localhost").as_ptr(),
        cstring(&cert_hex).as_ptr(),
    );
    assert!(client_handle > 0, "client handle should be valid");

    let payload = b"hello-quic";
    let mut send_ok = false;
    for _ in 0..5 {
        let send_result = unsafe {
            veil_quic_send(
                client_handle,
                cstring(&server_bind.to_string()).as_ptr(),
                payload.as_ptr(),
                payload.len(),
            )
        };
        if send_result == 0 {
            send_ok = true;
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(send_ok, "FFI send should succeed");

    let deadline = Instant::now() + Duration::from_secs(8);
    let mut received = None;
    while Instant::now() < deadline {
        let mut metrics = QuicMetrics {
            outbound_queued: 0,
            send_attempts: 0,
            send_success: 0,
            send_errors: 0,
            inbound_received: 0,
            inbound_dropped: 0,
        };
        let metrics_rc = unsafe { veil_quic_metrics(client_handle, &mut metrics) };
        assert_eq!(metrics_rc, 0, "metrics should be readable");
        if metrics.send_errors > 0 {
            panic!(
                "FFI send failed (attempts={}, success={}, errors={})",
                metrics.send_attempts, metrics.send_success, metrics.send_errors
            );
        }

        if let Some((_peer, bytes)) = server.recv() {
            received = Some(bytes);
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    veil_quic_stop(client_handle);

    let received = received.expect("server should receive payload");
    assert_eq!(received, payload);
}

#[test]
fn ffi_send_rejects_invalid_handle() {
    let payload = b"x";
    let result = unsafe {
        veil_quic_send(
            0,
            cstring("127.0.0.1:9").as_ptr(),
            payload.as_ptr(),
            payload.len(),
        )
    };
    assert_eq!(result, -1);
}
