use super::projections::update_queue_counts;
use super::{emit_event_locked, StateInner};
use crate::api::PublishRequest;
use crate::state_store::QueueItem;
use uuid::Uuid;

pub(super) fn enqueue_request(inner: &mut StateInner, request: PublishRequest) -> Uuid {
    let message_id = Uuid::new_v4();
    inner.queue.push_back(QueueItem {
        id: message_id,
        namespace: request.namespace,
        payload: request.payload,
    });
    update_queue_counts(inner);
    let pending = inner.queue_pending;
    emit_event_locked(
        inner,
        "publish_queued",
        serde_json::json!({
            "message_id": message_id,
            "pending": pending,
        }),
    );
    message_id
}

pub(super) fn take_due_item(inner: &mut StateInner, now_ms: u64) -> Option<QueueItem> {
    let queue_len = inner.queue.len();
    if queue_len == 0 || !inner.retry_schedule.due_for_queue(queue_len, now_ms) {
        return None;
    }
    let index = inner
        .queue
        .iter()
        .position(|item| inner.retry_schedule.is_due(item.id, now_ms))?;
    let item = inner.queue.remove(index)?;
    inner.retry_schedule.unschedule(item.id);
    inner.queue_inflight = inner.queue_inflight.saturating_add(1);
    let attempts = inner.queue_attempts.entry(item.id).or_insert(0);
    *attempts += 1;
    update_queue_counts(inner);
    Some(item)
}

pub(super) fn take_due_batch(
    inner: &mut StateInner,
    now_ms: u64,
    max_items: usize,
    target_batch_bytes: usize,
    max_item_bytes: usize,
) -> Vec<QueueItem> {
    if max_items == 0 {
        return Vec::new();
    }
    let queue_len = inner.queue.len();
    if queue_len == 0 || !inner.retry_schedule.due_for_queue(queue_len, now_ms) {
        return Vec::new();
    }
    let first_index = match inner
        .queue
        .iter()
        .position(|item| inner.retry_schedule.is_due(item.id, now_ms))
    {
        Some(index) => index,
        None => return Vec::new(),
    };

    let mut batch = Vec::with_capacity(max_items);
    let first = match inner.queue.remove(first_index) {
        Some(item) => item,
        None => return Vec::new(),
    };
    inner.retry_schedule.unschedule(first.id);
    let namespace = first.namespace;
    let mut total_bytes = first.payload.len();
    inner.queue_inflight = inner.queue_inflight.saturating_add(1);
    let attempts = inner.queue_attempts.entry(first.id).or_insert(0);
    *attempts += 1;
    batch.push(first);

    // Only aggregate additional small items. Large payloads still flow as
    // single-item publishes to avoid starving queue order.
    if total_bytes <= max_item_bytes && target_batch_bytes > total_bytes {
        let mut index = 0usize;
        while index < inner.queue.len()
            && batch.len() < max_items
            && total_bytes < target_batch_bytes
        {
            let due = inner.retry_schedule.is_due(inner.queue[index].id, now_ms);
            if !due {
                index += 1;
                continue;
            }
            let candidate = &inner.queue[index];
            if candidate.namespace != namespace {
                index += 1;
                continue;
            }
            let item_len = candidate.payload.len();
            if item_len > max_item_bytes || total_bytes + item_len > target_batch_bytes {
                index += 1;
                continue;
            }
            let item = match inner.queue.remove(index) {
                Some(item) => item,
                None => continue,
            };
            inner.retry_schedule.unschedule(item.id);
            total_bytes += item.payload.len();
            inner.queue_inflight = inner.queue_inflight.saturating_add(1);
            let attempts = inner.queue_attempts.entry(item.id).or_insert(0);
            *attempts += 1;
            batch.push(item);
        }
    }

    update_queue_counts(inner);
    batch
}

pub(super) fn mark_success(inner: &mut StateInner, item: &QueueItem) {
    inner.queue_inflight = inner.queue_inflight.saturating_sub(1);
    inner.queue_attempts.remove(&item.id);
    inner.retry_schedule.unschedule(item.id);
    emit_event_locked(
        inner,
        "publish_sent",
        serde_json::json!({ "message_id": item.id }),
    );
    update_queue_counts(inner);
}

pub(super) fn mark_failure(
    inner: &mut StateInner,
    item: QueueItem,
    retry_after_ms: u64,
    now_ms: u64,
) {
    inner.queue_failed = inner.queue_failed.saturating_add(1);
    inner.queue_inflight = inner.queue_inflight.saturating_sub(1);
    let message_id = item.id;
    let attempts = inner.queue_attempts.get(&message_id).copied().unwrap_or(0);
    let next_attempt = now_ms.saturating_add(retry_after_ms);
    inner.retry_schedule.schedule(message_id, next_attempt);
    inner.queue.push_back(item);
    emit_event_locked(
        inner,
        "publish_failed",
        serde_json::json!({
            "message_id": message_id,
            "attempts": attempts,
            "retry_after_ms": retry_after_ms,
        }),
    );
    update_queue_counts(inner);
}

pub(super) fn mark_dropped(inner: &mut StateInner, item: &QueueItem) {
    inner.queue_inflight = inner.queue_inflight.saturating_sub(1);
    inner.queue_attempts.remove(&item.id);
    inner.retry_schedule.unschedule(item.id);
    emit_event_locked(
        inner,
        "publish_failed",
        serde_json::json!({
            "message_id": item.id,
            "dropped": true,
        }),
    );
    update_queue_counts(inner);
}

pub(super) fn attempts_for(inner: &StateInner, item: &QueueItem) -> u32 {
    inner.queue_attempts.get(&item.id).copied().unwrap_or(0)
}
