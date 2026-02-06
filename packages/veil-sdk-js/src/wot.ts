export type TrustTier = "trusted" | "known" | "unknown" | "muted" | "blocked";

export type PubkeyHex = string & { readonly __brand: "PubkeyHex" };

export interface WotConfig {
  endorsementThreshold: number;
  maxHops: number;
  ageDecayWindowSteps: number;
  hopDecay: number;
  knownThreshold: number;
  trustedThreshold: number;
  tierWeights: Record<TrustTier, number>;
}

export interface TrustScoreExplanation {
  publisher: string;
  score: number;
  tier: TrustTier;
  blockedOverride: boolean;
  trustedOverride: boolean;
  mutedOverride: boolean;
  directEndorserCount: number;
  directScore: number;
  secondHopEndorserCount: number;
  secondHopScore: number;
}

export interface Endorsement {
  publisher: string;
  atStep: number;
}

export interface EndorsementRecord {
  endorser: string;
  publisher: string;
  atStep: number;
}

interface WotSnapshot {
  version: number;
  config: WotConfig;
  trusted: string[];
  muted: string[];
  blocked: string[];
  endorsements: Array<{ endorser: string; publisher: string; atStep: number }>;
}

const PUBKEY_HEX_LEN = 64;

function normalizePubkeyHex(pubkeyHex: string): PubkeyHex {
  const value = pubkeyHex.trim().toLowerCase();
  if (!/^[0-9a-f]+$/.test(value) || value.length !== PUBKEY_HEX_LEN) {
    throw new Error("publisher pubkey must be a 32-byte hex string");
  }
  return value as PubkeyHex;
}

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

export function defaultWotConfig(): WotConfig {
  return {
    endorsementThreshold: 2,
    maxHops: 2,
    ageDecayWindowSteps: 10_000,
    hopDecay: 0.45,
    knownThreshold: 0.4,
    trustedThreshold: 0.8,
    tierWeights: {
      trusted: 4,
      known: 3,
      unknown: 2,
      muted: 1,
      blocked: 0,
    },
  };
}

export class LocalWotPolicy {
  readonly config: WotConfig;
  private readonly trusted = new Set<PubkeyHex>();
  private readonly muted = new Set<PubkeyHex>();
  private readonly blocked = new Set<PubkeyHex>();
  private readonly endorsementsByEndorser = new Map<
    PubkeyHex,
    Map<PubkeyHex, Endorsement>
  >();

  constructor(config: Partial<WotConfig> = {}) {
    this.config = LocalWotPolicy.validateConfig({
      ...defaultWotConfig(),
      ...config,
    });
  }

  private static validateConfig(config: WotConfig): WotConfig {
    if (!Number.isFinite(config.endorsementThreshold) || config.endorsementThreshold < 1) {
      throw new Error("endorsementThreshold must be >= 1");
    }
    if (!Number.isInteger(config.maxHops) || config.maxHops < 0) {
      throw new Error("maxHops must be a non-negative integer");
    }
    if (!Number.isFinite(config.ageDecayWindowSteps) || config.ageDecayWindowSteps <= 0) {
      throw new Error("ageDecayWindowSteps must be > 0");
    }
    if (!Number.isFinite(config.hopDecay) || config.hopDecay < 0 || config.hopDecay > 1) {
      throw new Error("hopDecay must be between 0 and 1");
    }
    if (!Number.isFinite(config.knownThreshold) || config.knownThreshold < 0) {
      throw new Error("knownThreshold must be >= 0");
    }
    if (!Number.isFinite(config.trustedThreshold) || config.trustedThreshold < 0) {
      throw new Error("trustedThreshold must be >= 0");
    }
    return config;
  }

  trust(pubkeyHex: string): void {
    const pubkey = normalizePubkeyHex(pubkeyHex);
    this.blocked.delete(pubkey);
    this.muted.delete(pubkey);
    this.trusted.add(pubkey);
  }

