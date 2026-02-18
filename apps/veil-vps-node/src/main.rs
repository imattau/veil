use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use std::time::{Duration, Instant};

use clap::Parser;
use tracing::{error, info};

mod admin_auth;
mod cli;
mod config;
mod fallback_peers;
mod fallback_transport;
mod http_admin_policy;
mod http_server;
mod logger;
mod metrics_state;
mod node_bootstrap;
mod nostr_bridge;
mod nostr_bridge_event;
mod nostr_bridge_state;
mod nostr_secret;
mod peer_runtime;
mod peer_store;
mod recording_adapter;
mod runtime_admin;
mod runtime_bridge;
mod runtime_bridge_setup;
mod runtime_config_builder;
mod runtime_housekeeping;
mod runtime_identity_setup;
mod runtime_inputs;
mod runtime_metrics;
mod runtime_payloads;
mod runtime_startup;
mod runtime_state_setup;
mod runtime_tick;
mod runtime_transport_setup;
mod settings_db;
mod settings_runtime;
mod time_utils;

use admin_auth::{AdminAuthState, AdminLoginRequest, AdminSettingUpsertRequest};
use cli::{Cli, Commands, SettingsCommands};
#[cfg(test)]
use fallback_peers::merge_peers;
#[cfg(test)]
use fallback_peers::{encode_fallback_peers, parse_fallback_peer_strings};
#[cfg(test)]
use fallback_transport::FallbackPeer;
use logger::LogBuffer;
use node_bootstrap::{parse_core_tags, parse_required_signed_namespaces};
use runtime_admin::init_admin_runtime;
use runtime_bridge_setup::{init_bridge_setup, BridgeRuntimeSetup, BridgeSetupInputs};
use runtime_config_builder::{build_runtime_config, RuntimeConfigInputs};
use runtime_housekeeping::{
    handle_shutdown_if_requested, maybe_log_transport_health, maybe_snapshot_state,
};
use runtime_identity_setup::{init_runtime_identity, RuntimeIdentityInputs, RuntimeIdentitySetup};
use runtime_inputs::RuntimeInputs;
use runtime_metrics::set_nostr_bridge_relays_configured;
use runtime_startup::{apply_settings_mode, init_tracing, load_env_and_log_startup};
use runtime_state_setup::{init_runtime_state_setup, RuntimeStateSetup};
use runtime_tick::{run_runtime_tick, RuntimeTickInputs};
use runtime_transport_setup::{init_transport_setup, TransportSetup};
#[cfg(test)]
use settings_runtime::normalize_settings_key;
use settings_runtime::{maybe_handle_settings_command, settings_db_path_from_env};
use veil_core::Namespace;
use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::NostrVerifier;
use veil_node::batch::FeedBatcher;
use veil_node::persistence::load_state_or_default;
use veil_node::service::NodeRuntime;

use crate::config::VpsConfig;

