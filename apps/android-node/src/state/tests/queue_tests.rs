use super::*;

#[test]
fn queue_retries_with_backoff() {
    let state = NodeState::new("0.1-test");
    let _ = state.enqueue_publish(PublishRequest {
        namespace: 32,
        payload: "hello".to_string(),
    });
    let item = state.take_next_queued(now_millis()).expect("item");
    let status = state.status();
    assert_eq!(status.queue.pending, 0);
    assert_eq!(status.queue.inflight, 1);

    state.complete_failure(item, 0);
    let status = state.status();
    assert_eq!(status.queue.pending, 1);
    assert_eq!(status.queue.inflight, 0);
    assert_eq!(status.queue.failed, 1);

    let item = state.take_next_queued(now_millis()).expect("item");
    state.complete_success(&item);
    let status = state.status();
    assert_eq!(status.queue.pending, 0);
    assert_eq!(status.queue.inflight, 0);
}

#[test]
fn queue_retry_deadline_blocks_until_due() {
    let state = NodeState::new("0.1-test");
    let _ = state.enqueue_publish(PublishRequest {
        namespace: 32,
        payload: "hello".to_string(),
    });
    let item = state.take_next_queued(now_millis()).expect("item");
    state.complete_failure(item, 60_000);

    assert!(state.take_next_queued(now_millis()).is_none());
    let future = now_millis().saturating_add(61_000);
    assert!(state.take_next_queued(future).is_some());
}

#[test]
fn take_next_queued_batch_groups_small_same_namespace_items() {
    let state = NodeState::new("0.1-test");
    let _ = state.enqueue_publish(PublishRequest {
        namespace: 32,
        payload: "a".repeat(32),
    });
    let _ = state.enqueue_publish(PublishRequest {
        namespace: 32,
        payload: "b".repeat(24),
    });
    // Different namespace should not be coalesced into the same batch.
    let _ = state.enqueue_publish(PublishRequest {
        namespace: 64,
        payload: "c".repeat(24),
    });

    let batch = state.take_next_queued_batch(now_millis(), 16, 96 * 1024, 4 * 1024);
    assert_eq!(batch.len(), 2);
    assert!(batch.iter().all(|item| item.namespace == 32));

    for item in &batch {
        state.complete_success(item);
    }

    let next = state
        .take_next_queued(now_millis())
        .expect("remaining item");
    assert_eq!(next.namespace, 64);
}

#[test]
fn take_next_queued_batch_leaves_large_payload_as_single_item() {
    let state = NodeState::new("0.1-test");
    let _ = state.enqueue_publish(PublishRequest {
        namespace: 32,
        payload: "x".repeat(8 * 1024),
    });
    let _ = state.enqueue_publish(PublishRequest {
        namespace: 32,
        payload: "y".repeat(32),
    });

    let batch = state.take_next_queued_batch(now_millis(), 16, 96 * 1024, 4 * 1024);
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].payload.len(), 8 * 1024);

    state.complete_success(&batch[0]);
    let next = state
        .take_next_queued(now_millis())
        .expect("small item remains");
    assert_eq!(next.payload.len(), 32);
}
