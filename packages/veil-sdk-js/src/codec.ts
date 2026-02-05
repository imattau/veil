import { decode } from "cbor-x";

import { bytesToHex, hexToBytes } from "./bytes";
import { configureTagBackend, initVeilWasm } from "./tags";

type WasmModule = {
  decodeShardMeta: (bytes: Uint8Array) => ShardMeta;
  decodeObjectMeta: (bytes: Uint8Array) => ObjectMeta;
  validateShardCbor: (bytes: Uint8Array) => boolean;
  validateObjectCbor: (bytes: Uint8Array) => boolean;
};

const WASM_MODULE_PATH = "../wasm/veil_wasm.js";
let wasmModulePromise: Promise<WasmModule> | null = null;

async function loadWasmModule(): Promise<WasmModule> {
  if (!wasmModulePromise) {
    wasmModulePromise = import(/* @vite-ignore */ WASM_MODULE_PATH) as Promise<WasmModule>;
  }
  return wasmModulePromise;
}

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

export interface ShardMeta {
  version: number;
  namespace: number;
  epoch: number;
  tagHex: string;
  objectRootHex: string;
  k: number;
  n: number;
  index: number;
  payloadLen: number;
}

export interface ObjectMeta {
  version: number;
  namespace: number;
  epoch: number;
  flags: number;
  signed: boolean;
  public: boolean;
  ackRequested: boolean;
  batched: boolean;
  tagHex: string;
  objectRootHex: string;
  senderPubkeyHex?: string;
  nonceHex: string;
  ciphertextLen: number;
  paddingLen: number;
}

function decodeShardMetaPure(bytes: Uint8Array): ShardMeta {
  const decoded = asRecord(decode(bytes));
  const header = asRecord(decoded.header);
  const objectRoot = header.object_root ?? header.objectRoot;

  return {
    version: Number(header.version),
    namespace: Number(header.namespace),
    epoch: Number(header.epoch),
    tagHex: bytesToHex(toBytes(header.tag)),
    objectRootHex: bytesToHex(toBytes(objectRoot)),
    k: Number(header.k),
    n: Number(header.n),
    index: Number(header.index),
    payloadLen: toBytes(decoded.payload).length,
  };
}

function decodeObjectMetaPure(bytes: Uint8Array): ObjectMeta {
  const decoded = asRecord(decode(bytes));
  const flags = Number(decoded.flags);
  const objectRoot = decoded.object_root ?? decoded.objectRoot;
  const senderPubkey = decoded.sender_pubkey ?? decoded.senderPubkey;

  return {
    version: Number(decoded.version),
    namespace: Number(decoded.namespace),
    epoch: Number(decoded.epoch),
    flags,
    signed: (flags & 0x0001) !== 0,
    public: (flags & 0x0002) !== 0,
    ackRequested: (flags & 0x0004) !== 0,
    batched: (flags & 0x0008) !== 0,
    tagHex: bytesToHex(toBytes(decoded.tag)),
    objectRootHex: bytesToHex(toBytes(objectRoot)),
    senderPubkeyHex: senderPubkey ? bytesToHex(toBytes(senderPubkey)) : undefined,
    nonceHex: bytesToHex(toBytes(decoded.nonce)),
    ciphertextLen: toBytes(decoded.ciphertext).length,
    paddingLen: toBytes(decoded.padding).length,
  };
}

export async function decodeShardMeta(bytes: Uint8Array): Promise<ShardMeta> {
  try {
    const mod = await loadWasmModule();
    await initVeilWasm();
    return mod.decodeShardMeta(bytes);
  } catch {
    return decodeShardMetaPure(bytes);
  }
}

export async function decodeObjectMeta(bytes: Uint8Array): Promise<ObjectMeta> {
  try {
    const mod = await loadWasmModule();
    await initVeilWasm();
    return mod.decodeObjectMeta(bytes);
  } catch {
    return decodeObjectMetaPure(bytes);
  }
}

export async function validateShardCbor(bytes: Uint8Array): Promise<boolean> {
  try {
    const mod = await loadWasmModule();
    await initVeilWasm();
    return mod.validateShardCbor(bytes);
  } catch {
    try {
      decodeShardMetaPure(bytes);
      return true;
    } catch {
      return false;
    }
  }
}

export async function validateObjectCbor(bytes: Uint8Array): Promise<boolean> {
  try {
    const mod = await loadWasmModule();
    await initVeilWasm();
    return mod.validateObjectCbor(bytes);
  } catch {
    try {
      decodeObjectMetaPure(bytes);
      return true;
    } catch {
      return false;
    }
  }
}

// Re-export for callers managing backend policy centrally.
export { configureTagBackend, hexToBytes };
