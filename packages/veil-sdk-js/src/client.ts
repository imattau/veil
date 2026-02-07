import { decode as cborDecode, encode as cborEncode } from "cbor-x";
import { blake3 } from "@noble/hashes/blake3";

import { bytesToHex, concatBytes, hexToBytes, textBytes } from "./bytes";
import { decodeShardMeta } from "./codec";
import { MemoryShardCacheStore, type ShardCacheStore } from "./storage";
import type { LocalWotPolicy, TrustTier } from "./wot";

export type TagHex = string;

export interface TransportHealthSnapshot {
  outboundQueued: number;
  outboundSendOk: number;
  outboundSendErr: number;
  inboundReceived: number;
  inboundDropped: number;
  reconnectAttempts: number;
}

const EMPTY_TRANSPORT_HEALTH: TransportHealthSnapshot = {
  outboundQueued: 0,
  outboundSendOk: 0,
  outboundSendErr: 0,
  inboundReceived: 0,
  inboundDropped: 0,
  reconnectAttempts: 0,
};

const SHARD_REQUEST_PREFIX = textBytes("VEILREQ1");
const SHARD_REQUEST_VERSION = 1;

type ShardRequestPayload = {
  version: number;
  objectRootHex: string;
  tagHex: string;
  k: number;
  n: number;
  want: number[];
  hop: number;
};

function asRecord(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object") {
    throw new Error("expected object value");
  }
  return value as Record<string, unknown>;
}

function toBytes(value: unknown): Uint8Array {
  if (value instanceof Uint8Array) {
    return value;
  }
  if (Array.isArray(value)) {
    return Uint8Array.from(value as number[]);
  }
  throw new Error("expected byte array value");
}

function startsWith(bytes: Uint8Array, prefix: Uint8Array): boolean {
  if (bytes.length < prefix.length) {
    return false;
  }
  for (let i = 0; i < prefix.length; i += 1) {
    if (bytes[i] !== prefix[i]) {
      return false;
    }
  }
  return true;
}

function encodeShardRequest(payload: ShardRequestPayload): Uint8Array {
  const body = cborEncode({
    v: payload.version,
    object_root: hexToBytes(payload.objectRootHex),
    tag: hexToBytes(payload.tagHex),
    k: payload.k,
    n: payload.n,
    want: payload.want,
    hop: payload.hop,
  });
  return concatBytes(SHARD_REQUEST_PREFIX, body);
}

function decodeShardRequest(bytes: Uint8Array): ShardRequestPayload | null {
  if (!startsWith(bytes, SHARD_REQUEST_PREFIX)) {
    return null;
  }
  try {
    const decoded = asRecord(
      cborDecode(bytes.slice(SHARD_REQUEST_PREFIX.length)),
    );
    const version = Number(decoded.v ?? decoded.version ?? SHARD_REQUEST_VERSION);
    if (version !== SHARD_REQUEST_VERSION) {
      return null;
    }
    const objectRoot = decoded.object_root ?? decoded.objectRoot;
    const tag = decoded.tag;
    const wantRaw = decoded.want;
    if (!objectRoot || !tag || !Array.isArray(wantRaw)) {
      return null;
    }
    const k = Number(decoded.k ?? 0);
    const n = Number(decoded.n ?? 0);
    if (!Number.isFinite(k) || !Number.isFinite(n) || k <= 0 || n <= 0) {
      return null;
    }
    const want = wantRaw
      .map((idx) => Number(idx))
      .filter((idx) => Number.isInteger(idx) && idx >= 0 && idx < n);
    if (want.length === 0) {
      return null;
    }
    return {
      version,
      objectRootHex: bytesToHex(toBytes(objectRoot)).toLowerCase(),
      tagHex: bytesToHex(toBytes(tag)).toLowerCase(),
      k,
      n,
      want,
      hop: Number(decoded.hop ?? 0),
    };
  } catch {
    return null;
  }
}

export interface LaneAdapter {
  send(peer: string, bytes: Uint8Array): Promise<void>;
  recv(): Promise<{ peer: string; bytes: Uint8Array } | null>;
  healthSnapshot?(): TransportHealthSnapshot;
}

export interface VeilClientHooks {
  onShard?: (peer: string, bytes: Uint8Array) => void;
  onForward?: (peer: string, bytes: Uint8Array) => void;
  onForwardError?: (lane: "fast" | "fallback", peer: string, error: unknown) => void;
  onIgnoredDuplicate?: (shardIdHex: string) => void;
  onIgnoredMalformed?: (peer: string, error: unknown) => void;
  onIgnoredUnsubscribed?: (tagHex: string) => void;
  onLaneHealth?: (snapshot: LaneHealthSnapshot) => void;
  onError?: (error: unknown) => void;
  onObject?: (objectRootHex: string, objectBytes: Uint8Array) => void;
}

