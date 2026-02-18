use std::collections::HashSet;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use veil_transport::adapter::{TransportAdapter, TransportHealthSnapshot};
#[cfg(feature = "ble-btleplug")]
use veil_transport_ble::btleplug_backend::BtleplugLink;
#[cfg(all(feature = "ble", not(feature = "ble-btleplug")))]
use veil_transport_ble::MockBleLink;
#[cfg(feature = "ble")]
use veil_transport_ble::{BleAdapter, BlePeer};
use veil_transport_tor::TorSocksAdapter;
use veil_transport_websocket::{WebSocketAdapter, WebSocketServerAdapter};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum FallbackPeer {
    WebSocket(String),
    WebSocketServer(String),
    Tor(String),
    #[cfg(feature = "ble")]
    Ble(BlePeer),
}

impl std::fmt::Display for FallbackPeer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FallbackPeer::WebSocket(peer) => write!(f, "ws:{peer}"),
            FallbackPeer::WebSocketServer(peer) => write!(f, "wssrv:{peer}"),
            FallbackPeer::Tor(peer) => write!(f, "tor:{peer}"),
            #[cfg(feature = "ble")]
            FallbackPeer::Ble(peer) => write!(f, "ble:{}", peer.addr),
        }
    }
}

#[derive(Debug)]
pub(crate) enum FallbackSendError {
    WebSocket,
    WebSocketServer,
    Tor,
    MissingWebSocket,
    MissingWebSocketServer,
    MissingTor,
    #[cfg(feature = "ble")]
    Ble,
    #[cfg(feature = "ble")]
    MissingBle,
}

#[cfg(feature = "ble-btleplug")]
type BleLinkImpl = BtleplugLink;
#[cfg(all(feature = "ble", not(feature = "ble-btleplug")))]
type BleLinkImpl = MockBleLink;

pub(crate) struct CombinedFallbackAdapter {
    ws: Option<WebSocketAdapter>,
    ws_server: Option<WebSocketServerAdapter>,
    tor: Option<TorSocksAdapter>,
    #[cfg(feature = "ble")]
    ble: Option<BleAdapter<BleLinkImpl>>,
    recv_cursor: u8,
}

impl CombinedFallbackAdapter {
    pub(crate) fn new(
        ws: Option<WebSocketAdapter>,
        ws_server: Option<WebSocketServerAdapter>,
        tor: Option<TorSocksAdapter>,
        #[cfg(feature = "ble")] ble: Option<BleAdapter<BleLinkImpl>>,
    ) -> Self {
        Self {
            ws,
            ws_server,
            tor,
            #[cfg(feature = "ble")]
            ble,
            recv_cursor: 0,
        }
    }

    pub(crate) fn ws_enabled(&self) -> bool {
        self.ws.is_some()
    }

    pub(crate) fn ws_server_enabled(&self) -> bool {
        self.ws_server.is_some()
    }

    pub(crate) fn tor_enabled(&self) -> bool {
        self.tor.is_some()
    }

    #[cfg(feature = "ble")]
    pub(crate) fn ble_enabled(&self) -> bool {
        self.ble.is_some()
    }

    fn ws_mut(&mut self) -> Option<&mut WebSocketAdapter> {
        self.ws.as_mut()
    }

    fn ws_server_mut(&mut self) -> Option<&mut WebSocketServerAdapter> {
        self.ws_server.as_mut()
    }

    fn tor_mut(&mut self) -> Option<&mut TorSocksAdapter> {
        self.tor.as_mut()
    }

    #[cfg(feature = "ble")]
    fn ble_mut(&mut self) -> Option<&mut BleAdapter<BleLinkImpl>> {
        self.ble.as_mut()
    }

    fn recv_lane_count() -> usize {
        #[cfg(feature = "ble")]
        {
            return 4;
        }
        #[cfg(not(feature = "ble"))]
        {
            3
        }
    }

