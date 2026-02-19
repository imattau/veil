use super::{EventEnvelope, LaneDetail, NodeState};

impl NodeState {
    pub fn mark_lane_health(&self, lane: &str, connected: bool, last_error: Option<String>) {
        let mut inner = self.inner.lock().expect("state lock");
        super::mark_lane_health_inner(&mut inner, lane, connected, last_error);
    }

    pub fn mark_lane_details(&self, details: Vec<LaneDetail>) {
        let mut inner = self.inner.lock().expect("state lock");
        super::mark_lane_details_inner(&mut inner, details);
    }

    pub fn emit_payload(
        &self,
        object_root: &[u8; 32],
        payload: &[u8],
        namespace: u16,
        epoch: u32,
        tag: &[u8; 32],
        flags: u16,
    ) {
        let mut inner = self.inner.lock().expect("state lock");
        let group_key_updated = super::emit_payload_events(
            &mut inner,
            object_root,
            payload,
            namespace,
            epoch,
            tag,
            flags,
        );
        if group_key_updated {
            if let Some(store) = &inner.store {
                store.persist(&super::snapshot_from_inner(&inner));
            }
        }
    }

    pub fn ingest_endorsement_payload(&self, payload: &[u8], now_step: u64) -> bool {
        let endorsements = super::parse_endorsements(payload);
        if endorsements.is_empty() {
            return false;
        }

        let mut inner = self.inner.lock().expect("state lock");
        let mut changed = false;
        for endorsement in endorsements {
            let result = inner.wot_policy.ingest_endorsement(
                endorsement.endorser,
                endorsement.publisher,
                endorsement.at_step,
                now_step,
            );
            if matches!(result, super::EndorsementIngestResult::Applied) {
                changed = true;
            }
        }
        if changed {
            let summary = inner.wot_policy.summary();
            super::emit_event_locked(
                &mut inner,
                "policy_updated",
                serde_json::json!({
                    "trusted": summary.trusted,
                    "muted": summary.muted,
                    "blocked": summary.blocked,
                    "endorsements": summary.endorsements,
                }),
            );
            self.persist_policy_locked(&mut inner);
        }
        changed
    }

    pub fn subscribe(&self, tag: &str) -> bool {
        let mut inner = self.inner.lock().expect("state lock");
        super::subscribe_tag(&mut inner, tag)
    }

    pub fn unsubscribe(&self, tag: &str) -> bool {
        let mut inner = self.inner.lock().expect("state lock");
        super::unsubscribe_tag(&mut inner, tag)
    }

    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<EventEnvelope> {
        let inner = self.inner.lock().expect("state lock");
        super::subscribe_events_receiver(&inner)
    }

    pub fn subscribe_events_since(
        &self,
        since: Option<u64>,
    ) -> (
        Vec<EventEnvelope>,
        tokio::sync::broadcast::Receiver<EventEnvelope>,
    ) {
        let inner = self.inner.lock().expect("state lock");
        super::subscribe_events_since(&inner, since)
    }

    pub fn emit_status_event(&self) -> EventEnvelope {
        let mut inner = self.inner.lock().expect("state lock");
        super::emit_status_event(&mut inner)
    }

    pub fn persist(&self) {
        let mut inner = self.inner.lock().expect("state lock");
        self.persist_policy_locked(&mut inner);
    }

    pub fn ensure_group_key(&self, group_id: &str) -> (String, [u8; 32]) {
        let mut inner = self.inner.lock().expect("state lock");
        let material = super::ensure_group_key_inner(&mut inner, group_id);
        if let Some(store) = &inner.store {
            store.persist(&super::snapshot_from_inner(&inner));
        }
        material
    }

    pub fn rotate_group_key(&self, group_id: &str) -> (String, [u8; 32]) {
        let mut inner = self.inner.lock().expect("state lock");
        let material = super::rotate_group_key_inner(&mut inner, group_id);
        if let Some(store) = &inner.store {
            store.persist(&super::snapshot_from_inner(&inner));
        }
        material
    }

    pub fn inject_local_feed_bundle(&self, bundle: serde_json::Value, object_root: [u8; 32]) {
        let mut inner = self.inner.lock().expect("state lock");
        super::emit_local_feed_bundle_event(&mut inner, bundle, object_root);
    }
}