export interface VeilClientOptions {
  cacheStore?: ShardCacheStore;
  requiredSignedNamespaces?: number[];
  fastFanout?: number;
  fallbackFanout?: number;
  adaptiveLaneScoring?: boolean;
  minimumHealthyLaneScore?: number;
  pollIntervalMs?: number;
  maxSeenShardIds?: number;
  seenShardTtlMs?: number;
  laneHealthEmitMs?: number;
  tieredForwarding?: boolean;
  forwardingQuotas?: Partial<ForwardingQuotas>;
  unknownForwardFloor?: number;
  classifyPeerTier?: (peer: string, meta: Awaited<ReturnType<typeof decodeShardMeta>>) => TrustTier;
  resolvePublisher?:
    | ((
        peer: string,
        meta: Awaited<ReturnType<typeof decodeShardMeta>>,
      ) => string | null)
    | null;
  wotPolicy?: LocalWotPolicy;
  shouldAcceptShard?:
    | ((
        peer: string,
        meta: Awaited<ReturnType<typeof decodeShardMeta>>,
      ) => boolean | Promise<boolean>)
    | null;
  maxCacheShards?: number;
  tierCacheBudgets?: Partial<TierCacheBudgets>;
  enableShardRequests?: boolean;
  requestFanout?: number;
  requestHopLimit?: number;
  requestCooldownMs?: number;
  maxForwardHops?: number;
  plugins?: VeilClientPlugin[];
}

export interface VeilClientPlugin {
  onObject?: (client: VeilClient, objectRootHex: string, objectBytes: Uint8Array) => void;
}

export interface LaneHealth {
  score: number;
  sends: number;
  sendFailures: number;
  receives: number;
  consecutiveSendFailures: number;
  transport: TransportHealthSnapshot;
}

export interface LaneHealthSnapshot {
  fast: LaneHealth;
  fallback?: LaneHealth;
}

export interface ForwardingQuotas {
  trusted: number;
  known: number;
  unknown: number;
  muted: number;
  blocked: number;
}

export interface TierCacheBudgets {
  trusted: number;
  known: number;
  unknown: number;
  muted: number;
  blocked: number;
}

const DEFAULT_FORWARDING_QUOTAS: ForwardingQuotas = {
  trusted: 0.7,
  known: 0.25,
  unknown: 0.05,
  muted: 0,
  blocked: 0,
};

export class VeilClient {
  private readonly subscriptions = new Set<TagHex>();
  private readonly cacheStore: ShardCacheStore;
  private readonly seenShardIds = new Map<string, number>();
  private forwardPeers: string[] = [];
  private readonly fastFanout: number;
  private readonly fallbackFanout: number;
  private readonly adaptiveLaneScoring: boolean;
  private readonly minimumHealthyLaneScore: number;
  private readonly pollIntervalMs: number;
  private readonly maxSeenShardIds: number;
  private readonly seenShardTtlMs: number;
  private readonly laneHealthEmitMs: number;
  private readonly tieredForwarding: boolean;
  private readonly forwardingQuotas: ForwardingQuotas;
  private readonly unknownForwardFloor: number;
  private readonly classifyPeerTier?: (
    peer: string,
    meta: Awaited<ReturnType<typeof decodeShardMeta>>,
  ) => TrustTier;
  private readonly resolvePublisher?:
    | ((
        peer: string,
        meta: Awaited<ReturnType<typeof decodeShardMeta>>,
      ) => string | null)
    | null;
  private readonly wotPolicy?: LocalWotPolicy;
  private readonly shouldAcceptShard?:
    | ((
        peer: string,
        meta: Awaited<ReturnType<typeof decodeShardMeta>>,
      ) => boolean | Promise<boolean>)
    | null;
  private readonly maxCacheShards: number;
  private readonly tierCacheBudgets: TierCacheBudgets;
  private readonly cacheMeta = new Map<string, { tier: TrustTier; lastSeenMs: number }>();
  private readonly cacheSeenCount = new Map<string, number>();
  private readonly shardForwardHops = new Map<string, number>();
  private readonly objectShardState = new Map<
    string,
    { k: number; n: number; tagHex: string; indices: Set<number>; lastRequestAt: number }
  >();
  private readonly indexToShardId = new Map<string, string>();
  private readonly shardIdToIndex = new Map<string, { objectRootHex: string; index: number }>();
  private readonly enableShardRequests: boolean;
  private readonly requestFanout: number;
  private readonly requestHopLimit: number;
  private readonly requestCooldownMs: number;
  private readonly maxForwardHops: number;
  private readonly requiredSignedNamespaces: Set<number>;
  private readonly plugins: VeilClientPlugin[];
  private readonly objectRootPriority = new Map<string, number>();
  private seenSincePrune = 0;
  private running = false;
  private pollTimer: ReturnType<typeof setTimeout> | null = null;
  private tickInFlight: Promise<void> | null = null;
  private laneHealthEmitTimer: ReturnType<typeof setTimeout> | null = null;
  private lastLaneHealthEmitAt = 0;
  private laneHealth: LaneHealthSnapshot = {
    fast: {
      score: 1,
      sends: 0,
      sendFailures: 0,
      receives: 0,
      consecutiveSendFailures: 0,
      transport: { ...EMPTY_TRANSPORT_HEALTH },
    },
  };

