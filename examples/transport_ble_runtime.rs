use std::time::Duration;

use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::Ed25519Verifier;
use veil_node::config::NodeRuntimeConfig;
use veil_node::service::{NodeRuntime, NodeRuntimeRunnerConfig};
use veil_transport::adapter::InMemoryAdapter;
use veil_transport_ble::{BleAdapter, BleAdapterConfig, BlePeer, MockBleLink};

fn main() {
    let key = [0xA5_u8; 32];
    let node_cfg = NodeRuntimeConfig::edge_forwarder_hot_cache_defaults();

    let ble_link = MockBleLink::with_mtu(64);
    let ble_adapter = BleAdapter::new(ble_link, BleAdapterConfig::default());
    let fallback_adapter = InMemoryAdapter::default();

    let mut runtime = NodeRuntime::new(
        veil_node::state::NodeState::default(),
        ble_adapter,
        fallback_adapter,
        node_cfg,
        key,
        XChaCha20Poly1305Cipher,
        Ed25519Verifier,
    );

    let fast_peers = vec![BlePeer::new("ble-peer")];
    let fallback_peers = vec!["fallback-peer".to_string()];

    let exit = runtime.run_steps(
        4,
        &fast_peers,
        &fallback_peers,
        NodeRuntimeRunnerConfig {
            start_step: 0,
            tick_interval: Duration::from_millis(50),
            error_backoff: Duration::from_millis(250),
            max_consecutive_errors: Some(8),
        },
        None,
    );

    println!("transport_ble_runtime complete: {exit:?}");
}
