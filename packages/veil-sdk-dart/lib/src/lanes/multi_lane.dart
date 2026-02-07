import "lane.dart";

enum MultiLaneSendMode { roundRobin, broadcast }

class MultiLane implements VeilLane {
  final List<VeilLane> lanes;
  final MultiLaneSendMode sendMode;
  int _sendIndex = 0;
  int _recvIndex = 0;

  MultiLane({
    required List<VeilLane> lanes,
    this.sendMode = MultiLaneSendMode.roundRobin,
  }) : lanes = List.unmodifiable(lanes);

  @override
  Future<void> send(String peer, List<int> bytes) async {
    if (lanes.isEmpty) return;
    if (sendMode == MultiLaneSendMode.broadcast) {
      for (final lane in lanes) {
        await lane.send(peer, bytes);
      }
      return;
    }
    final lane = lanes[_sendIndex % lanes.length];
    _sendIndex = (_sendIndex + 1) % lanes.length;
    await lane.send(peer, bytes);
  }

  @override
  Future<LaneMessage?> recv() async {
    if (lanes.isEmpty) return null;
    for (var i = 0; i < lanes.length; i += 1) {
      final index = (_recvIndex + i) % lanes.length;
      final msg = await lanes[index].recv();
      if (msg != null) {
        _recvIndex = (index + 1) % lanes.length;
        return msg;
      }
    }
    return null;
  }

  @override
  LaneHealthSnapshot? healthSnapshot() {
    if (lanes.isEmpty) return null;
    var outboundQueued = 0;
    var outboundSendOk = 0;
    var outboundSendErr = 0;
    var inboundReceived = 0;
    var inboundDropped = 0;
    var reconnectAttempts = 0;
    for (final lane in lanes) {
      final snapshot = lane.healthSnapshot();
      if (snapshot == null) continue;
      outboundQueued += snapshot.outboundQueued;
      outboundSendOk += snapshot.outboundSendOk;
      outboundSendErr += snapshot.outboundSendErr;
      inboundReceived += snapshot.inboundReceived;
      inboundDropped += snapshot.inboundDropped;
      reconnectAttempts += snapshot.reconnectAttempts;
    }
    return LaneHealthSnapshot(
      outboundQueued: outboundQueued,
      outboundSendOk: outboundSendOk,
      outboundSendErr: outboundSendErr,
      inboundReceived: inboundReceived,
      inboundDropped: inboundDropped,
      reconnectAttempts: reconnectAttempts,
    );
  }
}
