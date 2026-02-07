# @veil/sdk-js

Client-native SDK scaffold for VEIL web/Node/React-Native clients.

## Runtime backend modes

- `auto` (default): wasm on web, pure-js hash path on React Native
- `wasm`: force wasm backend
- `pure-js`: force pure TypeScript backend

```ts
import { configureTagBackend } from "@veil/sdk-js";
configureTagBackend("auto");
```

## React Native notes

- Use `configureTagBackend("pure-js")` or `auto`.
- Provide a persistent `ShardCacheStore` implementation (AsyncStorage/MMKV/SQLite).
- Use `WebSocketLaneAdapter` (or your own adapter) for transport lanes.

## Transport health snapshots

`LaneAdapter` can optionally expose `healthSnapshot()` so client telemetry aligns
with Rust transport counters (`outbound*`, `inbound*`, reconnect attempts).

## Local WoT helpers (UI prioritization)

SDK-js now includes a local WoT policy helper aligned with the Rust node policy
model for tiering and explainability.

```ts
import { LocalWotPolicy, rankFeedItemsByTrust } from "@veil/sdk-js";

const policy = new LocalWotPolicy();
policy.trust(myFriendPubkeyHex);
policy.addEndorsement(myFriendPubkeyHex, creatorPubkeyHex, nowStep - 5);

const explanation = policy.explainPublisher(creatorPubkeyHex, nowStep);
// explanation.tier => "trusted" | "known" | "unknown" | "muted" | "blocked"

const ranked = rankFeedItemsByTrust(feedItems, policy, nowStep);
```

Use `policy.exportJson()` / `LocalWotPolicy.importJson(json)` to persist and
restore trust lists + endorsements in app storage.

## Tier-aware forwarding (runtime glue)

`VeilClient` can use a WoT policy to order forward peers and reserve an
unknown budget floor under load.

```ts
import { VeilClient, LocalWotPolicy } from "@veil/sdk-js";

const policy = new LocalWotPolicy();
policy.trust(myFriendPubkeyHex);

const client = new VeilClient(fastLane, fallbackLane, hooks, {
  wotPolicy: policy,
  resolvePublisher: (peerId) => peerToPublisherMap.get(peerId) ?? null,
  forwardingQuotas: { trusted: 0.7, known: 0.25, unknown: 0.05 },
  unknownForwardFloor: 0.05,
});
```

## Shard pull requests (missing shard recovery)

`VeilClient` can auto-request missing shards when it has `k-1` indices.
Requests are forwarded with a hop limit and cooldown to avoid flooding.

```ts
const client = new VeilClient(fastLane, fallbackLane, hooks, {
  enableShardRequests: true,
  requestFanout: 2,
  requestHopLimit: 2,
  requestCooldownMs: 2000,
  maxForwardHops: 6,
});
```

## Overlapping rendezvous tags

For epoch transitions, derive current + adjacent RV tags to overlap windows.

```ts
import { deriveRvTagWindowHex } from "@veil/sdk-js";

const tags = await deriveRvTagWindowHex(pubkeyHex, nowSeconds, namespace, {
  epochSeconds: 86_400,
  overlapSeconds: 3_600,
});
```

## Endorsement ingestion helpers

`LocalWotPolicy` de-dupes endorsements per endorser/publisher pair and can prune
stale edges.

```ts
policy.addEndorsement(endorserPubkey, publisherPubkey, nowStep);
policy.pruneStaleEndorsements(nowStep);
```

## Blob manifests (app-level, non-normative)

Use lightweight manifests to describe multi-object attachments.

```ts
import { encodeBlobManifestV1 } from "@veil/sdk-js";

const manifestBytes = encodeBlobManifestV1({
  version: 1,
  mime: "image/png",
  size: 245_103,
  hashHex: imageHashHex,
  chunks: [{ objectRootHex, tagHex, size: 131_072 }],
  filename: "avatar.png",
});
```

## Social app schemas (non-normative)

Use the app schema helpers for `post`, `media_desc`, and `chunk` payloads. Maps
are encoded with lexicographically sorted keys to keep deterministic roots.

```ts
import { encodeSocialPost, extractReferences } from "@veil/sdk-js";

const bytes = encodeSocialPost({
  type: "post",
  version: 1,
  body: "hello",
  mentions: ["<pubkey_hex>"],
  thread_root: threadRootHex,
  attachments: [mediaDescriptor],
});

const refs = extractReferences({ type: "post", version: 1, payload: { ... } });
```

## Auto-fetch + thread context plugins

Attach the built-in plugins to auto-subscribe for parent/thread roots and media
chunks.

```ts
import { VeilClient, createAutoFetchPlugin, createThreadContextPlugin } from "@veil/sdk-js";

const client = new VeilClient(fastLane, fallbackLane, hooks, {
  plugins: [
    createAutoFetchPlugin({
      resolveTagForRoot: (root) => rootTagIndex.get(root) ?? null,
    }),
    createThreadContextPlugin({
      resolveTagForRoot: (root) => rootTagIndex.get(root) ?? null,
    }),
  ],
});

// When your app reconstructs an object, forward it to the plugins:
// client.notifyObject(objectRootHex, objectBytes);
```

## Channel-scoped helpers

Use channel-scoped helpers to avoid cross-channel feed collisions:

```ts
import { deriveChannelNamespace, deriveChannelFeedTagHex } from "@veil/sdk-js";

const ns = deriveChannelNamespace(7, "general");
const tag = await deriveChannelFeedTagHex(pubkeyHex, 7, "general");
```

## Build

```bash
npm run wasm:build
npm run sdk:build
```
