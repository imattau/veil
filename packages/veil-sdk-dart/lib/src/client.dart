import "dart:async";
import "dart:typed_data";

import "bridge/veil_bridge.dart";
import "cache/shard_cache_store.dart";
import "lanes/lane.dart";
import "models/veil_types.dart";

class VeilClientHooks {
  final void Function(String peer, ShardMeta meta)? onShardMeta;
  final void Function(String objectRootHex, int have, int need)? onReconstructable;
  final void Function(String objectRootHex, Uint8List bytes)? onReconstructed;
  final void Function(String objectRootHex, ObjectMeta meta)? onObjectMeta;
  final void Function(String objectRootHex, Uint8List payload)? onPayload;
  final void Function(String tagHex)? onIgnoredUnsubscribed;
  final void Function(String peer, Object error)? onDecodeError;

  const VeilClientHooks({
    this.onShardMeta,
    this.onReconstructable,
    this.onReconstructed,
    this.onObjectMeta,
    this.onPayload,
    this.onIgnoredUnsubscribed,
    this.onDecodeError,
  });
}

class VeilClientOptions {
  final int maxCacheEntries;
  final Set<int> requiredSignedNamespaces;

  const VeilClientOptions({
    this.maxCacheEntries = 50_000,
    this.requiredSignedNamespaces = const {},
  });
}

class VeilClient {
  final VeilLane fastLane;
  final VeilLane? fallbackLane;
  final ShardCacheStore cacheStore;
  final VeilBridge bridge;
  final int pollIntervalMs;
  final VeilClientHooks hooks;
  final List<int>? decryptKey;
  final VeilClientOptions options;

  final Set<TagHex> _subscriptions = {};
  final List<String> _forwardPeers = [];
  final Map<String, Map<int, List<int>>> _inbox = {};
  final Map<String, int> _cacheLastSeen = {};
  final Map<String, int> _cacheSeenCount = {};
  bool _running = false;
  Timer? _timer;

  VeilClient({
    required this.fastLane,
    this.fallbackLane,
    ShardCacheStore? cacheStore,
    VeilBridge? bridge,
    this.pollIntervalMs = 50,
    this.hooks = const VeilClientHooks(),
    this.decryptKey,
    this.options = const VeilClientOptions(),
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

  Future<void> _evictIfNeeded() async {
    final maxEntries = options.maxCacheEntries;
    if (maxEntries <= 0) {
      return;
    }
    if (_cacheLastSeen.length <= maxEntries) {
      return;
    }

    final entries = _cacheLastSeen.entries.toList();
    entries.sort((a, b) {
      final countA = _cacheSeenCount[a.key] ?? 0;
      final countB = _cacheSeenCount[b.key] ?? 0;
      if (countA != countB) {
        return countB.compareTo(countA); // evict most common first
      }
      return a.value.compareTo(b.value); // then oldest
    });

    final overflow = entries.length - maxEntries;
    for (var i = 0; i < overflow; i += 1) {
      final key = entries[i].key;
      await cacheStore.delete(key);
      _cacheLastSeen.remove(key);
      _cacheSeenCount.remove(key);
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
      _cacheLastSeen[key] = DateTime.now().millisecondsSinceEpoch;
      _cacheSeenCount[key] = (_cacheSeenCount[key] ?? 0) + 1;
      await _evictIfNeeded();

      final bucket = _inbox.putIfAbsent(meta.objectRootHex, () => {});
      bucket[meta.index] = msg.bytes;
      if (bucket.length >= meta.k) {
        hooks.onReconstructable?.call(meta.objectRootHex, bucket.length, meta.k);
        final reconstructed = await bridge.reconstructObjectPadded(
          bucket.values.map(Uint8List.fromList).toList(),
          meta.objectRootHex,
        );
        hooks.onReconstructed?.call(meta.objectRootHex, reconstructed);

        final objMeta = await bridge.decodeObjectMeta(reconstructed);
        hooks.onObjectMeta?.call(meta.objectRootHex, objMeta);

        if (options.requiredSignedNamespaces.contains(objMeta.namespace) && !objMeta.signed) {
          _inbox.remove(meta.objectRootHex);
          return;
        }

        if (decryptKey != null) {
          final payload = await bridge.decryptObjectPayload(reconstructed, decryptKey!);
          hooks.onPayload?.call(meta.objectRootHex, payload);
        }
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
