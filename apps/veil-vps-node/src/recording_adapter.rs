use std::collections::HashSet;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use veil_transport::adapter::{TransportAdapter, TransportHealthSnapshot};

pub(crate) struct RecordingAdapter<A: TransportAdapter> {
    inner: A,
    seen: Arc<Mutex<HashSet<A::Peer>>>,
    max_seen_peers: usize,
}

impl<A: TransportAdapter> RecordingAdapter<A> {
    pub(crate) fn new_bounded(
        inner: A,
        seen: Arc<Mutex<HashSet<A::Peer>>>,
        max_seen_peers: usize,
    ) -> Self {
        Self {
            inner,
            seen,
            max_seen_peers,
        }
    }

    pub(crate) fn snapshot_seen(&self) -> Vec<A::Peer>
    where
        A::Peer: Clone,
    {
        let guard = self.seen.lock().unwrap_or_else(|e| e.into_inner());
        guard.iter().cloned().collect()
    }
}

impl<A: TransportAdapter> TransportAdapter for RecordingAdapter<A>
where
    A::Peer: Clone + Eq + Hash,
{
    type Peer = A::Peer;
    type Error = A::Error;

    fn send(&mut self, peer: &Self::Peer, bytes: &[u8]) -> Result<(), Self::Error> {
        self.inner.send(peer, bytes)
    }

    fn recv(&mut self) -> Option<(Self::Peer, Vec<u8>)> {
        let item = self.inner.recv();
        if let Some((ref peer, _)) = item {
            let mut guard = self.seen.lock().unwrap_or_else(|e| e.into_inner());
            if guard.contains(peer) {
                return item;
            }
            if self.max_seen_peers == 0 {
                return item;
            }
            if guard.len() >= self.max_seen_peers {
                if let Some(evicted) = guard.iter().next().cloned() {
                    guard.remove(&evicted);
                }
            }
            guard.insert(peer.clone());
        }
        item
    }

    fn max_payload_hint(&self) -> Option<usize> {
        self.inner.max_payload_hint()
    }

    fn can_send(&self) -> bool {
        self.inner.can_send()
    }

    fn can_recv(&self) -> bool {
        self.inner.can_recv()
    }

    fn health_snapshot(&self) -> TransportHealthSnapshot {
        self.inner.health_snapshot()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashSet, VecDeque};
    use std::sync::{Arc, Mutex};

    use veil_transport::adapter::{TransportAdapter, TransportHealthSnapshot};

    use super::RecordingAdapter;

    #[derive(Default)]
    struct MockAdapter {
        recv_queue: VecDeque<(String, Vec<u8>)>,
    }

    impl TransportAdapter for MockAdapter {
        type Peer = String;
        type Error = ();

        fn send(&mut self, _peer: &Self::Peer, _bytes: &[u8]) -> Result<(), Self::Error> {
            Ok(())
        }

        fn recv(&mut self) -> Option<(Self::Peer, Vec<u8>)> {
            self.recv_queue.pop_front()
        }

        fn max_payload_hint(&self) -> Option<usize> {
            None
        }

        fn can_send(&self) -> bool {
            true
        }

        fn can_recv(&self) -> bool {
            !self.recv_queue.is_empty()
        }

        fn health_snapshot(&self) -> TransportHealthSnapshot {
            TransportHealthSnapshot::default()
        }
    }

    #[test]
    fn recording_adapter_caps_seen_peers_when_bounded() {
        let seen = Arc::new(Mutex::new(HashSet::new()));
        let mut adapter = RecordingAdapter::new_bounded(
            MockAdapter {
                recv_queue: VecDeque::from([
                    ("peer-a".to_string(), vec![1]),
                    ("peer-b".to_string(), vec![2]),
                    ("peer-c".to_string(), vec![3]),
                ]),
            },
            Arc::clone(&seen),
            2,
        );

        let _ = adapter.recv();
        let _ = adapter.recv();
        let _ = adapter.recv();

        let snapshot = adapter.snapshot_seen();
        assert_eq!(snapshot.len(), 2);
        assert!(snapshot.contains(&"peer-c".to_string()));
    }

    #[test]
    fn recording_adapter_drops_new_peers_when_cap_is_zero() {
        let seen = Arc::new(Mutex::new(HashSet::new()));
        let mut adapter = RecordingAdapter::new_bounded(
            MockAdapter {
                recv_queue: VecDeque::from([("peer-a".to_string(), vec![1])]),
            },
            Arc::clone(&seen),
            0,
        );

        let _ = adapter.recv();
        assert!(adapter.snapshot_seen().is_empty());
    }

    #[test]
    fn recording_adapter_evicts_existing_peer_when_full() {
        let seen = Arc::new(Mutex::new(HashSet::new()));
        let mut adapter = RecordingAdapter::new_bounded(
            MockAdapter {
                recv_queue: VecDeque::from([
                    ("peer-a".to_string(), vec![1]),
                    ("peer-b".to_string(), vec![2]),
                    ("peer-c".to_string(), vec![3]),
                ]),
            },
            Arc::clone(&seen),
            2,
        );

        let _ = adapter.recv();
        let _ = adapter.recv();
        let _ = adapter.recv();

        let snapshot = adapter.snapshot_seen();
        assert_eq!(snapshot.len(), 2);
        assert!(snapshot.contains(&"peer-c".to_string()));
    }
}
