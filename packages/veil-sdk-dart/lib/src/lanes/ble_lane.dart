import "dart:async";

import "lane.dart";

class BleLane implements VeilLane {
  final int mtu;
  final List<LaneMessage> _inbox = [];
  final List<List<int>> _sendBuffer = [];

  BleLane({this.mtu = 180});

  @override
  Future<void> send(String peer, List<int> bytes) async {
    _sendBuffer.add(bytes);
    // Hook: integrate flutter_reactive_ble writeWithoutResponse.
  }

  @override
  Future<LaneMessage?> recv() async {
    if (_inbox.isEmpty) {
      return null;
    }
    return _inbox.removeAt(0);
  }

  @override
  LaneHealthSnapshot healthSnapshot() {
    return const LaneHealthSnapshot(
      outboundQueued: 0,
      outboundSendOk: 0,
      outboundSendErr: 0,
      inboundReceived: 0,
      inboundDropped: 0,
      reconnectAttempts: 0,
    );
  }

  // BLE chunking helpers.
  List<List<int>> splitIntoFrames(List<int> payload) {
    final headerLen = 36; // 32 shard id + 2 index + 2 total.
    final maxPayload = (mtu - headerLen).clamp(1, mtu);
    final total = (payload.length / maxPayload).ceil().clamp(1, 65535);
    final frames = <List<int>>[];
    for (var i = 0; i < total; i += 1) {
      final start = i * maxPayload;
      final end = (start + maxPayload).clamp(0, payload.length);
      frames.add(payload.sublist(start, end));
    }
    return frames;
  }
}
