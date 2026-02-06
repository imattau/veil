import "dart:async";
import "dart:typed_data";

import "bridge/api.dart" as frb_api;
import "bridge/veil_bridge.dart";
import "cache/shard_cache_store.dart";
import "lanes/lane.dart";
import "models/veil_types.dart";

class VeilClientHooks {
  final void Function(String peer, ShardMeta meta)? onShardMeta;
  final void Function(String objectRootHex, int have, int need)? onReconstructable;
  final void Function(String objectRootHex, Uint8List bytes)? onReconstructed;
  final void Function(String tagHex)? onIgnoredUnsubscribed;
  final void Function(String peer, Object error)? onDecodeError;

  const VeilClientHooks({
    this.onShardMeta,
    this.onReconstructable,
    this.onReconstructed,
    this.onIgnoredUnsubscribed,
    this.onDecodeError,
  });
}

class VeilClient {
  final VeilLane fastLane;
  final VeilLane? fallbackLane;
  final ShardCacheStore cacheStore;
  final VeilBridge bridge;
  final int pollIntervalMs;
  final VeilClientHooks hooks;

  final Set<TagHex> _subscriptions = {};
  final List<String> _forwardPeers = [];
  final Map<String, Map<int, List<int>>> _inbox = {};
  bool _running = false;
  Timer? _timer;

  VeilClient({
    required this.fastLane,
    this.fallbackLane,
    ShardCacheStore? cacheStore,
    VeilBridge? bridge,
    this.pollIntervalMs = 50,
    this.hooks = const VeilClientHooks(),
  })  : cacheStore = cacheStore ?? MemoryShardCacheStore(),
        bridge = bridge ?? const VeilBridge();

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
    try {
      final meta = await bridge.decodeShardMeta(msg.bytes);
      hooks.onShardMeta?.call(msg.peer, meta);
      final tagHex = meta.tagHex.toLowerCase();
      if (!_subscriptions.contains(tagHex)) {
        hooks.onIgnoredUnsubscribed?.call(tagHex);
        return;
      }

      final key = "${meta.objectRootHex}:${meta.index}";
      await cacheStore.set(key, msg.bytes);

      final bucket = _inbox.putIfAbsent(meta.objectRootHex, () => {});
      bucket[meta.index] = msg.bytes;
      if (bucket.length >= meta.k) {
        hooks.onReconstructable?.call(meta.objectRootHex, bucket.length, meta.k);
        final reconstructed = await frb_api.reconstructObjectPaddedFromShards(
          shardBytes: bucket.values.map((b) => Uint8List.fromList(b)).toList(),
          expectedRootHex: meta.objectRootHex,
        );
        hooks.onReconstructed?.call(meta.objectRootHex, reconstructed);
        _inbox.remove(meta.objectRootHex);
      }

      for (final peer in _forwardPeers) {
        if (peer == msg.peer) continue;
        await fastLane.send(peer, msg.bytes);
      }
    } catch (err) {
      hooks.onDecodeError?.call(msg.peer, err);
    }
  }
}
