pub(super) fn merge_unique_peers(configured: &[String], dynamic: &[String]) -> Vec<String> {
    let mut merged = configured.to_vec();
    for peer in dynamic {
        if !merged.contains(peer) {
            merged.push(peer.clone());
        }
    }
    merged
}

pub(super) fn finalize_publish_peer_lists(
    fast: Vec<String>,
    fallback: Vec<String>,
    ws_url: Option<&str>,
) -> Result<(Vec<String>, Vec<String>), String> {
    let (fast, fallback) = finalize_runtime_peer_lists(fast, fallback, ws_url);
    if fast.is_empty() && fallback.is_empty() {
        return Err("no peers configured for publish".to_string());
    }
    Ok((fast, fallback))
}

pub(super) fn finalize_runtime_peer_lists(
    fast: Vec<String>,
    mut fallback: Vec<String>,
    ws_url: Option<&str>,
) -> (Vec<String>, Vec<String>) {
    if fallback.is_empty() {
        if let Some(ws_url) = ws_url.map(str::trim).filter(|value| !value.is_empty()) {
            fallback.push(ws_url.to_string());
        }
    }
    (fast, fallback)
}

#[cfg(test)]
mod tests {
    use super::{finalize_publish_peer_lists, finalize_runtime_peer_lists, merge_unique_peers};

    #[test]
    fn merge_unique_peers_preserves_order_and_deduplicates() {
        let configured = vec!["peer-a".to_string(), "peer-b".to_string()];
        let dynamic = vec![
            "peer-b".to_string(),
            "peer-c".to_string(),
            "peer-a".to_string(),
            "peer-d".to_string(),
        ];

        let merged = merge_unique_peers(&configured, &dynamic);
        assert_eq!(
            merged,
            vec![
                "peer-a".to_string(),
                "peer-b".to_string(),
                "peer-c".to_string(),
                "peer-d".to_string(),
            ]
        );
    }

    #[test]
    fn finalize_publish_peer_lists_uses_trimmed_ws_when_fallback_empty() {
        let fast = vec!["quic://fast".to_string()];
        let fallback = Vec::new();

        let (fast_out, fallback_out) =
            finalize_publish_peer_lists(fast, fallback, Some(" ws://relay.example/ws "))
                .expect("peers");

        assert_eq!(fast_out, vec!["quic://fast".to_string()]);
        assert_eq!(fallback_out, vec!["ws://relay.example/ws".to_string()]);
    }

    #[test]
    fn finalize_publish_peer_lists_rejects_empty_sources() {
        let result = finalize_publish_peer_lists(Vec::new(), Vec::new(), Some("   "));
        assert_eq!(result, Err("no peers configured for publish".to_string()));
    }

    #[test]
    fn finalize_runtime_peer_lists_allows_empty_sources_for_inbound_polling() {
        let (fast, fallback) = finalize_runtime_peer_lists(Vec::new(), Vec::new(), Some("   "));
        assert!(fast.is_empty());
        assert!(fallback.is_empty());
    }
}
