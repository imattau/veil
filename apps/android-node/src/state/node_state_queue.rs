use super::{
    now_millis, queue_ops, snapshot_from_inner, NodeState, PublishRequest, QueueItem, Uuid,
};

impl NodeState {
    pub fn enqueue_publish(&self, request: PublishRequest) -> Uuid {
        let mut inner = self.inner.lock().expect("state lock");
        let message_id = queue_ops::enqueue_request(&mut inner, request);
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(&inner));
        }
        message_id
    }

    pub fn take_next_queued(&self, now_ms: u64) -> Option<QueueItem> {
        let mut inner = self.inner.lock().expect("state lock");
        queue_ops::take_due_item(&mut inner, now_ms)
    }

    pub fn take_next_queued_batch(
        &self,
        now_ms: u64,
        max_items: usize,
        target_batch_bytes: usize,
        max_item_bytes: usize,
    ) -> Vec<QueueItem> {
        let mut inner = self.inner.lock().expect("state lock");
        queue_ops::take_due_batch(
            &mut inner,
            now_ms,
            max_items,
            target_batch_bytes,
            max_item_bytes,
        )
    }

    pub fn complete_success(&self, item: &QueueItem) {
        let mut inner = self.inner.lock().expect("state lock");
        queue_ops::mark_success(&mut inner, item);
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(&inner));
        }
    }

    pub fn complete_failure(&self, item: QueueItem, retry_after_ms: u64) {
        let mut inner = self.inner.lock().expect("state lock");
        queue_ops::mark_failure(&mut inner, item, retry_after_ms, now_millis());
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(&inner));
        }
    }

    pub fn drop_item(&self, item: &QueueItem) {
        let mut inner = self.inner.lock().expect("state lock");
        queue_ops::mark_dropped(&mut inner, item);
        if let Some(store) = &inner.store {
            store.persist(&snapshot_from_inner(&inner));
        }
    }

    pub fn attempts_for(&self, item: &QueueItem) -> u32 {
        let inner = self.inner.lock().expect("state lock");
        queue_ops::attempts_for(&inner, item)
    }
}
