# VEIL Flutter Example

Minimal Flutter example app that connects to a VEIL WebSocket lane and renders
basic shard/payload events.

## Run

```bash
cd apps/veil-flutter-example
flutter pub get
flutter run
```

## Configure

The example points at `ws://127.0.0.1:9001` by default. Update the WebSocket
URL in `lib/main.dart` if you run a relay elsewhere.

## Notes

- Uses `veil_sdk` from `packages/veil-sdk-dart`.
- Uses `WebSocketLane` and `VeilClient`.
- If you want persistence, switch the client cache store to
  `SqfliteShardCacheStore` (see `packages/veil-sdk-dart/lib/src/cache`).
