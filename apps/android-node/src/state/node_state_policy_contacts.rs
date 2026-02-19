use veil_node::policy::{LocalWotPolicy, WotConfig, WotSummary};

use super::contact_book::ContactBook;
use super::contact_mutations::{execute_contact_mutation, ContactMutation};
use super::policy_lists::export_policy_lists;
use super::policy_mutations::{apply_pubkey_mutation, PolicyPubkeyMutation};
use super::NodeState;
use crate::api::ContactBundle;

impl NodeState {
    pub fn policy_summary(&self) -> WotSummary {
        let inner = self.inner.lock().expect("state lock");
        inner.wot_policy.summary()
    }

    pub fn policy_config(&self) -> WotConfig {
        let inner = self.inner.lock().expect("state lock");
        inner.wot_policy.config
    }

    pub fn update_policy_config(&self, config: WotConfig) {
        self.mutate_policy(|policy| policy.update_config(config));
    }

    pub fn trust_pubkey(&self, pubkey: [u8; 32]) {
        self.apply_policy_pubkey_mutation(PolicyPubkeyMutation::Trust, pubkey);
    }

    pub fn untrust_pubkey(&self, pubkey: [u8; 32]) {
        self.apply_policy_pubkey_mutation(PolicyPubkeyMutation::Untrust, pubkey);
    }

    pub fn mute_pubkey(&self, pubkey: [u8; 32]) {
        self.apply_policy_pubkey_mutation(PolicyPubkeyMutation::Mute, pubkey);
    }

    pub fn unmute_pubkey(&self, pubkey: [u8; 32]) {
        self.apply_policy_pubkey_mutation(PolicyPubkeyMutation::Unmute, pubkey);
    }

    pub fn block_pubkey(&self, pubkey: [u8; 32]) {
        self.apply_policy_pubkey_mutation(PolicyPubkeyMutation::Block, pubkey);
    }

    pub fn unblock_pubkey(&self, pubkey: [u8; 32]) {
        self.apply_policy_pubkey_mutation(PolicyPubkeyMutation::Unblock, pubkey);
    }

    pub fn wot_policy(&self) -> LocalWotPolicy {
        let inner = self.inner.lock().expect("state lock");
        inner.wot_policy.clone()
    }

    pub fn policy_lists(&self) -> crate::api::PolicyListsResponse {
        let inner = self.inner.lock().expect("state lock");
        export_policy_lists(&inner.wot_policy)
    }

    pub fn contacts(&self) -> Vec<ContactBundle> {
        let inner = self.inner.lock().expect("state lock");
        inner.contact_book.contacts()
    }

    pub fn add_contact(&self, contact: ContactBundle) {
        let _ = self.apply_contact_mutation(ContactMutation::AddOrMerge(contact));
    }

    pub fn set_contact(&self, contact: ContactBundle) {
        let _ = self.apply_contact_mutation(ContactMutation::Set(contact));
    }

    pub fn remove_contact(&self, peer_id: &str) -> bool {
        self.apply_contact_mutation(ContactMutation::RemoveByPeerId(peer_id.to_string()))
    }

    pub fn discovery_lookup_peer(&self, peer_id: &str, limit: usize) -> Vec<ContactBundle> {
        let inner = self.inner.lock().expect("state lock");
        inner.contact_book.lookup_peer(peer_id, limit)
    }

    pub fn discovery_lookup_pubkey(&self, pubkey_hex: &str, limit: usize) -> Vec<ContactBundle> {
        let inner = self.inner.lock().expect("state lock");
        inner.contact_book.lookup_pubkey(pubkey_hex, limit)
    }

    pub fn discovery_lookup_contact(
        &self,
        contact: &ContactBundle,
        limit: usize,
    ) -> Vec<ContactBundle> {
        let inner = self.inner.lock().expect("state lock");
        inner.contact_book.lookup_contact(contact, limit)
    }

    pub fn discovery_sample(&self, max: usize) -> Vec<ContactBundle> {
        let inner = self.inner.lock().expect("state lock");
        inner.contact_book.sample(max)
    }

    fn mutate_policy(&self, mutate: impl FnOnce(&mut LocalWotPolicy)) {
        let mut inner = self.inner.lock().expect("state lock");
        mutate(&mut inner.wot_policy);
        self.persist_policy_locked(&mut inner);
    }

    fn apply_policy_pubkey_mutation(&self, mutation: PolicyPubkeyMutation, pubkey: [u8; 32]) {
        self.mutate_policy(|policy| apply_pubkey_mutation(policy, mutation, pubkey));
    }

    fn mutate_contacts(&self, mutate: impl FnOnce(&mut ContactBook) -> bool) -> bool {
        let mut inner = self.inner.lock().expect("state lock");
        let changed = mutate(&mut inner.contact_book);
        if changed {
            self.persist_policy_locked(&mut inner);
        }
        changed
    }

    fn apply_contact_mutation(&self, mutation: ContactMutation) -> bool {
        self.mutate_contacts(|book| execute_contact_mutation(book, mutation))
    }
}
