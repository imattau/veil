import "dart:typed_data";

import "../utils/cbor_min.dart";

class BlobChunkRefV1 {
  final String objectRootHex;
  final String tagHex;
  final int size;

  const BlobChunkRefV1({
    required this.objectRootHex,
    required this.tagHex,
    required this.size,
  });

  Map<String, dynamic> toCborMap() => {
    "objectRootHex": objectRootHex,
    "tagHex": tagHex,
    "size": size,
  };
}

class BlobManifestV1 {
  final int version;
  final String mime;
  final int size;
  final String hashHex;
  final List<BlobChunkRefV1> chunks;
  final String? filename;

  const BlobManifestV1({
    this.version = 1,
    required this.mime,
    required this.size,
    required this.hashHex,
    required this.chunks,
    this.filename,
  });

  Map<String, dynamic> toCborMap() => {
    "version": version,
    "mime": mime,
    "size": size,
    "hashHex": hashHex,
    "chunks": chunks.map((c) => c.toCborMap()).toList(),
    if (filename != null) "filename": filename,
  };
}

class DirectoryEntryV1 {
  final String path;
  final BlobManifestV1 blob;

  const DirectoryEntryV1({required this.path, required this.blob});

  Map<String, dynamic> toCborMap() => {"path": path, "blob": blob.toCborMap()};
}

class DirectoryBundleV1 {
  final int version;
  final List<DirectoryEntryV1> entries;

  const DirectoryBundleV1({this.version = 1, required this.entries});

  Map<String, dynamic> toCborMap() => {
    "version": version,
    "entries": entries.map((e) => e.toCborMap()).toList(),
  };
}

Uint8List encodeBlobManifestV1(BlobManifestV1 manifest) {
  return encodeCbor(manifest.toCborMap());
}

Uint8List encodeDirectoryBundleV1(DirectoryBundleV1 bundle) {
  return encodeCbor(bundle.toCborMap());
}

BlobManifestV1 decodeBlobManifestV1Bytes(Uint8List bytes) {
  final decoded = decodeCbor(bytes);
  if (decoded is! Map) {
    throw ArgumentError("invalid blob manifest");
  }
  final version = decoded["version"] is int ? decoded["version"] as int : 1;
  if (version != 1) {
    throw ArgumentError("unsupported blob manifest version");
  }
  final chunksRaw = decoded["chunks"];
  final chunks = <BlobChunkRefV1>[];
  if (chunksRaw is List) {
    for (final entry in chunksRaw) {
      if (entry is Map) {
        chunks.add(
          BlobChunkRefV1(
            objectRootHex: entry["objectRootHex"] as String? ?? "",
            tagHex: entry["tagHex"] as String? ?? "",
            size: entry["size"] as int? ?? 0,
          ),
        );
      }
    }
  }
  return BlobManifestV1(
    version: version,
    mime: decoded["mime"] as String? ?? "",
    size: decoded["size"] as int? ?? 0,
    hashHex: decoded["hashHex"] as String? ?? "",
    chunks: chunks,
    filename: decoded["filename"] as String?,
  );
}

DirectoryBundleV1 decodeDirectoryBundleV1Bytes(Uint8List bytes) {
  final decoded = decodeCbor(bytes);
  if (decoded is! Map) {
    throw ArgumentError("invalid directory bundle");
  }
  final version = decoded["version"] is int ? decoded["version"] as int : 1;
  if (version != 1) {
    throw ArgumentError("unsupported directory bundle version");
  }
  final entriesRaw = decoded["entries"];
  final entries = <DirectoryEntryV1>[];
  if (entriesRaw is List) {
    for (final entry in entriesRaw) {
      if (entry is Map) {
        final blobMap = entry["blob"];
        if (blobMap is Map) {
          entries.add(
            DirectoryEntryV1(
              path: entry["path"] as String? ?? "",
              blob: decodeBlobManifestV1Bytes(encodeCbor(blobMap)),
            ),
          );
        }
      }
    }
  }
  return DirectoryBundleV1(version: version, entries: entries);
}
