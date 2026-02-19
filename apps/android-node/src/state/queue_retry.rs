use std::collections::{BTreeSet, HashMap};

use uuid::Uuid;

#[derive(Debug, Default)]
pub(super) struct RetrySchedule {
    next_attempt: HashMap<Uuid, u64>,
    deadlines: BTreeSet<(u64, u128)>,
}

impl RetrySchedule {
    pub(super) fn schedule(&mut self, id: Uuid, due_ms: u64) {
        let raw_id = id.as_u128();
        if let Some(previous_due) = self.next_attempt.insert(id, due_ms) {
            self.deadlines.remove(&(previous_due, raw_id));
        }
        self.deadlines.insert((due_ms, raw_id));
    }

    pub(super) fn unschedule(&mut self, id: Uuid) {
        if let Some(previous_due) = self.next_attempt.remove(&id) {
            self.deadlines.remove(&(previous_due, id.as_u128()));
        }
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
        self.deadlines
            .iter()
            .next()
            .map(|(due_ms, _)| now_ms >= *due_ms)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::RetrySchedule;
    use uuid::Uuid;

    #[test]
    fn due_for_queue_uses_earliest_deadline() {
        let mut schedule = RetrySchedule::default();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        schedule.schedule(a, 200);
        schedule.schedule(b, 100);

        assert!(!schedule.due_for_queue(2, 99));
        assert!(schedule.due_for_queue(2, 100));
    }

    #[test]
    fn rescheduling_replaces_previous_deadline() {
        let mut schedule = RetrySchedule::default();
        let id = Uuid::new_v4();

        schedule.schedule(id, 100);
        schedule.schedule(id, 250);

        assert!(!schedule.due_for_queue(1, 149));
        assert!(!schedule.due_for_queue(1, 249));
        assert!(schedule.due_for_queue(1, 250));
    }

    #[test]
    fn unschedule_removes_deadline_and_marks_item_due() {
        let mut schedule = RetrySchedule::default();
        let id = Uuid::new_v4();

        schedule.schedule(id, 1_000);
        assert!(!schedule.is_due(id, 500));

        schedule.unschedule(id);
        assert!(schedule.is_due(id, 500));
    }
}
