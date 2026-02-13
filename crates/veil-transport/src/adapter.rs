use std::collections::VecDeque;
use std::hash::Hash;

/// Coarse per-adapter transport health counters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransportHealthSnapshot {
    pub outbound_queued: u64,
    pub outbound_send_ok: u64,
    pub outbound_send_err: u64,
    pub inbound_received: u64,
    pub inbound_dropped: u64,
    pub reconnect_attempts: u64,
    pub last_error: Option<String>,
    pub last_error_code: Option<String>,
}

/// Byte-oriented transport contract used by the VEIL node runtime.
pub trait TransportAdapter {
    /// Opaque peer handle used for replies/routing.
    type Peer: Clone + Eq + Hash;
    /// Transport-specific send error.
    type Error;

    /// Attempts best-effort delivery of a byte payload to a peer.
    fn send(&mut self, peer: &Self::Peer, bytes: &[u8]) -> Result<(), Self::Error>;
    /// Returns the next inbound payload and its sending peer.
    fn recv(&mut self) -> Option<(Self::Peer, Vec<u8>)>;

    /// Optional maximum payload hint used for lane/policy decisions.
    fn max_payload_hint(&self) -> Option<usize> {
        None
    }

    /// Whether outbound send is currently available.
    fn can_send(&self) -> bool {
        true
    }

    /// Whether inbound receive is currently available.
    fn can_recv(&self) -> bool {
        true
    }

    /// Best-effort transport health counters for policy/ops decisions.
    fn health_snapshot(&self) -> TransportHealthSnapshot {
        TransportHealthSnapshot::default()
    }

    /// Optional p95 end-to-end latency estimate in milliseconds.
    ///
    /// Adapters may derive this from transport-specific telemetry.
    fn p95_latency_ms(&self) -> Option<u64> {
        None
    }

    /// Optional transport-level ACK success ratio in `[0.0, 1.0]`.
    ///
    /// If unavailable, lane-scoring logic should fall back to neutral/defaults.
    fn ack_success_rate(&self) -> Option<f64> {
        None
    }
}

/// In-memory adapter for tests and simulations.
#[derive(Debug, Default, Clone)]
pub struct InMemoryAdapter {
    inbound: VecDeque<(String, Vec<u8>)>,
    outbound: Vec<(String, Vec<u8>)>,
    payload_hint: Option<usize>,
    drop_outbound: bool,
    send_ok: u64,
    send_err: u64,
    recv_ok: u64,
}

impl InMemoryAdapter {
    /// Creates an in-memory adapter with a configured max payload hint.
    pub fn with_payload_hint(max_payload_hint: usize) -> Self {
        Self {
            payload_hint: Some(max_payload_hint),
            ..Self::default()
        }
    }

    /// Queues bytes as inbound traffic from `peer`.
    pub fn enqueue_inbound(&mut self, peer: impl Into<String>, bytes: Vec<u8>) {
        self.inbound.push_back((peer.into(), bytes));
    }

    /// Drains and returns all outbound sends captured so far.
    pub fn take_outbound(&mut self) -> Vec<(String, Vec<u8>)> {
        std::mem::take(&mut self.outbound)
    }

    /// If enabled, outbound sends are dropped (best-effort loss simulation).
    pub fn set_drop_outbound(&mut self, drop_outbound: bool) {
        self.drop_outbound = drop_outbound;
    }
}

/// Routes all captured outbound messages from one in-memory adapter into
/// another adapter's inbound queue, tagging them as sent by `from_peer`.
pub fn route_in_memory_outbound(
    from_adapter: &mut InMemoryAdapter,
    to_adapter: &mut InMemoryAdapter,
    from_peer: impl Into<String>,
) -> usize {
    let from_peer = from_peer.into();
    let outbound = from_adapter.take_outbound();
    let moved = outbound.len();
    for (_, bytes) in outbound {
        to_adapter.enqueue_inbound(from_peer.clone(), bytes);
    }
    moved
}

