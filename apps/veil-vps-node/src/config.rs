use config::{Config, ConfigError, Environment, File};
use serde::de::{self, Deserializer, Visitor};
use serde::Deserialize;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Deserialize, Clone)]
pub struct VpsConfig {
    pub quic_alpn: String,
    pub state_path: PathBuf,
    pub node_key_path: PathBuf,
    pub node_key: Option<String>,
    pub quic_cert_path: PathBuf,
    pub quic_key_path: PathBuf,
    #[serde(with = "humantime_serde")]
    pub snapshot_interval: Duration,
    #[serde(with = "humantime_serde")]
    pub tick_interval: Duration,
    pub health_bind: String,
    pub health_port: u16,
    pub admin_session_db_path: PathBuf,
    pub peer_db_path: PathBuf,
    pub max_dynamic_peers: usize,
    pub quic_bind: String,
    pub ws_url: Option<String>,
    pub ws_listen: Option<String>,
    pub ws_peer: Option<String>,
    pub tor_socks_addr: Option<String>,
    #[serde(deserialize_with = "deserialize_list")]
    pub fast_peers: Vec<String>,
    #[serde(deserialize_with = "deserialize_list")]
    pub core_tags: Vec<String>,
    #[serde(deserialize_with = "deserialize_list")]
    pub tor_peers: Vec<String>,
    pub ble_enabled: bool,
    #[serde(deserialize_with = "deserialize_list")]
    pub ble_peers: Vec<String>,
    #[serde(deserialize_with = "deserialize_list")]
    pub ble_allowlist: Vec<String>,
    pub ble_mtu: usize,
    pub adaptive_lane_scoring: bool,
    pub probabilistic_forwarding: bool,
    pub forwarding_min_probability: f64,
    pub forwarding_replica_divisor: u64,
    pub bloom_exchange: bool,
    pub bloom_interval_steps: u64,
    pub bloom_false_positive_rate: f64,
    pub max_cache_shards: usize,
    pub bucket_jitter: usize,
    pub open_relay: bool,
    #[serde(deserialize_with = "deserialize_list")]
    pub blocked_peers: Vec<String>,
    pub nostr_bridge_enabled: bool,
    #[serde(deserialize_with = "deserialize_list")]
    pub nostr_bridge_relays: Vec<String>,
    pub nostr_bridge_channel_id: String,
    pub nostr_bridge_namespace: u64,
    #[serde(with = "humantime_serde")]
    pub nostr_bridge_since: Duration,
    pub nostr_bridge_state_path: PathBuf,
    pub nostr_bridge_max_seen_ids: usize,
    pub nostr_bridge_persist_every_updates: usize,
    #[serde(deserialize_with = "deserialize_list")]
    pub required_signed_namespaces: Vec<String>,
    #[serde(deserialize_with = "deserialize_list")]
    pub quic_trusted_certs: Vec<String>,
}

fn deserialize_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct ListVisitor;

    impl<'de> Visitor<'de> for ListVisitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or a sequence of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value
                .split(|c| c == ',' || c == ';')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect())
        }

        fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
        where
            S: de::SeqAccess<'de>,
        {
            let mut vec = Vec::new();
            while let Some(element) = seq.next_element()? {
                vec.push(element);
            }
            Ok(vec)
        }
    }

    deserializer.deserialize_any(ListVisitor)
}

