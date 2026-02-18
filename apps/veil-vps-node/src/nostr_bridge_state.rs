use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub(crate) const NOSTR_EVENT_MAX_FUTURE_SKEW_SECS: u64 = 15 * 60;

pub(crate) fn current_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct NostrBridgeStateDisk {
    relay_last_created_at: HashMap<String, u64>,
    seen_event_ids: Vec<String>,
}

#[derive(Debug)]
pub(super) struct NostrBridgeState {
    pub(super) relay_last_created_at: HashMap<String, u64>,
    pub(super) seen_set: HashSet<String>,
    pub(super) seen_order: VecDeque<String>,
    state_path: Option<PathBuf>,
    max_seen_ids: usize,
    persist_every_updates: usize,
    dirty_updates: usize,
}

impl NostrBridgeState {
    pub(super) fn load(
        state_path: Option<PathBuf>,
        max_seen_ids: usize,
        persist_every_updates: usize,
    ) -> Self {
        let mut out = Self {
            relay_last_created_at: HashMap::new(),
            seen_set: HashSet::new(),
            seen_order: VecDeque::new(),
            state_path,
            max_seen_ids: max_seen_ids.max(1),
            persist_every_updates: persist_every_updates.max(1),
            dirty_updates: 0,
        };
        let Some(path) = out.state_path.clone() else {
            return out;
        };
        let Ok(bytes) = fs::read(path) else {
            return out;
        };
        let Ok(disk) = serde_json::from_slice::<NostrBridgeStateDisk>(&bytes) else {
            return out;
        };
        out.relay_last_created_at = disk.relay_last_created_at;
        for id in disk.seen_event_ids {
            if out.seen_set.insert(id.clone()) {
                out.seen_order.push_back(id);
            }
        }
        while out.seen_order.len() > out.max_seen_ids {
            if let Some(old) = out.seen_order.pop_front() {
                out.seen_set.remove(&old);
            }
        }
        out
    }

    pub(super) fn since_for_relay(&self, relay: &str, fallback_since_secs: u64) -> u64 {
        let baseline = current_unix().saturating_sub(fallback_since_secs);
        let checkpoint = self.relay_last_created_at.get(relay).copied().unwrap_or(0);
        baseline.max(checkpoint.saturating_sub(60))
    }

    pub(super) fn should_accept_event(
        &mut self,
        relay: &str,
        event_id: &str,
        created_at: u64,
    ) -> bool {
        // Protect per-relay checkpointing from future-timestamp poisoning that can
        // otherwise suppress valid events after reconnect.
        let max_allowed = current_unix().saturating_add(NOSTR_EVENT_MAX_FUTURE_SKEW_SECS);
        if created_at > max_allowed {
            return false;
        }
        if let Some(last) = self.relay_last_created_at.get_mut(relay) {
            *last = (*last).max(created_at);
        } else {
            self.relay_last_created_at
                .insert(relay.to_string(), created_at);
        }
        if self.seen_set.contains(event_id) {
            return false;
        }
        self.seen_set.insert(event_id.to_string());
        self.seen_order.push_back(event_id.to_string());
        while self.seen_order.len() > self.max_seen_ids {
            if let Some(old) = self.seen_order.pop_front() {
                self.seen_set.remove(&old);
            }
        }
        self.dirty_updates = self.dirty_updates.saturating_add(1);
        true
    }

    pub(super) fn persist_if_due(&mut self, force: bool) {
        if !force && self.dirty_updates < self.persist_every_updates {
            return;
        }
        let Some(path) = self.state_path.clone() else {
            self.dirty_updates = 0;
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let disk = NostrBridgeStateDisk {
            relay_last_created_at: self.relay_last_created_at.clone(),
            seen_event_ids: self.seen_order.iter().cloned().collect(),
        };
        let Ok(encoded) = serde_json::to_vec_pretty(&disk) else {
            return;
        };
        let tmp = path.with_extension("tmp");
        if fs::write(&tmp, encoded).is_ok() && fs::rename(&tmp, &path).is_ok() {
            self.dirty_updates = 0;
        }
    }
}
