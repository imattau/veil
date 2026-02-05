use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Local trust tiers used for prioritization decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrustTier {
    Trusted,
    Known,
    Unknown,
    Muted,
    Blocked,
}

/// Structured score details for explainable WoT classification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrustScoreExplanation {
    pub publisher: [u8; 32],
    pub score: f64,
    pub tier: TrustTier,
    pub blocked_override: bool,
    pub trusted_override: bool,
    pub muted_override: bool,
    pub direct_endorser_count: usize,
    pub direct_score: f64,
    pub second_hop_endorser_count: usize,
    pub second_hop_score: f64,
}

/// Metadata used to score shard eviction priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShardMeta {
    pub tier: TrustTier,
    pub replica_estimate: u64,
    pub age_steps: u64,
    pub requested_count: u64,
}

/// Policy interface for WoT-driven prioritization.
pub trait WotPolicy {
    /// Classifies a publisher pubkey into a trust tier.
    fn classify_publisher(&self, pubkey: [u8; 32], now_step: u64) -> TrustTier;
    /// Returns forwarding quota fraction for the given tier.
    fn forwarding_quota(&self, tier: TrustTier) -> f32;
    /// Returns storage budget in shard entries for the given tier.
    fn storage_budget(&self, tier: TrustTier) -> usize;
    /// Returns eviction score (higher means evict earlier).
    fn eviction_priority(&self, meta: ShardMeta) -> f64;
}

/// Tunable parameters for `LocalWotPolicy`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WotConfig {
    pub endorsement_threshold: usize,
    pub max_hops: u8,
    pub age_decay_window_steps: u64,
    pub hop_decay: f64,
    pub known_threshold: f64,
    pub trusted_threshold: f64,
    pub trusted_forward_quota: f32,
    pub known_forward_quota: f32,
    pub unknown_forward_quota: f32,
    pub muted_forward_quota: f32,
    pub blocked_forward_quota: f32,
    pub trusted_storage_budget: usize,
    pub known_storage_budget: usize,
    pub unknown_storage_budget: usize,
    pub muted_storage_budget: usize,
    pub blocked_storage_budget: usize,
}

