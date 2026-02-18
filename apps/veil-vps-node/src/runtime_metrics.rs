use std::sync::atomic::Ordering;

use veil_transport::adapter::TransportHealthSnapshot;

use crate::metrics_state::MetricsState;

pub(super) fn set_nostr_bridge_relays_configured(metrics: &MetricsState, relay_count: usize) {
    metrics
        .nostr_bridge_relays_configured
        .store(relay_count as u64, Ordering::Relaxed);
}

pub(super) fn note_send_failures(metrics: &MetricsState, count: usize) {
    metrics
        .send_failures
        .fetch_add(count as u64, Ordering::Relaxed);
}

pub(super) fn note_ack_clears(metrics: &MetricsState, count: usize) {
    metrics
        .ack_clears
        .fetch_add(count as u64, Ordering::Relaxed);
}

pub(super) fn note_tick(metrics: &MetricsState) {
    metrics.ticks.fetch_add(1, Ordering::Relaxed);
}

pub(super) fn apply_transport_health(
    metrics: &MetricsState,
    fast: &TransportHealthSnapshot,
    fallback: &TransportHealthSnapshot,
) {
    metrics
        .last_fast_outbound_ok
        .store(fast.outbound_send_ok, Ordering::Relaxed);
    metrics
        .last_fast_outbound_err
        .store(fast.outbound_send_err, Ordering::Relaxed);
    metrics
        .last_fallback_outbound_ok
        .store(fallback.outbound_send_ok, Ordering::Relaxed);
    metrics
        .last_fallback_outbound_err
        .store(fallback.outbound_send_err, Ordering::Relaxed);
    metrics
        .last_fast_inbound
        .store(fast.inbound_received, Ordering::Relaxed);
    metrics
        .last_fallback_inbound
        .store(fallback.inbound_received, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use veil_transport::adapter::TransportHealthSnapshot;

    use crate::metrics_state::MetricsState;

    use super::{apply_transport_health, note_ack_clears, note_send_failures, note_tick};

    #[test]
    fn helpers_update_counters_and_health_fields() {
        let metrics = MetricsState::default();

        note_send_failures(&metrics, 3);
        note_ack_clears(&metrics, 2);
        note_tick(&metrics);

        let fast = TransportHealthSnapshot {
            outbound_send_ok: 11,
            outbound_send_err: 1,
            inbound_received: 7,
            ..TransportHealthSnapshot::default()
        };
        let fallback = TransportHealthSnapshot {
            outbound_send_ok: 4,
            outbound_send_err: 2,
            inbound_received: 3,
            ..TransportHealthSnapshot::default()
        };
        apply_transport_health(&metrics, &fast, &fallback);

        assert_eq!(metrics.send_failures.load(Ordering::Relaxed), 3);
        assert_eq!(metrics.ack_clears.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.ticks.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.last_fast_outbound_ok.load(Ordering::Relaxed), 11);
        assert_eq!(metrics.last_fast_outbound_err.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.last_fallback_outbound_ok.load(Ordering::Relaxed), 4);
        assert_eq!(
            metrics.last_fallback_outbound_err.load(Ordering::Relaxed),
            2
        );
        assert_eq!(metrics.last_fast_inbound.load(Ordering::Relaxed), 7);
        assert_eq!(metrics.last_fallback_inbound.load(Ordering::Relaxed), 3);
    }
}
