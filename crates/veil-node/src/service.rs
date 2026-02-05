use std::time::Duration;
use veil_core::{Epoch, Namespace, Tag};
use veil_crypto::aead::AeadCipher;
use veil_crypto::signing::{Signer, Verifier};
use veil_transport::adapter::{TransportAdapter, TransportHealthSnapshot};

use crate::batch::FeedBatcher;
use crate::config::NodeRuntimeConfig;
use crate::policy::EndorsementIngestResult;
use crate::publish::{
    publish_service_tick_multi_lane, PublishError, PublishOptions, PublishQueueTickParams,
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

/// Typed publisher tick input using `PublishOptions` instead of raw bitflags.
#[derive(Debug, Clone, Copy)]
pub struct PublisherTickOptionsInput<'a, PFast, PFallback> {
    pub namespace: Namespace,
    pub epoch: Epoch,
    pub tag: Tag,
    pub now_step: u64,
    pub options: PublishOptions,
    pub interactive_flush: bool,
    pub fast_peers: &'a [PFast],
    pub fallback_peers: &'a [PFallback],
}

/// Optional callbacks fired after one node runtime tick.
pub type DeliveredCallback<'a> = dyn FnMut(veil_core::ObjectRoot, &[u8]) + 'a;
pub type CountCallback<'a> = dyn FnMut(usize) + 'a;

#[derive(Default)]
pub struct NodeRuntimeCallbacks<'a> {
    pub on_delivered: Option<&'a mut DeliveredCallback<'a>>,
    pub on_ack_cleared: Option<&'a mut CountCallback<'a>>,
    pub on_send_failure: Option<&'a mut CountCallback<'a>>,
    pub on_endorsement_ingested: Option<&'a mut CountCallback<'a>>,
}

/// Aggregated per-lane transport health snapshots for a node runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NodeRuntimeTransportHealth {
    pub fast_lane: TransportHealthSnapshot,
    pub fallback_lane: TransportHealthSnapshot,
}

/// Runtime loop configuration for `NodeRuntime` orchestration.
#[derive(Debug, Clone, Copy)]
pub struct NodeRuntimeRunnerConfig {
    /// Initial step value passed into the first `tick`.
    pub start_step: u64,
    /// Delay between successful ticks.
    pub tick_interval: Duration,
    /// Delay applied after an error before next attempt.
    pub error_backoff: Duration,
    /// If set, exits loop after this many consecutive tick errors.
    pub max_consecutive_errors: Option<u32>,
}

impl Default for NodeRuntimeRunnerConfig {
    fn default() -> Self {
        Self {
            start_step: 0,
            tick_interval: Duration::from_millis(50),
            error_backoff: Duration::from_millis(250),
            max_consecutive_errors: Some(32),
        }
    }
}

