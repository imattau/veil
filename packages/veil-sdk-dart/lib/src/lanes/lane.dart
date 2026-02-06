abstract class VeilLane {
  Future<void> send(String peer, List<int> bytes);
  Future<LaneMessage?> recv();
  LaneHealthSnapshot? healthSnapshot();
}

class LaneMessage {
  final String peer;
  final List<int> bytes;

  const LaneMessage({required this.peer, required this.bytes});
}

class LaneHealthSnapshot {
  final int outboundQueued;
  final int outboundSendOk;
  final int outboundSendErr;
  final int inboundReceived;
  final int inboundDropped;
  final int reconnectAttempts;

  const LaneHealthSnapshot({
    required this.outboundQueued,
    required this.outboundSendOk,
    required this.outboundSendErr,
    required this.inboundReceived,
    required this.inboundDropped,
    required this.reconnectAttempts,
  });
}
