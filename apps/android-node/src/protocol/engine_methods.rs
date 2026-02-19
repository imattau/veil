use super::*;

impl ProtocolEngine {
    pub async fn publish_discovery(
        &self,
        msg: crate::discovery::DiscoveryMessage,
    ) -> Result<(), String> {
        let payload = serde_json::to_vec(&msg).map_err(|e| e.to_string())?;
        let namespace = self.config.discovery_namespace;
        let tag = discovery_tag(namespace);
        self.publish_with_tag(payload, namespace, tag).await
    }

    pub async fn subscribe_pubkey(&self, pubkey: [u8; 32], namespace: Namespace) {
        let tag = derive_feed_tag(&pubkey, namespace);
        let mut runtime = self.inner.lock().await;
        runtime.state.subscriptions.insert(tag);
    }

    pub async fn subscribe_tag(&self, tag: [u8; 32]) {
        let mut runtime = self.inner.lock().await;
        runtime.state.subscriptions.insert(tag);
    }

    pub async fn has_subscription(&self, tag: [u8; 32]) -> bool {
        let runtime = self.inner.lock().await;
        runtime.state.subscriptions.contains(&tag)
    }

    pub async fn sync_subscriptions(
        &self,
        channels: &[String],
        contacts: &[crate::api::ContactBundle],
    ) {
        let my_pubkey = *self.identity_pubkey.lock().await;
        let tags = build_subscription_tags(
            channels,
            contacts,
            my_pubkey,
            self.config.namespace,
            self.config.discovery_namespace,
        );

        let mut runtime = self.inner.lock().await;
        runtime.state.subscriptions.clear();
        for tag in tags {
            runtime.state.subscriptions.insert(tag);
        }
    }

    pub async fn update_identity(&self, pubkey: [u8; 32], signer: NostrSigner) {
        let mut runtime = self.inner.lock().await;
        runtime.signer = Some(signer);
        let mut guard = self.identity_pubkey.lock().await;
        *guard = pubkey;
    }

    pub async fn update_wot_policy(&self, policy: LocalWotPolicy) {
        let mut runtime = self.inner.lock().await;
        runtime.config.wot_policy = policy;
    }

    pub async fn build_object(
        &self,
        item: Vec<u8>,
        namespace: u16,
        flags: u16,
    ) -> Result<(Vec<u8>, ObjectRoot), String> {
        let runtime = self.inner.lock().await;
        let now_step = self.steps.load(Ordering::Relaxed);
        let epoch = current_epoch();
        let pubkey = *self.identity_pubkey.lock().await;
        build_batched_object(
            &runtime,
            item,
            Namespace(namespace),
            flags,
            now_step,
            epoch,
            pubkey,
        )
    }

    pub async fn add_contact(&self, contact: &crate::api::ContactBundle) {
        let mut dynamic = self.dynamic_peers.lock().await;
        add_dynamic_contact(&mut dynamic, contact);
    }

    pub async fn sync_contacts(&self, contacts: &[crate::api::ContactBundle]) {
        let mut dynamic = self.dynamic_peers.lock().await;
        replace_dynamic_contacts(&mut dynamic, contacts);
    }

    pub async fn get_cached_shard(&self, shard_id: [u8; 32]) -> Option<Vec<u8>> {
        let runtime = self.inner.lock().await;
        cached_shard_bytes(&runtime.state, shard_id)
    }

    pub async fn reconstruct_object(&self, root: [u8; 32]) -> Option<Vec<u8>> {
        let runtime = self.inner.lock().await;
        reconstruct_cached_object(&runtime.state, root, runtime.config.erasure_coding_mode)
    }

    pub async fn reconstruct_payload(&self, root: [u8; 32]) -> Option<Vec<u8>> {
        let runtime = self.inner.lock().await;
        reconstruct_payload_for_root(
            &runtime.state,
            root,
            runtime.config.erasure_coding_mode,
            runtime.encrypt_key,
        )
    }

    pub async fn persist_cache_state(&self) {
        let path = match &self.config.cache_state_path {
            Some(path) => path.clone(),
            None => return,
        };
        let mut runtime = self.inner.lock().await;
        let _ = veil_node::persistence::save_state_to_path(path, &mut runtime.state);
    }

    pub async fn persist_state(&self) {
        // NodeState has its own persist method internally.
    }

    pub fn peer_id(&self) -> String {
        self.config.peer_id.clone()
    }

    pub fn ws_url(&self) -> Option<String> {
        self.config.ws_url.clone()
    }

    pub fn discovery_namespace(&self) -> Namespace {
        self.config.discovery_namespace
    }

    pub fn quic_bind_addr(&self) -> String {
        self.config.quic_bind_addr.clone()
    }

    pub async fn pump_inbound(&self) -> Result<Option<ReceiveEvent>, String> {
        let mut runtime = self.inner.lock().await;
        let mut stats = self.runtime_stats.lock().await;
        let (fast_peers, fallback_peers) = self.runtime_peer_lists().await;
        let cfg = self.config.runtime_config.clone();
        let dynamic = self.dynamic_peers.lock().await.peer_map().clone();
        let now_step = self.steps.fetch_add(1, Ordering::Relaxed) + 1;

        pump_inbound_once(
            &mut runtime,
            &cfg,
            &dynamic,
            &fast_peers,
            &fallback_peers,
            now_step,
            &mut stats,
            &self.verifier,
        )
    }

    async fn runtime_peer_lists(&self) -> (Vec<String>, Vec<String>) {
        let (fast, fallback) = {
            let dynamic = self.dynamic_peers.lock().await;
            merged_dynamic_publish_peers(
                &self.config.fast_peers,
                &self.config.fallback_peers,
                &dynamic,
            )
        };
        finalize_runtime_peer_lists(fast, fallback, self.config.ws_url.as_deref())
    }

    pub async fn lane_details(&self) -> Vec<LaneDetail> {
        let runtime = self.inner.lock().await;
        let mut details = Vec::new();
        details.extend(build_lane_details(
            "fast",
            runtime.fast_adapter.lane_snapshots(),
        ));
        details.extend(build_lane_details(
            "fallback",
            runtime.fallback_adapter.lane_snapshots(),
        ));
        details
    }

    pub(super) async fn publish_peer_lists(&self) -> Result<(Vec<String>, Vec<String>), String> {
        let (fast, fallback) = {
            let dynamic = self.dynamic_peers.lock().await;
            merged_dynamic_publish_peers(
                &self.config.fast_peers,
                &self.config.fallback_peers,
                &dynamic,
            )
        };
        finalize_publish_peer_lists(fast, fallback, self.config.ws_url.as_deref())
    }

    #[cfg(test)]
    pub(crate) async fn dynamic_peer_snapshot(&self) -> (Vec<String>, Vec<String>) {
        let dynamic = self.dynamic_peers.lock().await;
        dynamic_peer_snapshot_from_store(&dynamic)
    }

    #[cfg(test)]
    pub(crate) async fn dynamic_peer_map_snapshot(
        &self,
    ) -> std::collections::HashMap<String, [u8; 32]> {
        let dynamic = self.dynamic_peers.lock().await;
        dynamic_peer_map_snapshot_from_store(&dynamic)
    }
}
