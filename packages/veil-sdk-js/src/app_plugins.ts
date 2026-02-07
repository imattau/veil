import { decodeAppEnvelope, extractReferences } from "./app_schemas";
import type { AppEnvelope } from "./app_schemas";
import type { VeilClient, VeilClientPlugin } from "./client";

export type RootTagResolver = (
  objectRootHex: string,
  envelope?: AppEnvelope,
) => string | null;

export interface AutoFetchPluginOptions {
  resolveTagForRoot?: RootTagResolver;
  priority?: number;
}

export function createAutoFetchPlugin(
  options: AutoFetchPluginOptions = {},
): VeilClientPlugin {
  const priority = options.priority ?? 1;
  return {
    onObject(client: VeilClient, objectRootHex: string, objectBytes: Uint8Array) {
      let envelope: AppEnvelope;
      try {
        envelope = decodeAppEnvelope(objectBytes);
      } catch {
        return;
      }
      const refs = extractReferences(envelope);
      for (const tagHex of refs.chunkTagHexes) {
        client.subscribe(tagHex);
      }

      const resolveTag = options.resolveTagForRoot;
      const allRoots = [
        ...refs.parentRoots,
        ...refs.threadRoots,
        ...refs.chunkRoots,
      ];
      for (const root of allRoots) {
        if (priority > 0) {
          client.prioritizeObjectRoot(root, priority);
        }
        if (resolveTag) {
          const tag = resolveTag(root, envelope);
          if (tag) {
            client.subscribe(tag);
          }
        }
      }
    },
  };
}

export interface ThreadContextPluginOptions {
  resolveTagForRoot: RootTagResolver;
  priority?: number;
}

export function createThreadContextPlugin(
  options: ThreadContextPluginOptions,
): VeilClientPlugin {
  const priority = options.priority ?? 1;
  return {
    onObject(client: VeilClient, objectRootHex: string, objectBytes: Uint8Array) {
      let envelope: AppEnvelope;
      try {
        envelope = decodeAppEnvelope(objectBytes);
      } catch {
        return;
      }
      const refs = extractReferences(envelope);
      const roots = [...refs.parentRoots, ...refs.threadRoots];
      for (const root of roots) {
        if (priority > 0) {
          client.prioritizeObjectRoot(root, priority);
        }
        const tag = options.resolveTagForRoot(root, envelope);
        if (tag) {
          client.subscribe(tag);
        }
      }
    },
  };
}
