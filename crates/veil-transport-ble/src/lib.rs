//! Bluetooth mesh transport adapter for VEIL.
//!
//! This crate provides a lightweight, transport-agnostic skeleton that models
//! BLE mesh delivery as chunked frames over a lossy link. The actual BLE
//! platform integration is abstracted behind `BleLink`. Enable the
//! `btleplug` feature for the experimental hardware backend.

use std::collections::VecDeque;
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};

use thiserror::Error;
use veil_transport::adapter::{TransportAdapter, TransportHealthSnapshot};

#[cfg(feature = "btleplug")]
pub mod btleplug_backend;
pub mod chunking;
pub mod protocol;

use chunking::{split_into_frames, BleAssembler};
use protocol::BleFrame;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlePeer {
    pub addr: String,
    pub device_id: Option<[u8; 8]>,
}

impl BlePeer {
    pub fn new(addr: impl Into<String>) -> Self {
        Self {
            addr: addr.into(),
            device_id: None,
        }
    }

    pub fn with_device_id(addr: impl Into<String>, device_id: [u8; 8]) -> Self {
        Self {
            addr: addr.into(),
            device_id: Some(device_id),
        }
    }
}

impl std::fmt::Display for BlePeer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.addr)
    }
}

#[derive(Debug, Clone)]
pub struct BleAdapterConfig {
    pub mtu: usize,
    pub max_payload_hint: Option<usize>,
    pub drop_outbound: bool,
}

impl Default for BleAdapterConfig {
    fn default() -> Self {
        Self {
            mtu: 200,
            max_payload_hint: Some(16 * 1024),
            drop_outbound: false,
        }
    }
}

#[derive(Debug, Error)]
pub enum BleAdapterError {
    #[error("adapter is closed")]
    Closed,
    #[error("payload exceeds max payload hint ({hint} bytes)")]
    PayloadTooLarge { hint: usize },
}

/// Minimal BLE link abstraction used by the adapter.
pub trait BleLink {
    type Error;

    fn send_frame(&mut self, peer: &BlePeer, frame: &BleFrame) -> Result<(), Self::Error>;
    fn recv_frame(&mut self) -> Option<(BlePeer, BleFrame)>;
    fn mtu(&self) -> usize;
}

#[derive(Debug)]
pub struct BleAdapter<L: BleLink> {
    link: L,
    config: BleAdapterConfig,
    assembler: BleAssembler,
    outbound_queued: AtomicU64,
    outbound_send_ok: AtomicU64,
    outbound_send_err: AtomicU64,
    inbound_received: AtomicU64,
    inbound_dropped: AtomicU64,
}

impl<L: BleLink> BleAdapter<L> {
    pub fn new(link: L, config: BleAdapterConfig) -> Self {
        Self {
            link,
            config,
            assembler: BleAssembler::default(),
            outbound_queued: AtomicU64::new(0),
            outbound_send_ok: AtomicU64::new(0),
            outbound_send_err: AtomicU64::new(0),
            inbound_received: AtomicU64::new(0),
            inbound_dropped: AtomicU64::new(0),
        }
    }

    pub fn link_mut(&mut self) -> &mut L {
        &mut self.link
    }

    pub fn config(&self) -> &BleAdapterConfig {
        &self.config
    }

    fn bump(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

impl<L: BleLink> TransportAdapter for BleAdapter<L> {
    type Peer = BlePeer;
    type Error = BleAdapterError;

    fn send(&mut self, peer: &Self::Peer, bytes: &[u8]) -> Result<(), Self::Error> {
        if let Some(hint) = self.config.max_payload_hint {
            if bytes.len() > hint {
                return Err(BleAdapterError::PayloadTooLarge { hint });
            }
        }
        if self.config.drop_outbound {
            Self::bump(&self.outbound_send_err);
            return Ok(());
        }
        let shard_id = blake3::hash(bytes).as_bytes().to_owned();
        let frames = split_into_frames(shard_id, bytes, self.link.mtu());
        for frame in frames {
            self.outbound_queued.fetch_add(1, Ordering::Relaxed);
            if self.link.send_frame(peer, &frame).is_ok() {
                Self::bump(&self.outbound_send_ok);
            } else {
                Self::bump(&self.outbound_send_err);
            }
        }
        Ok(())
    }

    fn recv(&mut self) -> Option<(Self::Peer, Vec<u8>)> {
        while let Some((peer, frame)) = self.link.recv_frame() {
            Self::bump(&self.inbound_received);
            if let Some(payload) = self.assembler.ingest(frame) {
                return Some((peer, payload));
            }
        }
        None
    }

    fn max_payload_hint(&self) -> Option<usize> {
        self.config.max_payload_hint
    }

    fn health_snapshot(&self) -> TransportHealthSnapshot {
        TransportHealthSnapshot {
            outbound_queued: self.outbound_queued.load(Ordering::Relaxed),
            outbound_send_ok: self.outbound_send_ok.load(Ordering::Relaxed),
            outbound_send_err: self.outbound_send_err.load(Ordering::Relaxed),
            inbound_received: self.inbound_received.load(Ordering::Relaxed),
            inbound_dropped: self.inbound_dropped.load(Ordering::Relaxed),
            reconnect_attempts: 0,
        }
    }
}

#[derive(Debug, Default)]
pub struct MockBleLink {
    inbound: VecDeque<(BlePeer, BleFrame)>,
    outbound: Vec<(BlePeer, BleFrame)>,
    mtu: usize,
}

impl MockBleLink {
    pub fn with_mtu(mtu: usize) -> Self {
        Self {
            mtu,
            ..Self::default()
        }
    }

    pub fn enqueue_inbound(&mut self, peer: BlePeer, frame: BleFrame) {
        self.inbound.push_back((peer, frame));
    }

    pub fn take_outbound(&mut self) -> Vec<(BlePeer, BleFrame)> {
        std::mem::take(&mut self.outbound)
    }
}

impl BleLink for MockBleLink {
    type Error = ();

    fn send_frame(&mut self, peer: &BlePeer, frame: &BleFrame) -> Result<(), Self::Error> {
        self.outbound.push((peer.clone(), frame.clone()));
        Ok(())
    }

    fn recv_frame(&mut self) -> Option<(BlePeer, BleFrame)> {
        self.inbound.pop_front()
    }

    fn mtu(&self) -> usize {
        if self.mtu == 0 {
            200
        } else {
            self.mtu
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_roundtrip() {
        let link = MockBleLink::with_mtu(64);
        let mut adapter = BleAdapter::new(link, BleAdapterConfig::default());
        let peer = BlePeer::new("aa:bb:cc:dd:ee:ff");
        let payload = vec![9u8; 256];

        adapter.send(&peer, &payload).unwrap();
        let outbound = adapter.link_mut().take_outbound();
        for (peer, frame) in outbound {
            adapter.link_mut().enqueue_inbound(peer, frame);
        }

        let received = adapter.recv().unwrap();
        assert_eq!(received.0.addr, "aa:bb:cc:dd:ee:ff");
        assert_eq!(received.1, payload);
    }
}
