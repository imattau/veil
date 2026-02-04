import { blake3_32, bytesToHex, concatBytes, hexToBytes, textBytes, u16be, u32be } from "./bytes";
import { isBrowserRuntime, isReactNativeRuntime } from "./env";

type WasmModule = {
  default?: () => Promise<void>;
  deriveFeedTag: (pubkey: Uint8Array, namespace: number) => Uint8Array;
  deriveRvTag: (pubkey: Uint8Array, epoch: number, namespace: number) => Uint8Array;
  currentEpoch: (nowSeconds: number, epochSeconds: number) => number;
  bytesToHex: (bytes: Uint8Array) => string;
};

export type TagBackendMode = "auto" | "wasm" | "pure-js";

let backendMode: TagBackendMode = "auto";
let wasmModulePromise: Promise<WasmModule> | null = null;
const WASM_MODULE_PATH = "../wasm/veil_wasm.js";

export function configureTagBackend(mode: TagBackendMode): void {
  backendMode = mode;
}

function shouldUseWasm(): boolean {
  if (backendMode === "wasm") {
    return true;
  }
  if (backendMode === "pure-js") {
    return false;
  }
  // auto
  if (isReactNativeRuntime()) {
    return false;
  }
  return isBrowserRuntime();
}

async function loadWasmModule(): Promise<WasmModule> {
  if (!wasmModulePromise) {
    wasmModulePromise = import(/* @vite-ignore */ WASM_MODULE_PATH) as Promise<WasmModule>;
  }
  return wasmModulePromise;
}

export async function initVeilWasm(): Promise<void> {
  const mod = await loadWasmModule();
  if (typeof mod.default === "function") {
    await mod.default();
  }
}

function deriveFeedTagPure(publisherPubkey: Uint8Array, namespace: number): Uint8Array {
  const preimage = concatBytes(textBytes("feed"), publisherPubkey, u16be(namespace));
  return blake3_32(preimage);
}

function deriveRvTagPure(recipientPubkey: Uint8Array, epoch: number, namespace: number): Uint8Array {
  const preimage = concatBytes(textBytes("rv"), recipientPubkey, u32be(epoch), u16be(namespace));
  return blake3_32(preimage);
}

export async function deriveFeedTagHex(
  publisherPubkeyHex: string,
  namespace: number,
): Promise<string> {
  const pubkey = hexToBytes(publisherPubkeyHex);
  if (shouldUseWasm()) {
    try {
      const mod = await loadWasmModule();
      await initVeilWasm();
      return mod.bytesToHex(mod.deriveFeedTag(pubkey, namespace));
    } catch (error) {
      if (backendMode === "wasm") {
        throw error;
      }
    }
  }
  return bytesToHex(deriveFeedTagPure(pubkey, namespace));
}

export async function deriveRvTagHex(
  recipientPubkeyHex: string,
  epoch: number,
  namespace: number,
): Promise<string> {
  const pubkey = hexToBytes(recipientPubkeyHex);
  if (shouldUseWasm()) {
    try {
      const mod = await loadWasmModule();
      await initVeilWasm();
      return mod.bytesToHex(mod.deriveRvTag(pubkey, epoch, namespace));
    } catch (error) {
      if (backendMode === "wasm") {
        throw error;
      }
    }
  }
  return bytesToHex(deriveRvTagPure(pubkey, epoch, namespace));
}

export async function currentEpoch(nowSeconds: number, epochSeconds = 86_400): Promise<number> {
  if (epochSeconds <= 0) {
    throw new Error("epochSeconds must be > 0");
  }
  if (shouldUseWasm()) {
    try {
      const mod = await loadWasmModule();
      await initVeilWasm();
      return mod.currentEpoch(nowSeconds, epochSeconds);
    } catch (error) {
      if (backendMode === "wasm") {
        throw error;
      }
    }
  }
  return Math.floor(nowSeconds / epochSeconds);
}

export { bytesToHex, hexToBytes };
