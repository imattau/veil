use veil_core::{Epoch, Namespace, Tag};
use veil_crypto::aead::AeadCipher;
use veil_crypto::signing::{Signer, Verifier};
use veil_transport::adapter::TransportAdapter;

use crate::batch::FeedBatcher;
use crate::config::NodeRuntimeConfig;
use crate::publish::{
    publish_service_tick_multi_lane, PublishError, PublishQueueTickParams,
    PublishServiceTickParams, PublishServiceTickResult,
};
use crate::receive::{ReceiveError, ReceiveEvent};
use crate::runtime::{
    pump_multi_lane_tick_with_config_split, ConfigMultiLanePumpParams, RuntimeStats,
};
use crate::state::NodeState;

/// Inputs used by one publisher runtime tick.
#[derive(Debug, Clone, Copy)]
pub struct PublisherTickInput<'a, PFast, PFallback> {
    pub namespace: Namespace,
    pub epoch: Epoch,
    pub tag: Tag,
    pub now_step: u64,
    pub flags: u16,
    pub interactive_flush: bool,
    pub fast_peers: &'a [PFast],
    pub fallback_peers: &'a [PFallback],
}

/// Stateful node runtime facade around `pump_multi_lane_tick_with_config_split`.
///
/// This reduces call-site wiring by owning state, adapters, crypto handles,
/// key material, config, and stats.
pub struct NodeRuntime<AFast, AFallback, C, V>
where
    AFast: TransportAdapter,
    AFallback: TransportAdapter,
    C: AeadCipher,
    V: Verifier,
{
    pub state: NodeState,
    pub fast_adapter: AFast,
    pub fallback_adapter: AFallback,
    pub config: NodeRuntimeConfig,
    pub decrypt_key: [u8; 32],
    pub stats: RuntimeStats,
    cipher: C,
    verifier: V,
}

impl<AFast, AFallback, C, V> NodeRuntime<AFast, AFallback, C, V>
where
    AFast: TransportAdapter,
    AFallback: TransportAdapter,
    C: AeadCipher,
    V: Verifier,
{
    pub fn new(
        state: NodeState,
        fast_adapter: AFast,
        fallback_adapter: AFallback,
        config: NodeRuntimeConfig,
        decrypt_key: [u8; 32],
        cipher: C,
        verifier: V,
    ) -> Self {
        Self {
            state,
            fast_adapter,
            fallback_adapter,
            config,
            decrypt_key,
            stats: RuntimeStats::default(),
            cipher,
            verifier,
        }
    }

    pub fn tick(
        &mut self,
        now_step: u64,
        fast_peers: &[AFast::Peer],
        fallback_peers: &[AFallback::Peer],
    ) -> Result<Option<ReceiveEvent>, ReceiveError>
    where
        AFast::Peer: ToString,
        AFallback::Peer: ToString,
    {
        pump_multi_lane_tick_with_config_split(
            &mut self.state,
            &mut self.fast_adapter,
            &mut self.fallback_adapter,
            ConfigMultiLanePumpParams {
                fast_peers,
                fallback_peers,
                now_step,
                decrypt_key: &self.decrypt_key,
                config: &self.config,
                stats: &mut self.stats,
            },
            &self.cipher,
            &self.verifier,
        )
    }
}

/// Stateful publisher runtime facade around `publish_service_tick_multi_lane`.
///
/// This owns queue/batcher, runtime state, adapters, config, key material, and
/// optional signer for one-call publish ticks.
pub struct PublisherRuntime<AFast, AFallback, C, S>
where
    AFast: TransportAdapter,
    AFallback: TransportAdapter,
    C: AeadCipher,
    S: Signer,
{
    pub state: NodeState,
    pub batcher: FeedBatcher,
    pub fast_adapter: AFast,
    pub fallback_adapter: AFallback,
    pub config: NodeRuntimeConfig,
    pub encrypt_key: [u8; 32],
    pub signer: Option<S>,
    cipher: C,
}

impl<AFast, AFallback, C, S> PublisherRuntime<AFast, AFallback, C, S>
where
    AFast: TransportAdapter,
    AFallback: TransportAdapter,
    C: AeadCipher,
    S: Signer,
{
    pub fn new(
        state: NodeState,
        batcher: FeedBatcher,
        fast_adapter: AFast,
        fallback_adapter: AFallback,
        config: NodeRuntimeConfig,
        encrypt_key: [u8; 32],
        signer: Option<S>,
        cipher: C,
    ) -> Self {
        Self {
            state,
            batcher,
            fast_adapter,
            fallback_adapter,
            config,
            encrypt_key,
            signer,
            cipher,
        }
    }

    pub fn enqueue(&mut self, item: Vec<u8>) {
        self.batcher.enqueue(item);
    }

    pub fn tick(
        &mut self,
        input: PublisherTickInput<'_, AFast::Peer, AFallback::Peer>,
    ) -> Result<PublishServiceTickResult, PublishError> {
        publish_service_tick_multi_lane(
            &mut self.state,
            &mut self.fast_adapter,
            &mut self.fallback_adapter,
            PublishServiceTickParams {
                batcher: &mut self.batcher,
                publish: PublishQueueTickParams {
                    namespace: input.namespace,
                    epoch: input.epoch,
                    tag: input.tag,
                    encrypt_key: &self.encrypt_key,
                    now_step: input.now_step,
                    flags: input.flags,
                    interactive_flush: input.interactive_flush,
                    fast_peers: input.fast_peers,
                    fallback_peers: input.fallback_peers,
                },
            },
            &self.config,
            &self.cipher,
            self.signer.as_ref(),
        )
    }
}

#[cfg(test)]
mod tests {
    use veil_codec::object::OBJECT_FLAG_SIGNED;
    use veil_crypto::aead::XChaCha20Poly1305Cipher;
    use veil_crypto::signing::{Ed25519Signer, Ed25519Verifier};
    use veil_transport::adapter::InMemoryAdapter;

    use super::{NodeRuntime, PublisherRuntime, PublisherTickInput};

    #[test]
    fn publisher_runtime_tick_publishes_from_queue() {
        let mut rt = PublisherRuntime::new(
            crate::state::NodeState::default(),
            crate::batch::FeedBatcher::default(),
            InMemoryAdapter::default(),
            InMemoryAdapter::default(),
            crate::config::NodeRuntimeConfig::default(),
            [0xAA; 32],
            Some(Ed25519Signer::from_secret([0x11; 32])),
            XChaCha20Poly1305Cipher,
        );
        rt.enqueue(b"hello".to_vec());
        let peers = vec!["peer-a".to_string(), "peer-b".to_string()];

        let out = rt
            .tick(PublisherTickInput {
                namespace: veil_core::Namespace(1),
                epoch: veil_core::Epoch(1),
                tag: [0x22; 32],
                now_step: 1,
                flags: OBJECT_FLAG_SIGNED,
                interactive_flush: true,
                fast_peers: &peers,
                fallback_peers: &peers,
            })
            .expect("tick should succeed");

        assert!(out.published.is_some());
    }

    #[test]
    fn node_runtime_tick_runs_without_message() {
        let mut rt = NodeRuntime::new(
            crate::state::NodeState::default(),
            InMemoryAdapter::default(),
            InMemoryAdapter::default(),
            crate::config::NodeRuntimeConfig::default(),
            [0xAA; 32],
            XChaCha20Poly1305Cipher,
            Ed25519Verifier,
        );
        let peers = vec!["peer-a".to_string()];

        let out = rt.tick(1, &peers, &peers).expect("tick should succeed");
        assert!(out.is_none());
    }
}
