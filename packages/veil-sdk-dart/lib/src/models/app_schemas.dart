import "dart:typed_data";

import "../utils/cbor_min.dart";

class AppEnvelope {
  final String type;
  final int version;
  final Map<String, dynamic> payload;
  final Map<String, dynamic>? extensions;

  const AppEnvelope({
    required this.type,
    required this.version,
    required this.payload,
    this.extensions,
  });
}

class SocialPostV1 {
  final String body;
  final String? parentRoot;
  final String? threadRoot;
  final List<MediaDescriptorV1>? attachments;
  final Map<String, dynamic>? extensions;

  const SocialPostV1({
    required this.body,
    this.parentRoot,
    this.threadRoot,
    this.attachments,
    this.extensions,
  });
}

class MediaDescriptorV1 {
  final String mime;
  final int size;
  final String hashHex;
  final List<String> chunkRoots;
  final String? chunkTagHex;
  final Map<String, dynamic>? extensions;

  const MediaDescriptorV1({
    required this.mime,
    required this.size,
    required this.hashHex,
    required this.chunkRoots,
    this.chunkTagHex,
    this.extensions,
  });
}

class FileChunkV1 {
  final Uint8List data;
  final int index;
  final int total;
  final Map<String, dynamic>? extensions;

  const FileChunkV1({
    required this.data,
    required this.index,
    required this.total,
    this.extensions,
  });
}

const int maxObjectSize = 256 * 1024;
const int maxInlinePayload = 250000;

Uint8List encodeAppEnvelope(AppEnvelope envelope) {
  final map = _sortMap({
    "type": envelope.type,
    "version": envelope.version,
    "payload": _sortMap(envelope.payload),
    if (envelope.extensions != null)
      "extensions": _sortMap(envelope.extensions!),
  });
  return encodeCbor(map);
}

AppEnvelope decodeAppEnvelope(Uint8List bytes) {
  final decoded = decodeCbor(bytes);
  if (decoded is! Map) {
    throw ArgumentError("invalid app envelope");
  }
  final type = decoded["type"] as String? ?? "";
  final version = decoded["version"] as int? ?? 0;
  final payload = decoded["payload"];
  if (payload is! Map<String, dynamic>) {
    throw ArgumentError("invalid app envelope payload");
  }
  final extensions = decoded["extensions"];
  return AppEnvelope(
    type: type,
    version: version,
    payload: payload,
    extensions: extensions is Map<String, dynamic> ? extensions : null,
  );
}

Uint8List encodeSocialPost(SocialPostV1 post) {
  return encodeAppEnvelope(
    AppEnvelope(
      type: "post",
      version: 1,
      payload: _sortMap({
        "body": post.body,
        if (post.parentRoot != null) "parent_root": post.parentRoot,
        if (post.threadRoot != null) "thread_root": post.threadRoot,
        if (post.attachments != null)
          "attachments": post.attachments!.map(_mediaToMap).toList(),
        if (post.extensions != null) "extensions": _sortMap(post.extensions!),
      }),
    ),
  );
}

Uint8List encodeMediaDescriptor(MediaDescriptorV1 media) {
  return encodeAppEnvelope(
    AppEnvelope(
      type: "media_desc",
      version: 1,
      payload: _sortMap(_mediaToMap(media)),
    ),
  );
}

Uint8List encodeFileChunk(FileChunkV1 chunk) {
  return encodeAppEnvelope(
    AppEnvelope(
      type: "chunk",
      version: 1,
      payload: _sortMap({
        "data": chunk.data,
        "index": chunk.index,
        "total": chunk.total,
        if (chunk.extensions != null) "extensions": _sortMap(chunk.extensions!),
      }),
    ),
  );
}

List<FileChunkV1> splitIntoFileChunks(Uint8List bytes) {
  if (bytes.length <= maxInlinePayload) {
    return [FileChunkV1(data: bytes, index: 0, total: 1)];
  }
  final chunkSize = (maxInlinePayload < maxObjectSize - 1024)
      ? maxInlinePayload
      : maxObjectSize - 1024;
  final total = (bytes.length / chunkSize).ceil().clamp(1, 1000000);
  final chunks = <FileChunkV1>[];
  for (var index = 0; index < total; index += 1) {
    final start = index * chunkSize;
    final end = (start + chunkSize).clamp(0, bytes.length);
    chunks.add(
      FileChunkV1(data: bytes.sublist(start, end), index: index, total: total),
    );
  }
  return chunks;
}

Map<String, dynamic> _mediaToMap(MediaDescriptorV1 media) => {
  "mime": media.mime,
  "size": media.size,
  "hash_hex": media.hashHex,
  "chunk_roots": media.chunkRoots,
  if (media.chunkTagHex != null) "chunk_tag_hex": media.chunkTagHex,
  if (media.extensions != null) "extensions": _sortMap(media.extensions!),
};

Map<String, dynamic> _sortMap(Map<String, dynamic> map) {
  final keys = map.keys.toList()..sort();
  final sorted = <String, dynamic>{};
  for (final key in keys) {
    final value = map[key];
    if (value is Map<String, dynamic>) {
      sorted[key] = _sortMap(value);
    } else if (value is List) {
      sorted[key] = value.map((entry) {
        if (entry is Map<String, dynamic>) {
          return _sortMap(entry);
        }
        return entry;
      }).toList();
    } else {
      sorted[key] = value;
    }
  }
  return sorted;
}

class AppReferences {
  final List<String> parentRoots;
  final List<String> threadRoots;
  final List<String> chunkRoots;
  final List<String> chunkTagHexes;

  const AppReferences({
    required this.parentRoots,
    required this.threadRoots,
    required this.chunkRoots,
    required this.chunkTagHexes,
  });
}

AppReferences extractReferences(AppEnvelope envelope) {
  final parentRoots = <String>[];
  final threadRoots = <String>[];
  final chunkRoots = <String>[];
  final chunkTagHexes = <String>[];

  if (envelope.type == "post") {
    final payload = envelope.payload;
    final parent = payload["parent_root"];
    final thread = payload["thread_root"];
    if (parent is String) parentRoots.add(parent);
    if (thread is String) threadRoots.add(thread);
    final attachments = payload["attachments"];
    if (attachments is List) {
      for (final entry in attachments) {
        if (entry is Map<String, dynamic>) {
          final roots = entry["chunk_roots"];
          if (roots is List) {
            for (final root in roots) {
              if (root is String) chunkRoots.add(root);
            }
          }
          final tagHex = entry["chunk_tag_hex"];
          if (tagHex is String) chunkTagHexes.add(tagHex);
        }
      }
    }
  }

  if (envelope.type == "media_desc") {
    final payload = envelope.payload;
    final roots = payload["chunk_roots"];
    if (roots is List) {
      for (final root in roots) {
        if (root is String) chunkRoots.add(root);
      }
    }
    final tagHex = payload["chunk_tag_hex"];
    if (tagHex is String) chunkTagHexes.add(tagHex);
  }

  return AppReferences(
    parentRoots: parentRoots,
    threadRoots: threadRoots,
    chunkRoots: chunkRoots,
    chunkTagHexes: chunkTagHexes,
  );
}
