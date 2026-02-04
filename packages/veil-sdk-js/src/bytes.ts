import { blake3 } from "@noble/hashes/blake3";

const utf8 = new TextEncoder();

export function concatBytes(...parts: Uint8Array[]): Uint8Array {
  const total = parts.reduce((sum, p) => sum + p.length, 0);
  const out = new Uint8Array(total);
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

export function u16be(value: number): Uint8Array {
  const out = new Uint8Array(2);
  const view = new DataView(out.buffer);
  view.setUint16(0, value, false);
  return out;
}

export function u32be(value: number): Uint8Array {
  const out = new Uint8Array(4);
  const view = new DataView(out.buffer);
  view.setUint32(0, value, false);
  return out;
}

export function textBytes(input: string): Uint8Array {
  return utf8.encode(input);
}

export function blake3_32(input: Uint8Array): Uint8Array {
  return blake3(input, { dkLen: 32 });
}

export function hexToBytes(hex: string): Uint8Array {
  const normalized = hex.startsWith("0x") ? hex.slice(2) : hex;
  if (normalized.length !== 64) {
    throw new Error(`expected 64 hex chars (32 bytes), got ${normalized.length}`);
  }
  const out = new Uint8Array(32);
  for (let i = 0; i < 32; i += 1) {
    out[i] = Number.parseInt(normalized.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

export function bytesToHex(bytes: Uint8Array): string {
  let out = "";
  for (const b of bytes) {
    out += b.toString(16).padStart(2, "0");
  }
  return out;
}
