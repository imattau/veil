import { decode, encode } from "cbor-x";
import { hexToBytes, type TagHex } from "@veil/sdk-js";

import type { FeedEnvelope } from "./model";

function randomHex(bytes: number): string {
  const out = new Uint8Array(bytes);
  const cryptoObj = globalThis.crypto;
  if (cryptoObj?.getRandomValues) {
    cryptoObj.getRandomValues(out);
  } else {
    for (let i = 0; i < out.length; i += 1) {
      out[i] = Math.floor(Math.random() * 256);
    }
  }
  return [...out].map((b) => b.toString(16).padStart(2, "0")).join("");
}

function payloadFromEnvelope(envelope: FeedEnvelope): Uint8Array {
  return new TextEncoder().encode(JSON.stringify(envelope));
}

export function encodeFeedEnvelopeShard(
  envelope: FeedEnvelope,
  tagHex: TagHex,
  namespace: number,
): Uint8Array {
  const payload = payloadFromEnvelope(envelope);
  const objectRoot = hexToBytes(envelope.root);
  const tag = hexToBytes(tagHex);
  const record = {
    header: {
      version: 1,
      namespace,
      epoch: Math.floor(Date.now() / 1000 / 86400),
      tag,
      object_root: objectRoot,
      k: 1,
      n: 1,
      index: 0,
    },
    payload,
  };
  return encode(record);
}

export function decodeFeedEnvelopeShard(bytes: Uint8Array): FeedEnvelope | null {
  try {
    const decoded = decode(bytes) as { payload?: Uint8Array };
    if (!decoded?.payload) {
      return null;
    }
    const text = new TextDecoder().decode(decoded.payload);
    const envelope = JSON.parse(text) as FeedEnvelope;
    if (!envelope || typeof envelope !== "object") {
      return null;
    }
    if (typeof envelope.root !== "string" || !envelope.bundle) {
      return null;
    }
    return envelope;
  } catch {
    return null;
  }
}

export function randomPeerId(): string {
  return `peer-${randomHex(4)}`;
}