    fn recv_from_lane(&mut self, lane: usize) -> Option<(FallbackPeer, Vec<u8>)> {
        match lane {
            0 => self.ws_mut().and_then(|ws| {
                ws.recv()
                    .map(|(peer, bytes)| (FallbackPeer::WebSocket(peer), bytes))
            }),
            1 => self.ws_server_mut().and_then(|ws| {
                ws.recv()
                    .map(|(peer, bytes)| (FallbackPeer::WebSocketServer(peer), bytes))
            }),
            2 => self.tor_mut().and_then(|tor| {
                tor.recv()
                    .map(|(peer, bytes)| (FallbackPeer::Tor(peer), bytes))
            }),
            #[cfg(feature = "ble")]
            3 => self.ble_mut().and_then(|ble| {
                ble.recv()
                    .map(|(peer, bytes)| (FallbackPeer::Ble(peer), bytes))
            }),
            _ => None,
        }
    }

    fn bump_recv_cursor(&mut self, lane_count: usize) {
        self.recv_cursor = ((self.recv_cursor as usize + 1) % lane_count) as u8;
    }

    fn combined_max_payload_hint(&self) -> Option<usize> {
        let ws_hint = self.ws.as_ref().and_then(|w| w.max_payload_hint());
        let ws_srv_hint = self.ws_server.as_ref().and_then(|w| w.max_payload_hint());
        let tor_hint = self.tor.as_ref().and_then(|t| t.max_payload_hint());
        let hint = match (ws_hint, ws_srv_hint, tor_hint) {
            (Some(a), Some(b), Some(c)) => Some(a.min(b).min(c)),
            (Some(a), Some(b), None) => Some(a.min(b)),
            (Some(a), None, Some(c)) => Some(a.min(c)),
            (None, Some(b), Some(c)) => Some(b.min(c)),
            (Some(a), None, None) => Some(a),
            (None, Some(b), None) => Some(b),
            (None, None, Some(c)) => Some(c),
            (None, None, None) => None,
        };
        #[cfg(feature = "ble")]
        {
            let ble_hint = self.ble.as_ref().and_then(|b| b.max_payload_hint());
            return match (hint, ble_hint) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };
        }
        hint
    }

    fn combined_health_snapshot(&self) -> TransportHealthSnapshot {
        let mut out = TransportHealthSnapshot::default();
        if let Some(ws) = &self.ws {
            let h = ws.health_snapshot();
            out.outbound_queued += h.outbound_queued;
            out.outbound_send_ok += h.outbound_send_ok;
            out.outbound_send_err += h.outbound_send_err;
            out.inbound_received += h.inbound_received;
            out.inbound_dropped += h.inbound_dropped;
            out.reconnect_attempts += h.reconnect_attempts;
        }
        if let Some(ws) = &self.ws_server {
            let h = ws.health_snapshot();
            out.outbound_queued += h.outbound_queued;
            out.outbound_send_ok += h.outbound_send_ok;
            out.outbound_send_err += h.outbound_send_err;
            out.inbound_received += h.inbound_received;
            out.inbound_dropped += h.inbound_dropped;
            out.reconnect_attempts += h.reconnect_attempts;
        }
        if let Some(tor) = &self.tor {
            let h = tor.health_snapshot();
            out.outbound_queued += h.outbound_queued;
            out.outbound_send_ok += h.outbound_send_ok;
            out.outbound_send_err += h.outbound_send_err;
            out.inbound_received += h.inbound_received;
            out.inbound_dropped += h.inbound_dropped;
            out.reconnect_attempts += h.reconnect_attempts;
        }
        #[cfg(feature = "ble")]
        if let Some(ble) = &self.ble {
            let h = ble.health_snapshot();
            out.outbound_queued += h.outbound_queued;
            out.outbound_send_ok += h.outbound_send_ok;
            out.outbound_send_err += h.outbound_send_err;
            out.inbound_received += h.inbound_received;
            out.inbound_dropped += h.inbound_dropped;
            out.reconnect_attempts += h.reconnect_attempts;
        }
        out
    }
}

impl TransportAdapter for CombinedFallbackAdapter {
    type Peer = FallbackPeer;
    type Error = FallbackSendError;

