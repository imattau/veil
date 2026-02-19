use super::*;

#[test]
fn endorsement_payload_updates_policy() {
    let state = NodeState::new("0.1-test");
    let endorsement =
        veil_schema_feed::FeedBundle::Endorsement(veil_schema_feed::EndorsementBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_060,
            },
            channel_id: "general".to_string(),
            endorser_pubkey_hex: "aa".repeat(32),
            publisher_pubkey_hex: "bb".repeat(32),
            at_step: 10,
        });
    let payload = serde_json::to_vec(&endorsement).expect("encode");
    assert!(state.ingest_endorsement_payload(&payload, 10));
    let summary = state.policy_summary();
    assert_eq!(summary.endorsements, 1);
}
