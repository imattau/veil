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
  onIgnoredDuplicate?: (shardIdHex: string) => void;
  onIgnoredUnsubscribed?: (tagHex: string) => void;
  onError?: (error: unknown) => void;
}

export interface VeilClientOptions {
  cacheStore?: ShardCacheStore;
  fastFanout?: number;
  fallbackFanout?: number;
}

export class VeilClient {
  private readonly subscriptions = new Set<TagHex>();
  private readonly cacheStore: ShardCacheStore;
  private readonly seenShardIds = new Set<string>();
  private forwardPeers: string[] = [];
  private readonly fastFanout: number;
  private readonly fallbackFanout: number;

  constructor(
    private readonly fastLane: LaneAdapter,
    private readonly fallbackLane?: LaneAdapter,
    private readonly hooks: VeilClientHooks = {},
    options: VeilClientOptions = {},
  ) {
    this.cacheStore = options.cacheStore ?? new MemoryShardCacheStore();
    this.fastFanout = Math.max(0, options.fastFanout ?? 2);
    this.fallbackFanout = Math.max(0, options.fallbackFanout ?? 1);
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

  private shardIdHex(bytes: Uint8Array): string {
    return bytesToHex(blake3(bytes, { dkLen: 32 }));
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

    for (const forwardPeer of this.forwardPeers.slice(0, this.fastFanout)) {
      if (forwardPeer === peer) {
        continue;
      }
      await this.fastLane.send(forwardPeer, bytes);
      this.hooks.onForward?.(forwardPeer, bytes);
    }

    if (this.fallbackLane) {
      for (const forwardPeer of this.forwardPeers.slice(0, this.fallbackFanout)) {
        if (forwardPeer === peer) {
          continue;
        }
        await this.fallbackLane.send(forwardPeer, bytes);
        this.hooks.onForward?.(forwardPeer, bytes);
      }
    }
  }

  async tick(): Promise<void> {
    try {
      const msg = await this.fastLane.recv();
      if (msg) {
        await this.processInbound(msg.peer, msg.bytes);
      }

      if (this.fallbackLane) {
        const fallbackMsg = await this.fallbackLane.recv();
        if (fallbackMsg) {
          await this.processInbound(fallbackMsg.peer, fallbackMsg.bytes);
        }
      }
    } catch (error) {
      this.hooks.onError?.(error);
      throw error;
    }
  }
}
