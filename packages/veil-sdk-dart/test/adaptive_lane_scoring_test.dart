import "dart:typed_data";

import "package:test/test.dart";
import "package:veil_sdk/src/client.dart";
import "package:veil_sdk/src/lanes/lane.dart";
import "package:veil_sdk/src/models/veil_types.dart";
import "package:veil_sdk/src/bridge/veil_bridge.dart";

class _ScoredLane implements VeilLane {
  final List<LaneMessage> inbox = [];
  final List<String> sentPeers = [];
  LaneHealthSnapshot snapshot;

  _ScoredLane({required this.snapshot});

  @override
  Future<void> send(String peer, List<int> bytes) async {
    sentPeers.add(peer);
  }

  @override
  Future<LaneMessage?> recv() async {
    if (inbox.isEmpty) return null;
    return inbox.removeAt(0);
  }

  @override
  LaneHealthSnapshot? healthSnapshot() => snapshot;
}

class _Bridge extends VeilBridge {
  const _Bridge();

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
  test("adaptive lane scoring shifts fanout to fallback", () async {
    final fastLane = _ScoredLane(
      snapshot: const LaneHealthSnapshot(
        outboundQueued: 0,
        outboundSendOk: 0,
        outboundSendErr: 5,
        inboundReceived: 0,
        inboundDropped: 0,
        reconnectAttempts: 0,
      ),
    );
    final fallbackLane = _ScoredLane(
      snapshot: const LaneHealthSnapshot(
        outboundQueued: 0,
        outboundSendOk: 5,
        outboundSendErr: 0,
        inboundReceived: 0,
        inboundDropped: 0,
        reconnectAttempts: 0,
      ),
    );

    fastLane.inbox.add(const LaneMessage(peer: "origin", bytes: [1, 2, 3]));

    final client = VeilClient(
      fastLane: fastLane,
      fallbackLane: fallbackLane,
      bridge: const _Bridge(),
      options: const VeilClientOptions(
        enableShardRequests: false,
        fastFanout: 2,
        fallbackFanout: 1,
        minimumHealthyLaneScore: 0.2,
      ),
    );
    client.setForwardPeers(["p1", "p2", "p3"]);
    client.subscribe("aa");

    client.start();
    await client.tick();
    client.stop();

    expect(fastLane.sentPeers.length, 1);
    expect(fallbackLane.sentPeers.length, 2);
  });
}
