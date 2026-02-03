use std::collections::VecDeque;

/// Default target batch size for practical mode.
pub const DEFAULT_TARGET_BATCH_SIZE: usize = 96 * 1024;
/// Default maximum object payload size for practical mode.
pub const DEFAULT_MAX_OBJECT_SIZE: usize = 256 * 1024;

/// Size limits used by feed-item batching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchLimits {
    pub target_batch_size: usize,
    pub max_object_size: usize,
}

impl Default for BatchLimits {
    fn default() -> Self {
        Self {
            target_batch_size: DEFAULT_TARGET_BATCH_SIZE,
            max_object_size: DEFAULT_MAX_OBJECT_SIZE,
        }
    }
}

/// Stateful queue that drains feed-item batches for publish pipelines.
#[derive(Debug, Default)]
pub struct FeedBatcher {
    queue: VecDeque<Vec<u8>>,
    pub limits: BatchLimits,
}

impl FeedBatcher {
    /// Creates a new batcher with explicit limits.
    pub fn with_limits(limits: BatchLimits) -> Self {
        Self {
            queue: VecDeque::new(),
            limits,
        }
    }

    /// Enqueues a feed item payload.
    pub fn enqueue(&mut self, item: Vec<u8>) {
        self.queue.push_back(item);
    }

    /// Number of queued items.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Returns true when no items are queued.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Drains the next batch with practical-mode limits.
    pub fn drain_next_batch(&mut self) -> Vec<Vec<u8>> {
        batch_feed_items(&mut self.queue, self.limits, false)
    }

    /// Drains one item for interactive publish mode.
    pub fn drain_interactive(&mut self) -> Vec<Vec<u8>> {
        batch_feed_items(&mut self.queue, self.limits, true)
    }
}

/// Batches queued feed items into one object payload set.
///
/// If `interactive_flush` is true, one queued item is flushed immediately.
pub fn batch_feed_items(
    queue: &mut VecDeque<Vec<u8>>,
    limits: BatchLimits,
    interactive_flush: bool,
) -> Vec<Vec<u8>> {
    if interactive_flush {
        return queue.pop_front().map_or_else(Vec::new, |item| vec![item]);
    }

    let mut buf = Vec::new();
    let mut size = 0usize;

    while let Some(item) = queue.front() {
        let next_len = item.len();
        if size + next_len > limits.max_object_size && !buf.is_empty() {
            break;
        }

        let item = queue
            .pop_front()
            .expect("front item existed and should pop successfully");
        size += item.len();
        buf.push(item);

        if size >= limits.target_batch_size {
            break;
        }
    }

    buf
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::{batch_feed_items, BatchLimits, FeedBatcher};

    #[test]
    fn batch_stops_at_target_size() {
        let limits = BatchLimits {
            target_batch_size: 100,
            max_object_size: 500,
        };
        let mut queue = VecDeque::from(vec![vec![0_u8; 40], vec![1_u8; 40], vec![2_u8; 40]]);

        let batch = batch_feed_items(&mut queue, limits, false);
        assert_eq!(batch.len(), 3);
        assert!(queue.is_empty());
    }

    #[test]
    fn batch_respects_max_size_once_non_empty() {
        let limits = BatchLimits {
            target_batch_size: 1_000,
            max_object_size: 100,
        };
        let mut queue = VecDeque::from(vec![vec![0_u8; 60], vec![1_u8; 60], vec![2_u8; 20]]);

        let batch = batch_feed_items(&mut queue, limits, false);
        assert_eq!(batch.len(), 1);
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn interactive_flush_returns_single_item() {
        let limits = BatchLimits::default();
        let mut queue = VecDeque::from(vec![vec![0_u8; 10], vec![1_u8; 20]]);

        let batch = batch_feed_items(&mut queue, limits, true);
        assert_eq!(batch.len(), 1);
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn feed_batcher_drains_and_tracks_queue_len() {
        let mut batcher = FeedBatcher::with_limits(BatchLimits {
            target_batch_size: 64,
            max_object_size: 128,
        });
        batcher.enqueue(vec![0_u8; 32]);
        batcher.enqueue(vec![1_u8; 40]);
        batcher.enqueue(vec![2_u8; 50]);
        assert_eq!(batcher.len(), 3);

        let first = batcher.drain_next_batch();
        assert_eq!(first.len(), 2);
        assert_eq!(batcher.len(), 1);

        let second = batcher.drain_interactive();
        assert_eq!(second.len(), 1);
        assert!(batcher.is_empty());
    }
}
