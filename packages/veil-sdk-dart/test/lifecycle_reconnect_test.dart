import "dart:async";

import "package:test/test.dart";
import "package:veil_sdk/src/client.dart";
import "package:veil_sdk/src/lanes/lane.dart";

class FakeLane implements VeilLane {
  int sends = 0;
  final List<LaneMessage> inbox = [];

  @override
  Future<void> send(String peer, List<int> bytes) async {
    sends += 1;
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

void main() {
  test("client start/stop gates tick processing", () async {
    final lane = FakeLane();
    final client = VeilClient(fastLane: lane, pollIntervalMs: 20);

    client.start();
    lane.inbox.add(const LaneMessage(peer: "peer", bytes: [1, 2, 3]));
    await Future<void>.delayed(const Duration(milliseconds: 60));
    // After start, inbox should be drained by ticks.
    expect(lane.inbox.isEmpty, isTrue);

    client.stop();
    lane.inbox.add(const LaneMessage(peer: "peer", bytes: [4, 5]));
    await Future<void>.delayed(const Duration(milliseconds: 60));
    // After stop, inbox should remain.
    expect(lane.inbox.isEmpty, isFalse);
  });

  test("client restart resumes ticking", () async {
    final lane = FakeLane();
    final client = VeilClient(fastLane: lane, pollIntervalMs: 20);

    client.start();
    client.stop();
    lane.inbox.add(const LaneMessage(peer: "peer", bytes: [9]));
    await Future<void>.delayed(const Duration(milliseconds: 60));
    expect(lane.inbox.isEmpty, isFalse);

    client.start();
    await Future<void>.delayed(const Duration(milliseconds: 60));
    expect(lane.inbox.isEmpty, isTrue);
  });
}
