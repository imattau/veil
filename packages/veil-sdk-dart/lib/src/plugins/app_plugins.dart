import "dart:typed_data";

import "../client.dart";
import "../models/app_schemas.dart";

typedef RootTagResolver =
    String? Function(String objectRootHex, AppEnvelope? envelope);

class AutoFetchPlugin implements VeilClientPlugin {
  final RootTagResolver? resolveTagForRoot;
  final int priority;

  const AutoFetchPlugin({this.resolveTagForRoot, this.priority = 1});

  @override
  void onObject(
    VeilClient client,
    String objectRootHex,
    Uint8List objectBytes,
  ) {
    AppEnvelope? envelope;
    try {
      envelope = decodeAppEnvelope(objectBytes);
    } catch (_) {
      return;
    }
    final refs = extractReferences(envelope);
    for (final tag in refs.chunkTagHexes) {
      client.subscribe(tag);
    }

    final allRoots = <String>[
      ...refs.parentRoots,
      ...refs.threadRoots,
      ...refs.chunkRoots,
    ];
    for (final root in allRoots) {
      if (priority > 0) {
        client.prioritizeObjectRoot(root, priority: priority);
      }
      if (resolveTagForRoot != null) {
        final tag = resolveTagForRoot!(root, envelope);
        if (tag != null) {
          client.subscribe(tag);
        }
      }
    }
  }
}

class ThreadContextPlugin implements VeilClientPlugin {
  final RootTagResolver resolveTagForRoot;
  final int priority;

  const ThreadContextPlugin({
    required this.resolveTagForRoot,
    this.priority = 1,
  });

  @override
  void onObject(
    VeilClient client,
    String objectRootHex,
    Uint8List objectBytes,
  ) {
    AppEnvelope? envelope;
    try {
      envelope = decodeAppEnvelope(objectBytes);
    } catch (_) {
      return;
    }
    final refs = extractReferences(envelope);
    final roots = <String>[...refs.parentRoots, ...refs.threadRoots];
    for (final root in roots) {
      if (priority > 0) {
        client.prioritizeObjectRoot(root, priority: priority);
      }
      final tag = resolveTagForRoot(root, envelope);
      if (tag != null) {
        client.subscribe(tag);
      }
    }
  }
}
