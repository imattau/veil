declare module "../wasm/veil_wasm.js" {
  export default function init(): Promise<void>;
  export function deriveFeedTag(pubkey: Uint8Array, namespace: number): Uint8Array;
  export function deriveRvTag(pubkey: Uint8Array, epoch: number, namespace: number): Uint8Array;
  export function currentEpoch(nowSeconds: number, epochSeconds: number): number;
  export function bytesToHex(bytes: Uint8Array): string;
}
