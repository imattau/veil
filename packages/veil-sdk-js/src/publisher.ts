import { bytesToHex } from "@noble/hashes/utils";
import { blake3 } from "@noble/hashes/blake3";
import {
  encodeFileChunk,
  encodeMediaDescriptor,
  encodeProfile,
  encodeSocialPost,
  splitIntoFileChunks,
  type MediaDescriptorV1,
  type ProfileV1,
} from "./app_schemas";

export type PublishObject = {
  objectRootHex: string;
  objectBytes: Uint8Array;
};

export type PublishBatch = {
  rootObject: PublishObject;
  relatedObjects: PublishObject[];
};

function deriveObjectRootHex(bytes: Uint8Array): string {
  return bytesToHex(blake3(bytes, { dkLen: 32 }));
}

export function buildObject(bytes: Uint8Array): PublishObject {
  return { objectRootHex: deriveObjectRootHex(bytes), objectBytes: bytes };
}

export function buildSocialPost(
  body: string,
  options: {
    mentions?: string[];
    parentRoot?: string;
    threadRoot?: string;
    attachments?: MediaDescriptorV1[];
    extensions?: Record<string, unknown>;
  } = {},
): PublishObject {
  const bytes = encodeSocialPost({
    type: "post",
    version: 1,
    body,
    mentions: options.mentions,
    parent_root: options.parentRoot,
    thread_root: options.threadRoot,
    attachments: options.attachments,
    extensions: options.extensions,
  });
  return buildObject(bytes);
}

export function buildMediaDescriptor(media: MediaDescriptorV1): PublishObject {
  return buildObject(encodeMediaDescriptor(media));
}

export function buildProfile(profile: ProfileV1): PublishObject {
  return buildObject(encodeProfile(profile));
}

export function buildFileChunks(bytes: Uint8Array): PublishObject[] {
  return splitIntoFileChunks(bytes).map((chunk) => buildObject(encodeFileChunk(chunk)));
}

export function buildPostWithAttachments(
  body: string,
  attachments: Uint8Array[],
  mimeTypes: string[],
  mentions?: string[],
): PublishBatch {
  const relatedObjects: PublishObject[] = [];
  const descriptors: MediaDescriptorV1[] = [];

  attachments.forEach((data, index) => {
    const mime = mimeTypes[index] ?? "application/octet-stream";
    const chunkObjects = buildFileChunks(data);
    relatedObjects.push(...chunkObjects);
    const chunkRoots = chunkObjects.map((obj) => obj.objectRootHex);
    descriptors.push({
      type: "media_desc",
      version: 1,
      mime,
      size: data.length,
      hash_hex: "",
      chunk_roots: chunkRoots,
    });
  });

  const descriptorObjects = descriptors.map((desc) => buildMediaDescriptor(desc));
  relatedObjects.push(...descriptorObjects);

  const post = buildSocialPost(body, { attachments: descriptors, mentions });
  return { rootObject: post, relatedObjects };
}

export class PublishQueue {
  private readonly queue: PublishObject[] = [];
  private readonly maxQueueSize: number;

  constructor(maxQueueSize = 500) {
    this.maxQueueSize = maxQueueSize;
  }

  enqueue(object: PublishObject): void {
    if (this.queue.length >= this.maxQueueSize) {
      this.queue.shift();
    }
    this.queue.push(object);
  }

  enqueueAll(objects: PublishObject[]): void {
    objects.forEach((obj) => this.enqueue(obj));
  }

  pop(): PublishObject | undefined {
    return this.queue.shift();
  }

  clear(): void {
    this.queue.length = 0;
  }

  get isEmpty(): boolean {
    return this.queue.length === 0;
  }
}
