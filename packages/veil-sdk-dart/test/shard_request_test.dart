import "dart:typed_data";

import "package:veil_sdk/src/models/shard_request.dart";
import "package:veil_sdk/src/tags.dart";
import "package:veil_sdk/src/bridge/veil_bridge.dart";
import "package:veil_sdk/src/models/blob_manifest.dart";
import "package:test/test.dart";

class _StubBridge extends VeilBridge {
  const _StubBridge();

  @override
  Future<int> currentEpoch(int nowSeconds, int epochSeconds) async {
    return nowSeconds ~/ epochSeconds;
  }

  @override
  Future<String> deriveRvTagHex(
    String recipientPubkeyHex,
    int epoch,
    int namespace,
  ) async {
    return "$recipientPubkeyHex:$epoch:$namespace";
  }
}

void main() {
  test("encodes and decodes shard requests", () {
    final payload = ShardRequestPayload(
      objectRootHex: "11" * 32,
      tagHex: "22" * 32,
      k: 6,
      n: 10,
      want: [1, 2, 3],
      hop: 1,
    );
    final encoded = encodeShardRequest(payload);
    final decoded = decodeShardRequest(encoded);
    expect(decoded, isNotNull);
    expect(decoded!.objectRootHex, payload.objectRootHex);
    expect(decoded.tagHex, payload.tagHex);
    expect(decoded.k, payload.k);
    expect(decoded.n, payload.n);
    expect(decoded.want, payload.want);
    expect(decoded.hop, payload.hop);
  });

  test("derives overlapping RV tags", () async {
    const bridge = _StubBridge();
    final tags = await deriveRvTagWindowHex(
      "aa",
      95,
      7,
      epochSeconds: 100,
      overlapSeconds: 10,
      bridge: bridge,
    );
    expect(tags.length, greaterThan(1));
  });

  test("encodes blob manifest", () {
    final manifest = BlobManifestV1(
      mime: "image/png",
      size: 10,
      hashHex: "00" * 32,
      chunks: const [
        BlobChunkRefV1(objectRootHex: "11" * 32, tagHex: "22" * 32, size: 10),
      ],
    );
    final bytes = encodeBlobManifestV1(manifest);
    expect(bytes, isA<Uint8List>());
  });
}