/// Exit reason for `NodeRuntime` loop helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRuntimeRunnerExit {
    /// Loop ended because cancellation callback returned `true`.
    Cancelled { steps: u64 },
    /// Loop ended because requested step budget was fully consumed.
    Completed { steps: u64 },
    /// Loop ended because max consecutive errors threshold was reached.
    MaxConsecutiveErrors { steps: u64, consecutive_errors: u32 },
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

    /// Returns transport health counters for both lanes.
    pub fn transport_health(&self) -> NodeRuntimeTransportHealth {
        NodeRuntimeTransportHealth {
            fast_lane: self.fast_adapter.health_snapshot(),
            fallback_lane: self.fallback_adapter.health_snapshot(),
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

    pub fn tick_with_callbacks(
        &mut self,
        now_step: u64,
        fast_peers: &[AFast::Peer],
        fallback_peers: &[AFallback::Peer],
        callbacks: NodeRuntimeCallbacks<'_>,
    ) -> Result<Option<ReceiveEvent>, ReceiveError>
    where
        AFast::Peer: ToString,
        AFallback::Peer: ToString,
    {
        let mut callbacks = callbacks;
        self.tick_with_callbacks_ref(now_step, fast_peers, fallback_peers, &mut callbacks)
    }

    pub fn tick_with_callbacks_ref(
        &mut self,
        now_step: u64,
        fast_peers: &[AFast::Peer],
        fallback_peers: &[AFallback::Peer],
        callbacks: &mut NodeRuntimeCallbacks<'_>,
    ) -> Result<Option<ReceiveEvent>, ReceiveError>
    where
        AFast::Peer: ToString,
        AFallback::Peer: ToString,
    {
        let prev_ack = self.stats.ack_messages;
        let prev_fail = self.stats.send_failures;
        let mut endorsement_delta = 0usize;
        let event = self.tick(now_step, fast_peers, fallback_peers)?;

        if let Some(ReceiveEvent::Delivered {
            object_root,
            payload,
            ..
        }) = event.as_ref()
        {
            if let Some(endorsement) = crate::policy::parse_endorsement_payload(payload) {
                let outcome = self.config.wot_policy.ingest_endorsement(
                    endorsement.endorser,
                    endorsement.publisher,
                    endorsement.at_step,
                    now_step,
                );
                if outcome == EndorsementIngestResult::Applied {
                    endorsement_delta += 1;
                }
            }
            if let Some(cb) = callbacks.on_delivered.as_mut() {
                (*cb)(*object_root, payload);
            }
        }

        let ack_delta = self.stats.ack_messages.saturating_sub(prev_ack);
        if ack_delta > 0 {
            if let Some(cb) = callbacks.on_ack_cleared.as_mut() {
                (*cb)(ack_delta);
            }
        }

        let fail_delta = self.stats.send_failures.saturating_sub(prev_fail);
        if fail_delta > 0 {
            if let Some(cb) = callbacks.on_send_failure.as_mut() {
                (*cb)(fail_delta);
            }
        }
        if endorsement_delta > 0 {
            if let Some(cb) = callbacks.on_endorsement_ingested.as_mut() {
                (*cb)(endorsement_delta);
            }
        }

        Ok(event)
    }

    /// Runs ticks until cancellation callback returns true.
    pub fn run_until<F>(
        &mut self,
        fast_peers: &[AFast::Peer],
        fallback_peers: &[AFallback::Peer],
        config: NodeRuntimeRunnerConfig,
        mut should_stop: F,
        mut callbacks: Option<&mut NodeRuntimeCallbacks<'_>>,
    ) -> NodeRuntimeRunnerExit
    where
        AFast::Peer: ToString,
        AFallback::Peer: ToString,
        F: FnMut() -> bool,
    {
        let mut step = config.start_step;
        let mut steps = 0_u64;
        let mut consecutive_errors = 0_u32;

        loop {
            if should_stop() {
                return NodeRuntimeRunnerExit::Cancelled { steps };
            }

            let tick_result = if let Some(cb) = callbacks.as_deref_mut() {
                self.tick_with_callbacks_ref(step, fast_peers, fallback_peers, cb)
            } else {
                self.tick(step, fast_peers, fallback_peers)
            };

            match tick_result {
                Ok(_) => {
                    consecutive_errors = 0;
                    if !config.tick_interval.is_zero() {
                        std::thread::sleep(config.tick_interval);
                    }
                }
                Err(_) => {
                    consecutive_errors = consecutive_errors.saturating_add(1);
                    if let Some(max) = config.max_consecutive_errors {
                        if consecutive_errors >= max {
                            return NodeRuntimeRunnerExit::MaxConsecutiveErrors {
                                steps,
                                consecutive_errors,
                            };
                        }
                    }
                    if !config.error_backoff.is_zero() {
                        std::thread::sleep(config.error_backoff);
                    }
                }
            }

            step = step.saturating_add(1);
            steps = steps.saturating_add(1);
        }
    }

    /// Runs a fixed number of ticks.
    pub fn run_steps(
        &mut self,
        steps: u64,
        fast_peers: &[AFast::Peer],
        fallback_peers: &[AFallback::Peer],
        config: NodeRuntimeRunnerConfig,
        mut callbacks: Option<&mut NodeRuntimeCallbacks<'_>>,
    ) -> NodeRuntimeRunnerExit
    where
        AFast::Peer: ToString,
        AFallback::Peer: ToString,
    {
        let mut step = config.start_step;
        let mut ran = 0_u64;
        let mut consecutive_errors = 0_u32;

        while ran < steps {
            let tick_result = if let Some(cb) = callbacks.as_deref_mut() {
                self.tick_with_callbacks_ref(step, fast_peers, fallback_peers, cb)
            } else {
                self.tick(step, fast_peers, fallback_peers)
            };

            match tick_result {
                Ok(_) => {
                    consecutive_errors = 0;
                    if !config.tick_interval.is_zero() {
                        std::thread::sleep(config.tick_interval);
                    }
                }
                Err(_) => {
                    consecutive_errors = consecutive_errors.saturating_add(1);
                    if let Some(max) = config.max_consecutive_errors {
                        if consecutive_errors >= max {
                            return NodeRuntimeRunnerExit::MaxConsecutiveErrors {
                                steps: ran,
                                consecutive_errors,
                            };
                        }
                    }
                    if !config.error_backoff.is_zero() {
                        std::thread::sleep(config.error_backoff);
                    }
                }
            }

            ran = ran.saturating_add(1);
            step = step.saturating_add(1);
        }

        NodeRuntimeRunnerExit::Completed { steps: ran }
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
    #[allow(clippy::too_many_arguments)]
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

    pub fn tick_with_options(
        &mut self,
        input: PublisherTickOptionsInput<'_, AFast::Peer, AFallback::Peer>,
    ) -> Result<PublishServiceTickResult, PublishError> {
        self.tick(PublisherTickInput {
            namespace: input.namespace,
            epoch: input.epoch,
            tag: input.tag,
            now_step: input.now_step,
            flags: input.options.to_flags(),
            interactive_flush: input.interactive_flush,
            fast_peers: input.fast_peers,
            fallback_peers: input.fallback_peers,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use veil_codec::object::OBJECT_FLAG_SIGNED;
    use veil_crypto::aead::XChaCha20Poly1305Cipher;
    use veil_crypto::signing::{Ed25519Signer, Ed25519Verifier};
    use veil_transport::adapter::InMemoryAdapter;

    use super::{
        NodeRuntime, NodeRuntimeCallbacks, NodeRuntimeRunnerConfig, NodeRuntimeRunnerExit,
        NodeRuntimeTransportHealth, PublisherRuntime, PublisherTickInput,
        PublisherTickOptionsInput,
    };

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

    #[test]
    fn node_runtime_exposes_transport_health_snapshots() {
        let rt = NodeRuntime::new(
            crate::state::NodeState::default(),
            InMemoryAdapter::default(),
            InMemoryAdapter::default(),
            crate::config::NodeRuntimeConfig::default(),
            [0xAA; 32],
            XChaCha20Poly1305Cipher,
            Ed25519Verifier,
        );

        assert_eq!(rt.transport_health(), NodeRuntimeTransportHealth::default());
    }

    #[test]
    fn publisher_runtime_tick_with_options_works() {
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
            .tick_with_options(PublisherTickOptionsInput {
                namespace: veil_core::Namespace(1),
                epoch: veil_core::Epoch(1),
                tag: [0x22; 32],
                now_step: 1,
                options: crate::publish::PublishOptions::signed().with_ack_requested(true),
                interactive_flush: true,
                fast_peers: &peers,
                fallback_peers: &peers,
            })
            .expect("tick should succeed");

        assert!(out.published.is_some());
    }

    #[test]
    fn node_runtime_callbacks_receive_send_failure_delta() {
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
        let mut send_failure_count = 0usize;

        let _ = rt
            .tick_with_callbacks(
                1,
                &peers,
                &peers,
                NodeRuntimeCallbacks {
                    on_send_failure: Some(&mut |count| send_failure_count += count),
                    ..NodeRuntimeCallbacks::default()
                },
            )
            .expect("tick should succeed");

        assert_eq!(send_failure_count, 0);
    }

    #[test]
    fn node_runtime_run_steps_completes_requested_budget() {
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
        let exit = rt.run_steps(
            5,
            &peers,
            &peers,
            NodeRuntimeRunnerConfig {
                start_step: 10,
                tick_interval: Duration::ZERO,
                error_backoff: Duration::ZERO,
                max_consecutive_errors: Some(4),
            },
            None,
        );

        assert_eq!(exit, NodeRuntimeRunnerExit::Completed { steps: 5 });
    }

    #[test]
    fn node_runtime_run_until_honors_cancellation() {
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
        let mut polls = 0_u32;
        let exit = rt.run_until(
            &peers,
            &peers,
            NodeRuntimeRunnerConfig {
                start_step: 0,
                tick_interval: Duration::ZERO,
                error_backoff: Duration::ZERO,
                max_consecutive_errors: Some(4),
            },
            || {
                polls += 1;
                polls > 3
            },
            None,
        );

        assert_eq!(exit, NodeRuntimeRunnerExit::Cancelled { steps: 3 });
    }
}
