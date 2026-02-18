use std::sync::atomic::AtomicU64;

#[derive(Debug, Default)]
pub struct MetricsState {
    pub ticks: AtomicU64,
    pub delivered: AtomicU64,
    pub delivered_total: AtomicU64,
    pub send_failures: AtomicU64,
    pub ack_clears: AtomicU64,
    pub last_fast_outbound_ok: AtomicU64,
    pub last_fast_outbound_err: AtomicU64,
    pub last_fallback_outbound_ok: AtomicU64,
    pub last_fallback_outbound_err: AtomicU64,
    pub last_fast_inbound: AtomicU64,
    pub last_fallback_inbound: AtomicU64,
    pub nostr_bridge_events_total: AtomicU64,
    pub nostr_bridge_payload_bytes_total: AtomicU64,
    pub nostr_bridge_enabled: AtomicU64,
    pub nostr_bridge_relays_configured: AtomicU64,
}
