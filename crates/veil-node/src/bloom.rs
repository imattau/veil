use serde::{Deserialize, Serialize};
use veil_core::hash::blake3_32;
use veil_core::ShardId;

const BLOOM_EXCHANGE_V1: u16 = 1;
const BLOOM_PACKET_MAGIC: &[u8] = b"VEIL_BLOOM_V1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BloomFilter {
    pub bit_len: usize,
    pub hash_count: u8,
    pub salt: [u8; 16],
    pub bits: Vec<u8>,
}

impl BloomFilter {
    pub fn new(bit_len: usize, hash_count: u8, salt: [u8; 16]) -> Self {
        let bit_len = bit_len.max(8);
        let byte_len = bit_len.div_ceil(8);
        Self {
            bit_len,
            hash_count: hash_count.max(1),
            salt,
            bits: vec![0; byte_len],
        }
    }

    pub fn recommended(expected_items: usize, false_positive_rate: f64, salt: [u8; 16]) -> Self {
        let n = expected_items.max(1) as f64;
        let p = false_positive_rate.clamp(0.000_1, 0.999_9);
        let ln2 = std::f64::consts::LN_2;
        let m = (-(n * p.ln()) / (ln2 * ln2)).ceil() as usize;
        let k = ((m as f64 / n) * ln2).round().clamp(1.0, 16.0) as u8;
        Self::new(m.max(256), k, salt)
    }

    pub fn insert(&mut self, item: &ShardId) {
        let indices: Vec<usize> = self.bit_indices(item).collect();
        for idx in indices {
            let byte = idx / 8;
            let bit = (idx % 8) as u8;
            self.bits[byte] |= 1 << bit;
        }
    }

    pub fn might_contain(&self, item: &ShardId) -> bool {
        self.bit_indices(item).all(|idx| {
            let byte = idx / 8;
            let bit = (idx % 8) as u8;
            (self.bits[byte] & (1 << bit)) != 0
        })
    }

    fn bit_indices<'a>(&'a self, item: &'a ShardId) -> impl Iterator<Item = usize> + 'a {
        (0..self.hash_count).map(|round| {
            let mut preimage = Vec::with_capacity(7 + self.salt.len() + item.len() + 1);
            preimage.extend_from_slice(b"bloom-v1");
            preimage.extend_from_slice(&self.salt);
            preimage.extend_from_slice(item);
            preimage.push(round);
            let h = blake3_32(&preimage);
            let mixed = u64::from_be_bytes([h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]]);
            (mixed as usize) % self.bit_len
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BloomExchangeMessage {
    pub version: u16,
    pub epoch: u32,
    pub filter: BloomFilter,
}

pub fn encode_bloom_exchange_cbor(
    epoch: u32,
    filter: BloomFilter,
) -> Result<Vec<u8>, serde_cbor::Error> {
    serde_cbor::to_vec(&BloomExchangeMessage {
        version: BLOOM_EXCHANGE_V1,
        epoch,
        filter,
    })
}

pub fn decode_bloom_exchange_cbor(bytes: &[u8]) -> Result<BloomExchangeMessage, serde_cbor::Error> {
    let msg: BloomExchangeMessage = serde_cbor::from_slice(bytes)?;
    Ok(msg)
}

pub fn encode_bloom_exchange_packet(
    epoch: u32,
    filter: BloomFilter,
) -> Result<Vec<u8>, serde_cbor::Error> {
    let payload = encode_bloom_exchange_cbor(epoch, filter)?;
    let mut out = Vec::with_capacity(BLOOM_PACKET_MAGIC.len() + payload.len());
    out.extend_from_slice(BLOOM_PACKET_MAGIC);
    out.extend_from_slice(&payload);
    Ok(out)
}

pub fn decode_bloom_exchange_packet(bytes: &[u8]) -> Option<BloomExchangeMessage> {
    if bytes.len() <= BLOOM_PACKET_MAGIC.len() {
        return None;
    }
    if &bytes[..BLOOM_PACKET_MAGIC.len()] != BLOOM_PACKET_MAGIC {
        return None;
    }
    decode_bloom_exchange_cbor(&bytes[BLOOM_PACKET_MAGIC.len()..]).ok()
}

pub fn missing_against_filter(
    local_shards: impl IntoIterator<Item = ShardId>,
    remote_filter: &BloomFilter,
) -> Vec<ShardId> {
    local_shards
        .into_iter()
        .filter(|sid| !remote_filter.might_contain(sid))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        decode_bloom_exchange_cbor, decode_bloom_exchange_packet, encode_bloom_exchange_cbor,
        encode_bloom_exchange_packet, missing_against_filter, BloomFilter,
    };

    #[test]
    fn bloom_insert_and_query_work() {
        let mut bf = BloomFilter::recommended(128, 0.05, [0x11; 16]);
        let a = [0xAA; 32];
        let b = [0xBB; 32];
        bf.insert(&a);
        assert!(bf.might_contain(&a));
        assert!(!bf.might_contain(&b));
    }

    #[test]
    fn bloom_exchange_round_trip() {
        let mut bf = BloomFilter::recommended(64, 0.1, [0x22; 16]);
        bf.insert(&[0x10; 32]);
        let bytes = encode_bloom_exchange_cbor(42, bf.clone()).expect("encode");
        let decoded = decode_bloom_exchange_cbor(&bytes).expect("decode");
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.epoch, 42);
        assert_eq!(decoded.filter, bf);
    }

    #[test]
    fn bloom_packet_round_trip() {
        let bf = BloomFilter::recommended(64, 0.1, [0x42; 16]);
        let packet = encode_bloom_exchange_packet(7, bf).expect("encode packet");
        let decoded = decode_bloom_exchange_packet(&packet).expect("decode packet");
        assert_eq!(decoded.epoch, 7);
    }

    #[test]
    fn missing_against_filter_filters_known_ids() {
        let known = [0x01; 32];
        let missing = [0x02; 32];
        let mut bf = BloomFilter::recommended(32, 0.05, [0x33; 16]);
        bf.insert(&known);
        let out = missing_against_filter([known, missing], &bf);
        assert_eq!(out, vec![missing]);
    }
}
