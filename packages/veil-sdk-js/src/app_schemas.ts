import { decode, encode } from "cbor-x";


export type AppEnvelope = {
  type: string;
  version: number;
  payload: Record<string, unknown>;
  extensions?: Record<string, unknown>;
};

export type SocialPostV1 = {
  type: "post";
  version: 1;
  body: string;
  mentions?: string[];
  parent_root?: string;
  thread_root?: string;
  attachments?: MediaDescriptorV1[];
  extensions?: Record<string, unknown>;
};

export type MediaDescriptorV1 = {
  type: "media_desc";
  version: 1;
  mime: string;
  size: number;
  hash_hex: string;
  chunk_roots: string[];
  chunk_tag_hex?: string;
  blurhash?: string;
  extensions?: Record<string, unknown>;
};

export type FileChunkV1 = {
  type: "chunk";
  version: 1;
  data: Uint8Array;
  index: number;
  total: number;
  extensions?: Record<string, unknown>;
};

const MAX_OBJECT_SIZE = 256 * 1024;
const MAX_INLINE_PAYLOAD = 250_000;

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}

function sortObject(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map((entry) => sortObject(entry));
  }
  if (value instanceof Uint8Array) {
    return value;
  }
  if (isRecord(value)) {
    const keys = Object.keys(value).sort((a, b) => a.localeCompare(b));
    const out: Record<string, unknown> = {};
    for (const key of keys) {
      out[key] = sortObject(value[key]);
    }
    return out;
  }
  return value;
}

export function encodeAppEnvelope(envelope: AppEnvelope): Uint8Array {
  const sorted = sortObject(envelope);
  return encode(sorted);
}

export function decodeAppEnvelope(bytes: Uint8Array): AppEnvelope {
  const decoded = decode(bytes);
  if (!isRecord(decoded)) {
    throw new Error("invalid app envelope");
  }
  return decoded as AppEnvelope;
}

export function encodeSocialPost(post: SocialPostV1): Uint8Array {
  return encodeAppEnvelope({
    type: post.type,
    version: post.version,
    payload: sortObject({
      body: post.body,
      mentions: post.mentions,
      parent_root: post.parent_root,
      thread_root: post.thread_root,
      attachments: post.attachments,
      extensions: post.extensions,
    }) as Record<string, unknown>,
  });
}

export function encodeMediaDescriptor(media: MediaDescriptorV1): Uint8Array {
  return encodeAppEnvelope({
    type: media.type,
    version: media.version,
    payload: sortObject({
      mime: media.mime,
      size: media.size,
      hash_hex: media.hash_hex,
      chunk_roots: media.chunk_roots,
      chunk_tag_hex: media.chunk_tag_hex,
      blurhash: media.blurhash,
      extensions: media.extensions,
    }) as Record<string, unknown>,
  });
}

export function encodeFileChunk(chunk: FileChunkV1): Uint8Array {
  return encodeAppEnvelope({
    type: chunk.type,
    version: chunk.version,
    payload: sortObject({
      data: chunk.data,
      index: chunk.index,
      total: chunk.total,
      extensions: chunk.extensions,
    }) as Record<string, unknown>,
  });
}

export function splitIntoFileChunks(bytes: Uint8Array): FileChunkV1[] {
  if (bytes.length <= MAX_INLINE_PAYLOAD) {
    return [
      {
        type: "chunk",
        version: 1,
        data: bytes,
        index: 0,
        total: 1,
      },
    ];
  }
  const chunkSize = Math.min(MAX_INLINE_PAYLOAD, MAX_OBJECT_SIZE - 1024);
  const total = Math.max(1, Math.ceil(bytes.length / chunkSize));
  const chunks: FileChunkV1[] = [];
  for (let index = 0; index < total; index += 1) {
    const slice = bytes.slice(index * chunkSize, (index + 1) * chunkSize);
    chunks.push({
      type: "chunk",
      version: 1,
      data: slice,
      index,
      total,
    });
  }
  return chunks;
}

export function buildMediaDescriptorFromChunks(
  chunks: FileChunkV1[],
  options: {
    mime: string;
    size: number;
    hash_hex: string;
    chunk_tag_hex?: string;
    chunk_roots?: string[];
    blurhash?: string;
  },
): MediaDescriptorV1 {
  return {
    type: "media_desc",
    version: 1,
    mime: options.mime,
    size: options.size,
    hash_hex: options.hash_hex,
    chunk_roots: options.chunk_roots ?? chunks.map(() => ""),
    chunk_tag_hex: options.chunk_tag_hex,
    blurhash: options.blurhash,
  };
}

export function extractReferences(envelope: AppEnvelope): {
  parentRoots: string[];
  threadRoots: string[];
  chunkRoots: string[];
  chunkTagHexes: string[];
} {
  const parentRoots: string[] = [];
  const threadRoots: string[] = [];
  const chunkRoots: string[] = [];
  const chunkTagHexes: string[] = [];

  if (envelope.type === "post" && isRecord(envelope.payload)) {
    const parent = envelope.payload.parent_root;
    const thread = envelope.payload.thread_root;
    if (typeof parent === "string") parentRoots.push(parent);
    if (typeof thread === "string") threadRoots.push(thread);
    const attachments = envelope.payload.attachments;
    if (Array.isArray(attachments)) {
      for (const entry of attachments) {
        if (isRecord(entry)) {
          const roots = entry.chunk_roots;
          if (Array.isArray(roots)) {
            for (const root of roots) {
              if (typeof root === "string") chunkRoots.push(root);
            }
          }
          const tagHex = entry.chunk_tag_hex;
          if (typeof tagHex === "string") chunkTagHexes.push(tagHex);
        }
      }
    }
  }
  if (envelope.type === "media_desc" && isRecord(envelope.payload)) {
    const roots = envelope.payload.chunk_roots;
    if (Array.isArray(roots)) {
      for (const root of roots) {
        if (typeof root === "string") chunkRoots.push(root);
      }
    }
    const tagHex = envelope.payload.chunk_tag_hex;
    if (typeof tagHex === "string") chunkTagHexes.push(tagHex);
  }

  if (envelope.type === "progressive_image" && isRecord(envelope.payload)) {
    const roots = envelope.payload.chunk_roots;
    if (Array.isArray(roots)) {
      for (const root of roots) {
        if (typeof root === "string") chunkRoots.push(root);
      }
    }
    const tagHex = envelope.payload.chunk_tag_hex;
    if (typeof tagHex === "string") chunkTagHexes.push(tagHex);
  }

  if (envelope.type === "video_manifest" && isRecord(envelope.payload)) {
    const roots = envelope.payload.chunk_roots;
    if (Array.isArray(roots)) {
      for (const root of roots) {
        if (typeof root === "string") chunkRoots.push(root);
      }
    }
    const tagHex = envelope.payload.chunk_tag_hex;
    if (typeof tagHex === "string") chunkTagHexes.push(tagHex);
  }

  return { parentRoots, threadRoots, chunkRoots, chunkTagHexes };
}

export function extractMentions(envelope: AppEnvelope): string[] {
  if (envelope.type !== "post" || !isRecord(envelope.payload)) {
    return [];
  }
  const mentions = envelope.payload.mentions;
  if (!Array.isArray(mentions)) return [];
  return mentions.filter((entry): entry is string => typeof entry === "string");
}

export function encodeCanonicalMap(map: Record<string, unknown>): Uint8Array {
  return encode(sortObject(map));
}
