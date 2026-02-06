import "dart:typed_data";

import "api.dart" as frb_api;
import "frb_generated.dart";
import "package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart";
import "../models/veil_types.dart";

class VeilBridge {
  const VeilBridge();

  Future<void> init({ExternalLibrary? externalLibrary}) async {
    await VeilBridgeApi.init(externalLibrary: externalLibrary);
  }

  Future<void> dispose() async {
    VeilBridgeApi.dispose();
  }

  Future<TagHex> deriveFeedTagHex(
    String publisherPubkeyHex,
    int namespace,
  ) async {
    return frb_api.deriveFeedTagHex(
      publisherPubkeyHex: publisherPubkeyHex,
      namespace: namespace,
    );
  }

  Future<TagHex> deriveRvTagHex(
    String recipientPubkeyHex,
    int epoch,
    int namespace,
  ) async {
    return frb_api.deriveRvTagHex(
      recipientPubkeyHex: recipientPubkeyHex,
      epoch: epoch,
      namespace: namespace,
    );
  }

  Future<int> currentEpoch(int nowSeconds, int epochSeconds) async {
    final result = await frb_api.currentEpochSeconds(
      now: BigInt.from(nowSeconds),
      epochSeconds: BigInt.from(epochSeconds),
    );
    return result.toInt();
  }

  Future<ShardMeta> decodeShardMeta(List<int> shardBytes) async {
    final meta = await frb_api.decodeShardMeta(bytes: shardBytes);
    return ShardMeta(
      version: meta.version,
      namespace: meta.namespace,
      epoch: meta.epoch,
      tagHex: meta.tagHex,
      objectRootHex: meta.objectRootHex,
      k: meta.k,
      n: meta.n,
      index: meta.index,
      payloadLen: meta.payloadLen.toInt(),
    );
  }

  Future<ObjectMeta> decodeObjectMeta(List<int> objectBytes) async {
    final meta = await frb_api.decodeObjectMeta(bytes: objectBytes);
    return ObjectMeta(
      version: meta.version,
      namespace: meta.namespace,
      epoch: meta.epoch,
      flags: meta.flags,
      signed: meta.signed,
      public: meta.public,
      ackRequested: meta.ackRequested,
      batched: meta.batched,
      tagHex: meta.tagHex,
      objectRootHex: meta.objectRootHex,
      senderPubkeyHex: meta.senderPubkeyHex,
      nonceHex: meta.nonceHex,
      ciphertextLen: meta.ciphertextLen.toInt(),
      paddingLen: meta.paddingLen.toInt(),
    );
  }

  Future<String> deriveObjectRootHex(List<int> objectBytes) async {
    return frb_api.deriveObjectRootHex(objectBytes: objectBytes);
  }

  Future<Uint8List> reconstructObjectPadded(
    List<List<int>> shardBytes,
    String expectedRootHex,
  ) async {
    return frb_api.reconstructObjectPaddedFromShards(
      shardBytes: shardBytes.map(Uint8List.fromList).toList(),
      expectedRootHex: expectedRootHex,
    );
  }

  Future<Uint8List> decryptObjectPayload(
    List<int> objectBytes,
    List<int> keyBytes,
  ) async {
    return frb_api.decryptObjectPayload(
      objectBytes: Uint8List.fromList(objectBytes),
      keyBytes: Uint8List.fromList(keyBytes),
    );
  }
}
