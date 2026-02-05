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

## Build

```bash
npm run wasm:build
npm run sdk:build
```
