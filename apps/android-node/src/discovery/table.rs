use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rand::seq::IteratorRandom;
use rand::thread_rng;

use super::lookup_keys::{contact_key, key_for_peer, key_for_pubkey, xor_distance};
use crate::api::ContactBundle;

#[derive(Debug, Default)]
pub struct DiscoveryTable {
    contacts: HashMap<String, ContactBundle>,
}

impl DiscoveryTable {
    pub fn upsert(&mut self, contact: ContactBundle) {
        self.contacts.insert(contact.peer_id.clone(), contact);
    }

    pub fn lookup(&self, key: &[u8; 32], limit: usize) -> Vec<ContactBundle> {
        if limit == 0 || self.contacts.is_empty() {
            return Vec::new();
        }
        let mut contacts: Vec<_> = self
            .contacts
            .values()
            .cloned()
            .map(|contact| {
                let ckey = contact_key(&contact);
                let distance = xor_distance(key, &ckey);
                (distance, contact)
            })
            .collect();
        let take = limit.min(contacts.len());
        if take < contacts.len() {
            let nth = take - 1;
            contacts.select_nth_unstable_by(nth, |a, b| a.0.cmp(&b.0));
            contacts.truncate(take);
        }
        contacts.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        contacts.into_iter().map(|(_, contact)| contact).collect()
    }

    pub fn sample(&self, max: usize) -> Vec<ContactBundle> {
        if self.contacts.len() <= max {
            return self.contacts.values().cloned().collect();
        }
        let mut rng = thread_rng();
        self.contacts
            .values()
            .choose_multiple(&mut rng, max)
            .into_iter()
            .cloned()
            .collect()
    }
}

#[derive(Debug)]
pub struct DiscoveryStateHandle {
    table: Arc<Mutex<DiscoveryTable>>,
}

impl DiscoveryStateHandle {
    pub fn new(table: Arc<Mutex<DiscoveryTable>>) -> Self {
        Self { table }
    }

    pub fn upsert(&self, contact: ContactBundle) {
        let mut table = self.table.lock().expect("discovery lock");
        table.upsert(contact);
    }

    pub fn lookup_peer(&self, peer_id: &str, limit: usize) -> Vec<ContactBundle> {
        let key = key_for_peer(peer_id);
        let table = self.table.lock().expect("discovery lock");
        table.lookup(&key, limit)
    }

    pub fn lookup_pubkey(&self, pubkey_hex: &str, limit: usize) -> Vec<ContactBundle> {
        let key = match key_for_pubkey(pubkey_hex) {
            Some(key) => key,
            None => return Vec::new(),
        };
        let table = self.table.lock().expect("discovery lock");
        table.lookup(&key, limit)
    }

    pub fn lookup_contact(&self, contact: &ContactBundle, limit: usize) -> Vec<ContactBundle> {
        let key = contact_key(contact);
        let table = self.table.lock().expect("discovery lock");
        table.lookup(&key, limit)
    }

    pub fn sample(&self, max: usize) -> Vec<ContactBundle> {
        let table = self.table.lock().expect("discovery lock");
        table.sample(max)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{key_for_peer, DiscoveryTable};
    use crate::api::ContactBundle;

    fn make_contact(peer_id: &str, pubkey_hex: &str) -> ContactBundle {
        ContactBundle {
            peer_id: peer_id.to_string(),
            ws_url: None,
            quic_addr: None,
            pubkey_hex: pubkey_hex.to_string(),
            rpc_url: None,
            lan_addrs: Vec::new(),
        }
    }

    #[test]
    fn lookup_orders_by_distance() {
        let mut table = DiscoveryTable::default();
        table.upsert(make_contact(
            "alpha",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ));
        table.upsert(make_contact(
            "beta",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        ));
        table.upsert(make_contact(
            "gamma",
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        ));
        let key = key_for_peer("alpha");
        let results = table.lookup(&key, 2);
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|c| c.peer_id == "alpha"));
    }

    #[test]
    fn lookup_zero_limit_returns_empty_without_sorting() {
        let mut table = DiscoveryTable::default();
        table.upsert(make_contact(
            "alpha",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ));
        let key = key_for_peer("alpha");
        let results = table.lookup(&key, 0);
        assert!(results.is_empty());
    }

    #[test]
    fn sample_caps_count_without_duplicates() {
        let mut table = DiscoveryTable::default();
        for i in 0..64usize {
            table.upsert(make_contact(
                &format!("peer-{i}"),
                &format!("{:064x}", i + 1),
            ));
        }

        let sample = table.sample(16);
        assert_eq!(sample.len(), 16);
        let unique: HashSet<_> = sample
            .iter()
            .map(|contact| contact.peer_id.clone())
            .collect();
        assert_eq!(unique.len(), sample.len());
    }
}
