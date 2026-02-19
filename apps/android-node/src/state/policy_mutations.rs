use veil_node::policy::LocalWotPolicy;

#[derive(Debug, Clone, Copy)]
pub(super) enum PolicyPubkeyMutation {
    Trust,
    Untrust,
    Mute,
    Unmute,
    Block,
    Unblock,
}

pub(super) fn apply_pubkey_mutation(
    policy: &mut LocalWotPolicy,
    mutation: PolicyPubkeyMutation,
    pubkey: [u8; 32],
) {
    match mutation {
        PolicyPubkeyMutation::Trust => policy.trust(pubkey),
        PolicyPubkeyMutation::Untrust => policy.untrust(pubkey),
        PolicyPubkeyMutation::Mute => policy.mute(pubkey),
        PolicyPubkeyMutation::Unmute => policy.unmute(pubkey),
        PolicyPubkeyMutation::Block => policy.block(pubkey),
        PolicyPubkeyMutation::Unblock => policy.unblock(pubkey),
    }
}