  constructor(
    private readonly fastLane: LaneAdapter,
    private readonly fallbackLane?: LaneAdapter,
    private readonly hooks: VeilClientHooks = {},
    options: VeilClientOptions = {},
  ) {
    this.cacheStore = options.cacheStore ?? new MemoryShardCacheStore();
    this.fastFanout = Math.max(0, options.fastFanout ?? 2);
    this.fallbackFanout = Math.max(0, options.fallbackFanout ?? 1);
    this.adaptiveLaneScoring = options.adaptiveLaneScoring ?? true;
    this.minimumHealthyLaneScore = options.minimumHealthyLaneScore ?? 0.15;
    this.pollIntervalMs = Math.max(1, options.pollIntervalMs ?? 50);
    this.maxSeenShardIds = Math.max(1, options.maxSeenShardIds ?? 50_000);
    this.seenShardTtlMs = Math.max(1_000, options.seenShardTtlMs ?? 30 * 60 * 1_000);
    this.laneHealthEmitMs = Math.max(0, options.laneHealthEmitMs ?? 250);
    this.tieredForwarding = options.tieredForwarding ?? true;
    this.forwardingQuotas = { ...DEFAULT_FORWARDING_QUOTAS, ...options.forwardingQuotas };
    this.unknownForwardFloor = Math.max(0, options.unknownForwardFloor ?? 0.05);
    this.classifyPeerTier = options.classifyPeerTier;
    this.resolvePublisher = options.resolvePublisher ?? null;
    this.wotPolicy = options.wotPolicy;
    this.shouldAcceptShard = options.shouldAcceptShard ?? null;
    this.maxCacheShards = Math.max(0, options.maxCacheShards ?? Number.POSITIVE_INFINITY);
    this.tierCacheBudgets = {
      trusted: Number.POSITIVE_INFINITY,
      known: Number.POSITIVE_INFINITY,
      unknown: Number.POSITIVE_INFINITY,
      muted: Number.POSITIVE_INFINITY,
      blocked: 0,
      ...options.tierCacheBudgets,
    };
    this.enableShardRequests = options.enableShardRequests ?? true;
    this.requestFanout = Math.max(0, options.requestFanout ?? 2);
    this.requestHopLimit = Math.max(0, options.requestHopLimit ?? 2);
    this.requestCooldownMs = Math.max(0, options.requestCooldownMs ?? 2_000);
    this.maxForwardHops = Math.max(0, options.maxForwardHops ?? 6);
    this.requiredSignedNamespaces = new Set(
      (options.requiredSignedNamespaces ?? []).filter((value) => Number.isInteger(value)),
    );
    this.plugins = options.plugins ?? [];
    if (fallbackLane) {
      this.laneHealth.fallback = {
        score: 1,
        sends: 0,
        sendFailures: 0,
        receives: 0,
        consecutiveSendFailures: 0,
        transport: { ...EMPTY_TRANSPORT_HEALTH },
      };
    }
  }

  subscribe(tagHex: TagHex): void {
    this.subscriptions.add(tagHex.toLowerCase());
  }

  unsubscribe(tagHex: TagHex): void {
    this.subscriptions.delete(tagHex.toLowerCase());
  }

  listSubscriptions(): string[] {
    return [...this.subscriptions.values()];
  }

  setForwardPeers(peers: string[]): void {
    this.forwardPeers = peers;
  }

  async getCachedShard(shardId: string): Promise<Uint8Array | null> {
    return this.cacheStore.get(shardId);
  }

  prioritizeObjectRoot(objectRootHex: string, priority = 1): void {
    const key = objectRootHex.toLowerCase();
    const next = Math.max(0, priority);
    const current = this.objectRootPriority.get(key) ?? 0;
    if (next > current) {
      this.objectRootPriority.set(key, next);
    }
  }

  notifyObject(objectRootHex: string, objectBytes: Uint8Array): void {
    this.hooks.onObject?.(objectRootHex, objectBytes);
    for (const plugin of this.plugins) {
      plugin.onObject?.(this, objectRootHex, objectBytes);
    }
  }

  async publishBytes(bytes: Uint8Array, selfPeer = "self"): Promise<void> {
    const peers = this.forwardPeers.length > 0 ? this.forwardPeers : [selfPeer];
    for (const peer of peers) {
      await this.sendOnLane("fast", this.fastLane, peer, bytes);
    }
    if (this.fallbackLane && this.fallbackFanout > 0) {
      for (const peer of peers) {
        await this.sendOnLane("fallback", this.fallbackLane, peer, bytes);
      }
    }
  }

