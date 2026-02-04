import { MemoryShardCacheStore, type ShardCacheStore } from "./storage";

export type TagHex = string;

export interface LaneAdapter {
  send(peer: string, bytes: Uint8Array): Promise<void>;
  recv(): Promise<{ peer: string; bytes: Uint8Array } | null>;
}

export interface VeilClientHooks {
  onShard?: (peer: string, bytes: Uint8Array) => void;
  onError?: (error: unknown) => void;
}

export interface VeilClientOptions {
  cacheStore?: ShardCacheStore;
}

export class VeilClient {
  private readonly subscriptions = new Set<TagHex>();
  private readonly cacheStore: ShardCacheStore;

  constructor(
    private readonly fastLane: LaneAdapter,
    private readonly fallbackLane?: LaneAdapter,
    private readonly hooks: VeilClientHooks = {},
    options: VeilClientOptions = {},
  ) {
    this.cacheStore = options.cacheStore ?? new MemoryShardCacheStore();
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

  async getCachedShard(shardId: string): Promise<Uint8Array | null> {
    return this.cacheStore.get(shardId);
  }

  // Runtime scaffold for client-native mode. Full shard pipeline lands in phase 2.
  async tick(): Promise<void> {
    try {
      const msg = await this.fastLane.recv();
      if (msg) {
        this.hooks.onShard?.(msg.peer, msg.bytes);
      }

      if (this.fallbackLane) {
        const fallbackMsg = await this.fallbackLane.recv();
        if (fallbackMsg) {
          this.hooks.onShard?.(fallbackMsg.peer, fallbackMsg.bytes);
        }
      }
    } catch (error) {
      this.hooks.onError?.(error);
      throw error;
    }
  }
}
