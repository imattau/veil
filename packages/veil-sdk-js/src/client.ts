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
  private readonly seenShardIds = new Set<string>();
  private forwardPeers: string[] = [];
  private readonly fastFanout: number;
  private readonly fallbackFanout: number;
  private readonly adaptiveLaneScoring: boolean;
  private readonly minimumHealthyLaneScore: number;
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

  getLaneHealth(): LaneHealthSnapshot {
    return {
      fast: { ...this.laneHealth.fast },
      fallback: this.laneHealth.fallback ? { ...this.laneHealth.fallback } : undefined,
    };
  }

  private shardIdHex(bytes: Uint8Array): string {
    return bytesToHex(blake3(bytes, { dkLen: 32 }));
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

    this.hooks.onLaneHealth?.(this.getLaneHealth());
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
    const shardIdHex = this.shardIdHex(bytes);
    if (this.seenShardIds.has(shardIdHex)) {
      this.hooks.onIgnoredDuplicate?.(shardIdHex);
      return;
    }

    const meta = await decodeShardMeta(bytes);
    const tagHex = meta.tagHex.toLowerCase();
    if (!this.subscriptions.has(tagHex)) {
      this.hooks.onIgnoredUnsubscribed?.(tagHex);
      return;
    }

    this.seenShardIds.add(shardIdHex);
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
