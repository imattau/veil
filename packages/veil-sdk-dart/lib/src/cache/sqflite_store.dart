import "package:sqflite/sqflite.dart";

import "shard_cache_store.dart";

class SqfliteShardCacheStore implements ShardCacheStore, RarityTrackingStore {
  final Database db;
  final String table;

  SqfliteShardCacheStore({required this.db, this.table = "veil_shards"});

  Future<void> init() async {
    await db.execute(
      "CREATE TABLE IF NOT EXISTS $table (key TEXT PRIMARY KEY, bytes BLOB, seen_count INTEGER DEFAULT 0, last_seen INTEGER DEFAULT 0)",
    );
  }

  @override
  Future<List<int>?> get(String key) async {
    final rows = await db.query(table, where: "key = ?", whereArgs: [key]);
    if (rows.isEmpty) {
      return null;
    }
    final value = rows.first["bytes"];
    if (value is List<int>) {
      return value;
    }
    return null;
  }

  @override
  Future<void> set(String key, List<int> value) async {
    final now = DateTime.now().millisecondsSinceEpoch;
    await db.insert(
      table,
      {
        "key": key,
        "bytes": value,
        "seen_count": 0,
        "last_seen": now,
      },
      conflictAlgorithm: ConflictAlgorithm.ignore,
    );
    await db.update(
      table,
      {"bytes": value, "last_seen": now},
      where: "key = ?",
      whereArgs: [key],
    );
  }

  @override
  Future<void> delete(String key) async {
    await db.delete(table, where: "key = ?", whereArgs: [key]);
  }

  @override
  Future<List<String>> keys() async {
    final rows = await db.query(table, columns: ["key"]);
    return rows.map((row) => row["key"] as String).toList();
  }

  @override
  Future<void> noteSeen(String key) async {
    final now = DateTime.now().millisecondsSinceEpoch;
    final updated = await db.rawUpdate(
      "UPDATE $table SET seen_count = seen_count + 1, last_seen = ? WHERE key = ?",
      [now, key],
    );
    if (updated == 0) {
      await db.insert(table, {
        "key": key,
        "bytes": <int>[],
        "seen_count": 1,
        "last_seen": now,
      });
    }
  }

  @override
  Future<Map<String, int>> loadSeenCounts(List<String> keys) async {
    if (keys.isEmpty) {
      return {};
    }
    final placeholders = List.filled(keys.length, "?").join(", ");
    final rows = await db.query(
      table,
      columns: ["key", "seen_count"],
      where: "key IN ($placeholders)",
      whereArgs: keys,
    );
    final out = <String, int>{};
    for (final row in rows) {
      final key = row["key"] as String;
      final count = row["seen_count"] as int? ?? 0;
      out[key] = count;
    }
    return out;
  }
}