  isRunning(): boolean {
    return this.running;
  }

  start(): void {
    if (this.running) {
      return;
    }
    this.running = true;
    this.scheduleNextTick(0);
  }

  stop(): void {
    this.running = false;
    if (this.pollTimer) {
      clearTimeout(this.pollTimer);
      this.pollTimer = null;
    }
    if (this.laneHealthEmitTimer) {
      clearTimeout(this.laneHealthEmitTimer);
      this.laneHealthEmitTimer = null;
    }
  }

  getLaneHealth(): LaneHealthSnapshot {
    this.applyTransportSnapshot("fast", this.fastLane);
    this.applyTransportSnapshot("fallback", this.fallbackLane);
    return {
      fast: {
        ...this.laneHealth.fast,
        transport: { ...this.laneHealth.fast.transport },
      },
      fallback: this.laneHealth.fallback
        ? {
            ...this.laneHealth.fallback,
            transport: { ...this.laneHealth.fallback.transport },
          }
        : undefined,
    };
  }

  private applyTransportSnapshot(
    lane: "fast" | "fallback",
    adapter: LaneAdapter | undefined,
  ): void {
    const state = lane === "fast" ? this.laneHealth.fast : this.laneHealth.fallback;
    if (!state || !adapter?.healthSnapshot) {
      return;
    }

    const snapshot = adapter.healthSnapshot();
    state.transport = { ...snapshot };
    state.sends = snapshot.outboundSendOk + snapshot.outboundSendErr;
    state.sendFailures = snapshot.outboundSendErr;
    state.receives = snapshot.inboundReceived;
  }

  private shardIdHex(bytes: Uint8Array): string {
    return bytesToHex(blake3(bytes, { dkLen: 32 }));
  }

  private resolveTier(
    peer: string,
    meta: Awaited<ReturnType<typeof decodeShardMeta>>,
  ): TrustTier {
    if (this.classifyPeerTier) {
      return this.classifyPeerTier(peer, meta);
    }
    if (this.wotPolicy && this.resolvePublisher) {
      const publisher = this.resolvePublisher(peer, meta);
      if (publisher) {
        return this.wotPolicy.classifyPublisher(publisher, meta.epoch);
      }
    }
    return "unknown";
  }

  private buildTieredForwardPeers(
    meta: Awaited<ReturnType<typeof decodeShardMeta>>,
    totalFanout: number,
  ): string[] {
    if (!this.tieredForwarding || totalFanout <= 0) {
      return [...this.forwardPeers];
    }

    const tierBuckets: Record<TrustTier, string[]> = {
      trusted: [],
      known: [],
      unknown: [],
      muted: [],
      blocked: [],
    };
    for (const peer of this.forwardPeers) {
      const tier = this.resolveTier(peer, meta);
      tierBuckets[tier].push(peer);
    }

    const unknownFloor = Math.ceil(totalFanout * this.unknownForwardFloor);
    const desired: Record<TrustTier, number> = {
      trusted: Math.floor(totalFanout * this.forwardingQuotas.trusted),
      known: Math.floor(totalFanout * this.forwardingQuotas.known),
      unknown: Math.floor(totalFanout * this.forwardingQuotas.unknown),
      muted: Math.floor(totalFanout * this.forwardingQuotas.muted),
      blocked: 0,
    };
    if (unknownFloor > desired.unknown) {
      desired.unknown = unknownFloor;
    }

    const tierOrder: TrustTier[] = ["trusted", "known", "unknown", "muted"];
    let desiredTotal =
      desired.trusted + desired.known + desired.unknown + desired.muted;
    while (desiredTotal > totalFanout) {
      for (const tier of ["trusted", "known", "muted"] as const) {
        if (desiredTotal <= totalFanout) {
          break;
        }
        if (desired[tier] > 0) {
          desired[tier] -= 1;
          desiredTotal -= 1;
        }
      }
      if (desiredTotal <= totalFanout) {
        break;
      }
      if (desired.unknown > unknownFloor) {
        desired.unknown -= 1;
        desiredTotal -= 1;
      } else {
        break;
      }
    }

    const selected: string[] = [];
    for (const tier of tierOrder) {
      const take = Math.min(desired[tier], tierBuckets[tier].length);
      selected.push(...tierBuckets[tier].slice(0, take));
    }

    if (selected.length < totalFanout) {
      for (const tier of tierOrder) {
        if (selected.length >= totalFanout) {
          break;
        }
        for (const peer of tierBuckets[tier]) {
          if (selected.length >= totalFanout) {
            break;
          }
          if (!selected.includes(peer)) {
            selected.push(peer);
          }
        }
      }
    }

    return selected;
  }

