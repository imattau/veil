use crate::api::ContactBundle;

use super::contact_book::ContactBook;

#[derive(Debug, Clone)]
pub(super) enum ContactMutation {
    AddOrMerge(ContactBundle),
    Set(ContactBundle),
    RemoveByPeerId(String),
}

pub(super) fn execute_contact_mutation(book: &mut ContactBook, mutation: ContactMutation) -> bool {
    match mutation {
        ContactMutation::AddOrMerge(contact) => {
            book.add_or_merge(contact);
            true
        }
        ContactMutation::Set(contact) => {
            book.set(contact);
            true
        }
        ContactMutation::RemoveByPeerId(peer_id) => book.remove(&peer_id),
    }
}