/// In-memory adapter variant with explicit capability toggles and payload cap.
#[derive(Debug, Clone)]
pub struct CappedInMemoryAdapter {
    inbound: VecDeque<(String, Vec<u8>)>,
    outbound: Vec<(String, Vec<u8>)>,
    payload_hint: Option<usize>,
    max_send_bytes: usize,
    allow_send: bool,
    allow_recv: bool,
    send_ok: u64,
    send_err: u64,
    recv_ok: u64,
}

impl Default for CappedInMemoryAdapter {
    fn default() -> Self {
        Self {
            inbound: VecDeque::new(),
            outbound: Vec::new(),
            payload_hint: None,
            max_send_bytes: usize::MAX,
            allow_send: true,
            allow_recv: true,
            send_ok: 0,
            send_err: 0,
            recv_ok: 0,
        }
    }
}

impl CappedInMemoryAdapter {
    /// Creates an adapter that rejects sends larger than `max_send_bytes`.
    pub fn with_max_send_bytes(max_send_bytes: usize) -> Self {
        Self {
            max_send_bytes,
            ..Self::default()
        }
    }

    /// Sets optional payload hint exposed through `max_payload_hint`.
    pub fn set_payload_hint(&mut self, payload_hint: Option<usize>) {
        self.payload_hint = payload_hint;
    }

    /// Enables/disables outbound sending capability.
    pub fn set_allow_send(&mut self, allow_send: bool) {
        self.allow_send = allow_send;
    }

    /// Enables/disables inbound receive capability.
    pub fn set_allow_recv(&mut self, allow_recv: bool) {
        self.allow_recv = allow_recv;
    }

    /// Queues bytes as inbound traffic from `peer`.
    pub fn enqueue_inbound(&mut self, peer: impl Into<String>, bytes: Vec<u8>) {
        self.inbound.push_back((peer.into(), bytes));
    }

    /// Drains and returns all outbound sends captured so far.
    pub fn take_outbound(&mut self) -> Vec<(String, Vec<u8>)> {
        std::mem::take(&mut self.outbound)
    }
}

impl TransportAdapter for InMemoryAdapter {
    type Peer = String;
    type Error = &'static str;

    fn send(&mut self, peer: &Self::Peer, bytes: &[u8]) -> Result<(), Self::Error> {
        if self.drop_outbound {
            self.send_err += 1;
            return Ok(());
        }
        self.outbound.push((peer.clone(), bytes.to_vec()));
        self.send_ok += 1;
        Ok(())
    }

    fn recv(&mut self) -> Option<(Self::Peer, Vec<u8>)> {
        let msg = self.inbound.pop_front();
        if msg.is_some() {
            self.recv_ok += 1;
        }
        msg
    }

    fn max_payload_hint(&self) -> Option<usize> {
        self.payload_hint
    }

    fn health_snapshot(&self) -> TransportHealthSnapshot {
        TransportHealthSnapshot {
            outbound_queued: self.outbound.len() as u64,
            outbound_send_ok: self.send_ok,
            outbound_send_err: self.send_err,
            inbound_received: self.recv_ok,
            inbound_dropped: 0,
            reconnect_attempts: 0,
            last_error: None,
            last_error_code: None,
        }
    }
}

impl TransportAdapter for CappedInMemoryAdapter {
    type Peer = String;
    type Error = &'static str;

    fn send(&mut self, peer: &Self::Peer, bytes: &[u8]) -> Result<(), Self::Error> {
        if !self.allow_send {
            self.send_err += 1;
            return Err("send disabled");
        }
        if bytes.len() > self.max_send_bytes {
            self.send_err += 1;
            return Err("payload exceeds max_send_bytes");
        }
        self.outbound.push((peer.clone(), bytes.to_vec()));
        self.send_ok += 1;
        Ok(())
    }

    fn recv(&mut self) -> Option<(Self::Peer, Vec<u8>)> {
        if !self.allow_recv {
            return None;
        }
        let msg = self.inbound.pop_front();
        if msg.is_some() {
            self.recv_ok += 1;
        }
        msg
    }

    fn max_payload_hint(&self) -> Option<usize> {
        self.payload_hint
    }

