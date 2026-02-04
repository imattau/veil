# React Native Demo (Scaffold)

This folder is a starter note for integrating `@veil/sdk-js` in React Native.

## Recommended setup

1. Add the SDK package dependency (workspace/local while developing).
2. Use pure-js tag backend:

```ts
import { configureTagBackend } from "@veil/sdk-js";
configureTagBackend("pure-js");
```

3. Plug in a persistent cache store implementing `ShardCacheStore`.
4. Use `WebSocketLaneAdapter` or a custom lane adapter.

## Minimal snippet

```ts
import { VeilClient, WebSocketLaneAdapter } from "@veil/sdk-js";

const fast = new WebSocketLaneAdapter({ url: "wss://relay.example/ws", peerId: "fast" });
const fallback = new WebSocketLaneAdapter({ url: "wss://relay.example/ws2", peerId: "fallback" });

const client = new VeilClient(fast, fallback, {
  onShard(peer, bytes) {
    console.log("shard", peer, bytes.length);
  },
});
```
