use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use std::time::{Duration, Instant};

use clap::Parser;
use tracing::{error, info, warn};

mod admin_auth;
mod cli;
mod config;
mod fallback_peers;
mod fallback_transport;
mod http_server;
mod logger;
mod metrics_state;
mod node_bootstrap;
mod nostr_bridge;
mod nostr_secret;
mod peer_runtime;
mod peer_store;
mod runtime_bridge;
mod runtime_housekeeping;
mod runtime_metrics;
mod runtime_payloads;
mod settings_db;
mod settings_runtime;
mod time_utils;

use admin_auth::{AdminAuthState, AdminLoginRequest, AdminSettingUpsertRequest};
use cli::{Cli, Commands, SettingsCommands};
#[cfg(test)]
use fallback_peers::merge_peers;
use fallback_peers::parse_fallback_peers;
#[cfg(test)]
use fallback_peers::{encode_fallback_peers, parse_fallback_peer_strings};
#[cfg(test)]
use fallback_transport::FallbackPeer;
use fallback_transport::{CombinedFallbackAdapter, RecordingAdapter};
use logger::{AdminLoggerLayer, LogBuffer};
use metrics_state::MetricsState;
use node_bootstrap::{
    load_or_create_identity, load_or_create_node_key, load_trusted_certs, parse_core_tags,
    parse_required_signed_namespaces, pseudo_pubkey_for_peer,
};
use nostr_bridge::{start_nostr_bridge, NostrBridgeConfig};
use nostr_secret::{decode_nostr_secret_input, encode_nostr_nsec};
use peer_runtime::{compute_peer_lists, seed_discovered_peers};
use peer_store::{load_peer_list, open_peer_db};
use runtime_bridge::{drain_bridged_items, publish_bridge_batch};
use runtime_housekeeping::{
    handle_shutdown_if_requested, maybe_log_transport_health, maybe_snapshot_state,
};
use runtime_metrics::{
    note_ack_clears, note_send_failures, note_tick, set_nostr_bridge_relays_configured,
};
use runtime_payloads::handle_runtime_delivered_payload;
#[cfg(test)]
use settings_runtime::normalize_settings_key;
use settings_runtime::{
    apply_settings_db_overrides, maybe_handle_settings_command, settings_db_path_from_env,
};
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::flag;
use time_utils::current_epoch;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use veil_core::tags::derive_channel_feed_tag;
use veil_core::Namespace;
use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::{NostrSigner, NostrVerifier, Signer};
use veil_node::batch::FeedBatcher;
use veil_node::config::{
    AdaptiveLaneScoringConfig, BloomExchangeConfig, NodeRuntimeConfig,
    ProbabilisticForwardingConfig,
};
use veil_node::persistence::load_state_or_default;
use veil_node::publish::PublishQueueTickParams;
use veil_node::service::{NodeRuntime, NodeRuntimeCallbacks};
#[cfg(feature = "ble-btleplug")]
use veil_transport_ble::btleplug_backend::{BtleplugLink, BtleplugLinkConfig};
#[cfg(all(feature = "ble", not(feature = "ble-btleplug")))]
use veil_transport_ble::MockBleLink;
#[cfg(feature = "ble")]
use veil_transport_ble::{BleAdapter, BleAdapterConfig};
use veil_transport_quic::{QuicAdapter, QuicAdapterConfig};
use veil_transport_tor::{TorSocksAdapter, TorSocksAdapterConfig};
use veil_transport_websocket::{
    WebSocketAdapter, WebSocketAdapterConfig, WebSocketServerAdapter, WebSocketServerAdapterConfig,
};

use crate::config::VpsConfig;

