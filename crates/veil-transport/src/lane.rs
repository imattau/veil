use veil_codec::shard::ShardV1;

/// Legacy lane-level send abstraction used by some forwarding helpers.
#[deprecated(note = "use adapter::TransportAdapter for new integrations")]
pub trait TransportLane {
    /// Sends a shard to one peer selected by the lane implementation.
    fn send_to_one_peer(&self, shard: &ShardV1) -> Result<(), &'static str>;
    /// Sends a shard to two peers selected by the lane implementation.
    fn send_to_two_peers(&self, shard: &ShardV1) -> Result<(), &'static str>;
}

#[cfg(test)]
mod tests {
    #![allow(deprecated)]
    use std::cell::Cell;

    use super::TransportLane;
    use veil_codec::shard::{
        ShardErasureMode, ShardHeaderV1, ShardV1, SHARD_HEADER_LEN, SHARD_V1_VERSION,
    };
    use veil_core::{Epoch, Namespace};

    #[derive(Default)]
    struct MockLane {
        sent_one: Cell<u32>,
        sent_two: Cell<u32>,
    }

    impl TransportLane for MockLane {
        fn send_to_one_peer(&self, _shard: &ShardV1) -> Result<(), &'static str> {
            self.sent_one.set(self.sent_one.get() + 1);
            Ok(())
        }

        fn send_to_two_peers(&self, _shard: &ShardV1) -> Result<(), &'static str> {
            self.sent_two.set(self.sent_two.get() + 1);
            Ok(())
        }
    }

    fn sample_shard() -> ShardV1 {
        ShardV1 {
            header: ShardHeaderV1 {
                version: SHARD_V1_VERSION,
                namespace: Namespace(1),
                epoch: Epoch(1),
                tag: [1_u8; 32],
                object_root: [2_u8; 32],
                profile_id: 1,
                erasure_mode: ShardErasureMode::Systematic,
                bucket_size: (16 * 1024) as u32,
                k: 1,
                n: 1,
                index: 0,
            },
            payload: vec![9_u8; 16 * 1024 - SHARD_HEADER_LEN],
        }
    }

    #[test]
    fn transport_lane_trait_can_be_used_for_one_and_two_peer_sends() {
        let lane = MockLane::default();
        let shard = sample_shard();

        lane.send_to_one_peer(&shard)
            .expect("one-peer send should succeed");
        lane.send_to_two_peers(&shard)
            .expect("two-peer send should succeed");

        assert_eq!(lane.sent_one.get(), 1);
        assert_eq!(lane.sent_two.get(), 1);
    }
}
