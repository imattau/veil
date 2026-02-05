export interface ShardCacheStore {
  get(key: string): Promise<Uint8Array | null>;
  set(key: string, value: Uint8Array): Promise<void>;
  delete(key: string): Promise<void>;
  keys(): Promise<string[]>;
}

export class MemoryShardCacheStore implements ShardCacheStore {
  private readonly map = new Map<string, Uint8Array>();

  async get(key: string): Promise<Uint8Array | null> {
    return this.map.get(key) ?? null;
  }

  async set(key: string, value: Uint8Array): Promise<void> {
    this.map.set(key, value);
  }

  async delete(key: string): Promise<void> {
    this.map.delete(key);
  }

  async keys(): Promise<string[]> {
    return [...this.map.keys()];
  }
}

function bytesToHexAny(bytes: Uint8Array): string {
  let out = "";
  for (const b of bytes) {
    out += b.toString(16).padStart(2, "0");
  }
  return out;
}

function hexToBytesAny(hex: string): Uint8Array {
  if (hex.length % 2 !== 0) {
    throw new Error("invalid hex input length");
  }
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i += 1) {
    out[i] = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

export interface AsyncKeyValueStoreLike {
  getItem(key: string): Promise<string | null>;
  setItem(key: string, value: string): Promise<void>;
  removeItem(key: string): Promise<void>;
  getAllKeys(): Promise<string[]>;
}

export interface AsyncKeyValueShardCacheOptions {
  keyPrefix?: string;
}

// Works with React Native AsyncStorage/MMKV adapters via a thin wrapper.
export class AsyncKeyValueShardCacheStore implements ShardCacheStore {
  private readonly keyPrefix: string;

  constructor(
    private readonly store: AsyncKeyValueStoreLike,
    options: AsyncKeyValueShardCacheOptions = {},
  ) {
    this.keyPrefix = options.keyPrefix ?? "veil:cache:";
  }

  private key(raw: string): string {
    return `${this.keyPrefix}${raw}`;
  }

  async get(key: string): Promise<Uint8Array | null> {
    const encoded = await this.store.getItem(this.key(key));
    if (!encoded) {
      return null;
    }
    return hexToBytesAny(encoded);
  }

  async set(key: string, value: Uint8Array): Promise<void> {
    await this.store.setItem(this.key(key), bytesToHexAny(value));
  }

  async delete(key: string): Promise<void> {
    await this.store.removeItem(this.key(key));
  }

  async keys(): Promise<string[]> {
    const keys = await this.store.getAllKeys();
    return keys
      .filter((k) => k.startsWith(this.keyPrefix))
      .map((k) => k.slice(this.keyPrefix.length));
  }
}

interface IdbRequestLike<T> {
  result: T;
  error?: { message?: string };
  onsuccess: ((event: unknown) => void) | null;
  onerror: ((event: unknown) => void) | null;
}

interface IdbOpenRequestLike extends IdbRequestLike<IdbDatabaseLike> {
  onupgradeneeded: ((event: unknown) => void) | null;
}

interface IdbObjectStoreLike {
  get(key: string): IdbRequestLike<unknown>;
  put(value: unknown, key: string): IdbRequestLike<unknown>;
  delete(key: string): IdbRequestLike<unknown>;
  getAllKeys(): IdbRequestLike<unknown[]>;
}

interface IdbTransactionLike {
  objectStore(name: string): IdbObjectStoreLike;
  oncomplete: ((event: unknown) => void) | null;
  onerror: ((event: unknown) => void) | null;
  onabort: ((event: unknown) => void) | null;
}

interface IdbObjectStoreNamesLike {
  contains(name: string): boolean;
}

interface IdbDatabaseLike {
  objectStoreNames: IdbObjectStoreNamesLike;
  createObjectStore(name: string): unknown;
  transaction(
    storeName: string,
    mode?: "readonly" | "readwrite",
  ): IdbTransactionLike;
  close(): void;
}

interface IdbFactoryLike {
  open(name: string, version?: number): IdbOpenRequestLike;
}

function requestToPromise<T>(request: IdbRequestLike<T>): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () =>
      reject(new Error(request.error?.message ?? "indexeddb request failed"));
  });
}

