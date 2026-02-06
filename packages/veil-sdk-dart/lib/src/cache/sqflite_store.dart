import "package:sqflite/sqflite.dart";

import "shard_cache_store.dart";

class SqfliteShardCacheStore implements ShardCacheStore {
  final Database db;
  final String table;

  SqfliteShardCacheStore({required this.db, this.table = "veil_shards"});

  Future<void> init() async {
    await db.execute(
      "CREATE TABLE IF NOT EXISTS $table (key TEXT PRIMARY KEY, bytes BLOB)",
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
    await db.insert(table, {
      "key": key,
      "bytes": value,
    }, conflictAlgorithm: ConflictAlgorithm.replace);
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
}