#[tokio::main]
async fn main() {
    let log_buffer = Arc::new(LogBuffer::new(1000));
    init_tracing(&log_buffer);

    let cli = Cli::parse();
    load_env_and_log_startup(cli.config.as_deref());

    if maybe_handle_settings_command(cli.command.as_ref()) {
        return;
    }

    let settings_db_path = settings_db_path_from_env();
    apply_settings_mode(cli.safe_mode, &settings_db_path);

    let runtime_inputs = match VpsConfig::new(cli.config.clone()) {
        Ok(cfg) => RuntimeInputs::from_config(cfg),
        Err(err) => {
            error!("failed to load config: {err}");
            std::process::exit(1);
        }
    };

    let RuntimeInputs {
        quic_alpn,
        state_path,
        node_key_path,
        node_key_input,
        quic_cert_path,
        quic_key_path,
        snapshot_interval,
        tick_interval,
        health_bind,
        health_port,
        admin_session_db_path,
        peer_db_path,
        max_dynamic_peers,
        max_peer_db_rows,
        quic_bind,
        ws_url,
        ws_listen,
        ws_peer,
        ws_peer_id,
        tor_socks_addr,
        fast_peers,
        core_tags,
        tor_peers,
        #[cfg(feature = "ble")]
        ble_enabled,
        #[cfg(feature = "ble")]
        ble_peers,
        #[cfg(feature = "ble")]
        ble_allowlist,
        #[cfg(feature = "ble")]
        ble_mtu,
        adaptive_scoring,
        probabilistic_forwarding,
        forwarding_min_probability,
        forwarding_replica_divisor,
        bloom_exchange,
        bloom_interval_steps,
        bloom_false_positive_rate,
        max_cache_shards,
        bucket_jitter,
        open_relay,
        blocked_peers,
        nostr_bridge_enabled,
        nostr_bridge_relays,
        nostr_bridge_channel,
        nostr_bridge_namespace,
        nostr_bridge_since,
        nostr_bridge_state_path,
        nostr_bridge_max_seen,
        nostr_bridge_persist_every,
        required_signed_raw,
        quic_trusted_certs,
    } = runtime_inputs;

    if !quic_alpn.trim().is_empty() {
        std::env::set_var("VEIL_QUIC_ALPN", &quic_alpn);
        info!("quic: using VEIL_VPS_QUIC_ALPN from config: {quic_alpn}");
    }

    let required_signed = parse_required_signed_namespaces(&required_signed_raw);

    info!(
        "nostr bridge config: enabled={}, relays={:?}, channel={}, namespace={}, since={:?}, state={}",
        nostr_bridge_enabled,
        nostr_bridge_relays,
        nostr_bridge_channel,
        nostr_bridge_namespace,
        nostr_bridge_since,
        nostr_bridge_state_path.display(),
    );

    let RuntimeIdentitySetup {
        decrypt_key,
        node_signer,
        node_pubkey,
        node_secret_hex,
        node_secret_nsec,
        node_pubkey_hex,
        identity,
        trusted,
    } = match init_runtime_identity(RuntimeIdentityInputs {
        node_key_input: node_key_input.as_deref(),
        node_key_path: &node_key_path,
        quic_cert_path: &quic_cert_path,
        quic_key_path: &quic_key_path,
        quic_trusted_certs: &quic_trusted_certs,
    }) {
        Ok(setup) => setup,
        Err(err) => {
            error!("{err}");
            return;
        }
    };

    if let Some(Commands::Identity) = &cli.command {
        println!("nsec: {node_secret_nsec}");
        println!("hex:  {node_secret_hex}");
        return;
    }

    let mut state = load_state_or_default(&state_path).unwrap_or_default();
    let core_tags = parse_core_tags(&core_tags);
    if !core_tags.is_empty() {
        let before = state.subscriptions.len();
        for tag in core_tags {
            state.subscriptions.insert(tag);
        }
        let added = state.subscriptions.len().saturating_sub(before);
        info!("auto-subscribed to {added} core tags");
    }

    let cfg = build_runtime_config(RuntimeConfigInputs {
        max_cache_shards,
        bucket_jitter,
        required_signed,
        adaptive_scoring,
        probabilistic_forwarding,
        forwarding_min_probability,
        forwarding_replica_divisor,
        bloom_exchange,
        bloom_interval_steps,
        bloom_false_positive_rate,
        open_relay,
        blocked_peers,
    });

    let discovery_namespace = Namespace(4096);
    let discovery_tag = veil_android_node::discovery_tag(discovery_namespace);
    state.subscriptions.insert(discovery_tag);

    let runtime_config = Arc::new(Mutex::new(cfg));
    let discovery_table: Arc<Mutex<HashMap<String, veil_android_node::ContactBundle>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let TransportSetup {
        fast_adapter,
        fallback_adapter,
        fallback_peers,
        peer_db,
    } = match init_transport_setup(
        &quic_bind,
        identity,
        trusted,
        ws_url,
        ws_listen,
        ws_peer_id,
        ws_peer,
        tor_socks_addr,
        tor_peers,
        #[cfg(feature = "ble")]
        ble_enabled,
        #[cfg(feature = "ble")]
        ble_peers,
        #[cfg(feature = "ble")]
        ble_allowlist,
        #[cfg(feature = "ble")]
        ble_mtu,
        &peer_db_path,
        max_dynamic_peers,
    ) {
        Ok(setup) => setup,
        Err(err) => {
            error!("{err}");
            return;
        }
    };

    let BridgeRuntimeSetup {
        bridge_namespace,
        bridge_tag,
        mut nostr_bridge_rx,
    } = init_bridge_setup(
        &mut state,
        &node_pubkey,
        BridgeSetupInputs {
            enabled: nostr_bridge_enabled,
            relays: &nostr_bridge_relays,
            channel_id: &nostr_bridge_channel,
            namespace: nostr_bridge_namespace,
            since: nostr_bridge_since,
            state_path: &nostr_bridge_state_path,
            max_seen_ids: nostr_bridge_max_seen,
            persist_every_updates: nostr_bridge_persist_every,
        },
    );

    let mut runtime = NodeRuntime::new(
        state,
        fast_adapter,
        fallback_adapter,
        runtime_config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone(),
        decrypt_key,
        XChaCha20Poly1305Cipher,
        NostrVerifier,
    );
    let mut bridge_batcher = FeedBatcher::default();

    let mut last_snapshot = Instant::now();
    let mut last_health_log = Instant::now();
    let health_log_interval = Duration::from_secs(30);

    let RuntimeStateSetup {
        metrics,
        shutdown,
        peer_snapshot,
        feed_history,
    } = init_runtime_state_setup(nostr_bridge_enabled, nostr_bridge_relays.len());
    let _admin_auth = match init_admin_runtime(
        &admin_session_db_path,
        &settings_db_path,
        node_pubkey,
        &node_pubkey_hex,
        &node_secret_hex,
        &node_secret_nsec,
        &health_bind,
        health_port,
        &metrics,
        &peer_snapshot,
        &feed_history,
        &discovery_table,
        &shutdown,
        &log_buffer,
        &runtime_config,
    )
    .await
    {
        Ok(admin_auth) => admin_auth,
        Err(err) => {
            error!("{err}");
            return;
        }
    };

    let mut now_step = 0_u64;
    loop {
        if shutdown.load(Ordering::Relaxed)
            && handle_shutdown_if_requested(
                &shutdown,
                &state_path,
                &mut runtime.state,
                peer_db.as_ref(),
                &peer_snapshot,
                runtime.fast_adapter.snapshot_seen(),
                runtime.fallback_adapter.snapshot_seen(),
                max_peer_db_rows,
            )
        {
            break;
        }
        let metrics_ref = Arc::clone(&metrics);
        set_nostr_bridge_relays_configured(metrics_ref.as_ref(), nostr_bridge_relays.len());

        now_step = run_runtime_tick(RuntimeTickInputs {
            runtime: &mut runtime,
            runtime_config: &runtime_config,
            fast_peers: &fast_peers,
            fallback_peers: &fallback_peers,
            max_dynamic_peers,
            metrics: &metrics,
            discovery_table: &discovery_table,
            feed_history: &feed_history,
            nostr_bridge_rx: nostr_bridge_rx.as_mut(),
            bridge_batcher: &mut bridge_batcher,
            bridge_namespace,
            bridge_tag,
            now_step,
            node_signer: &node_signer,
        });

        maybe_snapshot_state(
            &mut last_snapshot,
            snapshot_interval,
            &state_path,
            &mut runtime.state,
            peer_db.as_ref(),
            &peer_snapshot,
            runtime.fast_adapter.snapshot_seen(),
            runtime.fallback_adapter.snapshot_seen(),
            max_peer_db_rows,
        );

        if last_health_log.elapsed() >= health_log_interval {
            let health = runtime.transport_health();
            maybe_log_transport_health(
                &mut last_health_log,
                health_log_interval,
                metrics.as_ref(),
                &health.fast_lane,
                &health.fallback_lane,
            );
        }

        tokio::time::sleep(tick_interval).await;
    }
}

#[cfg(test)]
mod main_tests;
