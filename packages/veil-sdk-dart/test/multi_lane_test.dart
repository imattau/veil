import "package:test/test.dart";
import "package:veil_sdk/src/lanes/lane.dart";
import "package:veil_sdk/src/lanes/multi_lane.dart";

class FakeLane implements VeilLane {
  final List<List<int>> sent = [];
  final List<LaneMessage> inbox = [];
  final LaneHealthSnapshot snapshot;

  FakeLane(this.snapshot);

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
  LaneHealthSnapshot? healthSnapshot() => snapshot;
}

void main() {
  test("round robin sends across lanes", () async {
    final laneA = FakeLane(
      const LaneHealthSnapshot(
        outboundQueued: 0,
        outboundSendOk: 0,
        outboundSendErr: 0,
        inboundReceived: 0,
        inboundDropped: 0,
        reconnectAttempts: 0,
      ),
    );
    final laneB = FakeLane(
      const LaneHealthSnapshot(
        outboundQueued: 0,
        outboundSendOk: 0,
        outboundSendErr: 0,
        inboundReceived: 0,
        inboundDropped: 0,
        reconnectAttempts: 0,
      ),
    );
    final multi = MultiLane(lanes: [laneA, laneB]);
    await multi.send("peer", [1]);
    await multi.send("peer", [2]);
    await multi.send("peer", [3]);

    expect(laneA.sent.length, 2);
    expect(laneB.sent.length, 1);
    expect(laneA.sent.first, [1]);
    expect(laneB.sent.first, [2]);
  });

  test("broadcast sends to all lanes", () async {
    final laneA = FakeLane(
      const LaneHealthSnapshot(
        outboundQueued: 0,
        outboundSendOk: 0,
        outboundSendErr: 0,
        inboundReceived: 0,
        inboundDropped: 0,
        reconnectAttempts: 0,
      ),
    );
    final laneB = FakeLane(
      const LaneHealthSnapshot(
        outboundQueued: 0,
        outboundSendOk: 0,
        outboundSendErr: 0,
        inboundReceived: 0,
        inboundDropped: 0,
        reconnectAttempts: 0,
      ),
    );
    final multi = MultiLane(
      lanes: [laneA, laneB],
      sendMode: MultiLaneSendMode.broadcast,
    );
    await multi.send("peer", [9]);

    expect(laneA.sent.length, 1);
    expect(laneB.sent.length, 1);
  });

  test("recv polls lanes in order", () async {
    final laneA = FakeLane(
      const LaneHealthSnapshot(
        outboundQueued: 0,
        outboundSendOk: 0,
        outboundSendErr: 0,
        inboundReceived: 0,
        inboundDropped: 0,
        reconnectAttempts: 0,
      ),
    );
    final laneB = FakeLane(
      const LaneHealthSnapshot(
        outboundQueued: 0,
        outboundSendOk: 0,
        outboundSendErr: 0,
        inboundReceived: 0,
        inboundDropped: 0,
        reconnectAttempts: 0,
      ),
    );
    laneB.inbox.add(const LaneMessage(peer: "b", bytes: [2]));
    laneA.inbox.add(const LaneMessage(peer: "a", bytes: [1]));
    final multi = MultiLane(lanes: [laneA, laneB]);

    final first = await multi.recv();
    final second = await multi.recv();

    expect(first?.peer, "a");
    expect(second?.peer, "b");
  });

  test("aggregates health snapshots", () {
    final laneA = FakeLane(
      const LaneHealthSnapshot(
        outboundQueued: 1,
        outboundSendOk: 2,
        outboundSendErr: 3,
        inboundReceived: 4,
        inboundDropped: 5,
        reconnectAttempts: 6,
      ),
    );
    final laneB = FakeLane(
      const LaneHealthSnapshot(
        outboundQueued: 2,
        outboundSendOk: 3,
        outboundSendErr: 4,
        inboundReceived: 5,
        inboundDropped: 6,
        reconnectAttempts: 7,
      ),
    );
    final multi = MultiLane(lanes: [laneA, laneB]);
    final snapshot = multi.healthSnapshot();

    expect(snapshot?.outboundQueued, 3);
    expect(snapshot?.outboundSendOk, 5);
    expect(snapshot?.outboundSendErr, 7);
    expect(snapshot?.inboundReceived, 9);
    expect(snapshot?.inboundDropped, 11);
    expect(snapshot?.reconnectAttempts, 13);
  });
}
