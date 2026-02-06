use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::Ed25519Verifier;
use veil_node::config::NodeRuntimeConfig;
use veil_node::service::{NodeRuntime, NodeRuntimeRunnerConfig, NodeRuntimeRunnerExit};
use veil_transport::adapter::TransportAdapter;
use veil_transport_tor::{TorSocksAdapter, TorSocksAdapterConfig};
use veil_transport_websocket::{WebSocketAdapter, WebSocketAdapterConfig};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e2e_transport_runtime_smoke_starts_and_stops() {
    if std::env::var("VEIL_E2E_NETWORK").is_err() {
        eprintln!("skipping transport runtime smoke test (set VEIL_E2E_NETWORK=1 to enable)");
        return;
    }

    let ws_listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!(
                "skipping transport runtime smoke test (bind failed: {err})"
            );
            return;
        }
    };
    let ws_addr = ws_listener.local_addr().expect("ws addr should resolve");
    let ws_url = format!("ws://{ws_addr}");

    let ws_server = tokio::spawn(async move {
        let (stream, _) = ws_listener.accept().await.expect("ws accept should work");
        let ws = accept_async(stream)
            .await
            .expect("ws handshake should work");
        let (mut write, mut read) = ws.split();
        write
            .send(Message::Binary(vec![0xDE, 0xAD]))
            .await
            .expect("ws write should work");
        let _ = tokio::time::timeout(Duration::from_millis(300), read.next()).await;
    });

    let socks_listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!(
                "skipping transport runtime smoke test (socks bind failed: {err})"
            );
            return;
        }
    };
    let socks_addr = socks_listener
        .local_addr()
        .expect("socks addr should resolve");
    let (payload_tx, payload_rx) = tokio::sync::oneshot::channel::<Vec<u8>>();
    let socks_server = tokio::spawn(async move {
        let (mut sock, _) = socks_listener
            .accept()
            .await
            .expect("socks accept should work");

        let mut hello = [0_u8; 3];
        sock.read_exact(&mut hello).await.expect("socks hello");
        sock.write_all(&[0x05, 0x00])
            .await
            .expect("socks method ack");

        let mut head = [0_u8; 4];
        sock.read_exact(&mut head)
            .await
            .expect("socks connect head");
        let atyp = head[3];
        if atyp == 0x03 {
            let mut len = [0_u8; 1];
            sock.read_exact(&mut len).await.expect("domain len");
            let mut domain = vec![0_u8; len[0] as usize];
            sock.read_exact(&mut domain).await.expect("domain");
        } else {
            panic!("expected domain atyp in test");
        }
        let mut port = [0_u8; 2];
        sock.read_exact(&mut port).await.expect("port");

        sock.write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
            .await
            .expect("socks connect ack");

        let mut payload = vec![0_u8; 4];
        sock.read_exact(&mut payload).await.expect("payload");
        let _ = payload_tx.send(payload);
    });

    let fast_adapter = WebSocketAdapter::connect(WebSocketAdapterConfig {
        url: ws_url,
        peer_id: "ws-server".to_string(),
        reconnect: true,
        reconnect_initial: Duration::from_millis(50),
        reconnect_max: Duration::from_millis(250),
        outbound_queue_capacity: 256,
        inbound_queue_capacity: 256,
        max_payload_hint: Some(64 * 1024),
    })
    .expect("websocket adapter should initialize");

    let fallback_adapter = TorSocksAdapter::connect(TorSocksAdapterConfig {
        socks_proxy_addr: socks_addr.to_string(),
        connect_timeout: Duration::from_secs(2),
        send_timeout: Duration::from_secs(2),
        outbound_queue_capacity: 128,
        max_payload_hint: Some(64 * 1024),
    })
    .expect("tor adapter should initialize");

    let mut runtime = NodeRuntime::new(
        veil_node::state::NodeState::default(),
        fast_adapter,
        fallback_adapter,
        NodeRuntimeConfig::edge_forwarder_hot_cache_defaults(),
        [0xA5; 32],
        XChaCha20Poly1305Cipher,
        Ed25519Verifier,
    );

    runtime
        .fallback_adapter
        .send(&"example.com:443".to_string(), b"ping")
        .expect("fallback send should queue");

    let peers = vec!["peer-a".to_string()];
    let exit = runtime.run_steps(
        4,
        &peers,
        &peers,
        NodeRuntimeRunnerConfig {
            start_step: 0,
            tick_interval: Duration::ZERO,
            error_backoff: Duration::ZERO,
            max_consecutive_errors: Some(4),
        },
        None,
    );
    assert_eq!(exit, NodeRuntimeRunnerExit::Completed { steps: 4 });

    let payload = tokio::time::timeout(Duration::from_secs(2), payload_rx)
        .await
        .expect("tor payload should arrive")
        .expect("tor payload should be captured");
    assert_eq!(payload, b"ping".to_vec());

    ws_server.await.expect("ws server should complete");
    socks_server.await.expect("socks server should complete");
}
