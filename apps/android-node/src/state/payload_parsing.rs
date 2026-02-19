use veil_node::policy::{parse_endorsement_payload, ParsedEndorsement};
use veil_schema_feed::FeedBundle;

pub(super) fn parse_feed_bundles(payload: &[u8]) -> Vec<FeedBundle> {
    if let Ok(bundle) = serde_json::from_slice::<FeedBundle>(payload) {
        return vec![bundle];
    }

    let mut bundles = Vec::new();
    if let Ok(batch) = ciborium::de::from_reader::<Vec<Vec<u8>>, _>(payload) {
        for item in batch {
            if let Ok(bundle) = serde_json::from_slice::<FeedBundle>(&item) {
                bundles.push(bundle);
            }
        }
    }
    bundles
}

pub(super) fn parse_endorsements(payload: &[u8]) -> Vec<ParsedEndorsement> {
    if let Some(parsed) = parse_endorsement_payload(payload) {
        return vec![parsed];
    }

    let mut endorsements = Vec::new();
    if let Ok(batch) = ciborium::de::from_reader::<Vec<Vec<u8>>, _>(payload) {
        for item in batch {
            if let Some(parsed) = parse_endorsement_payload(&item) {
                endorsements.push(parsed);
            }
        }
    }
    endorsements
}
