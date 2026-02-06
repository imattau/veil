import "dart:html";
import "dart:typed_data";

import "shard_cache_store.dart";

class IndexedDbShardCacheStore implements ShardCacheStore {
  final String dbName;
  final String storeName;
  Database? _db;

  IndexedDbShardCacheStore({
    this.dbName = "veil-cache",
    this.storeName = "shards",
  });

  Future<Database> _open() async {
    if (_db != null) {
      return _db!;
    }
    final completer = Completer<Database>();
    final request = window.indexedDB!.open(dbName, version: 1,
        onUpgradeNeeded: (e) {
      final db = (e.target as Request).result as Database;
      if (!db.objectStoreNames!.contains(storeName)) {
        db.createObjectStore(storeName);
      }
    });
    request.onSuccess.listen((event) {
      _db = request.result as Database;
      completer.complete(_db!);
    });
    request.onError.listen((event) {
      completer.completeError(request.error ?? "indexeddb open failed");
    });
    return completer.future;
  }

  @override
  Future<List<int>?> get(String key) async {
    final db = await _open();
    final txn = db.transaction(storeName, "readonly");
    final store = txn.objectStore(storeName);
    final request = store.getObject(key);
    final completer = Completer<List<int>?>();
    request.onSuccess.listen((_) {
      final result = request.result;
      if (result is ByteBuffer) {
        completer.complete(Uint8List.view(result));
      } else if (result is Uint8List) {
        completer.complete(result);
      } else {
        completer.complete(null);
      }
    });
    request.onError.listen((_) {
      completer.completeError(request.error ?? "indexeddb get failed");
    });
    return completer.future;
  }

  @override
  Future<void> set(String key, List<int> value) async {
    final db = await _open();
    final txn = db.transaction(storeName, "readwrite");
    final store = txn.objectStore(storeName);
    store.put(value, key);
    await txn.completed;
  }

  @override
  Future<void> delete(String key) async {
    final db = await _open();
    final txn = db.transaction(storeName, "readwrite");
    final store = txn.objectStore(storeName);
    store.delete(key);
    await txn.completed;
  }

  @override
  Future<List<String>> keys() async {
    final db = await _open();
    final txn = db.transaction(storeName, "readonly");
    final store = txn.objectStore(storeName);
    final request = store.getAllKeys();
    final completer = Completer<List<String>>();
    request.onSuccess.listen((_) {
      final result = request.result;
      final keys = <String>[];
      if (result is List) {
        for (final item in result) {
          if (item is String) {
            keys.add(item);
          }
        }
      }
      completer.complete(keys);
    });
    request.onError.listen((_) {
      completer.completeError(request.error ?? "indexeddb keys failed");
    });
    return completer.future;
  }
}

ShardCacheStore createIndexedDbShardCacheStore({
  String dbName = "veil-cache",
  String storeName = "shards",
}) {
  return IndexedDbShardCacheStore(dbName: dbName, storeName: storeName);
}
