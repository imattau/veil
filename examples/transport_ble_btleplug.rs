use std::env;
use std::time::Duration;

use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::Ed25519Verifier;
use veil_node::config::NodeRuntimeConfig;
use veil_node::service::{NodeRuntime, NodeRuntimeRunnerConfig};
use veil_transport::adapter::InMemoryAdapter;
use veil_transport_ble::BlePeer;

#[cfg(feature = "btleplug")]
use veil_transport_ble::btleplug_backend::{BtleplugLink, BtleplugLinkConfig};
#[cfg(feature = "btleplug")]
use veil_transport_ble::{BleAdapter, BleAdapterConfig};

#[cfg(feature = "btleplug")]
fn main() {
    let key = [0xA5_u8; 32];
    let node_cfg = NodeRuntimeConfig::edge_forwarder_hot_cache_defaults();

    let allowlist = env::var("VEIL_BLE_ALLOWLIST")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    let link = BtleplugLink::spawn(BtleplugLinkConfig {
        allowlist,
        ..BtleplugLinkConfig::default()
    })
    .expect("btleplug link should start");

    let ble_adapter = BleAdapter::new(link, BleAdapterConfig::default());
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

    let fast_peers = env::var("VEIL_BLE_PEERS")
        .unwrap_or_default()
        .split(',')
        .map(|s| BlePeer::new(s.trim()))
        .collect::<Vec<_>>();
    let fallback_peers = vec!["fallback-peer".to_string()];

    let exit = runtime.run_steps(
        20,
        &fast_peers,
        &fallback_peers,
        NodeRuntimeRunnerConfig {
            start_step: 0,
            tick_interval: Duration::from_millis(100),
            error_backoff: Duration::from_millis(250),
            max_consecutive_errors: Some(16),
        },
        None,
    );

    println!("transport_ble_btleplug complete: {exit:?}");
}

#[cfg(not(feature = "btleplug"))]
fn main() {
    eprintln!("Enable the btleplug backend: cargo run --example transport_ble_btleplug --features veil-transport-ble/btleplug");
}
