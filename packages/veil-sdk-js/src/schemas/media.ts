import { decodeAppEnvelope, encodeAppEnvelope } from "../app_schemas";
import type { AppEnvelope } from "../app_schemas";

export type VideoManifestV1 = {
  type: "video_manifest";
  version: 1;
  mime: string;
  size: number;
  hash_hex: string;
  chunk_roots: string[];
  chunk_tag_hex?: string;
  duration_ms?: number;
  extensions?: Record<string, unknown>;
};

export type ProgressiveImageV1 = {
  type: "progressive_image";
  version: 1;
  mime: string;
  size: number;
  hash_hex: string;
  chunk_roots: string[];
  chunk_tag_hex?: string;
  blurhash?: string;
  extensions?: Record<string, unknown>;
};

export function encodeVideoManifest(manifest: VideoManifestV1): Uint8Array {
  return encodeAppEnvelope({
    type: manifest.type,
    version: manifest.version,
    payload: {
      mime: manifest.mime,
      size: manifest.size,
      hash_hex: manifest.hash_hex,
      chunk_roots: manifest.chunk_roots,
      chunk_tag_hex: manifest.chunk_tag_hex,
      duration_ms: manifest.duration_ms,
      extensions: manifest.extensions,
    },
  });
}

export function encodeProgressiveImage(image: ProgressiveImageV1): Uint8Array {
  return encodeAppEnvelope({
    type: image.type,
    version: image.version,
    payload: {
      mime: image.mime,
      size: image.size,
      hash_hex: image.hash_hex,
      chunk_roots: image.chunk_roots,
      chunk_tag_hex: image.chunk_tag_hex,
      blurhash: image.blurhash,
      extensions: image.extensions,
    },
  });
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function decodeChunkedPayload(
  envelope: AppEnvelope,
): {
  mime: string;
  size: number;
  hash_hex: string;
  chunk_roots: string[];
  chunk_tag_hex?: string;
  extra: Record<string, unknown>;
} | null {
  if (!asRecord(envelope.payload)) {
    return null;
  }
  const payload = envelope.payload as Record<string, unknown>;
  if (typeof payload.mime !== "string" || typeof payload.size !== "number") {
    return null;
  }
  if (typeof payload.hash_hex !== "string" || !Array.isArray(payload.chunk_roots)) {
    return null;
  }
  const chunk_roots = payload.chunk_roots.filter(
    (root): root is string => typeof root === "string",
  );
  return {
    mime: payload.mime,
    size: payload.size,
    hash_hex: payload.hash_hex,
    chunk_roots,
    chunk_tag_hex: typeof payload.chunk_tag_hex === "string" ? payload.chunk_tag_hex : undefined,
    extra: payload,
  };
}

export function decodeVideoManifest(bytes: Uint8Array): VideoManifestV1 | null {
  let envelope: AppEnvelope;
  try {
    envelope = decodeAppEnvelope(bytes);
  } catch {
    return null;
  }
  if (envelope.type !== "video_manifest" || envelope.version !== 1) {
    return null;
  }
  const payload = decodeChunkedPayload(envelope);
  if (!payload) {
    return null;
  }
  return {
    type: "video_manifest",
    version: 1,
    mime: payload.mime,
    size: payload.size,
    hash_hex: payload.hash_hex,
    chunk_roots: payload.chunk_roots,
    chunk_tag_hex: payload.chunk_tag_hex,
    duration_ms:
      typeof payload.extra.duration_ms === "number"
        ? payload.extra.duration_ms
        : undefined,
    extensions: asRecord(payload.extra.extensions) ?? undefined,
  };
}

export function decodeProgressiveImage(bytes: Uint8Array): ProgressiveImageV1 | null {
  let envelope: AppEnvelope;
  try {
    envelope = decodeAppEnvelope(bytes);
  } catch {
    return null;
  }
  if (envelope.type !== "progressive_image" || envelope.version !== 1) {
    return null;
  }
  const payload = decodeChunkedPayload(envelope);
  if (!payload) {
    return null;
  }
  return {
    type: "progressive_image",
    version: 1,
    mime: payload.mime,
    size: payload.size,
    hash_hex: payload.hash_hex,
    chunk_roots: payload.chunk_roots,
    chunk_tag_hex: payload.chunk_tag_hex,
    blurhash:
      typeof payload.extra.blurhash === "string" ? payload.extra.blurhash : undefined,
    extensions: asRecord(payload.extra.extensions) ?? undefined,
  };
}
