import { encode as cborEncode, decode as cborDecode } from "cbor-x";
import {
  exportEd25519PrivateKeyPkcs8,
  exportPublicKeyRaw,
  generateEd25519KeyPair,
  importEd25519PrivateKeyPkcs8,
  importEd25519PublicKeyRaw,
  signEd25519,
  verifyEd25519,
} from "./keys";
import { AsyncKeyValueStoreLike } from "./storage";

export type IdentityKeypair = {
  publicKey: Uint8Array;
  secretKey: Uint8Array;
};

export interface IdentityStore {
  load(): Promise<IdentityKeypair | null>;
  save(identity: IdentityKeypair): Promise<void>;
  clear(): Promise<void>;
}

export class MemoryIdentityStore implements IdentityStore {
  private identity: IdentityKeypair | null = null;

  async load(): Promise<IdentityKeypair | null> {
    return this.identity;
  }

  async save(identity: IdentityKeypair): Promise<void> {
    this.identity = identity;
  }

  async clear(): Promise<void> {
    this.identity = null;
  }
}

export class AsyncKeyValueIdentityStore implements IdentityStore {
  private readonly keyPublic: string;
  private readonly keySecret: string;

  constructor(
    private readonly store: AsyncKeyValueStoreLike,
    options: { keyPrefix?: string } = {},
  ) {
    const prefix = options.keyPrefix ?? "veil:identity:";
    this.keyPublic = `${prefix}pub`;
    this.keySecret = `${prefix}sec`;
  }

  async load(): Promise<IdentityKeypair | null> {
    const pubHex = await this.store.getItem(this.keyPublic);
    const secHex = await this.store.getItem(this.keySecret);
    if (!pubHex || !secHex) return null;
    return { publicKey: hexToBytesAny(pubHex), secretKey: hexToBytesAny(secHex) };
  }

  async save(identity: IdentityKeypair): Promise<void> {
    await this.store.setItem(this.keyPublic, bytesToHexAny(identity.publicKey));
    await this.store.setItem(this.keySecret, bytesToHexAny(identity.secretKey));
  }

  async clear(): Promise<void> {
    await this.store.removeItem(this.keyPublic);
    await this.store.removeItem(this.keySecret);
  }
}

export async function generateIdentity(): Promise<IdentityKeypair> {
  const keyPair = await generateEd25519KeyPair();
  const publicKey = await exportPublicKeyRaw(keyPair.publicKey);
  const secretKey = await exportEd25519PrivateKeyPkcs8(keyPair.privateKey);
  return { publicKey, secretKey };
}

export async function loadOrCreateIdentity(store: IdentityStore): Promise<IdentityKeypair> {
  const existing = await store.load();
  if (existing) return existing;
  const created = await generateIdentity();
  await store.save(created);
  return created;
}

export async function signMessage(
  message: Uint8Array,
  identity: IdentityKeypair,
): Promise<Uint8Array> {
  const privateKey = await importEd25519PrivateKeyPkcs8(identity.secretKey);
  return signEd25519(privateKey, message);
}

export async function verifyMessage(
  message: Uint8Array,
  signature: Uint8Array,
  publicKey: Uint8Array,
): Promise<boolean> {
  const key = await importEd25519PublicKeyRaw(publicKey);
  return verifyEd25519(key, message, signature);
}

export type ContactBundle = {
  version: number;
  pubkeyHex: string;
  quicCertHex: string;
  endpoints: string[];
  createdAt: number;
  signatureHex?: string;
};

export type ContactBundleImportResult = {
  bundle: ContactBundle;
  wsEndpoints: string[];
  quicEndpoints: string[];
  otherEndpoints: string[];
  pubkeyHex: string;
  quicCertHex: string;
};

export type ContactBundleMergeResult = {
  wsEndpoints: string[];
  quicEndpoints: string[];
  quicCertsByEndpoint: Record<string, string>;
  pubkeyHex: string;
};

export function contactBundleSignedBytes(bundle: ContactBundle): Uint8Array {
  const payload = [
    bundle.version,
    bundle.pubkeyHex,
    bundle.quicCertHex,
    bundle.endpoints,
    bundle.createdAt,
  ];
  return cborEncode(payload);
}

export function encodeContactBundle(bundle: ContactBundle): Uint8Array {
  const payload = [
    bundle.version,
    bundle.pubkeyHex,
    bundle.quicCertHex,
    bundle.endpoints,
    bundle.createdAt,
    bundle.signatureHex ?? "",
  ];
  return cborEncode(payload);
}

export function decodeContactBundle(bytes: Uint8Array): ContactBundle {
  const decoded = cborDecode(bytes) as unknown;
  if (!Array.isArray(decoded) || decoded.length < 5) {
    throw new Error("invalid contact bundle");
  }
  return {
    version: decoded[0] as number,
    pubkeyHex: decoded[1] as string,
    quicCertHex: decoded[2] as string,
    endpoints: decoded[3] as string[],
    createdAt: decoded[4] as number,
    signatureHex: decoded.length > 5 ? (decoded[5] as string) : undefined,
  };
}

