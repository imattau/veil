import { blake3 } from "@noble/hashes/blake3";

import { bytesToHex } from "./bytes";
import { decodeShardMeta } from "./codec";
import { MemoryShardCacheStore, type ShardCacheStore } from "./storage";

export type TagHex = string;

export interface LaneAdapter {
  send(peer: string, bytes: Uint8Array): Promise<void>;
  recv(): Promise<{ peer: string; bytes: Uint8Array } | null>;
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
}

export interface VeilClientOptions {
  cacheStore?: ShardCacheStore;
  fastFanout?: number;
  fallbackFanout?: number;
  adaptiveLaneScoring?: boolean;
  minimumHealthyLaneScore?: number;
  pollIntervalMs?: number;
  maxSeenShardIds?: number;
  seenShardTtlMs?: number;
  laneHealthEmitMs?: number;
}

export interface LaneHealth {
  score: number;
  sends: number;
  sendFailures: number;
  receives: number;
  consecutiveSendFailures: number;
}

export interface LaneHealthSnapshot {
  fast: LaneHealth;
  fallback?: LaneHealth;
}

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
    if (fallbackLane) {
      this.laneHealth.fallback = {
        score: 1,
        sends: 0,
        sendFailures: 0,
        receives: 0,
        consecutiveSendFailures: 0,
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
    return {
      fast: { ...this.laneHealth.fast },
      fallback: this.laneHealth.fallback ? { ...this.laneHealth.fallback } : undefined,
    };
  }

  private shardIdHex(bytes: Uint8Array): string {
    return bytesToHex(blake3(bytes, { dkLen: 32 }));
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
    }

    while (this.seenShardIds.size > this.maxSeenShardIds) {
      const oldest = this.seenShardIds.keys().next();
      if (oldest.done) {
        break;
      }
      this.seenShardIds.delete(oldest.value);
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
    const state =
      lane === "fast"
        ? this.laneHealth.fast
        : this.laneHealth.fallback;
    if (!state) {
      return;
    }

    if (event === "recv") {
      state.receives += 1;
      state.score = Math.min(1, state.score * 0.95 + 0.05);
    } else if (event === "send_ok") {
      state.sends += 1;
      state.consecutiveSendFailures = 0;
      state.score = Math.min(1, state.score * 0.9 + 0.1);
    } else {
      state.sends += 1;
      state.sendFailures += 1;
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

  private async processInbound(peer: string, bytes: Uint8Array): Promise<void> {
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
    const tagHex = meta.tagHex.toLowerCase();
    if (!this.subscriptions.has(tagHex)) {
      this.hooks.onIgnoredUnsubscribed?.(tagHex);
      return;
    }

    this.seenShardIds.set(shardIdHex, nowMs);
    await this.cacheStore.set(shardIdHex, bytes);
    this.hooks.onShard?.(peer, bytes);

    const fanout = this.computeLaneFanout();
    const fastPeers = this.forwardPeers.slice(0, fanout.fast);
    let fallbackPeers = this.forwardPeers.slice(fanout.fast, fanout.fast + fanout.fallback);
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
  }

  async tick(): Promise<void> {
    try {
      const msg = await this.fastLane.recv();
      if (msg) {
        this.updateLaneHealth("fast", "recv");
        await this.processInbound(msg.peer, msg.bytes);
      }

      if (this.fallbackLane) {
        const fallbackMsg = await this.fallbackLane.recv();
        if (fallbackMsg) {
          this.updateLaneHealth("fallback", "recv");
          await this.processInbound(fallbackMsg.peer, fallbackMsg.bytes);
        }
      }
    } catch (error) {
      this.hooks.onError?.(error);
      throw error;
    }
  }
}