  mute(pubkeyHex: string): void {
    this.muted.add(normalizePubkeyHex(pubkeyHex));
  }

  block(pubkeyHex: string): void {
    this.blocked.add(normalizePubkeyHex(pubkeyHex));
  }

  addEndorsement(endorserHex: string, publisherHex: string, atStep: number): void {
    const endorser = normalizePubkeyHex(endorserHex);
    const publisher = normalizePubkeyHex(publisherHex);
    const edges = this.endorsementsByEndorser.get(endorser) ?? new Map();
    const existing = edges.get(publisher);
    if (!existing || existing.atStep < atStep) {
      edges.set(publisher, { publisher, atStep });
      this.endorsementsByEndorser.set(endorser, edges);
    }
  }

  ingestEndorsement(record: EndorsementRecord): void {
    this.addEndorsement(record.endorser, record.publisher, record.atStep);
  }

  ingestEndorsements(records: EndorsementRecord[]): void {
    for (const record of records) {
      this.ingestEndorsement(record);
    }
  }

  pruneStaleEndorsements(nowStep: number, maxAgeSteps?: number): void {
    const maxAge =
      maxAgeSteps ?? Math.max(1, this.config.ageDecayWindowSteps) * 4;
    for (const [endorser, edges] of this.endorsementsByEndorser.entries()) {
      for (const [publisher, edge] of edges.entries()) {
        if (nowStep - edge.atStep > maxAge) {
          edges.delete(publisher);
        }
      }
      if (edges.size === 0) {
        this.endorsementsByEndorser.delete(endorser);
      }
    }
  }

  scorePublisher(pubkeyHex: string, nowStep: number): number {
    const publisher = normalizePubkeyHex(pubkeyHex);
    if (this.blocked.has(publisher)) {
      return 0;
    }
    if (this.trusted.has(publisher)) {
      return 1;
    }
    return this.computeScoreComponents(publisher, nowStep).score;
  }

  classifyPublisher(pubkeyHex: string, nowStep: number): TrustTier {
    const publisher = normalizePubkeyHex(pubkeyHex);
    if (this.blocked.has(publisher)) {
      return "blocked";
    }
    if (this.trusted.has(publisher)) {
      return "trusted";
    }
    if (this.muted.has(publisher)) {
      return "muted";
    }
    const score = this.computeScoreComponents(publisher, nowStep).score;
    return this.classifyWithScore(score);
  }

  explainPublisher(pubkeyHex: string, nowStep: number): TrustScoreExplanation {
    const publisher = normalizePubkeyHex(pubkeyHex);
    const components = this.computeScoreComponents(publisher, nowStep);
    const score = this.blocked.has(publisher)
      ? 0
      : this.trusted.has(publisher)
        ? 1
        : components.score;
    const tier = this.blocked.has(publisher)
      ? "blocked"
      : this.trusted.has(publisher)
        ? "trusted"
        : this.muted.has(publisher)
          ? "muted"
          : this.classifyWithScore(score);

    return {
      publisher,
      score,
      tier,
      blockedOverride: this.blocked.has(publisher),
      trustedOverride: this.trusted.has(publisher),
      mutedOverride: this.muted.has(publisher),
      directEndorserCount: components.directEndorserCount,
      directScore: components.directScore,
      secondHopEndorserCount: components.secondHopEndorserCount,
      secondHopScore: components.secondHopScore,
    };
  }

  exportJson(): string {
    const endorsements: WotSnapshot["endorsements"] = [];
    for (const [endorser, edges] of this.endorsementsByEndorser.entries()) {
      for (const edge of edges.values()) {
        endorsements.push({
          endorser,
          publisher: edge.publisher,
          atStep: edge.atStep,
        });
      }
    }
    return JSON.stringify(
      {
        version: 1,
        config: this.config,
        trusted: Array.from(this.trusted),
        muted: Array.from(this.muted),
        blocked: Array.from(this.blocked),
        endorsements,
      } satisfies WotSnapshot,
      null,
      2,
    );
  }

