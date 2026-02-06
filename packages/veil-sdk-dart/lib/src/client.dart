import "dart:async";
import "dart:typed_data";

import "bridge/veil_bridge.dart";
import "cache/shard_cache_store.dart";
import "lanes/lane.dart";
import "models/shard_request.dart";
import "models/veil_types.dart";

class VeilClientHooks {
  final void Function(String peer, ShardMeta meta)? onShardMeta;
  final void Function(String objectRootHex, int have, int need)?
      onReconstructable;
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
  final bool enableShardRequests;
  final int requestFanout;
  final int requestHopLimit;
  final int requestCooldownMs;
  final int maxForwardHops;

  const VeilClientOptions({
    this.maxCacheEntries = 50000,
    this.requiredSignedNamespaces = const {},
    this.enableShardRequests = true,
    this.requestFanout = 2,
    this.requestHopLimit = 2,
    this.requestCooldownMs = 2000,
    this.maxForwardHops = 6,
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
  final Map<String, _ObjectShardState> _objectShardState = {};
  final Map<String, int> _cacheLastSeen = {};
  final Map<String, int> _cacheSeenCount = {};
  final Map<String, int> _shardForwardHops = {};
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
  void unsubscribe(TagHex tagHex) =>
      _subscriptions.remove(tagHex.toLowerCase());
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
      await _processInbound(msg, fastLane);
    }
    if (fallbackLane != null) {
      final fallback = await fallbackLane!.recv();
      if (fallback != null) {
        await _processInbound(fallback, fallbackLane!);
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
      _shardForwardHops.remove(key);
    }
  }

  Future<void> _processInbound(LaneMessage msg, VeilLane lane) async {
    try {
      final request = decodeShardRequest(Uint8List.fromList(msg.bytes));
      if (request != null) {
        await _handleShardRequest(msg.peer, lane, request);
        return;
      }

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
      _noteShard(meta, key);

      final bucket = _inbox.putIfAbsent(meta.objectRootHex, () => {});
      bucket[meta.index] = msg.bytes;
      if (bucket.length >= meta.k) {
        hooks.onReconstructable?.call(
          meta.objectRootHex,
          bucket.length,
          meta.k,
        );
        final reconstructed = await bridge.reconstructObjectPadded(
          bucket.values.map(Uint8List.fromList).toList(),
          meta.objectRootHex,
        );
        hooks.onReconstructed?.call(meta.objectRootHex, reconstructed);

        final objMeta = await bridge.decodeObjectMeta(reconstructed);
        hooks.onObjectMeta?.call(meta.objectRootHex, objMeta);

        if (options.requiredSignedNamespaces.contains(objMeta.namespace) &&
            !objMeta.signed) {
          _inbox.remove(meta.objectRootHex);
          return;
        }

        if (decryptKey != null) {
          final payload = await bridge.decryptObjectPayload(
            reconstructed,
            decryptKey!,
          );
          hooks.onPayload?.call(meta.objectRootHex, payload);
        }
        _inbox.remove(meta.objectRootHex);
      }

      await _maybeRequestMissing(meta, msg.peer);

      final hops = _shardForwardHops[key] ?? 0;
      if (options.maxForwardHops > 0 && hops >= options.maxForwardHops) {
        return;
      }

      for (final peer in _forwardPeers) {
        if (peer == msg.peer) continue;
        await fastLane.send(peer, msg.bytes);
      }
      if (_forwardPeers.isNotEmpty) {
        _shardForwardHops[key] = hops + 1;
      }
    } catch (err) {
      hooks.onDecodeError?.call(msg.peer, err);
    }
  }

  void _noteShard(ShardMeta meta, String key) {
    final state = _objectShardState.putIfAbsent(
      meta.objectRootHex,
      () => _ObjectShardState(
        k: meta.k,
        n: meta.n,
        tagHex: meta.tagHex.toLowerCase(),
      ),
    );
    state.k = meta.k;
    state.n = meta.n;
    state.tagHex = meta.tagHex.toLowerCase();
    state.indices.add(meta.index);
    state.lastSeenAt = DateTime.now().millisecondsSinceEpoch;
  }

  Future<void> _handleShardRequest(
    String peer,
    VeilLane lane,
    ShardRequestPayload request,
  ) async {
    if (!options.enableShardRequests) {
      return;
    }
    for (final index in request.want) {
      final key = "${request.objectRootHex}:${index}";
      final bytes = await cacheStore.get(key);
      if (bytes != null) {
        await lane.send(peer, bytes);
      }
    }
    if (!_subscriptions.contains(request.tagHex.toLowerCase())) {
      return;
    }
    if (options.requestHopLimit <= 0 ||
        request.hop >= options.requestHopLimit) {
      return;
    }
    await _sendShardRequest(peer, request.copyWith(hop: request.hop + 1));
  }

  Future<void> _maybeRequestMissing(ShardMeta meta, String peer) async {
    if (!options.enableShardRequests) {
      return;
    }
    final state = _objectShardState[meta.objectRootHex];
    if (state == null) {
      return;
    }
    if (state.indices.length >= state.k) {
      return;
    }
    if (state.indices.length < state.k - 1) {
      return;
    }
    final nowMs = DateTime.now().millisecondsSinceEpoch;
    if (nowMs - state.lastRequestAt < options.requestCooldownMs) {
      return;
    }
    final missing = <int>[];
    for (var idx = 0; idx < state.n; idx += 1) {
      if (!state.indices.contains(idx)) {
        missing.add(idx);
      }
    }
    if (missing.isEmpty) {
      return;
    }
    final needed = (state.k - state.indices.length).clamp(1, missing.length);
    final want = missing.take(needed).toList();
    state.lastRequestAt = nowMs;
    await _sendShardRequest(
      peer,
      ShardRequestPayload(
        objectRootHex: meta.objectRootHex,
        tagHex: meta.tagHex.toLowerCase(),
        k: meta.k,
        n: meta.n,
        want: want,
        hop: 0,
      ),
    );
  }

  Future<void> _sendShardRequest(
    String sourcePeer,
    ShardRequestPayload payload,
  ) async {
    if (options.requestFanout <= 0) {
      return;
    }
    final requestBytes = encodeShardRequest(payload);
    var sent = 0;
    for (final peer in _forwardPeers) {
      if (peer == sourcePeer) continue;
      await fastLane.send(peer, requestBytes);
      sent += 1;
      if (sent >= options.requestFanout) {
        break;
      }
    }
  }
}

class _ObjectShardState {
  int k;
  int n;
  String tagHex;
  final Set<int> indices = {};
  int lastRequestAt = 0;
  int lastSeenAt = 0;

  _ObjectShardState({required this.k, required this.n, required this.tagHex});
}
