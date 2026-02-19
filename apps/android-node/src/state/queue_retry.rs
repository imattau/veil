use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use uuid::Uuid;

#[derive(Debug, Default)]
pub(super) struct RetrySchedule {
    next_attempt: HashMap<Uuid, u64>,
    deadlines: BinaryHeap<Reverse<(u64, u128)>>,
}

impl RetrySchedule {
    pub(super) fn schedule(&mut self, id: Uuid, due_ms: u64) {
        self.next_attempt.insert(id, due_ms);
        self.deadlines.push(Reverse((due_ms, id.as_u128())));
    }

    pub(super) fn unschedule(&mut self, id: Uuid) {
        self.next_attempt.remove(&id);
    }

    pub(super) fn is_due(&self, id: Uuid, now_ms: u64) -> bool {
        self.next_attempt
            .get(&id)
            .copied()
            .map(|next| now_ms >= next)
            .unwrap_or(true)
    }

    pub(super) fn due_for_queue(&mut self, queue_len: usize, now_ms: u64) -> bool {
        if self.next_attempt.len() < queue_len {
            return true;
        }
        self.prune();
        self.deadlines
            .peek()
            .map(|Reverse((due_ms, _))| now_ms >= *due_ms)
            .unwrap_or(false)
    }

    fn prune(&mut self) {
        while let Some(Reverse((due_ms, id_raw))) = self.deadlines.peek().copied() {
            let id = Uuid::from_u128(id_raw);
            match self.next_attempt.get(&id).copied() {
                Some(current_due) if current_due == due_ms => break,
                _ => {
                    self.deadlines.pop();
                }
            }
        }
    }
}
