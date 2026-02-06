import "dart:async";

import "cache/shard_cache_store.dart";
import "lanes/lane.dart";
import "models/veil_types.dart";

class VeilClient {
  final VeilLane fastLane;
  final VeilLane? fallbackLane;
  final ShardCacheStore cacheStore;
  final int pollIntervalMs;

  final Set<TagHex> _subscriptions = {};
  final List<String> _forwardPeers = [];
  bool _running = false;
  Timer? _timer;

  VeilClient({
    required this.fastLane,
    this.fallbackLane,
    ShardCacheStore? cacheStore,
    this.pollIntervalMs = 50,
  }) : cacheStore = cacheStore ?? MemoryShardCacheStore();

  void subscribe(TagHex tagHex) => _subscriptions.add(tagHex.toLowerCase());
  void unsubscribe(TagHex tagHex) => _subscriptions.remove(tagHex.toLowerCase());
  List<TagHex> subscriptions() => _subscriptions.toList();

  void setForwardPeers(List<String> peers) {
    _forwardPeers
      ..clear()
      ..addAll(peers);
  }

  void start() {
    if (_running) return;
    _running = true;
    _timer = Timer.periodic(Duration(milliseconds: pollIntervalMs), (_) {
      unawaited(tick());
    });
  }

  void stop() {
    _running = false;
    _timer?.cancel();
    _timer = null;
  }

  Future<void> tick() async {
    if (!_running) return;
    final msg = await fastLane.recv();
    if (msg != null) {
      await _processInbound(msg);
    }
    if (fallbackLane != null) {
      final fallback = await fallbackLane!.recv();
      if (fallback != null) {
        await _processInbound(fallback);
      }
    }
  }

  Future<void> _processInbound(LaneMessage msg) async {
    // Placeholder: decode shard meta via Rust bridge.
    // If subscribed, cache and forward.
    await cacheStore.set("${DateTime.now().microsecondsSinceEpoch}", msg.bytes);

    for (final peer in _forwardPeers) {
      if (peer == msg.peer) continue;
      await fastLane.send(peer, msg.bytes);
    }
  }
}
