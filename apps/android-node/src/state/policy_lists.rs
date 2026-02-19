use crate::api::PolicyListsResponse;
use veil_node::policy::LocalWotPolicy;

#[derive(Debug, serde::Deserialize)]
struct PolicyJsonLists {
    #[serde(default)]
    trusted: Vec<[u8; 32]>,
    #[serde(default)]
    muted: Vec<[u8; 32]>,
    #[serde(default)]
    blocked: Vec<[u8; 32]>,
}

pub(super) fn export_policy_lists(policy: &LocalWotPolicy) -> PolicyListsResponse {
    let json = policy.export_json().ok();
    let Some(json) = json else {
        return PolicyListsResponse::default();
    };
    let parsed = serde_json::from_str::<PolicyJsonLists>(&json).ok();
    let Some(parsed) = parsed else {
        return PolicyListsResponse::default();
    };
    PolicyListsResponse {
        trusted_pubkeys: parsed.trusted.iter().map(hex::encode).collect(),
        muted_pubkeys: parsed.muted.iter().map(hex::encode).collect(),
        blocked_pubkeys: parsed.blocked.iter().map(hex::encode).collect(),
    }
}
