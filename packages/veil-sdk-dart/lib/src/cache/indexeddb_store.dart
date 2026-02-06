import "shard_cache_store.dart";

// Web-only IndexedDB implementation (dart:html). Throws on other platforms.
// Using a factory helps avoid importing dart:html on non-web platforms.
ShardCacheStore createIndexedDbShardCacheStore({
  String dbName = "veil-cache",
  String storeName = "shards",
}) {
  throw UnsupportedError("IndexedDbShardCacheStore is only available on web");
}