  private async dropShard(shardId: string): Promise<void> {
    await this.cacheStore.delete(shardId);
    this.cacheMeta.delete(shardId);
    this.cacheSeenCount.delete(shardId);
    this.shardForwardHops.delete(shardId);
    const indexMeta = this.shardIdToIndex.get(shardId);
    if (indexMeta) {
      const key = `${indexMeta.objectRootHex}:${indexMeta.index}`;
      this.indexToShardId.delete(key);
      this.shardIdToIndex.delete(shardId);
      const state = this.objectShardState.get(indexMeta.objectRootHex);
      if (state) {
        state.indices.delete(indexMeta.index);
        if (state.indices.size === 0) {
          this.objectShardState.delete(indexMeta.objectRootHex);
        }
      }
    }
  }

  private shardPriority(shardId: string): number {
    const indexMeta = this.shardIdToIndex.get(shardId);
    if (!indexMeta) {
      return 0;
    }
    return this.objectRootPriority.get(indexMeta.objectRootHex) ?? 0;
  }

  private noteShardIndex(
    meta: Awaited<ReturnType<typeof decodeShardMeta>>,
    shardIdHex: string,
  ): void {
    const objectRootHex = meta.objectRootHex.toLowerCase();
    const key = `${objectRootHex}:${meta.index}`;
    this.indexToShardId.set(key, shardIdHex);
    this.shardIdToIndex.set(shardIdHex, { objectRootHex, index: meta.index });

    const state = this.objectShardState.get(objectRootHex) ?? {
      k: meta.k,
      n: meta.n,
      tagHex: meta.tagHex.toLowerCase(),
      indices: new Set<number>(),
      lastRequestAt: 0,
    };
    state.k = meta.k;
    state.n = meta.n;
    state.tagHex = meta.tagHex.toLowerCase();
    state.indices.add(meta.index);
    this.objectShardState.set(objectRootHex, state);
  }

  private async enforceCacheBudgets(): Promise<void> {
    const total = this.cacheMeta.size;
    if (total === 0) {
      return;
    }

    const tierCounts: Record<TrustTier, number> = {
      trusted: 0,
      known: 0,
      unknown: 0,
      muted: 0,
      blocked: 0,
    };
    for (const meta of this.cacheMeta.values()) {
      tierCounts[meta.tier] += 1;
    }

    const evictTier = async (tier: TrustTier, limit: number): Promise<void> => {
      if (limit <= 0) {
        return;
      }
      const candidates = [...this.cacheMeta.entries()]
        .filter(([, meta]) => meta.tier === tier)
        .sort((a, b) => {
          const priorityA = this.shardPriority(a[0]);
          const priorityB = this.shardPriority(b[0]);
          if (priorityA !== priorityB) {
            return priorityA - priorityB;
          }
          const seenA = this.cacheSeenCount.get(a[0]) ?? 0;
          const seenB = this.cacheSeenCount.get(b[0]) ?? 0;
          if (seenA !== seenB) {
            return seenB - seenA;
          }
          return a[1].lastSeenMs - b[1].lastSeenMs;
        })
        .slice(0, limit);
      for (const [shardId] of candidates) {
        await this.dropShard(shardId);
        tierCounts[tier] -= 1;
      }
    };

    for (const tier of ["blocked", "muted", "unknown", "known", "trusted"] as const) {
      const over = tierCounts[tier] - (this.tierCacheBudgets[tier] ?? Infinity);
      if (over > 0) {
        await evictTier(tier, over);
      }
    }

    while (this.cacheMeta.size > this.maxCacheShards) {
      for (const tier of ["blocked", "muted", "unknown", "known", "trusted"] as const) {
        if (this.cacheMeta.size <= this.maxCacheShards) {
          break;
        }
        const candidates = [...this.cacheMeta.entries()]
          .filter(([, meta]) => meta.tier === tier)
          .sort((a, b) => {
            const priorityA = this.shardPriority(a[0]);
            const priorityB = this.shardPriority(b[0]);
            if (priorityA !== priorityB) {
              return priorityA - priorityB;
            }
            const seenA = this.cacheSeenCount.get(a[0]) ?? 0;
            const seenB = this.cacheSeenCount.get(b[0]) ?? 0;
            if (seenA !== seenB) {
              return seenB - seenA;
            }
            return a[1].lastSeenMs - b[1].lastSeenMs;
          });
        if (candidates.length === 0) {
          continue;
        }
        const [shardId] = candidates[0];
        await this.dropShard(shardId);
      }
      if (this.cacheMeta.size <= this.maxCacheShards) {
        break;
      }
    }
  }

  private scheduleNextTick(delayMs: number): void {
    if (!this.running || this.pollTimer) {
      return;
    }
    this.pollTimer = setTimeout(() => {
      this.pollTimer = null;
      void this.runTickLoop();
    }, delayMs);
  }

