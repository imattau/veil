abstract class ShardCacheStore {
  Future<List<int>?> get(String key);
  Future<void> set(String key, List<int> value);
  Future<void> delete(String key);
  Future<List<String>> keys();
}

class MemoryShardCacheStore implements ShardCacheStore {
  final Map<String, List<int>> _store = {};

  @override
  Future<List<int>?> get(String key) async => _store[key];

  @override
  Future<void> set(String key, List<int> value) async {
    _store[key] = value;
  }

  @override
  Future<void> delete(String key) async {
    _store.remove(key);
  }

  @override
  Future<List<String>> keys() async => _store.keys.toList();
}
