import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:veil_sdk/veil_sdk.dart';

import 'package:veil_sdk/src/bridge/frb_generated.dart';

class _MockBridgeApi extends VeilBridgeApiApi {
  @override
  Future<BigInt> crateApiCurrentEpochSeconds({
    required BigInt now,
    required BigInt epochSeconds,
  }) async {
    return BigInt.zero;
  }

  @override
  Future<ObjectMeta> crateApiDecodeObjectMeta({required List<int> bytes}) async {
    throw UnimplementedError();
  }

  @override
  Future<ShardMeta> crateApiDecodeShardMeta({required List<int> bytes}) async {
    throw UnimplementedError();
  }

  @override
  Future<Uint8List> crateApiDecryptObjectPayload({
    required List<int> objectBytes,
    required List<int> keyBytes,
  }) async {
    throw UnimplementedError();
  }

  @override
  Future<String> crateApiDeriveFeedTagHex({
    required String publisherPubkeyHex,
    required int namespace,
  }) async {
    return '0' * 64;
  }

  @override
  Future<String> crateApiDeriveObjectRootHex({required List<int> objectBytes}) async {
    return '1' * 64;
  }

  @override
  Future<String> crateApiDeriveRvTagHex({
    required String recipientPubkeyHex,
    required int epoch,
    required int namespace,
  }) async {
    return '2' * 64;
  }

  @override
  Future<Uint8List> crateApiReconstructObjectPaddedFromShards({
    required List<Uint8List> shardBytes,
    required String expectedRootHex,
  }) async {
    return Uint8List(0);
  }
}

void main() {
  setUpAll(() async {
    VeilBridgeApi.initMock(api: _MockBridgeApi());
  });

  test('publisher builds post with attachments', () async {
    final publisher = VeilPublisher();
    final attachment = Uint8List.fromList(List<int>.generate(1024, (i) => i % 256));
    final batch = await publisher.buildPostWithAttachments(
      'Hello VEIL',
      [attachment],
      ['image/png'],
      ['0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef'],
    );

    expect(batch.rootObject.objectBytes.isNotEmpty, true);
    expect(batch.rootObject.objectRootHex.length, 64);
    expect(batch.relatedObjects.isNotEmpty, true);
  });

  test('publish queue preserves order', () {
    final queue = PublishQueue(maxQueueSize: 10);
    final obj1 = PublishObject(objectRootHex: 'a' * 64, objectBytes: Uint8List(1));
    final obj2 = PublishObject(objectRootHex: 'b' * 64, objectBytes: Uint8List(1));
    queue.enqueue(obj1);
    queue.enqueue(obj2);

    expect(queue.pop()?.objectRootHex, obj1.objectRootHex);
    expect(queue.pop()?.objectRootHex, obj2.objectRootHex);
    expect(queue.pop(), null);
  });
}