impl Default for WotConfig {
    fn default() -> Self {
        Self {
            endorsement_threshold: 2,
            max_hops: 2,
            age_decay_window_steps: 10_000,
            hop_decay: 0.45,
            known_threshold: 0.4,
            trusted_threshold: 0.8,
            trusted_forward_quota: 0.70,
            known_forward_quota: 0.25,
            unknown_forward_quota: 0.05,
            muted_forward_quota: 0.01,
            blocked_forward_quota: 0.0,
            trusted_storage_budget: 50_000,
            known_storage_budget: 20_000,
            unknown_storage_budget: 5_000,
            muted_storage_budget: 500,
            blocked_storage_budget: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Endorsement {
    pub publisher: [u8; 32],
    pub at_step: u64,
}

/// Default local WoT policy implementation with bounded transitive scoring.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LocalWotPolicy {
    pub config: WotConfig,
    trusted: HashSet<[u8; 32]>,
    muted: HashSet<[u8; 32]>,
    blocked: HashSet<[u8; 32]>,
    // endorser -> endorsements they issued
    endorsements_by_endorser: HashMap<[u8; 32], Vec<Endorsement>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EndorsementEdge {
    endorser: [u8; 32],
    publisher: [u8; 32],
    at_step: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LocalWotPolicyJson {
    config: WotConfig,
    trusted: Vec<[u8; 32]>,
    muted: Vec<[u8; 32]>,
    blocked: Vec<[u8; 32]>,
    endorsements: Vec<EndorsementEdge>,
}

impl LocalWotPolicy {
    /// Creates a policy instance from explicit config.
    pub fn new(config: WotConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    /// Marks a publisher as explicitly trusted.
    pub fn trust(&mut self, pubkey: [u8; 32]) {
        self.blocked.remove(&pubkey);
        self.muted.remove(&pubkey);
        self.trusted.insert(pubkey);
    }

    /// Marks a publisher as muted.
    pub fn mute(&mut self, pubkey: [u8; 32]) {
        self.muted.insert(pubkey);
    }

    /// Marks a publisher as blocked.
    pub fn block(&mut self, pubkey: [u8; 32]) {
        self.blocked.insert(pubkey);
    }

    /// Adds a directed endorsement edge at `at_step`.
    pub fn add_endorsement(&mut self, endorser: [u8; 32], publisher: [u8; 32], at_step: u64) {
        self.endorsements_by_endorser
            .entry(endorser)
            .or_default()
            .push(Endorsement { publisher, at_step });
    }

    /// Returns deterministic bounded WoT score in `[0.0, 1.0]`.
    pub fn score_publisher(&self, publisher: [u8; 32], now_step: u64) -> f64 {
        if self.blocked.contains(&publisher) {
            return 0.0;
        }
        if self.trusted.contains(&publisher) {
            return 1.0;
        }
        self.bounded_score(publisher, now_step)
    }

    /// Explains score/tier outcome for debugging and UI explainability.
    pub fn explain_publisher(&self, publisher: [u8; 32], now_step: u64) -> TrustScoreExplanation {
        let (direct_score, direct_endorser_count) =
            self.direct_trusted_endorsers_score_with_count(publisher, now_step);
        let (second_hop_score, second_hop_endorser_count) =
            self.second_hop_score_with_count(publisher, now_step);
        let score = if self.blocked.contains(&publisher) {
            0.0
        } else if self.trusted.contains(&publisher) {
            1.0
        } else {
            ((direct_score + second_hop_score) / 3.0).clamp(0.0, 1.0)
        };
        let tier = self.classify_publisher(publisher, now_step);

        TrustScoreExplanation {
            publisher,
            score,
            tier,
            blocked_override: self.blocked.contains(&publisher),
            trusted_override: self.trusted.contains(&publisher),
            muted_override: self.muted.contains(&publisher),
            direct_endorser_count,
            direct_score,
            second_hop_endorser_count,
            second_hop_score,
        }
    }

    /// Exports complete local policy state as JSON.
    pub fn export_json(&self) -> Result<String, serde_json::Error> {
        let endorsements = self
            .endorsements_by_endorser
            .iter()
            .flat_map(|(endorser, edges)| {
                edges.iter().map(|e| EndorsementEdge {
                    endorser: *endorser,
                    publisher: e.publisher,
                    at_step: e.at_step,
                })
            })
            .collect::<Vec<_>>();
        let snapshot = LocalWotPolicyJson {
            config: self.config,
            trusted: self.trusted.iter().copied().collect(),
            muted: self.muted.iter().copied().collect(),
            blocked: self.blocked.iter().copied().collect(),
            endorsements,
        };
        serde_json::to_string_pretty(&snapshot)
    }

    /// Imports complete local policy state from JSON.
    pub fn import_json(json: &str) -> Result<Self, serde_json::Error> {
        let snapshot: LocalWotPolicyJson = serde_json::from_str(json)?;
        let mut endorsements_by_endorser = HashMap::<[u8; 32], Vec<Endorsement>>::new();
        for edge in snapshot.endorsements {
            endorsements_by_endorser
                .entry(edge.endorser)
                .or_default()
                .push(Endorsement {
                    publisher: edge.publisher,
                    at_step: edge.at_step,
                });
        }
        Ok(Self {
            config: snapshot.config,
            trusted: snapshot.trusted.into_iter().collect(),
            muted: snapshot.muted.into_iter().collect(),
            blocked: snapshot.blocked.into_iter().collect(),
            endorsements_by_endorser,
        })
    }

    fn age_weight(&self, at_step: u64, now_step: u64) -> f64 {
        let age = now_step.saturating_sub(at_step) as f64;
        let window = self.config.age_decay_window_steps.max(1) as f64;
        1.0 / (1.0 + (age / window))
    }

    fn direct_trusted_endorsers_score_with_count(
        &self,
        publisher: [u8; 32],
        now_step: u64,
    ) -> (f64, usize) {
        let mut endorsers = HashSet::<[u8; 32]>::new();
        let mut score = 0.0_f64;
        for trusted in &self.trusted {
            if let Some(edges) = self.endorsements_by_endorser.get(trusted) {
                for e in edges {
                    if e.publisher == publisher {
                        if endorsers.insert(*trusted) {
                            score += self.age_weight(e.at_step, now_step);
                        }
                        break;
                    }
                }
            }
        }
        if endorsers.len() < self.config.endorsement_threshold {
            (0.0, endorsers.len())
        } else {
            (score, endorsers.len())
        }
    }

    fn second_hop_score_with_count(&self, publisher: [u8; 32], now_step: u64) -> (f64, usize) {
        if self.config.max_hops < 2 {
            return (0.0, 0);
        }

        let mut second_hop_entities = HashSet::<[u8; 32]>::new();
        for trusted in &self.trusted {
            if let Some(edges) = self.endorsements_by_endorser.get(trusted) {
                for e in edges {
                    second_hop_entities.insert(e.publisher);
                }
            }
        }

        let mut score = 0.0;
        let mut endorsers = HashSet::<[u8; 32]>::new();
        for endorser in second_hop_entities {
            if let Some(edges) = self.endorsements_by_endorser.get(&endorser) {
                for e in edges {
                    if e.publisher == publisher {
                        if endorsers.insert(endorser) {
                            score += self.age_weight(e.at_step, now_step) * self.config.hop_decay;
                        }
                        break;
                    }
                }
            }
        }
        if endorsers.len() < self.config.endorsement_threshold {
            (0.0, endorsers.len())
        } else {
            (score, endorsers.len())
        }
    }

    fn bounded_score(&self, publisher: [u8; 32], now_step: u64) -> f64 {
        let (direct, _) = self.direct_trusted_endorsers_score_with_count(publisher, now_step);
        let (second_hop, _) = self.second_hop_score_with_count(publisher, now_step);
        let base = direct + second_hop;
        (base / 3.0).clamp(0.0, 1.0)
    }
}

impl WotPolicy for LocalWotPolicy {
    fn classify_publisher(&self, pubkey: [u8; 32], now_step: u64) -> TrustTier {
        if self.blocked.contains(&pubkey) {
            return TrustTier::Blocked;
        }
        if self.trusted.contains(&pubkey) {
            return TrustTier::Trusted;
        }
        if self.muted.contains(&pubkey) {
            return TrustTier::Muted;
        }

        let score = self.score_publisher(pubkey, now_step);
        if score >= self.config.trusted_threshold {
            TrustTier::Trusted
        } else if score >= self.config.known_threshold {
            TrustTier::Known
        } else {
            TrustTier::Unknown
        }
    }

    fn forwarding_quota(&self, tier: TrustTier) -> f32 {
        match tier {
            TrustTier::Trusted => self.config.trusted_forward_quota,
            TrustTier::Known => self.config.known_forward_quota,
            TrustTier::Unknown => self.config.unknown_forward_quota,
            TrustTier::Muted => self.config.muted_forward_quota,
            TrustTier::Blocked => self.config.blocked_forward_quota,
        }
    }

    fn storage_budget(&self, tier: TrustTier) -> usize {
        match tier {
            TrustTier::Trusted => self.config.trusted_storage_budget,
            TrustTier::Known => self.config.known_storage_budget,
            TrustTier::Unknown => self.config.unknown_storage_budget,
            TrustTier::Muted => self.config.muted_storage_budget,
            TrustTier::Blocked => self.config.blocked_storage_budget,
        }
    }

    fn eviction_priority(&self, meta: ShardMeta) -> f64 {
        let trust_factor = match meta.tier {
            TrustTier::Trusted => 0.0,
            TrustTier::Known => 0.35,
            TrustTier::Unknown => 0.7,
            TrustTier::Muted => 0.95,
            TrustTier::Blocked => 1.0,
        };
        let replica_factor = (meta.replica_estimate as f64 / 12.0).clamp(0.0, 1.0);
        let age_factor = (meta.age_steps as f64 / 20_000.0).clamp(0.0, 1.0);
        let request_bonus = (meta.requested_count as f64 / 16.0).clamp(0.0, 0.6);

        let score = 0.5 * trust_factor + 0.4 * replica_factor + 0.1 * age_factor - request_bonus;
        score.clamp(0.0, 1.0)
    }
}

pub fn fanout_for_tier(base_fanout: usize, tier: TrustTier, policy: &impl WotPolicy) -> usize {
    let quota = policy.forwarding_quota(tier).clamp(0.0, 1.0);
    (base_fanout as f32 * quota).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::{fanout_for_tier, LocalWotPolicy, ShardMeta, TrustTier, WotPolicy};

    #[test]
    fn explicit_overrides_dominate() {
        let mut policy = LocalWotPolicy::default();
        let a = [0xAA_u8; 32];
        policy.trust(a);
        assert_eq!(policy.classify_publisher(a, 100), TrustTier::Trusted);

        policy.block(a);
        assert_eq!(policy.classify_publisher(a, 100), TrustTier::Blocked);
    }

    #[test]
    fn bounded_wot_classifies_known_with_thresholded_endorsements() {
        let mut policy = LocalWotPolicy::default();
        let t1 = [0x01_u8; 32];
        let t2 = [0x02_u8; 32];
        let p = [0x99_u8; 32];
        policy.trust(t1);
        policy.trust(t2);
        policy.add_endorsement(t1, p, 95);
        policy.add_endorsement(t2, p, 96);

        let tier = policy.classify_publisher(p, 100);
        assert!(matches!(tier, TrustTier::Known | TrustTier::Trusted));
    }

    #[test]
    fn fanout_respects_quota() {
        let policy = LocalWotPolicy::default();
        assert_eq!(fanout_for_tier(10, TrustTier::Trusted, &policy), 7);
        assert_eq!(fanout_for_tier(10, TrustTier::Known, &policy), 3);
        assert_eq!(fanout_for_tier(10, TrustTier::Unknown, &policy), 1);
        assert_eq!(fanout_for_tier(10, TrustTier::Blocked, &policy), 0);
    }

    #[test]
    fn eviction_prefers_evicting_common_low_trust_first() {
        let policy = LocalWotPolicy::default();
        let rare_trusted = ShardMeta {
            tier: TrustTier::Trusted,
            replica_estimate: 1,
            age_steps: 1_000,
            requested_count: 3,
        };
        let common_unknown = ShardMeta {
            tier: TrustTier::Unknown,
            replica_estimate: 20,
            age_steps: 1_000,
            requested_count: 0,
        };
        assert!(policy.eviction_priority(common_unknown) > policy.eviction_priority(rare_trusted));
    }

    #[test]
    fn score_is_bounded_and_explainable() {
        let mut policy = LocalWotPolicy::default();
        let trusted_a = [0xA1; 32];
        let trusted_b = [0xB2; 32];
        let target = [0xCC; 32];
        policy.trust(trusted_a);
        policy.trust(trusted_b);
        policy.add_endorsement(trusted_a, target, 95);
        policy.add_endorsement(trusted_b, target, 96);

        let score = policy.score_publisher(target, 100);
        assert!((0.0..=1.0).contains(&score));

        let explanation = policy.explain_publisher(target, 100);
        assert_eq!(explanation.publisher, target);
        assert_eq!(explanation.score, score);
        assert!(explanation.direct_endorser_count >= 2);
        assert!(!explanation.blocked_override);
    }

    #[test]
    fn export_import_json_round_trip() {
        let mut policy = LocalWotPolicy::default();
        let trusted = [0x11; 32];
        let target = [0x22; 32];
        policy.trust(trusted);
        policy.add_endorsement(trusted, target, 10);

        let json = policy.export_json().expect("export should succeed");
        let imported = LocalWotPolicy::import_json(&json).expect("import should succeed");

        assert_eq!(imported.classify_publisher(trusted, 20), TrustTier::Trusted);
        assert!(imported.score_publisher(target, 20) >= 0.0);
    }
}
