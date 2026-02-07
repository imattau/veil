import "lane.dart";

class QuicLane implements VeilLane {
  final String endpoint;
  final String peerId;
  final int mtuHint;

  int _outboundErr = 0;

  QuicLane({
    required this.endpoint,
    required this.peerId,
    this.mtuHint = 1200,
  });

  @override
  Future<void> send(String peer, List<int> bytes) async {
    _outboundErr += 1;
    throw UnimplementedError("QUIC lane is a placeholder");
  }

  @override
  Future<LaneMessage?> recv() async {
    return null;
  }

  @override
  LaneHealthSnapshot healthSnapshot() {
    return LaneHealthSnapshot(
      outboundQueued: 0,
      outboundSendOk: 0,
      outboundSendErr: _outboundErr,
      inboundReceived: 0,
      inboundDropped: 0,
      reconnectAttempts: 0,
    );
  }
}
