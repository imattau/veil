use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::api::ContactBundle;
use crate::discovery::{DiscoveryStateHandle, DiscoveryTable};

pub(super) const MAX_CONTACTS_TOTAL: usize = 2048;

#[derive(Debug)]
pub(super) struct ContactBook {
    contacts: Vec<ContactBundle>,
    discovery: DiscoveryStateHandle,
}

impl ContactBook {
    pub(super) fn from_contacts(mut contacts: Vec<ContactBundle>) -> Self {
        trim_contacts(&mut contacts);
        let discovery = discovery_handle_from_contacts(&contacts);
        Self {
            contacts,
            discovery,
        }
    }

    pub(super) fn contacts(&self) -> Vec<ContactBundle> {
        self.contacts.clone()
    }

    pub(super) fn add_or_merge(&mut self, contact: ContactBundle) {
        let upsert_contact = if let Some(existing) = self
            .contacts
            .iter_mut()
            .find(|existing| existing.peer_id == contact.peer_id)
        {
            let identity_matches =
                existing.pubkey_hex.is_empty() || existing.pubkey_hex == contact.pubkey_hex;
            if identity_matches {
                if let Some(ws_url) = contact.ws_url.clone() {
                    existing.ws_url = Some(ws_url);
                }
                if let Some(quic_addr) = contact.quic_addr.clone() {
                    existing.quic_addr = Some(quic_addr);
                }
                if let Some(rpc_url) = contact.rpc_url.clone() {
                    existing.rpc_url = Some(rpc_url);
                }
                if existing.pubkey_hex.is_empty() {
                    existing.pubkey_hex = contact.pubkey_hex.clone();
                }
                let mut seen_lan: HashSet<String> = existing.lan_addrs.iter().cloned().collect();
                for addr in &contact.lan_addrs {
                    if seen_lan.insert(addr.clone()) {
                        existing.lan_addrs.push(addr.clone());
                    }
                }
            }
            existing.clone()
        } else {
            self.contacts.push(contact.clone());
            contact
        };
        self.discovery.upsert(upsert_contact);
        self.enforce_limit();
    }

    pub(super) fn set(&mut self, contact: ContactBundle) {
        if let Some(existing) = self
            .contacts
            .iter_mut()
            .find(|existing| existing.peer_id == contact.peer_id)
        {
            *existing = contact.clone();
        } else {
            self.contacts.push(contact.clone());
        }
        self.discovery.upsert(contact);
        self.enforce_limit();
    }

    pub(super) fn remove(&mut self, peer_id: &str) -> bool {
        let before = self.contacts.len();
        self.contacts.retain(|contact| contact.peer_id != peer_id);
        let removed = self.contacts.len() != before;
        if removed {
            self.discovery = discovery_handle_from_contacts(&self.contacts);
        }
        removed
    }

    pub(super) fn lookup_peer(&self, peer_id: &str, limit: usize) -> Vec<ContactBundle> {
        self.discovery.lookup_peer(peer_id, limit)
    }

    pub(super) fn lookup_pubkey(&self, pubkey_hex: &str, limit: usize) -> Vec<ContactBundle> {
        self.discovery.lookup_pubkey(pubkey_hex, limit)
    }

    pub(super) fn lookup_contact(
        &self,
        contact: &ContactBundle,
        limit: usize,
    ) -> Vec<ContactBundle> {
        self.discovery.lookup_contact(contact, limit)
    }

    pub(super) fn sample(&self, max: usize) -> Vec<ContactBundle> {
        self.discovery.sample(max)
    }

    fn enforce_limit(&mut self) {
        if self.contacts.len() <= MAX_CONTACTS_TOTAL {
            return;
        }
        trim_contacts(&mut self.contacts);
        self.discovery = discovery_handle_from_contacts(&self.contacts);
    }
}

fn trim_contacts(contacts: &mut Vec<ContactBundle>) {
    if contacts.len() <= MAX_CONTACTS_TOTAL {
        return;
    }
    let overflow = contacts.len().saturating_sub(MAX_CONTACTS_TOTAL);
    contacts.drain(0..overflow);
}

fn discovery_handle_from_contacts(contacts: &[ContactBundle]) -> DiscoveryStateHandle {
    let table = Arc::new(Mutex::new(DiscoveryTable::default()));
    {
        let mut guard = table.lock().expect("discovery lock");
        for contact in contacts {
            guard.upsert(contact.clone());
        }
    }
    DiscoveryStateHandle::new(table)
}
