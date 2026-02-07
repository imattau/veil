# VEIL Social App Schemas (Non-Normative)

These application schemas sit **above** the core VEIL protocol. They are optional,
forward-compatible, and designed for deterministic delivery across clients.

## 1) Root Wrapper (Required)

Every application payload MUST be wrapped in an envelope that includes a `type`
string and a `version` integer.

```
AppEnvelope {
  type: string
  version: u16
  payload: map
  extensions?: map
}
```

Rules:
- `type` routes to a handler (e.g., `"post"`, `"media_desc"`, `"chunk"`).
- `version` supports safe evolution over time.
- `extensions` allows experiments without a breaking bump.
- CBOR maps MUST be encoded with **lexicographically sorted keys** to keep
  `object_root` deterministic across clients.

## 2) Namespace Guidance (Protocol Header)

Use the `ObjectV1.namespace` field to segregate categories:
- Namespace 1: Public social
- Namespace 2: Private messaging
- Namespace 3+: Optional app-specific domains

Clients can apply different ingest policies per namespace.

## 3) Core Social + Media Schemas

### SocialPost Payload (envelope type = "post", version = 1)
```
SocialPostV1 {
  body: string
  parent_root?: bytes32
  thread_root?: bytes32
  attachments?: MediaDescriptorV1[]
  extensions?: map
}
```

### MediaDescriptor Payload (envelope type = "media_desc", version = 1)
```
MediaDescriptorV1 {
  mime: string
  size: u64
  hash_hex: string
  chunk_roots: bytes32[]        // ordered
  chunk_tag_hex?: string        // optional tag for chunk discovery
  extensions?: map
}
```

### FileChunk Payload (envelope type = "chunk", version = 1)
```
FileChunkV1 {
  data: bytes
  index: u32
  total: u32
  extensions?: map
}
```

## 4) Deterministic Plugins (Suggested)

**Auto-Fetcher**: On reconstructed payloads, detect `MediaDescriptor` or
`parent_root/thread_root` and subscribe to missing tags/roots.

**Thread Context Manager**: For `parent_root` / `thread_root`, request missing
objects to fill the conversation context.

### Example (SDK-js)

```ts
import { VeilClient, createAutoFetchPlugin, createThreadContextPlugin } from "@veil/sdk-js";

const client = new VeilClient(fastLane, fallbackLane, hooks, {
  plugins: [
    createAutoFetchPlugin({
      resolveTagForRoot: (root) => rootToTag.get(root) ?? null,
    }),
    createThreadContextPlugin({
      resolveTagForRoot: (root) => rootToTag.get(root) ?? null,
    }),
  ],
});

// After your app reconstructs a full object:
client.notifyObject(objectRootHex, objectBytes);
```

### Example (SDK-dart)

```dart
import "package:veil_sdk/veil_sdk.dart";

final client = VeilClient(
  fastLane: fastLane,
  fallbackLane: fallbackLane,
  hooks: const VeilClientHooks(),
  options: VeilClientOptions(
    plugins: [
      AutoFetchPlugin(
        resolveTagForRoot: (root, _) => rootToTag[root],
      ),
      ThreadContextPlugin(
        resolveTagForRoot: (root, _) => rootToTag[root],
      ),
    ],
  ),
);

// After reconstruction:
client.notifyObject(objectRootHex, objectBytes);
```

## 5) Constraints (Implementer Notes)

- **Size enforcement**: if payload > ~250,000 bytes, split into `FileChunk` and
  reference via `MediaDescriptor`.
- **Signature enforcement**: identity-linked types (profiles) MUST set
  `OBJECT_FLAG_SIGNED`.
- **Cache priority**: auto-fetched shards should be kept longer by rarity-biased
  eviction to complete reconstruction.
