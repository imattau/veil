use std::collections::HashMap;

use crate::protocol::BleFrame;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShardId(pub [u8; 32]);

#[derive(Debug)]
struct Assembly {
    total: u16,
    received: Vec<Option<Vec<u8>>>,
    received_count: usize,
}

impl Assembly {
    fn new(total: u16) -> Self {
        Self {
            total,
            received: vec![None; total as usize],
            received_count: 0,
        }
    }

    fn insert(&mut self, index: u16, payload: Vec<u8>) -> bool {
        let idx = index as usize;
        if idx >= self.received.len() {
            return false;
        }
        if self.received[idx].is_none() {
            self.received[idx] = Some(payload);
            self.received_count += 1;
        }
        self.received_count == self.received.len()
    }

    fn reassemble(self) -> Vec<u8> {
        let mut out = Vec::new();
        for bytes in self.received.into_iter().flatten() {
            out.extend_from_slice(&bytes);
        }
        out
    }
}

#[derive(Debug, Default)]
pub struct BleAssembler {
    assemblies: HashMap<ShardId, Assembly>,
}

impl BleAssembler {
    pub fn ingest(&mut self, frame: BleFrame) -> Option<Vec<u8>> {
        let shard_id = ShardId(frame.header.shard_id);
        let total = frame.header.total;
        let entry = self
            .assemblies
            .entry(shard_id)
            .or_insert_with(|| Assembly::new(total));
        if entry.total != total {
            return None;
        }
        let complete = entry.insert(frame.header.index, frame.payload);
        if complete {
            let assembly = self.assemblies.remove(&shard_id)?;
            return Some(assembly.reassemble());
        }
        None
    }
}

pub fn split_into_frames(shard_id: [u8; 32], payload: &[u8], mtu: usize) -> Vec<BleFrame> {
    let header_len = crate::protocol::BLE_FRAME_HEADER_LEN;
    let max_payload = mtu.saturating_sub(header_len).max(1);
    let mut frames = Vec::new();
    let total = payload.len().div_ceil(max_payload).max(1) as u16;

    for (index, chunk) in payload.chunks(max_payload).enumerate() {
        frames.push(BleFrame::new(shard_id, index as u16, total, chunk.to_vec()));
    }

    if frames.is_empty() {
        frames.push(BleFrame::new(shard_id, 0, 1, Vec::new()));
    }

    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_and_reassembles() {
        let shard_id = [7u8; 32];
        let payload = vec![42u8; 512];
        let frames = split_into_frames(shard_id, &payload, 64);
        assert!(frames.len() > 1);

        let mut assembler = BleAssembler::default();
        let mut out = None;
        for frame in frames {
            let maybe = assembler.ingest(frame);
            if maybe.is_some() {
                out = maybe;
            }
        }
        assert_eq!(out.unwrap(), payload);
    }
}
