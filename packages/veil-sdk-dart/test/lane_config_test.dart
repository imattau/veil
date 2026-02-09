import "package:test/test.dart";
import "package:veil_sdk/src/lanes/lane.dart";
import "package:veil_sdk/src/lanes/lane_config.dart";
import "package:veil_sdk/src/lanes/multi_lane.dart";

class DummyLane implements VeilLane {
  final String name;
  DummyLane(this.name);

  @override
  Future<void> send(String peer, List<int> bytes) async {}

  @override
  Future<LaneMessage?> recv() async => null;

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
  test("buildLaneConfig prefers quic with ws fallback", () {
    final ws = DummyLane("ws");
    final quic = DummyLane("quic");
    final result = buildLaneConfig(
      wsLane: ws,
      quicLane: quic,
      p2pAnyLane: false,
    );
    expect(result.fastLane, isA<MultiLane>());
    final multi = result.fastLane as MultiLane;
    expect(multi.sendMode, MultiLaneSendMode.primaryThenFallback);
  });

  test("buildLaneConfig respects ghost mode tor preference", () {
    final ws = DummyLane("ws");
    final quic = DummyLane("quic");
    final tor = DummyLane("tor");
    final result = buildLaneConfig(
      wsLane: ws,
      quicLane: quic,
      torLane: tor,
      ghostMode: true,
      p2pAnyLane: false,
    );
    expect(result.fastLane, tor);
    expect(result.fallbackLane, isNotNull);
  });

  test("buildLaneConfig ghost mode prefers BLE when tor absent", () {
    final ws = DummyLane("ws");
    final quic = DummyLane("quic");
    final ble = DummyLane("ble");
    final result = buildLaneConfig(
      wsLane: ws,
      quicLane: quic,
      bleLane: ble,
      ghostMode: true,
      p2pAnyLane: false,
    );
    expect(result.fastLane, ble);
    expect(result.fallbackLane, isNotNull);
  });

  test("buildLaneConfig uses ws when quic missing", () {
    final ws = DummyLane("ws");
    final result = buildLaneConfig(wsLane: ws, p2pAnyLane: false);
    expect(result.fastLane, ws);
    expect(result.fallbackLane, isNull);
  });

  test("buildLaneConfig uses broadcast mesh when enabled", () {
    final ws = DummyLane("ws");
    final quic = DummyLane("quic");
    final tor = DummyLane("tor");
    final result = buildLaneConfig(
      wsLane: ws,
      quicLane: quic,
      torLane: tor,
      p2pAnyLane: true,
    );
    expect(result.fastLane, isA<MultiLane>());
    final multi = result.fastLane as MultiLane;
    expect(multi.sendMode, MultiLaneSendMode.broadcast);
    expect(result.publishLane, isA<MultiLane>());
  });
}
