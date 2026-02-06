import "../models/veil_types.dart";

class VeilBridge {
  const VeilBridge();

  Future<TagHex> deriveFeedTagHex(String publisherPubkeyHex, int namespace) {
    throw UnimplementedError("FFI bridge not wired");
  }

  Future<TagHex> deriveRvTagHex(String recipientPubkeyHex, int epoch, int namespace) {
    throw UnimplementedError("FFI bridge not wired");
  }

  Future<int> currentEpoch(int epochSeconds) {
    throw UnimplementedError("FFI bridge not wired");
  }

  Future<ShardMeta> decodeShardMeta(List<int> shardBytes) {
    throw UnimplementedError("FFI bridge not wired");
  }

  Future<ObjectMeta> decodeObjectMeta(List<int> objectBytes) {
    throw UnimplementedError("FFI bridge not wired");
  }
}