  private async runTickLoop(): Promise<void> {
    if (!this.running || this.tickInFlight) {
      this.scheduleNextTick(this.pollIntervalMs);
      return;
    }

    this.tickInFlight = this.tick().catch(() => {
      // tick() already emits onError hooks; runtime loop continues.
    });
    await this.tickInFlight;
    this.tickInFlight = null;
    this.scheduleNextTick(this.pollIntervalMs);
  }

  private maybeEmitLaneHealth(): void {
    if (!this.hooks.onLaneHealth) {
      return;
    }
    this.applyTransportSnapshot("fast", this.fastLane);
    this.applyTransportSnapshot("fallback", this.fallbackLane);

    const now = Date.now();
    if (
      this.laneHealthEmitMs === 0 ||
      now - this.lastLaneHealthEmitAt >= this.laneHealthEmitMs
    ) {
      this.lastLaneHealthEmitAt = now;
      this.hooks.onLaneHealth(this.getLaneHealth());
      return;
    }

    if (this.laneHealthEmitTimer) {
      return;
    }
    const delay = this.laneHealthEmitMs - (now - this.lastLaneHealthEmitAt);
    this.laneHealthEmitTimer = setTimeout(() => {
      this.laneHealthEmitTimer = null;
      this.lastLaneHealthEmitAt = Date.now();
      this.hooks.onLaneHealth?.(this.getLaneHealth());
    }, delay);
  }

  private pruneSeenShardIds(nowMs: number): void {
    const expiryBefore = nowMs - this.seenShardTtlMs;
    for (const [shardId, seenAt] of this.seenShardIds) {
      if (seenAt >= expiryBefore) {
        break;
      }
      this.seenShardIds.delete(shardId);
      this.shardForwardHops.delete(shardId);
    }

    while (this.seenShardIds.size > this.maxSeenShardIds) {
      const oldest = this.seenShardIds.keys().next();
      if (oldest.done) {
        break;
      }
      this.seenShardIds.delete(oldest.value);
      this.shardForwardHops.delete(oldest.value);
    }
  }

  private maybePruneSeenShardIds(nowMs: number): void {
    this.seenSincePrune += 1;
    if (this.seenShardIds.size <= this.maxSeenShardIds && this.seenSincePrune < 64) {
      return;
    }
    this.seenSincePrune = 0;
    this.pruneSeenShardIds(nowMs);
  }

  private updateLaneHealth(
    lane: "fast" | "fallback",
    event: "send_ok" | "send_error" | "recv",
  ): void {
    const state = lane === "fast" ? this.laneHealth.fast : this.laneHealth.fallback;
    if (!state) {
      return;
    }
    const adapter = lane === "fast" ? this.fastLane : this.fallbackLane;
    const usingAdapterSnapshot = Boolean(adapter?.healthSnapshot);

    if (event === "recv") {
      if (!usingAdapterSnapshot) {
        state.receives += 1;
        state.transport.inboundReceived += 1;
      }
      state.score = Math.min(1, state.score * 0.95 + 0.05);
    } else if (event === "send_ok") {
      if (!usingAdapterSnapshot) {
        state.sends += 1;
        state.transport.outboundSendOk += 1;
      }
      state.consecutiveSendFailures = 0;
      state.score = Math.min(1, state.score * 0.9 + 0.1);
    } else {
      if (!usingAdapterSnapshot) {
        state.sends += 1;
        state.sendFailures += 1;
        state.transport.outboundSendErr += 1;
      }
      state.consecutiveSendFailures += 1;
      state.score = Math.max(0, state.score * 0.55);
    }

    this.maybeEmitLaneHealth();
  }

  private computeLaneFanout(): { fast: number; fallback: number } {
    if (!this.fallbackLane) {
      return { fast: this.fastFanout, fallback: 0 };
    }
    if (!this.adaptiveLaneScoring) {
      return { fast: this.fastFanout, fallback: this.fallbackFanout };
    }

    const total = this.fastFanout + this.fallbackFanout;
    if (total <= 0) {
      return { fast: 0, fallback: 0 };
    }

    const fastScore =
      this.laneHealth.fast.score >= this.minimumHealthyLaneScore
        ? this.laneHealth.fast.score
        : 0;
    const fallbackScore =
      (this.laneHealth.fallback?.score ?? 0) >= this.minimumHealthyLaneScore
        ? (this.laneHealth.fallback?.score ?? 0)
        : 0;

    if (fastScore === 0 && fallbackScore === 0) {
      return { fast: this.fastFanout, fallback: this.fallbackFanout };
    }
    if (fastScore === 0) {
      return { fast: 0, fallback: total };
    }
    if (fallbackScore === 0) {
      return { fast: total, fallback: 0 };
    }

    let fast = Math.round((total * fastScore) / (fastScore + fallbackScore));
    fast = Math.max(1, Math.min(total - 1, fast));
    return { fast, fallback: total - fast };
  }

