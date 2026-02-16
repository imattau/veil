use std::collections::HashSet;

use crate::api::ContactBundle;

pub(super) const DISCOVERY_MAX_HTTP_TARGETS: usize = 64;
pub(super) const DISCOVERY_MAX_CONCURRENT_HTTP_GOSSIP: usize = 8;

pub(super) fn collect_http_targets(
    bootstrap_urls: &[String],
    contacts: &[ContactBundle],
) -> Vec<String> {
    let mut targets = Vec::new();
    let mut seen = HashSet::new();

    let mut push_target = |candidate: &str| {
        if targets.len() >= DISCOVERY_MAX_HTTP_TARGETS {
            return;
        }
        if !(candidate.starts_with("http://") || candidate.starts_with("https://")) {
            return;
        }
        if seen.insert(candidate.to_string()) {
            targets.push(candidate.to_string());
        }
    };

    // Keep bootstrap endpoints sticky at the front so known seeds remain reachable
    // even when many contact-provided RPC URLs are present.
    for target in bootstrap_urls {
        push_target(target);
    }
    for contact in contacts {
        if let Some(rpc_url) = &contact.rpc_url {
            push_target(rpc_url);
        }
    }
    targets
}

pub(super) fn bounded_http_gossip_concurrency(targets: usize) -> usize {
    if targets == 0 {
        0
    } else {
        targets.min(DISCOVERY_MAX_CONCURRENT_HTTP_GOSSIP)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        bounded_http_gossip_concurrency, collect_http_targets,
        DISCOVERY_MAX_CONCURRENT_HTTP_GOSSIP, DISCOVERY_MAX_HTTP_TARGETS,
    };
    use crate::api::ContactBundle;

    #[test]
    fn collect_http_targets_filters_dedups_and_caps() {
        let bootstrap = vec![
            "wss://example.invalid/ws".to_string(),
            "https://seed-a.example".to_string(),
            "https://seed-a.example".to_string(),
            "http://seed-b.example".to_string(),
        ];
        let mut contacts = Vec::new();
        for i in 0..(DISCOVERY_MAX_HTTP_TARGETS + 20) {
            contacts.push(ContactBundle {
                peer_id: format!("peer-{i}"),
                ws_url: None,
                quic_addr: None,
                pubkey_hex: format!("{:064x}", i + 1),
                rpc_url: Some(format!("http://peer-{i}.example")),
                lan_addrs: Vec::new(),
            });
        }
        contacts.push(ContactBundle {
            peer_id: "peer-extra".to_string(),
            ws_url: None,
            quic_addr: None,
            pubkey_hex: "11".repeat(32),
            rpc_url: Some("quic://not-http".to_string()),
            lan_addrs: Vec::new(),
        });

        let targets = collect_http_targets(&bootstrap, &contacts);
        assert!(targets.len() <= DISCOVERY_MAX_HTTP_TARGETS);
        assert!(targets
            .iter()
            .all(|url| url.starts_with("http://") || url.starts_with("https://")));
        let unique: std::collections::HashSet<_> = targets.iter().collect();
        assert_eq!(unique.len(), targets.len());
        assert_eq!(
            targets.first().map(String::as_str),
            Some("https://seed-a.example")
        );
        assert!(targets.iter().any(|url| url == "http://seed-b.example"));
        assert!(targets.iter().any(|url| url == "http://peer-0.example"));
    }

    #[test]
    fn bounded_http_gossip_concurrency_caps_parallelism() {
        assert_eq!(bounded_http_gossip_concurrency(0), 0);
        assert_eq!(bounded_http_gossip_concurrency(1), 1);
        assert_eq!(
            bounded_http_gossip_concurrency(DISCOVERY_MAX_CONCURRENT_HTTP_GOSSIP),
            DISCOVERY_MAX_CONCURRENT_HTTP_GOSSIP
        );
        assert_eq!(
            bounded_http_gossip_concurrency(DISCOVERY_MAX_CONCURRENT_HTTP_GOSSIP + 50),
            DISCOVERY_MAX_CONCURRENT_HTTP_GOSSIP
        );
    }
}
