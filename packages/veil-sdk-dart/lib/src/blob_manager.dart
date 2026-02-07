import "dart:typed_data";

import "models/app_schemas.dart";

class BlobAssembly {
  final String rootHex;
  final MediaDescriptorV1 descriptor;
  final Uint8List bytes;

  const BlobAssembly({
    required this.rootHex,
    required this.descriptor,
    required this.bytes,
  });
}

class BlobManager {
  final Map<String, MediaDescriptorV1> _media = {};
  final Map<String, FileChunkV1> _chunks = {};

  void ingest(String objectRootHex, Uint8List objectBytes) {
    AppEnvelope envelope;
    try {
      envelope = decodeAppEnvelope(objectBytes);
    } catch (_) {
      return;
    }
    if (envelope.type == "media_desc") {
      final media = _parseMediaDescriptor(envelope);
      if (media != null) {
        _media[objectRootHex.toLowerCase()] = media;
      }
    } else if (envelope.type == "chunk") {
      final chunk = _parseFileChunk(envelope);
      if (chunk != null) {
        _chunks[objectRootHex.toLowerCase()] = chunk;
      }
    }
  }

  BlobAssembly? tryAssemble(String mediaRootHex) {
    final key = mediaRootHex.toLowerCase();
    final descriptor = _media[key];
    if (descriptor == null) {
      return null;
    }
    final parts = <FileChunkV1>[];
    for (final root in descriptor.chunkRoots) {
      final chunk = _chunks[root.toLowerCase()];
      if (chunk == null) {
        return null;
      }
      parts.add(chunk);
    }
    parts.sort((a, b) => a.index.compareTo(b.index));
    final total = parts.first.total;
    if (parts.any((c) => c.total != total) || parts.length != total) {
      return null;
    }
    final length = parts.fold<int>(0, (sum, chunk) => sum + chunk.data.length);
    final buffer = BytesBuilder(copy: false);
    for (final chunk in parts) {
      buffer.add(chunk.data);
    }
    final bytes = buffer.toBytes();
    if (bytes.length != length) {
      return null;
    }
    return BlobAssembly(rootHex: key, descriptor: descriptor, bytes: bytes);
  }

  MediaDescriptorV1? _parseMediaDescriptor(AppEnvelope envelope) {
    final payload = envelope.payload;
    final mime = payload["mime"];
    final size = payload["size"];
    final hashHex = payload["hash_hex"];
    final chunkRoots = payload["chunk_roots"];
    if (mime is! String ||
        size is! int ||
        hashHex is! String ||
        chunkRoots is! List) {
      return null;
    }
    final roots = <String>[];
    for (final root in chunkRoots) {
      if (root is String) {
        roots.add(root);
      }
    }
    return MediaDescriptorV1(
      mime: mime,
      size: size,
      hashHex: hashHex,
      chunkRoots: roots,
      chunkTagHex: payload["chunk_tag_hex"] as String?,
      extensions: payload["extensions"] as Map<String, dynamic>?,
    );
  }

  FileChunkV1? _parseFileChunk(AppEnvelope envelope) {
    final payload = envelope.payload;
    final data = payload["data"];
    final index = payload["index"];
    final total = payload["total"];
    if (data is! Uint8List || index is! int || total is! int) {
      return null;
    }
    return FileChunkV1(
      data: data,
      index: index,
      total: total,
      extensions: payload["extensions"] as Map<String, dynamic>?,
    );
  }
}
