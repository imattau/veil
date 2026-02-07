abstract class ShardCacheStore {
  Future<List<int>?> get(String key);
  Future<void> set(String key, List<int> value);
  Future<void> delete(String key);
  Future<List<String>> keys();
}

abstract class RarityTrackingStore {
  Future<void> noteSeen(String key);
  Future<Map<String, int>> loadSeenCounts(List<String> keys);
}

class MemoryShardCacheStore implements ShardCacheStore, RarityTrackingStore {
  final Map<String, List<int>> _store = {};
  final Map<String, int> _seen = {};

  @override
  Future<List<int>?> get(String key) async => _store[key];

  @override
  Future<void> set(String key, List<int> value) async {
    _store[key] = value;
  }

  @override
  Future<void> delete(String key) async {
    _store.remove(key);
    _seen.remove(key);
  }

  @override
  Future<List<String>> keys() async => _store.keys.toList();

  @override
  Future<void> noteSeen(String key) async {
    _seen.update(key, (value) => value + 1, ifAbsent: () => 1);
  }

  @override
  Future<Map<String, int>> loadSeenCounts(List<String> keys) async {
    final out = <String, int>{};
    for (final key in keys) {
      final count = _seen[key];
      if (count != null) {
        out[key] = count;
      }
    }
    return out;
  }
}