  private computeRequestFanout(): { fast: number; fallback: number } {
    const total = this.requestFanout;
    if (total <= 0) {
      return { fast: 0, fallback: 0 };
    }
    if (!this.fallbackLane) {
      return { fast: total, fallback: 0 };
    }
    if (!this.adaptiveLaneScoring) {
      const fast = Math.min(total, this.fastFanout);
      return { fast, fallback: Math.max(0, total - fast) };
    }

    const fastScore =
      this.laneHealth.fast.score >= this.minimumHealthyLaneScore
        ? this.laneHealth.fast.score
        : 0;
    const fallbackScore =
      (this.laneHealth.fallback?.score ?? 0) >= this.minimumHealthyLaneScore
        ? (this.laneHealth.fallback?.score ?? 0)
        : 0;

    if (fastScore === 0 && fallbackScore === 0) {
      return { fast: Math.min(total, this.fastFanout), fallback: total - Math.min(total, this.fastFanout) };
    }
    if (fastScore === 0) {
      return { fast: 0, fallback: total };
    }
    if (fallbackScore === 0) {
      return { fast: total, fallback: 0 };
    }

    let fast = Math.round((total * fastScore) / (fastScore + fallbackScore));
    fast = Math.max(1, Math.min(total - 1, fast));
    return { fast, fallback: total - fast };
  }

  private async sendOnLane(
    lane: "fast" | "fallback",
    adapter: LaneAdapter,
    peer: string,
    bytes: Uint8Array,
  ): Promise<void> {
    try {
      await adapter.send(peer, bytes);
      this.updateLaneHealth(lane, "send_ok");
      this.hooks.onForward?.(peer, bytes);
    } catch (error) {
      this.updateLaneHealth(lane, "send_error");
      this.hooks.onForwardError?.(lane, peer, error);
    }
  }

  private async sendShardRequest(
    sourcePeer: string,
    payload: ShardRequestPayload,
  ): Promise<void> {
    if (!this.enableShardRequests || this.requestFanout <= 0) {
      return;
    }
    const requestBytes = encodeShardRequest(payload);
    const fanout = this.computeRequestFanout();
    const total = fanout.fast + fanout.fallback;
    if (total <= 0) {
      return;
    }
    const peers = this.forwardPeers.filter((peer) => peer !== sourcePeer);
    const selected = peers.slice(0, total);
    const fastPeers = selected.slice(0, fanout.fast);
    let fallbackPeers = selected.slice(fanout.fast, fanout.fast + fanout.fallback);
    if (fallbackPeers.length === 0 && fanout.fallback > 0) {
      fallbackPeers = peers.slice(0, fanout.fallback);
    }

    for (const peer of fastPeers) {
      await this.sendOnLane("fast", this.fastLane, peer, requestBytes);
    }
    if (this.fallbackLane) {
      for (const peer of fallbackPeers) {
        await this.sendOnLane("fallback", this.fallbackLane, peer, requestBytes);
      }
    }
  }

  private async handleShardRequest(
    lane: "fast" | "fallback",
    adapter: LaneAdapter,
    peer: string,
    request: ShardRequestPayload,
  ): Promise<void> {
    if (!this.enableShardRequests) {
      return;
    }

    const tagHex = request.tagHex.toLowerCase();
    const objectRootHex = request.objectRootHex.toLowerCase();

    for (const index of request.want) {
      const key = `${objectRootHex}:${index}`;
      const shardId = this.indexToShardId.get(key);
      if (!shardId) {
        continue;
      }
      const shardBytes = await this.cacheStore.get(shardId);
      if (!shardBytes) {
        continue;
      }
      await this.sendOnLane(lane, adapter, peer, shardBytes);
    }

    if (!this.subscriptions.has(tagHex)) {
      return;
    }
    if (this.requestHopLimit <= 0 || request.hop >= this.requestHopLimit) {
      return;
    }
    await this.sendShardRequest(peer, {
      ...request,
      hop: request.hop + 1,
    });
  }

  private async maybeRequestMissing(
    meta: Awaited<ReturnType<typeof decodeShardMeta>>,
    sourcePeer: string,
    nowMs: number,
  ): Promise<void> {
    if (!this.enableShardRequests || this.requestFanout <= 0) {
      return;
    }
    const objectRootHex = meta.objectRootHex.toLowerCase();
    const state = this.objectShardState.get(objectRootHex);
    if (!state) {
      return;
    }
    if (state.indices.size >= state.k) {
      return;
    }
    const threshold = Math.max(1, state.k - 1);
    if (state.indices.size < threshold) {
      return;
    }
    if (nowMs - state.lastRequestAt < this.requestCooldownMs) {
      return;
    }

    const missing: number[] = [];
    for (let idx = 0; idx < state.n; idx += 1) {
      if (!state.indices.has(idx)) {
        missing.push(idx);
      }
    }
    if (missing.length === 0) {
      return;
    }
    const needed = Math.max(1, state.k - state.indices.size);
    const want = missing.slice(0, Math.min(needed, 8));

    state.lastRequestAt = nowMs;
    await this.sendShardRequest(sourcePeer, {
      version: SHARD_REQUEST_VERSION,
      objectRootHex,
      tagHex: state.tagHex,
      k: state.k,
      n: state.n,
      want,
      hop: 0,
    });
  }