  static importJson(json: string): LocalWotPolicy {
    const raw = JSON.parse(json) as Partial<WotSnapshot>;
    const policy = new LocalWotPolicy(raw.config ?? {});

    for (const pubkey of raw.trusted ?? []) {
      policy.trust(pubkey);
    }
    for (const pubkey of raw.muted ?? []) {
      policy.mute(pubkey);
    }
    for (const pubkey of raw.blocked ?? []) {
      policy.block(pubkey);
    }
    for (const edge of raw.endorsements ?? []) {
      policy.addEndorsement(edge.endorser, edge.publisher, edge.atStep);
    }
    return policy;
  }

  private ageWeight(atStep: number, nowStep: number): number {
    const age = Math.max(0, nowStep - atStep);
    const window = Math.max(1, this.config.ageDecayWindowSteps);
    return 1 / (1 + age / window);
  }

  private computeScoreComponents(publisher: string, nowStep: number): {
    score: number;
    directScore: number;
    secondHopScore: number;
    directEndorserCount: number;
    secondHopEndorserCount: number;
  } {
    const directEndorsers = new Set<string>();
    let directScore = 0;
    for (const trusted of this.trusted) {
      const edges = this.endorsementsByEndorser.get(trusted);
      const newestEdge = edges?.get(publisher as PubkeyHex);
      if (newestEdge) {
        directEndorsers.add(trusted);
        directScore += this.ageWeight(newestEdge.atStep, nowStep);
      }
    }
    if (directEndorsers.size < this.config.endorsementThreshold) {
      directScore = 0;
    }

    let secondHopScore = 0;
    const secondHopEndorsers = new Set<string>();
    if (this.config.maxHops >= 2) {
      const secondHopEntities = new Set<string>();
      for (const trusted of this.trusted) {
        const edges = this.endorsementsByEndorser.get(trusted);
        if (!edges) {
          continue;
        }
        for (const publisher of edges.keys()) {
          secondHopEntities.add(publisher);
        }
      }
      for (const endorser of secondHopEntities) {
        const edges = this.endorsementsByEndorser.get(endorser as PubkeyHex);
        const newestEdge = edges?.get(publisher as PubkeyHex);
        if (newestEdge) {
          secondHopEndorsers.add(endorser);
          secondHopScore +=
            this.ageWeight(newestEdge.atStep, nowStep) * this.config.hopDecay;
        }
      }
      if (secondHopEndorsers.size < this.config.endorsementThreshold) {
        secondHopScore = 0;
      }
    }

    return {
      score: clamp01((directScore + secondHopScore) / 3),
      directScore,
      secondHopScore,
      directEndorserCount: directEndorsers.size,
      secondHopEndorserCount: secondHopEndorsers.size,
    };
  }

  private classifyWithScore(score: number): TrustTier {
    if (score >= this.config.trustedThreshold) {
      return "trusted";
    }
    if (score >= this.config.knownThreshold) {
      return "known";
    }
    return "unknown";
  }
}

export interface RankableFeedItem {
  publisher: string;
  createdAtStep: number;
  [key: string]: unknown;
}

export function rankFeedItemsByTrust<T extends RankableFeedItem>(
  items: T[],
  policy: LocalWotPolicy,
  nowStep: number,
): T[] {
  const tierWeight = (tier: TrustTier): number => policy.config.tierWeights[tier] ?? 0;

  return [...items]
    .filter((item) => policy.classifyPublisher(item.publisher, nowStep) !== "blocked")
    .sort((a, b) => {
      const aTier = policy.classifyPublisher(a.publisher, nowStep);
      const bTier = policy.classifyPublisher(b.publisher, nowStep);
      const tierDelta = tierWeight(bTier) - tierWeight(aTier);
      if (tierDelta !== 0) {
        return tierDelta;
      }
      return b.createdAtStep - a.createdAtStep;
    });
}