function transactionDone(transaction: IdbTransactionLike): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(new Error("indexeddb transaction failed"));
    transaction.onabort = () => reject(new Error("indexeddb transaction aborted"));
  });
}

function toIdbFactory(): IdbFactoryLike {
  const maybeFactory = (globalThis as { indexedDB?: IdbFactoryLike }).indexedDB;
  if (!maybeFactory) {
    throw new Error("indexedDB is unavailable in this runtime");
  }
  return maybeFactory;
}

export interface IndexedDbShardCacheOptions {
  dbName?: string;
  storeName?: string;
  dbVersion?: number;
  keyPrefix?: string;
  indexedDbFactory?: IdbFactoryLike;
}

export class IndexedDbShardCacheStore implements ShardCacheStore {
  private readonly dbName: string;
  private readonly storeName: string;
  private readonly dbVersion: number;
  private readonly keyPrefix: string;
  private readonly indexedDbFactory: IdbFactoryLike;
  private dbPromise: Promise<IdbDatabaseLike> | null = null;

  constructor(options: IndexedDbShardCacheOptions = {}) {
    this.dbName = options.dbName ?? "veil-cache";
    this.storeName = options.storeName ?? "shards";
    this.dbVersion = options.dbVersion ?? 1;
    this.keyPrefix = options.keyPrefix ?? "";
    this.indexedDbFactory = options.indexedDbFactory ?? toIdbFactory();
  }

  private key(raw: string): string {
    return `${this.keyPrefix}${raw}`;
  }

  private async db(): Promise<IdbDatabaseLike> {
    if (!this.dbPromise) {
      this.dbPromise = new Promise<IdbDatabaseLike>((resolve, reject) => {
        const request = this.indexedDbFactory.open(this.dbName, this.dbVersion);
        request.onupgradeneeded = () => {
          if (!request.result.objectStoreNames.contains(this.storeName)) {
            request.result.createObjectStore(this.storeName);
          }
        };
        request.onsuccess = () => resolve(request.result);
        request.onerror = () =>
          reject(new Error(request.error?.message ?? "indexeddb open failed"));
      });
    }
    return this.dbPromise;
  }

  async get(key: string): Promise<Uint8Array | null> {
    const db = await this.db();
    const tx = db.transaction(this.storeName, "readonly");
    const req = tx.objectStore(this.storeName).get(this.key(key));
    const result = await requestToPromise(req);
    if (result instanceof Uint8Array) {
      return result;
    }
    if (result instanceof ArrayBuffer) {
      return new Uint8Array(result);
    }
    return null;
  }

  async set(key: string, value: Uint8Array): Promise<void> {
    const db = await this.db();
    const tx = db.transaction(this.storeName, "readwrite");
    tx.objectStore(this.storeName).put(new Uint8Array(value), this.key(key));
    await transactionDone(tx);
  }

  async delete(key: string): Promise<void> {
    const db = await this.db();
    const tx = db.transaction(this.storeName, "readwrite");
    tx.objectStore(this.storeName).delete(this.key(key));
    await transactionDone(tx);
  }

  async keys(): Promise<string[]> {
    const db = await this.db();
    const tx = db.transaction(this.storeName, "readonly");
    const req = tx.objectStore(this.storeName).getAllKeys();
    const keys = await requestToPromise(req);
    return keys
      .filter((k): k is string => typeof k === "string")
      .filter((k) => k.startsWith(this.keyPrefix))
      .map((k) => k.slice(this.keyPrefix.length));
  }

  async close(): Promise<void> {
    if (!this.dbPromise) {
      return;
    }
    const db = await this.dbPromise;
    db.close();
    this.dbPromise = null;
  }
}
