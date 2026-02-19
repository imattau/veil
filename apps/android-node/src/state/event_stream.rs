use tokio::sync::broadcast;

use crate::api::EventEnvelope;

use super::projections::status_from_inner;
use super::{emit_event_locked, StateInner};

pub(super) fn feed_snapshot(inner: &StateInner, limit: usize) -> Vec<EventEnvelope> {
    inner
        .event_buffer
        .iter()
        .rev()
        .take(limit)
        .cloned()
        .collect()
}

pub(super) fn subscriptions_snapshot(inner: &StateInner) -> Vec<String> {
    inner.subscriptions.iter().cloned().collect()
}

pub(super) fn subscribe_tag(inner: &mut StateInner, tag: &str) -> bool {
    inner.subscriptions.insert(tag.to_string())
}

pub(super) fn unsubscribe_tag(inner: &mut StateInner, tag: &str) -> bool {
    inner.subscriptions.remove(tag)
}

pub(super) fn subscribe_events_receiver(inner: &StateInner) -> broadcast::Receiver<EventEnvelope> {
    inner.events.subscribe()
}

pub(super) fn subscribe_events_since(
    inner: &StateInner,
    since: Option<u64>,
) -> (Vec<EventEnvelope>, broadcast::Receiver<EventEnvelope>) {
    let backlog = match since {
        Some(seq) => inner
            .event_buffer
            .iter()
            .filter(|event| event.seq > seq)
            .cloned()
            .collect(),
        None => Vec::new(),
    };
    (backlog, inner.events.subscribe())
}

pub(super) fn emit_status_event(inner: &mut StateInner) -> EventEnvelope {
    let status = status_from_inner(inner);
    emit_event_locked(
        inner,
        "node_status",
        serde_json::to_value(status).unwrap_or_default(),
    )
}