export async function signContactBundle(
  bundle: ContactBundle,
  identity: IdentityKeypair,
): Promise<ContactBundle> {
  const sig = await signMessage(contactBundleSignedBytes(bundle), identity);
  return { ...bundle, signatureHex: bytesToHexAny(sig) };
}

export async function verifyContactBundle(bundle: ContactBundle): Promise<boolean> {
  if (!bundle.signatureHex) return false;
  const sig = hexToBytesAny(bundle.signatureHex);
  const pub = hexToBytesAny(bundle.pubkeyHex);
  return verifyMessage(contactBundleSignedBytes(bundle), sig, pub);
}

export function contactBundleToQr(bundle: ContactBundle): string {
  const data = base64UrlEncode(encodeContactBundle(bundle));
  return `veil://contact?b=${data}`;
}

export function contactBundleFromQr(value: string): ContactBundle {
  const uri = new URL(value);
  const data = uri.searchParams.get("b");
  if (!data) {
    throw new Error("missing bundle data");
  }
  return decodeContactBundle(base64UrlDecode(data));
}

export function classifyContactBundle(bundle: ContactBundle): ContactBundleImportResult {
  const wsEndpoints: string[] = [];
  const quicEndpoints: string[] = [];
  const otherEndpoints: string[] = [];
  for (const endpoint of bundle.endpoints ?? []) {
    const lower = endpoint.toLowerCase();
    if (lower.startsWith("quic://")) {
      quicEndpoints.push(endpoint);
    } else if (
      lower.startsWith("ws://") ||
      lower.startsWith("wss://") ||
      lower.startsWith("http://") ||
      lower.startsWith("https://")
    ) {
      wsEndpoints.push(endpoint);
    } else {
      otherEndpoints.push(endpoint);
    }
  }
  return {
    bundle,
    wsEndpoints,
    quicEndpoints,
    otherEndpoints,
    pubkeyHex: bundle.pubkeyHex,
    quicCertHex: bundle.quicCertHex,
  };
}

export async function contactBundleImportFromQr(
  value: string,
): Promise<ContactBundleImportResult | null> {
  let bundle: ContactBundle;
  try {
    bundle = contactBundleFromQr(value);
  } catch {
    return null;
  }
  try {
    const ok = await verifyContactBundle(bundle);
    if (!ok) return null;
  } catch {
    return null;
  }
  return classifyContactBundle(bundle);
}

export function mergeContactBundleImport(
  imported: ContactBundleImportResult,
  options: {
    existingWs?: string[];
    existingQuic?: string[];
    existingQuicCerts?: Record<string, string>;
  } = {},
): ContactBundleMergeResult {
  const ws = [...(options.existingWs ?? [])];
  for (const endpoint of imported.wsEndpoints) {
    if (!ws.includes(endpoint)) {
      ws.push(endpoint);
    }
  }
  const quic = [...(options.existingQuic ?? [])];
  for (const endpoint of imported.quicEndpoints) {
    if (!quic.includes(endpoint)) {
      quic.push(endpoint);
    }
  }
  const certs = { ...(options.existingQuicCerts ?? {}) };
  if (imported.quicCertHex && imported.quicEndpoints.length > 0) {
    const first = imported.quicEndpoints[0];
    if (!certs[first]) {
      certs[first] = imported.quicCertHex;
    }
  }
  return {
    wsEndpoints: ws,
    quicEndpoints: quic,
    quicCertsByEndpoint: certs,
    pubkeyHex: imported.pubkeyHex,
  };
}

function bytesToHexAny(bytes: Uint8Array): string {
  let out = "";
  for (const b of bytes) {
    out += b.toString(16).padStart(2, "0");
  }
  return out;
}

function hexToBytesAny(hex: string): Uint8Array {
  const normalized = hex.startsWith("0x") ? hex.slice(2) : hex;
  if (normalized.length % 2 !== 0) {
    throw new Error("invalid hex length");
  }
  const out = new Uint8Array(normalized.length / 2);
  for (let i = 0; i < out.length; i += 1) {
    out[i] = Number.parseInt(normalized.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

function base64UrlEncode(bytes: Uint8Array): string {
  if (typeof Buffer !== "undefined") {
    return Buffer.from(bytes).toString("base64url");
  }
  let binary = "";
  for (const b of bytes) {
    binary += String.fromCharCode(b);
  }
  const b64 = btoa(binary);
  return b64.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function base64UrlDecode(input: string): Uint8Array {
  if (typeof Buffer !== "undefined") {
    return new Uint8Array(Buffer.from(input, "base64url"));
  }
  const padded = input.replace(/-/g, "+").replace(/_/g, "/");
  const padLen = (4 - (padded.length % 4)) % 4;
  const b64 = padded + "=".repeat(padLen);
  const binary = atob(b64);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    out[i] = binary.charCodeAt(i);
  }
  return out;
}
