# Android Node RPC (Localhost HTTP + WS)

This document defines the **local node RPC contract** used by the Android UI.
The UI **must not use the SDK**; it must talk directly to the node API.

## Transport
- Base URL: `http://127.0.0.1:<port>`
- WebSocket: `ws://127.0.0.1:<port>/events`
- Auth: `x-veil-token` header (optional if `VEIL_NODE_TOKEN` is empty)

## Versioning
- Event envelope has `version` and monotonically increasing `seq`.
- The server currently uses `version: 1`.

## Errors
For `400 Bad Request`, the server responds with:
```json
{ "code": "invalid_bundle", "message": "bundle serialization failed" }
```

Common error codes:
- `payload_too_large`
- `bundle_too_large`
- `invalid_channel`
- `author_mismatch` / `follower_mismatch` / `muter_mismatch` / `blocker_mismatch`
- `invalid_followee` / `invalid_muted` / `invalid_blocked`

## Endpoints

### `GET /health`
Unauthenticated liveness probe.
Response:
```json
{ "status": "ok", "version": "0.1.0" }
```

### `GET /status`
Returns node status, lane health, queue depth, and cache stats.

### `GET /identity`
Returns node identity pubkey.

### `POST /identity/rotate`
Rotates node identity, updates signer, returns new pubkey.

### `POST /publish`
Queues a raw payload string for publish.
Payload size limit: `256 KiB`.

### `POST /profile`
Queues a `ProfileBundle` with identity-bound author.
Bundle size limit: `256 KiB`.

### `POST /post`
Queues a `PostBundle` with identity-bound author.

### `POST /reaction`
Queues a `ReactionBundle` with identity-bound author.

### `POST /direct_message`
Queues a `DirectMessageBundle` with identity-bound author.

### `POST /group_message`
Queues a `GroupMessageBundle` with identity-bound author.

### `POST /media`
Queues a `MediaBundle` with identity-bound author.

### `POST /list`
Queues a `ListBundle` (bookmarks, curated feeds, pinned items).

### `POST /app_preferences`
Queues an `AppPreferencesBundle` (JSON settings synced via identity).

### `POST /zap`
Queues a `ZapBundle` (lightning payment receipt/proof).

### `POST /repost`
Queues a `RepostBundle` (boost).

### `POST /poll`
Queues a `PollBundle`.

### `POST /poll_vote`
Queues a `PollVoteBundle`.

### `POST /follow`
Queues a `FollowBundle` with identity-bound follower.

### `POST /mute`
Queues a `MuteBundle` with identity-bound muter.

### `POST /block`
Queues a `BlockBundle` with identity-bound blocker.

### `POST /subscribe`
Subscribes to a tag (hex).

### `POST /unsubscribe`
Unsubscribes from a tag (hex).

### `GET /policy`
Returns WoT policy summary (trusted/muted/blocked counts + config).

### `POST /policy/config`
Updates WoT policy configuration (quotas, thresholds, budgets).

### `POST /policy/trust`
Adds a pubkey to the trusted set.

### `POST /policy/untrust`
Removes a pubkey from the trusted set.

### `POST /policy/mute`
Adds a pubkey to the muted set.

### `POST /policy/unmute`
Removes a pubkey from the muted set.

### `POST /policy/block`
Adds a pubkey to the blocked set.

### `POST /policy/unblock`
Removes a pubkey from the blocked set.

### `POST /policy/explain`
Returns WoT explanation for a publisher pubkey.

### `GET /contact/self`
Returns a contact bundle for this node (peer id, ws/quic endpoints, pubkey, rpc url, lan addrs).

### `GET /contact`
Returns known contact bundles.

### `POST /contact`
Imports a contact bundle and adds it to peer lists.

### `POST /discovery/announce`
Announces a contact bundle and returns nearest neighbors (DHT-style lookup).
Also publishes a discovery announce over transport lanes.

### `POST /discovery/lookup`
Returns nearest contacts for a peer id or pubkey hex.
Also publishes a discovery lookup over transport lanes.

### `POST /discovery/gossip`
Exchanges contact bundles with another node (bounded by gossip limits).
Also publishes a discovery gossip over transport lanes.

### `GET /shard/:id`
Fetches cached shard bytes (base64).

### `GET /object/:root`
Attempts object reconstruction from cached shards (base64 encoded object).

## WebSocket Events

Endpoint: `GET /events?since=<seq>`
- Optional `since` query parameter replays buffered events with `seq > since`.

Envelope:
```json
{
  "version": 1,
  "seq": 42,
  "event": "payload",
  "data": { ... }
}
```

Event types:
- `node_status`
- `publish_queued`
- `publish_sent`
- `publish_failed`
- `lane_health`
- `payload`
- `feed_bundle`
- `policy_updated`

## Notes
- Events are buffered in memory and replayed for a small window.
- The node emits `node_status` on every WS connect.

## Android Embedded Binary
- Android UI uses an embedded node binary copied from app assets to `filesDir/veil_node`.
- The foreground service is responsible for starting/stopping the binary.
- The service generates a local auth token (`filesDir/node_token.txt`) and sets `VEIL_NODE_TOKEN`.
- UI reads the same token and attaches it as `x-veil-token` for HTTP/WS.
- The service sets `VEIL_NODE_CACHE_STATE` to persist the shard cache (`node_cache.cbor`).
- The service sets `VEIL_NODE_QUIC_BIND` + `VEIL_NODE_QUIC_SERVER_NAME` for inbound QUIC.
- Optional: `VEIL_NODE_QUIC_PUBLIC`, `VEIL_NODE_WS_PUBLIC`, `VEIL_NODE_RPC_URL` to advertise public endpoints.
- Optional: `VEIL_DISCOVERY_BOOTSTRAP` (comma-separated URLs), `VEIL_DISCOVERY_INTERVAL_MS`,
  `VEIL_DISCOVERY_GOSSIP_MAX`, `VEIL_DISCOVERY_TRANSPORT` for gossip-based discovery.
- Optional: `VEIL_LAN_DISCOVERY=1`, `VEIL_LAN_DISCOVERY_PORT`, `VEIL_LAN_DISCOVERY_INTERVAL_MS`
  to enable LAN broadcast discovery.
- Optional: `VEIL_DISCOVERY_NAMESPACE` to override the transport discovery namespace (default 4096).
- Build the per-ABI assets via `scripts/build_android_node.sh` which writes:
  - `apps/android-node/src/ui/android/app/src/main/assets/veil_node_arm64-v8a`
  - `apps/android-node/src/ui/android/app/src/main/assets/veil_node_armeabi-v7a`
  - `apps/android-node/src/ui/android/app/src/main/assets/veil_node_x86_64`