  private async forwardOnLane(
    lane: "fast" | "fallback",
    adapter: LaneAdapter,
    peers: string[],
    sourcePeer: string,
    bytes: Uint8Array,
  ): Promise<void> {
    for (const forwardPeer of peers) {
      if (forwardPeer === sourcePeer) {
        continue;
      }
      try {
        await adapter.send(forwardPeer, bytes);
        this.updateLaneHealth(lane, "send_ok");
        this.hooks.onForward?.(forwardPeer, bytes);
      } catch (error) {
        this.updateLaneHealth(lane, "send_error");
        this.hooks.onForwardError?.(lane, forwardPeer, error);
      }
    }
  }

  private async processInbound(
    lane: "fast" | "fallback",
    adapter: LaneAdapter,
    peer: string,
    bytes: Uint8Array,
  ): Promise<void> {
    const request = decodeShardRequest(bytes);
    if (request) {
      await this.handleShardRequest(lane, adapter, peer, request);
      return;
    }

    const nowMs = Date.now();
    this.maybePruneSeenShardIds(nowMs);
    const shardIdHex = this.shardIdHex(bytes);
    if (this.seenShardIds.has(shardIdHex)) {
      this.hooks.onIgnoredDuplicate?.(shardIdHex);
      return;
    }

    let meta: Awaited<ReturnType<typeof decodeShardMeta>>;
    try {
      meta = await decodeShardMeta(bytes);
    } catch (error) {
      this.hooks.onIgnoredMalformed?.(peer, error);
      return;
    }

    if (this.shouldAcceptShard) {
      const accepted = await this.shouldAcceptShard(peer, meta);
      if (!accepted) {
        return;
      }
    }

    // requiredSignedNamespaces is advisory at shard stage; enforce at object level
    // via shouldAcceptShard or application-level verification.

    const tagHex = meta.tagHex.toLowerCase();
    if (!this.subscriptions.has(tagHex)) {
      this.hooks.onIgnoredUnsubscribed?.(tagHex);
      return;
    }

    const inboundTier = this.resolveTier(peer, meta);
    this.seenShardIds.set(shardIdHex, nowMs);
    await this.cacheStore.set(shardIdHex, bytes);
    this.cacheMeta.set(shardIdHex, { tier: inboundTier, lastSeenMs: nowMs });
    this.cacheSeenCount.set(
      shardIdHex,
      (this.cacheSeenCount.get(shardIdHex) ?? 0) + 1,
    );
    this.noteShardIndex(meta, shardIdHex);
    await this.enforceCacheBudgets();
    this.hooks.onShard?.(peer, bytes);
    await this.maybeRequestMissing(meta, peer, nowMs);

    const hops = this.shardForwardHops.get(shardIdHex) ?? 0;
    if (this.maxForwardHops === 0 || hops >= this.maxForwardHops) {
      return;
    }

    const fanout = this.computeLaneFanout();
    const totalFanout = fanout.fast + fanout.fallback;
    const forwardPeers = this.buildTieredForwardPeers(meta, totalFanout);
    const fastPeers = forwardPeers.slice(0, fanout.fast);
    let fallbackPeers = forwardPeers.slice(fanout.fast, fanout.fast + fanout.fallback);
    if (fallbackPeers.length === 0 && fanout.fallback > 0) {
      // If peer list is smaller than desired total fanout, allow overlap.
      fallbackPeers = this.forwardPeers.slice(0, fanout.fallback);
    }

    await this.forwardOnLane("fast", this.fastLane, fastPeers, peer, bytes);
    if (this.fallbackLane) {
      await this.forwardOnLane(
        "fallback",
        this.fallbackLane,
        fallbackPeers,
        peer,
        bytes,
      );
    }
    if (totalFanout > 0) {
      this.shardForwardHops.set(shardIdHex, hops + 1);
    }
  }

  async tick(): Promise<void> {
    try {
      const msg = await this.fastLane.recv();
      if (msg) {
        this.updateLaneHealth("fast", "recv");
        await this.processInbound("fast", this.fastLane, msg.peer, msg.bytes);
      }

      if (this.fallbackLane) {
        const fallbackMsg = await this.fallbackLane.recv();
        if (fallbackMsg) {
          this.updateLaneHealth("fallback", "recv");
          await this.processInbound(
            "fallback",
            this.fallbackLane,
            fallbackMsg.peer,
            fallbackMsg.bytes,
          );
        }
      }
    } catch (error) {
      this.hooks.onError?.(error);
      throw error;
    }
  }
}
