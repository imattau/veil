import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:veil_sdk/veil_sdk.dart';

void main() {
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