#[tokio::main]
async fn main() {
    let log_buffer = Arc::new(LogBuffer::new(1000));
    let filter = std::env::var("VEIL_LOG").unwrap_or_else(|_| "info".to_string());

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(filter))
        .with(tracing_subscriber::fmt::layer())
        .with(AdminLoggerLayer {
            buffer: Arc::clone(&log_buffer),
        })
        .init();

    dotenvy::dotenv().ok();
    if let Ok(cwd) = std::env::current_dir() {
        info!("starting veil-vps-node in {}", cwd.display());
        if cwd.to_string_lossy().starts_with("/home/")
            && !cwd.to_string_lossy().contains("workspace")
        {
            warn!("Node is running from a home directory. Ensure this is intentional and that absolute paths are set for 'data/' directories if running as a service user.");
        }
    }

    let cli = Cli::parse();

    if let Some(config_path) = &cli.config {
        if config_path.extension().and_then(|ext| ext.to_str()) == Some("env") {
            match dotenvy::from_path(config_path) {
                Ok(_) => info!("pre-loaded environment from {}", config_path.display()),
                Err(err) => warn!(
                    "failed to pre-load .env from {}: {}",
                    config_path.display(),
                    err
                ),
            }
        }
    }

    if maybe_handle_settings_command(cli.command.as_ref()) {
        return;
    }

    let settings_db_path = settings_db_path_from_env();
    if cli.safe_mode {
        warn!(
            "SAFE MODE ENABLED: Ignoring all settings overrides from {}",
            settings_db_path.display()
        );
    } else {
        apply_settings_db_overrides(&settings_db_path);
    }

    let config = match VpsConfig::new(cli.config.clone()) {
        Ok(cfg) => cfg,
        Err(err) => {
            error!("failed to load config: {err}");
            std::process::exit(1);
        }
    };

    let raw_alpn = &config.quic_alpn;
    if !raw_alpn.trim().is_empty() {
        std::env::set_var("VEIL_QUIC_ALPN", raw_alpn);
        info!("quic: using VEIL_VPS_QUIC_ALPN from config: {raw_alpn}");
    }

    let state_path = config.state_path.clone();
    let node_key_path = config.node_key_path.clone();
    let quic_cert_path = config.quic_cert_path.clone();
    let quic_key_path = config.quic_key_path.clone();
    let snapshot_interval = config.snapshot_interval;
    let tick_interval = config.tick_interval;
    let health_bind = config.health_bind.clone();
    let health_port = config.health_port;
    let admin_session_db_path = config.admin_session_db_path.clone();
    let fast_peers = config.fast_peers.clone();
    let core_tags = config.core_tags.clone();
    let tor_peers = config.tor_peers.clone();
    #[cfg(feature = "ble")]
    let ble_enabled = config.ble_enabled;
    #[cfg(feature = "ble")]
    let ble_peers = config.ble_peers.clone();
    #[cfg(feature = "ble")]
    let ble_allowlist = config.ble_allowlist.clone();
    #[cfg(feature = "ble")]
    let ble_mtu = config.ble_mtu;
    let peer_db_path = config.peer_db_path.clone();
    let max_dynamic_peers = config.max_dynamic_peers;
    let max_peer_db_rows = max_dynamic_peers.saturating_mul(2).max(1);

    let quic_bind = config.quic_bind.clone();
    let ws_url = config.ws_url.clone().filter(|s| !s.trim().is_empty());
    let ws_listen = config.ws_listen.clone().filter(|s| !s.trim().is_empty());
    let ws_peer = config.ws_peer.clone().filter(|s| !s.trim().is_empty());
    let ws_peer_id = ws_peer.clone().unwrap_or_else(|| "ws-peer".to_string());
    let tor_socks_addr = config
        .tor_socks_addr
        .clone()
        .filter(|s| !s.trim().is_empty());

    let adaptive_scoring = config.adaptive_lane_scoring;
    let probabilistic_forwarding = config.probabilistic_forwarding;
    let forwarding_min_probability = config.forwarding_min_probability;
    let forwarding_replica_divisor = config.forwarding_replica_divisor;
    let bloom_exchange = config.bloom_exchange;
    let bloom_interval_steps = config.bloom_interval_steps;
    let bloom_false_positive_rate = config.bloom_false_positive_rate;
    let max_cache_shards = config.max_cache_shards;
    let bucket_jitter = config.bucket_jitter;
    let open_relay = config.open_relay;
    let blocked_peers = config.blocked_peers.clone();
    let nostr_bridge_enabled = config.nostr_bridge_enabled;
    let nostr_bridge_relays = config.nostr_bridge_relays.clone();
    let nostr_bridge_channel = config.nostr_bridge_channel_id.clone();
    let nostr_bridge_namespace = config.nostr_bridge_namespace as u16;
    let nostr_bridge_since = config.nostr_bridge_since;
    let nostr_bridge_state_path = config.nostr_bridge_state_path.clone();
    let nostr_bridge_max_seen = config.nostr_bridge_max_seen_ids;
    let nostr_bridge_persist_every = config.nostr_bridge_persist_every_updates;
    let required_signed = parse_required_signed_namespaces(&config.required_signed_namespaces);

    info!(
        "nostr bridge config: enabled={}, relays={:?}, channel={}, namespace={}, since={:?}, state={}",
        nostr_bridge_enabled,
        nostr_bridge_relays,
        nostr_bridge_channel,
        nostr_bridge_namespace,
        nostr_bridge_since,
        nostr_bridge_state_path.display(),
    );

    let node_key = if let Some(key_input) = &config.node_key {
        match decode_nostr_secret_input(key_input) {
            Some(key) => {
                info!("using node key from configuration/environment");
                key
            }
            None => {
                error!("fatal: invalid node_key provided in configuration/environment");
                return;
            }
        }
    } else {
        match load_or_create_node_key(&node_key_path) {
            Ok(key) => key,
            Err(err) => {
                error!("fatal: {err}");
                return;
            }
        }
    };
    let node_signer = NostrSigner::from_secret(node_key).expect("node key validated");
    let node_pubkey = node_signer.public_key();
    let node_secret_hex = hex::encode(node_key);
    let node_secret_nsec = encode_nostr_nsec(node_key).unwrap_or_default();
    let node_pubkey_hex = hex::encode(node_pubkey);
    info!("node identity (nostr x-only pubkey): {node_pubkey_hex}");

    if let Some(Commands::Identity) = &cli.command {
        println!("nsec: {node_secret_nsec}");
        println!("hex:  {node_secret_hex}");
        return;
    }

    let identity = match load_or_create_identity(&quic_cert_path, &quic_key_path) {
        Ok(identity) => identity,
        Err(err) => {
            error!("fatal: {err}");
            return;
        }
    };

    let trusted_cert_paths = config.quic_trusted_certs.clone();
    let mut trusted = load_trusted_certs(&trusted_cert_paths);
    if trusted.is_empty() {
        trusted.push(identity.cert_chain_der[0].clone());
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

    let mut cfg = NodeRuntimeConfig::edge_forwarder_hot_cache_defaults();
    cfg.max_cache_shards = max_cache_shards;
    cfg.bucket_jitter_extra_levels = bucket_jitter;
    cfg.required_signed_namespaces = required_signed;
    cfg.adaptive_lane_scoring = AdaptiveLaneScoringConfig {
        enabled: adaptive_scoring,
        ..AdaptiveLaneScoringConfig::default()
    };
    cfg.probabilistic_forwarding = ProbabilisticForwardingConfig {
        enabled: probabilistic_forwarding,
        min_probability: forwarding_min_probability.clamp(0.0, 1.0),
        replica_divisor: forwarding_replica_divisor.max(1),
    };
    cfg.bloom_exchange = BloomExchangeConfig {
        enabled: bloom_exchange,
        interval_steps: bloom_interval_steps.max(1),
        false_positive_rate: bloom_false_positive_rate.clamp(0.001, 0.5),
    };
    if open_relay {
        cfg.accept_all_tags = true;
        cfg.probabilistic_forwarding.enabled = false;
        let mut wot_cfg = cfg.wot_policy.config;
        wot_cfg.trusted_forward_quota = 1.0;
        wot_cfg.known_forward_quota = 1.0;
        wot_cfg.unknown_forward_quota = 1.0;
        wot_cfg.muted_forward_quota = 1.0;
        wot_cfg.blocked_forward_quota = 0.0;
        cfg.wot_policy.update_config(wot_cfg);
        info!("open relay mode enabled: accepting all tags and full non-blocked forwarding");
    }
    for peer in blocked_peers {
        let pseudo = pseudo_pubkey_for_peer(&peer);
        cfg.bind_peer_publisher(peer.clone(), pseudo);
        cfg.wot_policy.block(pseudo);
    }

    let discovery_namespace = Namespace(4096);
    let discovery_tag = veil_android_node::discovery_tag(discovery_namespace);
    state.subscriptions.insert(discovery_tag);

    let runtime_config = Arc::new(Mutex::new(cfg));
    let discovery_table: Arc<Mutex<HashMap<String, veil_android_node::ContactBundle>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let quic_bind_addr = match quic_bind.parse() {
        Ok(addr) => addr,
        Err(err) => {
            eprintln!("fatal: invalid VEIL_VPS_QUIC_BIND: {err}");
            return;
        }
    };

    let fast_adapter_raw = match QuicAdapter::connect(QuicAdapterConfig {
        bind_addr: quic_bind_addr,
        server_name: "veil-node".to_string(),
        identity,
        trusted_peer_certs_der: trusted,
        connect_timeout: Duration::from_secs(3),
        send_timeout: Duration::from_secs(3),
        outbound_queue_capacity: 2048,
        inbound_queue_capacity: 4096,
        max_recv_bytes: 128 * 1024,
        max_payload_hint: Some(64 * 1024),
    }) {
        Ok(adapter) => adapter,
        Err(err) => {
            error!("fatal: quic adapter failed to start: {err}");
            return;
        }
    };

    let ws_adapter = ws_url.map(|url| {
        WebSocketAdapter::connect(WebSocketAdapterConfig {
            url,
            peer_id: ws_peer_id.clone(),
            reconnect: true,
            reconnect_initial: Duration::from_millis(250),
            reconnect_max: Duration::from_secs(10),
            outbound_queue_capacity: 1024,
            inbound_queue_capacity: 4096,
            max_payload_hint: Some(64 * 1024),
        })
        .expect("websocket adapter should start")
    });

    let ws_server_adapter = ws_listen.map(|addr| {
        let adapter = WebSocketServerAdapter::listen(WebSocketServerAdapterConfig::new(&addr))
            .expect("websocket server should start");
        info!("websocket server listening on {addr}");
        adapter
    });

    let tor_adapter = tor_socks_addr.map(|addr| {
        TorSocksAdapter::connect(TorSocksAdapterConfig {
            socks_proxy_addr: addr,
            connect_timeout: Duration::from_secs(8),
            send_timeout: Duration::from_secs(8),
            outbound_queue_capacity: 1024,
            max_payload_hint: Some(64 * 1024),
        })
        .expect("tor adapter should start")
    });

    #[cfg(feature = "ble")]
    let ble_adapter = if ble_enabled {
        #[cfg(feature = "ble-btleplug")]
        let link = match BtleplugLink::spawn(BtleplugLinkConfig {
            allowlist: ble_allowlist,
            ..BtleplugLinkConfig::default()
        }) {
            Ok(link) => link,
            Err(err) => {
                error!("ble adapter failed to start: {err:?}");
                return;
            }
        };
        #[cfg(all(feature = "ble", not(feature = "ble-btleplug")))]
        let link = MockBleLink::with_mtu(ble_mtu);

        Some(BleAdapter::new(
            link,
            BleAdapterConfig {
                mtu: ble_mtu,
                max_payload_hint: Some(16 * 1024),
                drop_outbound: false,
            },
        ))
    } else {
        None
    };

    let fallback_adapter = CombinedFallbackAdapter::new(
        ws_adapter,
        ws_server_adapter,
        tor_adapter,
        #[cfg(feature = "ble")]
        ble_adapter,
    );
    let ws_enabled = fallback_adapter.ws_enabled();
    let ws_server_enabled = fallback_adapter.ws_server_enabled();
    let tor_enabled = fallback_adapter.tor_enabled();
    #[cfg(feature = "ble")]
    let ble_enabled_runtime = fallback_adapter.ble_enabled();
    let fallback_peers = parse_fallback_peers(
        ws_peer,
        tor_peers,
        #[cfg(feature = "ble")]
        ble_peers,
    );

    let discovered_fast = Arc::new(Mutex::new(HashSet::new()));
    let discovered_fallback = Arc::new(Mutex::new(HashSet::new()));

    let fast_adapter = RecordingAdapter::new_bounded(
        fast_adapter_raw,
        Arc::clone(&discovered_fast),
        max_dynamic_peers,
    );
    let fallback_adapter = RecordingAdapter::new_bounded(
        fallback_adapter,
        Arc::clone(&discovered_fallback),
        max_dynamic_peers,
    );

    let peer_db = open_peer_db(&peer_db_path);
    let discovered_seed = peer_db
        .as_ref()
        .map(|conn| load_peer_list(conn, max_dynamic_peers))
        .unwrap_or_default();
    seed_discovered_peers(
        &discovered_seed,
        &discovered_fast,
        &discovered_fallback,
        ws_enabled,
        ws_server_enabled,
        tor_enabled,
        #[cfg(feature = "ble")]
        ble_enabled_runtime,
    );

    let bridge_namespace = Namespace(nostr_bridge_namespace);
    let bridge_tag = derive_channel_feed_tag(&node_pubkey, bridge_namespace, &nostr_bridge_channel);
    state.subscriptions.insert(bridge_tag);

    let mut runtime = NodeRuntime::new(
        state,
        fast_adapter,
        fallback_adapter,
        runtime_config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone(),
        veil_crypto::keys::derive_encrypt_key(&node_key),
        XChaCha20Poly1305Cipher,
        NostrVerifier,
    );
    let mut bridge_batcher = FeedBatcher::default();
    let mut nostr_bridge_rx = if nostr_bridge_enabled {
        if nostr_bridge_relays.is_empty() {
            warn!("nostr bridge enabled but VEIL_VPS_NOSTR_BRIDGE_RELAYS is empty; bridge not started");
            None
        } else {
            info!(
                "nostr bridge enabled with {} relays ({:?}), channel={}, namespace={}",
                nostr_bridge_relays.len(),
                nostr_bridge_relays,
                nostr_bridge_channel,
                nostr_bridge_namespace
            );
            Some(start_nostr_bridge(NostrBridgeConfig {
                relays: nostr_bridge_relays.clone(),
                channel_id: nostr_bridge_channel.clone(),
                namespace: nostr_bridge_namespace,
                since: nostr_bridge_since,
                state_path: Some(nostr_bridge_state_path.clone()),
                max_seen_ids: nostr_bridge_max_seen,
                persist_every_updates: nostr_bridge_persist_every,
            }))
        }
    } else {
        None
    };

    let mut last_snapshot = Instant::now();
    let mut last_health_log = Instant::now();
    let health_log_interval = Duration::from_secs(30);

    let metrics = Arc::new(MetricsState::default());
    metrics
        .nostr_bridge_enabled
        .store(u64::from(nostr_bridge_enabled), Ordering::Relaxed);
    metrics
        .nostr_bridge_relays_configured
        .store(nostr_bridge_relays.len() as u64, Ordering::Relaxed);
    let shutdown = Arc::new(AtomicBool::new(false));
    let _ = flag::register(SIGTERM, Arc::clone(&shutdown));
    let _ = flag::register(SIGINT, Arc::clone(&shutdown));
    let peer_snapshot: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let feed_history: Arc<Mutex<VecDeque<serde_json::Value>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(50)));
    if let Err(err) = AdminAuthState::bootstrap_session_db(&admin_session_db_path) {
        error!("fatal: admin auth bootstrap failed: {err}");
        std::process::exit(1);
    }
    let restored_sessions = AdminAuthState::load_sessions_from_db(&admin_session_db_path);
    if !restored_sessions.is_empty() {
        info!(
            "admin auth: restored {} active sessions from {}",
            restored_sessions.len(),
            admin_session_db_path.display()
        );
    }
    let admin_auth = Arc::new(AdminAuthState {
        server_pubkey: node_pubkey,
        server_pubkey_hex: node_pubkey_hex.clone(),
        server_secret_hex: node_secret_hex,
        server_secret_nsec: node_secret_nsec,
        session_ttl_secs: 24 * 60 * 60,
        session_db_path: admin_session_db_path,
        settings_db_path: settings_db_path.clone(),
        sessions: Mutex::new(restored_sessions),
    });
    if health_port != 0 {
        let app_state = http_server::VpsAppState {
            metrics: Arc::clone(&metrics),
            peer_snapshot: Arc::clone(&peer_snapshot),
            feed_history: Arc::clone(&feed_history),
            discovery_table: Arc::clone(&discovery_table),
            admin_auth: Arc::clone(&admin_auth),
            shutdown: Arc::clone(&shutdown),
            log_buffer: Arc::clone(&log_buffer),
            runtime_config: Arc::clone(&runtime_config),
        };
        if let Err(err) =
            http_server::spawn_health_server(app_state, &health_bind, health_port).await
        {
            error!("{err}");
            return;
        }
    }

    let mut now_step = 0_u64;
    loop {
        if handle_shutdown_if_requested(&shutdown, &state_path, &mut runtime.state) {
            break;
        }
        let metrics_ref = Arc::clone(&metrics);
        set_nostr_bridge_relays_configured(metrics_ref.as_ref(), nostr_bridge_relays.len());

        // Sync runtime config from Mutex
        {
            let cfg = runtime_config.lock().unwrap_or_else(|e| e.into_inner());
            runtime.config = cfg.clone();
        }

        let discovered_fast_snapshot = runtime.fast_adapter.snapshot_seen();
        let discovered_fallback_snapshot = runtime.fallback_adapter.snapshot_seen();
        let (fast_peer_list, fallback_peer_list) = compute_peer_lists(
            &fast_peers,
            &fallback_peers,
            &discovered_fast_snapshot,
            &discovered_fallback_snapshot,
            max_dynamic_peers,
        );

        if let Some(rx) = &mut nostr_bridge_rx {
            drain_bridged_items(
                rx,
                metrics_ref.as_ref(),
                &feed_history,
                &mut bridge_batcher,
                64,
            );
            publish_bridge_batch(
                &mut runtime.state,
                &mut runtime.fast_adapter,
                &mut runtime.fallback_adapter,
                &mut bridge_batcher,
                PublishQueueTickParams {
                    namespace: bridge_namespace,
                    epoch: current_epoch(),
                    tag: bridge_tag,
                    encrypt_key: &[0u8; 32],
                    now_step,
                    flags: veil_codec::object::OBJECT_FLAG_SIGNED
                        | veil_codec::object::OBJECT_FLAG_PUBLIC,
                    interactive_flush: false,
                    fast_peers: &fast_peer_list,
                    fallback_peers: &fallback_peer_list,
                },
                &runtime.config,
                &XChaCha20Poly1305Cipher,
                Some(&node_signer),
            );
        }

        let feed_history_ref: Arc<Mutex<VecDeque<serde_json::Value>>> = Arc::clone(&feed_history);
        let discovery_table_ref: Arc<Mutex<HashMap<String, veil_android_node::ContactBundle>>> =
            Arc::clone(&discovery_table);
        let _ = runtime.tick_with_callbacks(
            now_step,
            &fast_peer_list,
            &fallback_peer_list,
            NodeRuntimeCallbacks {
                on_delivered: Some(&mut |_root, payload| {
                    handle_runtime_delivered_payload(
                        payload,
                        metrics_ref.as_ref(),
                        &discovery_table_ref,
                        &feed_history_ref,
                    )
                }),
                on_send_failure: Some(&mut |count| {
                    note_send_failures(metrics_ref.as_ref(), count);
                }),
                on_ack_cleared: Some(&mut |count| {
                    note_ack_clears(metrics_ref.as_ref(), count);
                }),
                ..NodeRuntimeCallbacks::default()
            },
        );
        now_step = now_step.saturating_add(1);
        note_tick(metrics.as_ref());

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
mod tests {
    use super::{
        encode_fallback_peers, merge_peers, normalize_settings_key, parse_fallback_peer_strings,
        Cli, Commands, FallbackPeer, SettingsCommands,
    };

    #[test]
    fn parse_fallback_peer_strings_supports_websocket_server_prefix() {
        let parsed = parse_fallback_peer_strings(&[
            "wssrv:127.0.0.1:8080".to_string(),
            "ws:relay-a".to_string(),
            "tor:peer.onion:5000".to_string(),
        ]);
        assert!(parsed.contains(&FallbackPeer::WebSocketServer("127.0.0.1:8080".to_string())));
        assert!(parsed.contains(&FallbackPeer::WebSocket("relay-a".to_string())));
        assert!(parsed.contains(&FallbackPeer::Tor("peer.onion:5000".to_string())));
    }

    #[test]
    fn merge_peers_deduplicates_and_caps() {
        let configured = vec!["a".to_string(), "b".to_string(), "a".to_string()];
        let discovered = vec![
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
            "e".to_string(),
        ];
        let merged = merge_peers(&configured, &discovered, 4);
        assert_eq!(merged, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn encode_and_parse_roundtrip_keeps_websocket_server_peers() {
        let peers = vec![
            FallbackPeer::WebSocket("relay-a".to_string()),
            FallbackPeer::WebSocketServer("192.168.1.10:8080".to_string()),
            FallbackPeer::Tor("peer.onion:5000".to_string()),
        ];
        let encoded = encode_fallback_peers(&peers);
        let decoded = parse_fallback_peer_strings(&encoded);
        assert_eq!(decoded, peers);
    }

    #[test]
    fn normalize_settings_key_supports_legacy_nostr_toggle_name() {
        assert_eq!(
            normalize_settings_key("VEIL_VPS_NOSTR_BRIDGE_ENABLE"),
            Some("VEIL_VPS_NOSTR_BRIDGE_ENABLED")
        );
        assert_eq!(
            normalize_settings_key("VEIL_VPS_NOSTR_BRIDGE_ENABLED"),
            Some("VEIL_VPS_NOSTR_BRIDGE_ENABLED")
        );
        assert_eq!(normalize_settings_key("VEIL_VPS_UNKNOWN"), None);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn nostr_bridge_payload_publishes_and_android_receives_feed_bundle() {
        use sha2::{Digest, Sha256};
        use std::net::TcpListener;
        use std::thread;
        use std::time::Duration;
        use tokio::time::timeout;
        use tokio_tungstenite::tungstenite::{accept, Message};
        use veil_android_node::NodeState as AndroidNodeState;
        use veil_codec::object::OBJECT_FLAG_SIGNED;
        use veil_core::tags::derive_channel_feed_tag;
        use veil_core::{Epoch, Namespace};
        use veil_crypto::aead::XChaCha20Poly1305Cipher;
        use veil_crypto::signing::{NostrSigner, NostrVerifier, Signer};
        use veil_node::batch::FeedBatcher;
        use veil_node::config::NodeRuntimeConfig;
        use veil_node::publish::{publish_queue_tick_multi_lane, PublishQueueTickParams};
        use veil_node::runtime::{
            pump_multi_lane_tick_with_config, ConfigMultiLanePumpParams, RuntimeStats,
        };
        use veil_transport::adapter::{route_in_memory_outbound, InMemoryAdapter};

        let relay_listener = TcpListener::bind("127.0.0.1:0").expect("bind relay");
        let relay_addr = relay_listener.local_addr().expect("relay addr");
        let relay_url = format!("ws://{relay_addr}");
        let relay_signer = NostrSigner::from_secret([0x21; 32]).expect("valid relay signer");
        let relay_pubkey = hex::encode(relay_signer.public_key());
        let relay_created_at = 1_700_000_123u64;
        let relay_content = "bridge e2e hello";
        let relay_canonical = serde_json::json!([
            0,
            relay_pubkey,
            relay_created_at,
            1,
            serde_json::json!([]),
            relay_content
        ]);
        let relay_event_id = hex::encode(Sha256::digest(relay_canonical.to_string().as_bytes()));
        let relay_event_id_bytes = hex::decode(&relay_event_id).expect("relay event id hex");
        let relay_event_id_bytes =
            <[u8; 32]>::try_from(relay_event_id_bytes.as_slice()).expect("relay event id bytes");
        let relay_sig = hex::encode(
            relay_signer
                .sign(&relay_event_id_bytes)
                .expect("relay event should sign"),
        );
        let event_message = serde_json::json!([
            "EVENT",
            "veil-bridge",
            {
                "id": relay_event_id,
                "pubkey": relay_pubkey,
                "kind": 1,
                "created_at": relay_created_at,
                "tags": [],
                "content": relay_content,
                "sig": relay_sig
            }
        ])
        .to_string();

        let relay_thread = thread::spawn(move || {
            let (stream, _) = relay_listener.accept().expect("accept relay connection");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("set read timeout");
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .expect("set write timeout");
            let mut ws = accept(stream).expect("ws handshake");
            let req = ws.read().expect("read nostr req");
            let req_text = match req {
                Message::Text(text) => text,
                other => panic!("expected text req, got {other:?}"),
            };
            assert!(req_text.contains("\"REQ\""), "expected nostr REQ frame");
            ws.send(Message::Text(event_message))
                .expect("write nostr event");
        });

        let mut bridge_rx =
            crate::nostr_bridge::start_nostr_bridge(crate::nostr_bridge::NostrBridgeConfig {
                relays: vec![relay_url.clone()],
                channel_id: "nostr-bridge".to_string(),
                namespace: 32,
                since: Duration::from_secs(600),
                state_path: None,
                max_seen_ids: 128,
                persist_every_updates: 1,
            });

        let bridged = timeout(Duration::from_secs(10), bridge_rx.recv())
            .await
            .expect("bridge recv timeout")
            .expect("bridge should emit item");
        assert_eq!(bridged.source_relay, relay_url);

        relay_thread.join().expect("relay thread join");

        let signer = NostrSigner::from_secret([0x33; 32]).expect("valid signer");
        let publisher_pubkey = signer.public_key();
        let decrypt_key = [0x42; 32];
        let namespace = Namespace(32);
        let tag = derive_channel_feed_tag(&publisher_pubkey, namespace, "nostr-bridge");
        let cfg = NodeRuntimeConfig::builder()
            .base_fast_fanout(1)
            .base_fallback_fanout(1)
            .fallback_redundancy_fanout(1)
            .build();

        let mut sender_state = veil_node::state::NodeState::default();
        let mut sender_fast = InMemoryAdapter::default();
        let mut sender_fallback = InMemoryAdapter::default();
        let mut sender_batcher = FeedBatcher::default();
        let mut receiver_state = veil_node::state::NodeState::default();
        receiver_state.subscriptions.insert(tag);
        let mut receiver_fast = InMemoryAdapter::default();
        let mut receiver_fallback = InMemoryAdapter::default();
        let mut receiver_stats = RuntimeStats::default();
        let peers = vec!["receiver".to_string()];

        sender_batcher.enqueue(bridged.payload.clone());
        let _ = publish_queue_tick_multi_lane(
            &mut sender_state,
            &mut sender_fast,
            &mut sender_fallback,
            &mut sender_batcher,
            PublishQueueTickParams {
                namespace,
                epoch: Epoch(1),
                tag,
                encrypt_key: &decrypt_key,
                now_step: 1,
                flags: OBJECT_FLAG_SIGNED,
                interactive_flush: false,
                fast_peers: &peers,
                fallback_peers: &peers,
            },
            &cfg,
            &XChaCha20Poly1305Cipher,
            Some(&signer),
        );

        route_in_memory_outbound(&mut sender_fast, &mut receiver_fast, "vps");
        route_in_memory_outbound(&mut sender_fallback, &mut receiver_fallback, "vps");

        let mut delivered_payload = None;
        for step in 1..=12 {
            let event = pump_multi_lane_tick_with_config(
                &mut receiver_state,
                &mut receiver_fast,
                &mut receiver_fallback,
                ConfigMultiLanePumpParams {
                    fast_peers: &peers,
                    fallback_peers: &peers,
                    now_step: step,
                    decrypt_key: &decrypt_key,
                    config: &cfg,
                    stats: &mut receiver_stats,
                },
                &XChaCha20Poly1305Cipher,
                &NostrVerifier,
            )
            .expect("pump ok");
            if let Some(veil_node::receive::ReceiveEvent::Delivered {
                payload,
                tag: delivered_tag,
                ..
            }) = event
            {
                if delivered_tag == tag {
                    delivered_payload = Some(payload);
                    break;
                }
            }
        }
        let delivered_payload = delivered_payload.expect("expected delivered bridged payload");

        let android_state = AndroidNodeState::new("0.1-test");
        android_state.emit_payload(&[0xAB; 32], &delivered_payload, 32, 1, &tag, 0);
        let (events, _) = android_state.subscribe_events_since(Some(0));

        assert!(
            events.iter().any(|event| event.event == "payload"),
            "android node should emit payload event"
        );
        let feed_event = events
            .iter()
            .find(|event| event.event == "feed_bundle")
            .expect("android node should emit feed_bundle event");
        assert!(
            feed_event.data.to_string().contains("bridge e2e hello"),
            "feed bundle should carry bridged nostr text"
        );
    }

    #[test]
    fn test_cli_parsing() {
        use clap::Parser;

        // Test 'run' (implicit)
        let cli = Cli::try_parse_from(["veil-vps-node"]).unwrap();
        assert!(cli.command.is_none());

        // Test 'run' (explicit)
        let cli = Cli::try_parse_from(["veil-vps-node", "run"]).unwrap();
        match cli.command {
            Some(Commands::Run) => {}
            _ => panic!("expected Run command"),
        }

        // Test 'settings'
        let cli = Cli::try_parse_from(["veil-vps-node", "settings", "list"]).unwrap();
        match cli.command {
            Some(Commands::Settings {
                action: SettingsCommands::List,
                ..
            }) => {}
            _ => panic!("expected Settings List command"),
        }

        // Test 'settings' with custom DB
        let cli = Cli::try_parse_from([
            "veil-vps-node",
            "settings",
            "--db",
            "custom.db",
            "get",
            "key",
        ])
        .unwrap();
        match cli.command {
            Some(Commands::Settings {
                ref db,
                action: SettingsCommands::Get { ref key },
            }) => {
                assert_eq!(db, &std::path::PathBuf::from("custom.db"));
                assert_eq!(key, "key");
            }
            _ => panic!("expected Settings Get command"),
        }
    }
}
