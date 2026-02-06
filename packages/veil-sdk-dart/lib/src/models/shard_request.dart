import "dart:typed_data";

import "../utils/cbor_min.dart";
import "../utils/hex.dart";

const _shardRequestPrefix = "VEILREQ1";
const shardRequestVersion = 1;

class ShardRequestPayload {
  final int version;
  final String objectRootHex;
  final String tagHex;
  final int k;
  final int n;
  final List<int> want;
  final int hop;

  const ShardRequestPayload({
    this.version = shardRequestVersion,
    required this.objectRootHex,
    required this.tagHex,
    required this.k,
    required this.n,
    required this.want,
    this.hop = 0,
  });

  ShardRequestPayload copyWith({
    int? version,
    String? objectRootHex,
    String? tagHex,
    int? k,
    int? n,
    List<int>? want,
    int? hop,
  }) {
    return ShardRequestPayload(
      version: version ?? this.version,
      objectRootHex: objectRootHex ?? this.objectRootHex,
      tagHex: tagHex ?? this.tagHex,
      k: k ?? this.k,
      n: n ?? this.n,
      want: want ?? this.want,
      hop: hop ?? this.hop,
    );
  }
}

Uint8List encodeShardRequest(ShardRequestPayload payload) {
  final map = <String, dynamic>{
    "v": payload.version,
    "object_root": hexToBytes(payload.objectRootHex),
    "tag": hexToBytes(payload.tagHex),
    "k": payload.k,
    "n": payload.n,
    "want": payload.want,
    "hop": payload.hop,
  };
  final body = encodeCbor(map);
  final prefix = Uint8List.fromList(_shardRequestPrefix.codeUnits);
  final out = Uint8List(prefix.length + body.length);
  out.setAll(0, prefix);
  out.setAll(prefix.length, body);
  return out;
}

ShardRequestPayload? decodeShardRequest(Uint8List bytes) {
  final prefix = _shardRequestPrefix.codeUnits;
  if (bytes.length < prefix.length) {
    return null;
  }
  for (var i = 0; i < prefix.length; i += 1) {
    if (bytes[i] != prefix[i]) {
      return null;
    }
  }
  final body = bytes.sublist(prefix.length);
  final decoded = decodeCbor(body);
  if (decoded is! Map) {
    return null;
  }
  final version = (decoded["v"] ?? shardRequestVersion) is int
      ? decoded["v"] as int
      : shardRequestVersion;
  if (version != shardRequestVersion) {
    return null;
  }
  final objectRoot = decoded["object_root"];
  final tag = decoded["tag"];
  if (objectRoot is! Uint8List || tag is! Uint8List) {
    return null;
  }
  final k = decoded["k"];
  final n = decoded["n"];
  if (k is! int || n is! int || k <= 0 || n <= 0) {
    return null;
  }
  final wantRaw = decoded["want"];
  if (wantRaw is! List) {
    return null;
  }
  final want = wantRaw
      .whereType<int>()
      .where((idx) => idx >= 0 && idx < n)
      .toList();
  if (want.isEmpty) {
    return null;
  }
  final hop = decoded["hop"] is int ? decoded["hop"] as int : 0;
  return ShardRequestPayload(
    version: version,
    objectRootHex: bytesToHex(objectRoot),
    tagHex: bytesToHex(tag),
    k: k,
    n: n,
    want: want,
    hop: hop,
  );
}