    fn send(&mut self, peer: &Self::Peer, bytes: &[u8]) -> Result<(), Self::Error> {
        match peer {
            FallbackPeer::WebSocket(ws_peer) => {
                let ws = self.ws_mut().ok_or(FallbackSendError::MissingWebSocket)?;
                ws.send(ws_peer, bytes)
                    .map_err(|_| FallbackSendError::WebSocket)
            }
            FallbackPeer::WebSocketServer(ws_peer) => {
                let ws = self
                    .ws_server_mut()
                    .ok_or(FallbackSendError::MissingWebSocketServer)?;
                ws.send(ws_peer, bytes)
                    .map_err(|_| FallbackSendError::WebSocketServer)
            }
            FallbackPeer::Tor(tor_peer) => {
                let tor = self.tor_mut().ok_or(FallbackSendError::MissingTor)?;
                tor.send(tor_peer, bytes)
                    .map_err(|_| FallbackSendError::Tor)
            }
            #[cfg(feature = "ble")]
            FallbackPeer::Ble(ble_peer) => {
                let ble = self.ble_mut().ok_or(FallbackSendError::MissingBle)?;
                ble.send(ble_peer, bytes)
                    .map_err(|_| FallbackSendError::Ble)
            }
        }
    }

    fn recv(&mut self) -> Option<(Self::Peer, Vec<u8>)> {
        let lane_count = Self::recv_lane_count();
        for offset in 0..lane_count {
            let lane = (self.recv_cursor as usize + offset) % lane_count;
            if let Some(item) = self.recv_from_lane(lane) {
                self.recv_cursor = ((lane + 1) % lane_count) as u8;
                return Some(item);
            }
        }
        self.bump_recv_cursor(lane_count);
        None
    }

    fn max_payload_hint(&self) -> Option<usize> {
        self.combined_max_payload_hint()
    }

    fn can_send(&self) -> bool {
        let ok = self.ws.as_ref().map(|w| w.can_send()).unwrap_or(false)
            || self
                .ws_server
                .as_ref()
                .map(|w| w.can_send())
                .unwrap_or(false)
            || self.tor.as_ref().map(|t| t.can_send()).unwrap_or(false);
        #[cfg(feature = "ble")]
        {
            return ok || self.ble.as_ref().map(|b| b.can_send()).unwrap_or(false);
        }
        ok
    }

    fn can_recv(&self) -> bool {
        let ok = self.ws.as_ref().map(|w| w.can_recv()).unwrap_or(false)
            || self
                .ws_server
                .as_ref()
                .map(|w| w.can_recv())
                .unwrap_or(false)
            || self.tor.as_ref().map(|t| t.can_recv()).unwrap_or(false);
        #[cfg(feature = "ble")]
        {
            return ok || self.ble.as_ref().map(|b| b.can_recv()).unwrap_or(false);
        }
        ok
    }

    fn health_snapshot(&self) -> TransportHealthSnapshot {
        self.combined_health_snapshot()
    }
}

#[cfg(feature = "ble")]
impl FallbackPeer {
    fn peer_ble(self) -> BlePeer {
        match self {
            FallbackPeer::Ble(p) => p,
            _ => panic!("not a ble peer"),
        }
    }
}

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
    use super::{CombinedFallbackAdapter, RecordingAdapter};
    use std::collections::{HashSet, VecDeque};
    use std::sync::{Arc, Mutex};
    use veil_transport::adapter::{TransportAdapter, TransportHealthSnapshot};

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

    #[test]
    fn combined_fallback_adapter_rotates_recv_cursor_when_idle() {
        let mut adapter = CombinedFallbackAdapter::new(
            None,
            None,
            None,
            #[cfg(feature = "ble")]
            None,
        );
        let lane_count = CombinedFallbackAdapter::recv_lane_count();
        assert!(lane_count >= 3);
        assert_eq!(adapter.recv_cursor, 0);

        let _ = adapter.recv();
        assert_eq!(adapter.recv_cursor as usize, 1 % lane_count);
        let _ = adapter.recv();
        assert_eq!(adapter.recv_cursor as usize, 2 % lane_count);
        let _ = adapter.recv();
        assert_eq!(adapter.recv_cursor as usize, 3 % lane_count);
    }
}
