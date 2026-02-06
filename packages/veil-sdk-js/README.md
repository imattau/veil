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

## Endorsement ingestion helpers

`LocalWotPolicy` de-dupes endorsements per endorser/publisher pair and can prune
stale edges.

```ts
policy.addEndorsement(endorserPubkey, publisherPubkey, nowStep);
policy.pruneStaleEndorsements(nowStep);
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