impl VpsConfig {
    pub fn new(config_path: Option<PathBuf>) -> Result<Self, ConfigError> {
        let mut builder = Config::builder()
            .set_default("quic_alpn", "veil-quic/1,veil/1,veil-node,veil,h3,hq-29")?
            .set_default("state_path", "data/veil-vps-node-state.cbor")?
            .set_default("node_key_path", "data/node_identity.key")?
            .set_default("node_key", None::<String>)?
            .set_default("quic_cert_path", "data/quic_cert.der")?
            .set_default("quic_key_path", "data/quic_key.der")?
            .set_default("snapshot_interval", "60s")?
            .set_default("tick_interval", "50ms")?
            .set_default("health_bind", "0.0.0.0")?
            .set_default("health_port", 9090)?
            .set_default("admin_session_db_path", "data/admin-sessions.db")?
            .set_default("peer_db_path", "data/peers.db")?
            .set_default("max_dynamic_peers", 512)?
            .set_default("quic_bind", "0.0.0.0:5000")?
            .set_default("ws_peer", "ws-peer")?
            .set_default("adaptive_lane_scoring", true)?
            .set_default("probabilistic_forwarding", true)?
            .set_default("forwarding_min_probability", 0.10)?
            .set_default("forwarding_replica_divisor", 8)?
            .set_default("bloom_exchange", true)?
            .set_default("bloom_interval_steps", 128)?
            .set_default("bloom_false_positive_rate", 0.05)?
            .set_default("max_cache_shards", 200_000)?
            .set_default("bucket_jitter", 0)?
            .set_default("open_relay", false)?
            .set_default("nostr_bridge_enabled", false)?
            .set_default("nostr_bridge_channel_id", "nostr-bridge")?
            .set_default("nostr_bridge_namespace", 32)?
            .set_default("nostr_bridge_since", "1h")?
            .set_default("nostr_bridge_state_path", "data/nostr-bridge-state.json")?
            .set_default("nostr_bridge_max_seen_ids", 20_000)?
            .set_default("nostr_bridge_persist_every_updates", 32)?
            .set_default("ble_enabled", false)?
            .set_default("ble_mtu", 180)?
            .set_default("fast_peers", Vec::<String>::new())?
            .set_default("core_tags", Vec::<String>::new())?
            .set_default("tor_peers", Vec::<String>::new())?
            .set_default("ble_peers", Vec::<String>::new())?
            .set_default("ble_allowlist", Vec::<String>::new())?
            .set_default("blocked_peers", Vec::<String>::new())?
            .set_default("nostr_bridge_relays", Vec::<String>::new())?
            .set_default("required_signed_namespaces", Vec::<String>::new())?
            .set_default("quic_trusted_certs", Vec::<String>::new())?;

        if let Some(path) = config_path {
            if path.extension().and_then(|ext| ext.to_str()) == Some("env") {
                // For .env files, load them into the environment instead of using as a config source
                // This allows the Environment::with_prefix source to pick them up.
                match dotenvy::from_path(&path) {
                    Ok(_) => tracing::info!("loaded environment from {}", path.display()),
                    Err(err) => {
                        tracing::warn!("failed to load .env from {}: {}", path.display(), err)
                    }
                }
            } else {
                builder = builder.add_source(File::from(path));
            }
        }

        if let Ok(legacy) = std::env::var("VEIL_VPS_NOSTR_RELAYS") {
            if !legacy.trim().is_empty() {
                builder = builder.set_default("nostr_bridge_relays", legacy)?;
            }
        }

        builder = builder.add_source(Environment::with_prefix("VEIL_VPS").try_parsing(true));

        builder.build()?.try_deserialize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;
    use std::time::Duration;

    fn with_env<F>(vars: &[(&str, &str)], test: F)
    where
        F: FnOnce(),
    {
        let mut old = Vec::new();
        for (k, v) in vars {
            old.push((k.to_string(), env::var(k).ok()));
            env::set_var(k, v);
        }

        test();

        for (k, maybe_old) in old {
            match maybe_old {
                Some(val) => env::set_var(k, val),
                None => env::remove_var(k),
            }
        }
    }

    #[test]
    fn defaults_are_applied() {
        let cfg = VpsConfig::new(None).expect("failed to build config");

        assert_eq!(cfg.quic_alpn, "veil-quic/1,veil/1,veil-node,veil,h3,hq-29");
        assert_eq!(
            cfg.state_path,
            PathBuf::from("data/veil-vps-node-state.cbor")
        );
        assert_eq!(cfg.snapshot_interval, Duration::from_secs(60));
        assert_eq!(cfg.tick_interval, Duration::from_millis(50));
        assert_eq!(cfg.health_bind, "127.0.0.1");
        assert_eq!(cfg.health_port, 9090);
        assert_eq!(cfg.max_dynamic_peers, 512);
        assert_eq!(cfg.quic_bind, "0.0.0.0:5000");
        assert_eq!(cfg.ws_peer.as_deref(), Some("ws-peer"));
        assert!(cfg.adaptive_lane_scoring);
        assert!(cfg.probabilistic_forwarding);
        assert_eq!(cfg.forwarding_min_probability, 0.10);
        assert_eq!(cfg.forwarding_replica_divisor, 8);
        assert!(cfg.bloom_exchange);
        assert_eq!(cfg.bloom_interval_steps, 128);
        assert_eq!(cfg.bloom_false_positive_rate, 0.05);
        assert_eq!(cfg.max_cache_shards, 200_000);
        assert_eq!(cfg.bucket_jitter, 0);
        assert!(!cfg.open_relay);
        assert!(!cfg.nostr_bridge_enabled);
        assert_eq!(cfg.nostr_bridge_channel_id, "nostr-bridge");
        assert_eq!(cfg.nostr_bridge_namespace, 32);
        assert_eq!(cfg.nostr_bridge_since, Duration::from_secs(3600));
        assert_eq!(
            cfg.nostr_bridge_state_path,
            PathBuf::from("data/nostr-bridge-state.json")
        );
        assert_eq!(cfg.nostr_bridge_max_seen_ids, 20_000);
        assert_eq!(cfg.nostr_bridge_persist_every_updates, 32);
        assert!(!cfg.ble_enabled);
        assert_eq!(cfg.ble_mtu, 180);
    }

    #[test]
    fn env_vars_override_defaults() {
        with_env(
            &[
                ("VEIL_VPS_QUIC_ALPN", "custom-alpn"),
                ("VEIL_VPS_HEALTH_PORT", "1234"),
                ("VEIL_VPS_ADAPTIVE_LANE_SCORING", "false"),
                ("VEIL_VPS_BLOOM_FALSE_POSITIVE_RATE", "0.123"),
                ("VEIL_VPS_NOSTR_BRIDGE_ENABLED", "true"),
            ],
            || {
                let cfg = VpsConfig::new(None).expect("failed to build config");
                assert_eq!(cfg.quic_alpn, "custom-alpn");
                assert_eq!(cfg.health_port, 1234);
                assert!(!cfg.adaptive_lane_scoring);
                assert!((cfg.bloom_false_positive_rate - 0.123).abs() < f64::EPSILON);
                assert!(cfg.nostr_bridge_enabled);
            },
        );
    }

    #[test]
    fn human_readable_durations_are_parsed() {
        with_env(
            &[
                ("VEIL_VPS_SNAPSHOT_INTERVAL", "10s"),
                ("VEIL_VPS_TICK_INTERVAL", "5ms"),
                ("VEIL_VPS_NOSTR_BRIDGE_SINCE", "2h30m"),
            ],
            || {
                let cfg = VpsConfig::new(None).expect("failed to build config");
                assert_eq!(cfg.snapshot_interval, Duration::from_secs(10));
                assert_eq!(cfg.tick_interval, Duration::from_millis(5));
                assert_eq!(cfg.nostr_bridge_since, Duration::from_secs(9_000));
            },
        );
    }

    #[test]
    fn list_separator_parses_vec_fields() {
        with_env(
            &[
                ("VEIL_VPS_FAST_PEERS", "peer-a,peer-b,peer-c"),
                ("VEIL_VPS_CORE_TAGS", "tag1,tag2"),
                ("VEIL_VPS_BLOCKED_PEERS", "bad1,bad2"),
                (
                    "VEIL_VPS_NOSTR_BRIDGE_RELAYS",
                    "wss://relay1.example,wss://relay2.example",
                ),
            ],
            || {
                let cfg = VpsConfig::new(None).expect("failed to build config");

                assert_eq!(
                    cfg.fast_peers,
                    vec![
                        "peer-a".to_string(),
                        "peer-b".to_string(),
                        "peer-c".to_string()
                    ]
                );
                assert_eq!(cfg.core_tags, vec!["tag1".to_string(), "tag2".to_string()]);
                assert_eq!(
                    cfg.blocked_peers,
                    vec!["bad1".to_string(), "bad2".to_string()]
                );
                assert_eq!(
                    cfg.nostr_bridge_relays,
                    vec![
                        "wss://relay1.example".to_string(),
                        "wss://relay2.example".to_string()
                    ]
                );
            },
        );
    }

    #[test]
    fn file_overrides_take_precedence_over_defaults_and_env() {
        use std::io::Write;

        let mut tmp = tempfile::Builder::new()
            .suffix(".toml")
            .tempfile()
            .expect("temp file");
        writeln!(
            tmp,
            r#"
quic_alpn = "file-alpn"
health_port = 4242
snapshot_interval = "15s"
fast_peers = ["file-peer1","file-peer2"]
"#
        )
        .expect("write to temp file");

        with_env(&[("VEIL_VPS_HEALTH_PORT", "9999")], || {
            let cfg = VpsConfig::new(Some(PathBuf::from(tmp.path()))).expect("load config");
            assert_eq!(cfg.quic_alpn, "file-alpn");
            assert_eq!(cfg.health_port, 9999);
            assert_eq!(cfg.snapshot_interval, Duration::from_secs(15));
            assert_eq!(
                cfg.fast_peers,
                vec!["file-peer1".to_string(), "file-peer2".to_string()]
            );
        });
    }

    #[test]
    fn deserialize_list_logic_is_robust() {
        let cases = vec![
            ("a,b,c", vec!["a", "b", "c"]),
            ("a;b;c", vec!["a", "b", "c"]),
            ("a, b ; c ", vec!["a", "b", "c"]),
            (",a,,b;", vec!["a", "b"]),
            ("  ", Vec::<&str>::new()),
            ("", Vec::<&str>::new()),
        ];

        for (input, expected) in cases {
            let actual: Vec<String> = input
                .split(|c| c == ',' || c == ';')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let expected_vec: Vec<String> = expected.into_iter().map(|s| s.to_string()).collect();
            assert_eq!(actual, expected_vec, "failed on input: {}", input);
        }
    }
}
