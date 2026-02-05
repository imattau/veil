import { describe, expect, test } from "vitest";

import {
  AsyncKeyValueShardCacheStore,
  IndexedDbShardCacheStore,
} from "../src/storage";

class InMemoryAsyncStore {
  private readonly data = new Map<string, string>();

  async getItem(key: string): Promise<string | null> {
    return this.data.get(key) ?? null;
  }

  async setItem(key: string, value: string): Promise<void> {
    this.data.set(key, value);
  }

  async removeItem(key: string): Promise<void> {
    this.data.delete(key);
  }

  async getAllKeys(): Promise<string[]> {
    return [...this.data.keys()];
  }
}

type Listener = ((event: unknown) => void) | null;

class FakeRequest<T> {
  result: T;
  error: { message?: string } | undefined;
  onsuccess: Listener = null;
  onerror: Listener = null;

  constructor(result: T) {
    this.result = result;
  }

  succeed(): void {
    this.onsuccess?.({});
  }
}

class FakeOpenRequest<T> extends FakeRequest<T> {
  onupgradeneeded: Listener = null;
}

class FakeObjectStoreNames {
  constructor(private readonly names: Set<string>) {}

  contains(name: string): boolean {
    return this.names.has(name);
  }
}

class FakeObjectStore {
  constructor(private readonly data: Map<string, Uint8Array>) {}

  get(key: string): FakeRequest<unknown> {
    const req = new FakeRequest(this.data.get(key));
    queueMicrotask(() => req.succeed());
    return req;
  }

  put(value: unknown, key: string): FakeRequest<unknown> {
    this.data.set(key, value as Uint8Array);
    const req = new FakeRequest(undefined);
    queueMicrotask(() => req.succeed());
    return req;
  }

  delete(key: string): FakeRequest<unknown> {
    this.data.delete(key);
    const req = new FakeRequest(undefined);
    queueMicrotask(() => req.succeed());
    return req;
  }

  getAllKeys(): FakeRequest<unknown[]> {
    const req = new FakeRequest([...this.data.keys()]);
    queueMicrotask(() => req.succeed());
    return req;
  }
}

class FakeTransaction {
  oncomplete: Listener = null;
  onerror: Listener = null;
  onabort: Listener = null;

  constructor(private readonly store: FakeObjectStore) {}

  objectStore(): FakeObjectStore {
    return this.store;
  }

  complete(): void {
    this.oncomplete?.({});
  }
}

class FakeDb {
  private readonly storeNames = new Set<string>();
  private readonly stores = new Map<string, Map<string, Uint8Array>>();
  objectStoreNames = new FakeObjectStoreNames(this.storeNames);

  createObjectStore(name: string): unknown {
    this.storeNames.add(name);
    this.stores.set(name, new Map());
    return {};
  }

  transaction(name: string): FakeTransaction {
    const store = this.stores.get(name);
    if (!store) {
      throw new Error("missing object store");
    }
    const tx = new FakeTransaction(new FakeObjectStore(store));
    queueMicrotask(() => tx.complete());
    return tx;
  }

  close(): void {}
}

class FakeIndexedDbFactory {
  readonly db = new FakeDb();

  open(): FakeOpenRequest<FakeDb> {
    const req = new FakeOpenRequest(this.db);
    queueMicrotask(() => {
      req.onupgradeneeded?.({});
      req.succeed();
    });
    return req;
  }
}

describe("cache adapters", () => {
  test("AsyncKeyValueShardCacheStore round-trips bytes and keys", async () => {
    const cache = new AsyncKeyValueShardCacheStore(new InMemoryAsyncStore(), {
      keyPrefix: "test:",
    });
    const bytes = new Uint8Array([1, 2, 3, 4]);
    await cache.set("shard-a", bytes);

    await expect(cache.get("shard-a")).resolves.toEqual(bytes);
    await expect(cache.keys()).resolves.toEqual(["shard-a"]);

    await cache.delete("shard-a");
    await expect(cache.get("shard-a")).resolves.toBeNull();
  });

  test("IndexedDbShardCacheStore round-trips bytes and keys", async () => {
    const factory = new FakeIndexedDbFactory();
    const cache = new IndexedDbShardCacheStore({
      indexedDbFactory: factory,
      dbName: "veil-test",
      storeName: "shards",
      keyPrefix: "p:",
    });

    const bytes = new Uint8Array([9, 8, 7]);
    await cache.set("one", bytes);

    await expect(cache.get("one")).resolves.toEqual(bytes);
    await expect(cache.keys()).resolves.toEqual(["one"]);

    await cache.delete("one");
    await expect(cache.get("one")).resolves.toBeNull();
    await cache.close();
  });
});
