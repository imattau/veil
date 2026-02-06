pub const BLE_FRAME_HEADER_LEN: usize = 32 + 2 + 2;
pub const BLE_SERVICE_UUID: &str = "4b1d0f6c-3a5e-4c5f-8f65-7a7f0dbf2a90";
pub const BLE_SHARD_CHAR_UUID: &str = "15c06b19-9b4e-4b36-8af6-2f93a7a6fbc0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BleFrameHeader {
    pub shard_id: [u8; 32],
    pub index: u16,
    pub total: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BleFrame {
    pub header: BleFrameHeader,
    pub payload: Vec<u8>,
}

impl BleFrame {
    pub fn new(shard_id: [u8; 32], index: u16, total: u16, payload: Vec<u8>) -> Self {
        Self {
            header: BleFrameHeader {
                shard_id,
                index,
                total,
            },
            payload,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(BLE_FRAME_HEADER_LEN + self.payload.len());
        out.extend_from_slice(&self.header.shard_id);
        out.extend_from_slice(&self.header.index.to_be_bytes());
        out.extend_from_slice(&self.header.total.to_be_bytes());
        out.extend_from_slice(&self.payload);
        out
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < BLE_FRAME_HEADER_LEN {
            return None;
        }
        let mut shard_id = [0u8; 32];
        shard_id.copy_from_slice(&bytes[..32]);
        let index = u16::from_be_bytes([bytes[32], bytes[33]]);
        let total = u16::from_be_bytes([bytes[34], bytes[35]]);
        let payload = bytes[36..].to_vec();
        Some(Self::new(shard_id, index, total, payload))
    }
}
