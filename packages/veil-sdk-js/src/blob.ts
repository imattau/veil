import { decode, encode } from "cbor-x";

export interface BlobChunkRefV1 {
  objectRootHex: string;
  tagHex: string;
  size: number;
}

export interface BlobManifestV1 {
  version: 1;
  mime: string;
  size: number;
  hashHex: string;
  chunks: BlobChunkRefV1[];
  filename?: string;
}

export interface DirectoryEntryV1 {
  path: string;
  blob: BlobManifestV1;
}

export interface DirectoryBundleV1 {
  version: 1;
  entries: DirectoryEntryV1[];
}

function asRecord(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object") {
    throw new Error("expected object value");
  }
  return value as Record<string, unknown>;
}

function asNumber(value: unknown, fallback = 0): number {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  return fallback;
}

function asString(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function decodeBlobManifestV1(value: unknown): BlobManifestV1 {
  const decoded = asRecord(value);
  const version = asNumber(decoded.version ?? decoded.v, 1);
  if (version !== 1) {
    throw new Error("unsupported blob manifest version");
  }
  const chunks = Array.isArray(decoded.chunks)
    ? decoded.chunks.map((chunk) => {
        const entry = asRecord(chunk);
        return {
          objectRootHex: asString(entry.objectRootHex ?? entry.object_root_hex),
          tagHex: asString(entry.tagHex ?? entry.tag_hex),
          size: asNumber(entry.size),
        };
      })
    : [];
  return {
    version: 1,
    mime: asString(decoded.mime),
    size: asNumber(decoded.size),
    hashHex: asString(decoded.hashHex ?? decoded.hash_hex),
    chunks,
    filename: decoded.filename ? asString(decoded.filename) : undefined,
  };
}

export function encodeBlobManifestV1(manifest: BlobManifestV1): Uint8Array {
  return encode(manifest);
}

export function decodeBlobManifestV1Bytes(bytes: Uint8Array): BlobManifestV1 {
  return decodeBlobManifestV1(decode(bytes));
}

export function encodeDirectoryBundleV1(bundle: DirectoryBundleV1): Uint8Array {
  return encode(bundle);
}

export function decodeDirectoryBundleV1Bytes(bytes: Uint8Array): DirectoryBundleV1 {
  const decoded = asRecord(decode(bytes));
  const version = asNumber(decoded.version ?? decoded.v, 1);
  if (version !== 1) {
    throw new Error("unsupported directory bundle version");
  }
  const entries = Array.isArray(decoded.entries)
    ? decoded.entries.map((entry) => {
        const record = asRecord(entry);
        return {
          path: asString(record.path),
          blob: decodeBlobManifestV1(record.blob),
        };
      })
    : [];
  return { version: 1, entries };
}
