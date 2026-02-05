declare module "../wasm/veil_wasm.js" {
  export default function init(): Promise<void>;
  export function deriveFeedTag(pubkey: Uint8Array, namespace: number): Uint8Array;
  export function deriveRvTag(pubkey: Uint8Array, epoch: number, namespace: number): Uint8Array;
  export function decodeShardMeta(bytes: Uint8Array): {
    version: number;
    namespace: number;
    epoch: number;
    tagHex: string;
    objectRootHex: string;
    k: number;
    n: number;
    index: number;
    payloadLen: number;
  };
  export function decodeObjectMeta(bytes: Uint8Array): {
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
  };
  export function validateShardCbor(bytes: Uint8Array): boolean;
  export function validateObjectCbor(bytes: Uint8Array): boolean;
  export function currentEpoch(nowSeconds: number, epochSeconds: number): number;
  export function bytesToHex(bytes: Uint8Array): string;
}