    fn can_send(&self) -> bool {
        self.allow_send
    }

    fn can_recv(&self) -> bool {
        self.allow_recv
    }

    fn health_snapshot(&self) -> TransportHealthSnapshot {
        TransportHealthSnapshot {
            outbound_queued: self.outbound.len() as u64,
            outbound_send_ok: self.send_ok,
            outbound_send_err: self.send_err,
            inbound_received: self.recv_ok,
            inbound_dropped: 0,
            reconnect_attempts: 0,
            last_error: None,
            last_error_code: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        route_in_memory_outbound, CappedInMemoryAdapter, InMemoryAdapter, TransportAdapter,
        TransportHealthSnapshot,
    };

    #[test]
    fn in_memory_adapter_send_and_recv_work() {
        let mut adapter = InMemoryAdapter::with_payload_hint(16 * 1024);
        adapter.enqueue_inbound("alice", vec![1, 2, 3]);

        let inbound = adapter.recv().expect("should receive one message");
        assert_eq!(inbound.0, "alice");
        assert_eq!(inbound.1, vec![1, 2, 3]);
        assert_eq!(adapter.max_payload_hint(), Some(16 * 1024));

        adapter
            .send(&"bob".to_string(), &[9, 8])
            .expect("send should succeed");
        let outbound = adapter.take_outbound();
        assert_eq!(outbound, vec![("bob".to_string(), vec![9, 8])]);
        let health = adapter.health_snapshot();
        assert_eq!(
            health,
            TransportHealthSnapshot {
                outbound_queued: 0,
                outbound_send_ok: 1,
                outbound_send_err: 0,
                inbound_received: 1,
                inbound_dropped: 0,
                reconnect_attempts: 0,
                last_error: None,
                last_error_code: None,
            }
        );
    }

    #[test]
    fn in_memory_adapter_can_simulate_lossy_outbound() {
        let mut adapter = InMemoryAdapter::default();
        adapter.set_drop_outbound(true);
        adapter
            .send(&"bob".to_string(), &[1, 2, 3])
            .expect("best-effort drop should still return ok");
        assert!(adapter.take_outbound().is_empty());
    }

    #[test]
    fn capped_adapter_enforces_send_cap_and_capabilities() {
        let mut adapter = CappedInMemoryAdapter::with_max_send_bytes(4);
        adapter.set_payload_hint(Some(4));
        adapter.enqueue_inbound("alice", vec![1, 2, 3]);

        assert_eq!(adapter.max_payload_hint(), Some(4));
        assert!(adapter.can_send());
        assert!(adapter.can_recv());
        assert!(adapter.recv().is_some());

        let err = adapter
            .send(&"bob".to_string(), &[0, 1, 2, 3, 4])
            .expect_err("oversized sends should be rejected");
        assert_eq!(err, "payload exceeds max_send_bytes");

        adapter.set_allow_send(false);
        let err = adapter
            .send(&"bob".to_string(), &[1, 2])
            .expect_err("disabled send should fail");
        assert_eq!(err, "send disabled");

        adapter.set_allow_recv(false);
        assert!(!adapter.can_recv());
        assert!(adapter.recv().is_none());
    }

    #[test]
    fn route_in_memory_outbound_moves_messages_to_receiver_inbox() {
        let mut src = InMemoryAdapter::default();
        let mut dst = InMemoryAdapter::default();
        src.send(&"peer-a".to_string(), &[1, 2, 3])
            .expect("send should succeed");
        src.send(&"peer-b".to_string(), &[4, 5])
            .expect("send should succeed");

        let moved = route_in_memory_outbound(&mut src, &mut dst, "src-peer");
        assert_eq!(moved, 2);

        let (peer1, bytes1) = dst.recv().expect("first inbound expected");
        assert_eq!(peer1, "src-peer");
        assert_eq!(bytes1, vec![1, 2, 3]);

        let (peer2, bytes2) = dst.recv().expect("second inbound expected");
        assert_eq!(peer2, "src-peer");
        assert_eq!(bytes2, vec![4, 5]);
    }
}
