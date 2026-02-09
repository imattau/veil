import "dart:typed_data";

import "package:test/test.dart";
import "package:veil_sdk/src/client.dart";
import "package:veil_sdk/src/lanes/lane.dart";
import "package:veil_sdk/src/models/veil_types.dart";
import "package:veil_sdk/src/bridge/veil_bridge.dart";

class _FakeLane implements VeilLane {
  final List<LaneMessage> inbox = [];
  final List<List<int>> sent = [];

  @override
  Future<void> send(String peer, List<int> bytes) async {
    sent.add(bytes);
  }

  @override
  Future<LaneMessage?> recv() async {
    if (inbox.isEmpty) return null;
    return inbox.removeAt(0);
  }

  @override
  LaneHealthSnapshot? healthSnapshot() {
    return const LaneHealthSnapshot(
      outboundQueued: 0,
      outboundSendOk: 0,
      outboundSendErr: 0,
      inboundReceived: 0,
      inboundDropped: 0,
      reconnectAttempts: 0,
    );
  }
}

class _FakeBridge extends VeilBridge {
  const _FakeBridge();

  @override
  Future<ShardMeta> decodeShardMeta(List<int> shardBytes) async {
    return const ShardMeta(
      version: 1,
      namespace: 1,
      epoch: 1,
      tagHex: "aa",
      objectRootHex: "bb",
      k: 1,
      n: 1,
      index: 0,
      payloadLen: 3,
    );
  }

  @override
  Future<Uint8List> reconstructObjectPadded(
    List<List<int>> shardBytes,
    String expectedRootHex,
  ) async {
    return Uint8List.fromList([9, 9, 9]);
  }

  @override
  Future<ObjectMeta> decodeObjectMeta(List<int> objectBytes) async {
    return const ObjectMeta(
      version: 1,
      namespace: 1,
      epoch: 1,
      flags: 0,
      signed: false,
      public: false,
      ackRequested: false,
      batched: false,
      tagHex: "aa",
      objectRootHex: "bb",
      senderPubkeyHex: null,
      nonceHex: "00",
      ciphertextLen: 0,
      paddingLen: 0,
    );
  }
}

void main() {
  test("veil client processes shard and reconstructs object", () async {
    final lane = _FakeLane();
    lane.inbox.add(const LaneMessage(peer: "peer", bytes: [1, 2, 3]));

    var sawShardMeta = false;
    var sawReconstructable = false;
    var sawReconstructed = false;
    var sawObjectMeta = false;
    var sawObjectBytes = false;

    final client = VeilClient(
      fastLane: lane,
      bridge: const _FakeBridge(),
      options: const VeilClientOptions(enableShardRequests: false),
      hooks: VeilClientHooks(
        onShardMeta: (_, __) => sawShardMeta = true,
        onReconstructable: (_, __, ___) => sawReconstructable = true,
        onReconstructed: (_, __) => sawReconstructed = true,
        onObjectMeta: (_, __) => sawObjectMeta = true,
        onObjectBytes: (_, __) => sawObjectBytes = true,
      ),
    );

    client.subscribe("aa");
    client.start();
    await client.tick();
    client.stop();

    expect(sawShardMeta, isTrue);
    expect(sawReconstructable, isTrue);
    expect(sawReconstructed, isTrue);
    expect(sawObjectMeta, isTrue);
    expect(sawObjectBytes, isTrue);
  });
}
