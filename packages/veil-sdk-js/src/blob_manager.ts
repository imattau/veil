import { decodeAppEnvelope } from "./app_schemas";
import type { AppEnvelope, FileChunkV1, MediaDescriptorV1 } from "./app_schemas";

export interface BlobAssembly {
  rootHex: string;
  descriptor: MediaDescriptorV1;
  bytes: Uint8Array;
}

export class BlobManager {
  private readonly media = new Map<string, MediaDescriptorV1>();
  private readonly chunks = new Map<string, FileChunkV1>();

  ingest(objectRootHex: string, objectBytes: Uint8Array): void {
    let envelope: AppEnvelope;
    try {
      envelope = decodeAppEnvelope(objectBytes);
    } catch {
      return;
    }
    if (envelope.type === "media_desc") {
      const media = parseMediaDescriptor(envelope);
      if (media) {
        this.media.set(objectRootHex.toLowerCase(), media);
      }
      return;
    }
    if (envelope.type === "chunk") {
      const chunk = parseFileChunk(envelope);
      if (chunk) {
        this.chunks.set(objectRootHex.toLowerCase(), chunk);
      }
    }
  }

  tryAssemble(rootHex: string): BlobAssembly | null {
    const key = rootHex.toLowerCase();
    const descriptor = this.media.get(key);
    if (!descriptor) {
      return null;
    }
    const parts: FileChunkV1[] = [];
    for (const chunkRoot of descriptor.chunk_roots) {
      const chunk = this.chunks.get(chunkRoot.toLowerCase());
      if (!chunk) {
        return null;
      }
      parts.push(chunk);
    }
    if (parts.length === 0) {
      return null;
    }
    parts.sort((a, b) => a.index - b.index);
    const total = parts[0].total;
    if (parts.some((c) => c.total !== total) || parts.length !== total) {
      return null;
    }
    const bytes = new Uint8Array(parts.reduce((sum, chunk) => sum + chunk.data.length, 0));
    let offset = 0;
    for (const chunk of parts) {
      bytes.set(chunk.data, offset);
      offset += chunk.data.length;
    }
    return { rootHex: key, descriptor, bytes };
  }
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function parseMediaDescriptor(envelope: AppEnvelope): MediaDescriptorV1 | null {
  if (!asRecord(envelope.payload)) {
    return null;
  }
  const payload = envelope.payload as Record<string, unknown>;
  if (
    typeof payload.mime !== "string" ||
    typeof payload.size !== "number" ||
    typeof payload.hash_hex !== "string" ||
    !Array.isArray(payload.chunk_roots)
  ) {
    return null;
  }
  const chunk_roots = payload.chunk_roots.filter(
    (root): root is string => typeof root === "string",
  );
  return {
    type: "media_desc",
    version: 1,
    mime: payload.mime,
    size: payload.size,
    hash_hex: payload.hash_hex,
    chunk_roots,
    chunk_tag_hex: typeof payload.chunk_tag_hex === "string" ? payload.chunk_tag_hex : undefined,
    blurhash: typeof payload.blurhash === "string" ? payload.blurhash : undefined,
    extensions: asRecord(payload.extensions) ?? undefined,
  };
}

function parseFileChunk(envelope: AppEnvelope): FileChunkV1 | null {
  if (!asRecord(envelope.payload)) {
    return null;
  }
  const payload = envelope.payload as Record<string, unknown>;
  if (!(payload.data instanceof Uint8Array)) {
    return null;
  }
  if (typeof payload.index !== "number" || typeof payload.total !== "number") {
    return null;
  }
  return {
    type: "chunk",
    version: 1,
    data: payload.data,
    index: payload.index,
    total: payload.total,
    extensions: asRecord(payload.extensions) ?? undefined,
  };
}
