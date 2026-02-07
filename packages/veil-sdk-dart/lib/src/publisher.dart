import "dart:typed_data";

import "bridge/veil_bridge.dart";
import "models/app_schemas.dart";

class PublishObject {
  final String objectRootHex;
  final Uint8List objectBytes;

  const PublishObject({required this.objectRootHex, required this.objectBytes});
}

class PublishBatch {
  final PublishObject rootObject;
  final List<PublishObject> relatedObjects;

  const PublishBatch({required this.rootObject, required this.relatedObjects});
}

class VeilPublisher {
  final VeilBridge bridge;

  const VeilPublisher({this.bridge = const VeilBridge()});

  Future<PublishObject> buildObject(Uint8List bytes) async {
    final root = await bridge.deriveObjectRootHex(bytes);
    return PublishObject(objectRootHex: root, objectBytes: bytes);
  }

  Future<PublishObject> buildSocialPost(String body,
      {String? parentRoot,
      String? threadRoot,
      List<MediaDescriptorV1>? attachments,
      Map<String, dynamic>? extensions}) async {
    final bytes = encodeSocialPost(
      SocialPostV1(
        body: body,
        parentRoot: parentRoot,
        threadRoot: threadRoot,
        attachments: attachments,
        extensions: extensions,
      ),
    );
    return buildObject(bytes);
  }

  Future<PublishObject> buildMediaDescriptor(MediaDescriptorV1 media) async {
    final bytes = encodeMediaDescriptor(media);
    return buildObject(bytes);
  }

  Future<List<PublishObject>> buildFileChunks(Uint8List bytes) async {
    final chunks = splitIntoFileChunks(bytes);
    final out = <PublishObject>[];
    for (final chunk in chunks) {
      out.add(await buildObject(encodeFileChunk(chunk)));
    }
    return out;
  }

  Future<PublishBatch> buildPostWithAttachments(
    String body,
    List<Uint8List> attachments,
    List<String> mimeTypes,
  ) async {
    final related = <PublishObject>[];
    final descriptors = <MediaDescriptorV1>[];

    for (var i = 0; i < attachments.length; i += 1) {
      final data = attachments[i];
      final mime = mimeTypes.length > i
          ? mimeTypes[i]
          : "application/octet-stream";
      final chunkObjects = await buildFileChunks(data);
      related.addAll(chunkObjects);
      final chunkRoots = chunkObjects.map((obj) => obj.objectRootHex).toList();
      descriptors.add(
        MediaDescriptorV1(
          mime: mime,
          size: data.length,
          hashHex: "",
          chunkRoots: chunkRoots,
        ),
      );
    }

    final descriptorObjects = <PublishObject>[];
    for (final desc in descriptors) {
      descriptorObjects.add(await buildMediaDescriptor(desc));
    }
    related.addAll(descriptorObjects);

    final post = await buildSocialPost(body, attachments: descriptors);
    return PublishBatch(rootObject: post, relatedObjects: related);
  }
}

class PublishQueue {
  final List<PublishObject> _queue = [];
  int maxQueueSize;

  PublishQueue({this.maxQueueSize = 500});

  void enqueue(PublishObject object) {
    _trimForCapacity();
    _queue.add(object);
  }

  void enqueueAll(Iterable<PublishObject> objects) {
    for (final object in objects) {
      enqueue(object);
    }
  }

  bool get isEmpty => _queue.isEmpty;

  PublishObject? pop() {
    if (_queue.isEmpty) return null;
    return _queue.removeAt(0);
  }

  List<PublishObject> snapshot() => List<PublishObject>.from(_queue);

  void updateMaxSize(int value) {
    maxQueueSize = value < 1 ? 1 : value;
    _trimForCapacity();
  }

  void _trimForCapacity() {
    while (_queue.length >= maxQueueSize && _queue.isNotEmpty) {
      _queue.removeAt(0);
    }
  }

  void clear() => _queue.clear();
}
