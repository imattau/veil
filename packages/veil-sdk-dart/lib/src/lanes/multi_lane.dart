import "lane.dart";
import "quic_lane.dart";

enum MultiLaneSendMode { roundRobin, broadcast, primaryThenFallback }

class MultiLane implements VeilLane {
  final List<VeilLane> lanes;
  final MultiLaneSendMode sendMode;
  final Duration fallbackProbeTimeout;
  final Duration fallbackProbeInterval;
  int _sendIndex = 0;
  int _recvIndex = 0;

  MultiLane({
    required List<VeilLane> lanes,
    this.sendMode = MultiLaneSendMode.roundRobin,
    this.fallbackProbeTimeout = const Duration(milliseconds: 350),
    this.fallbackProbeInterval = const Duration(milliseconds: 30),
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
    if (sendMode == MultiLaneSendMode.primaryThenFallback) {
      Object? lastError;
      for (final lane in lanes) {
        try {
          if (lane is QuicLane) {
            final before = lane.metricsSnapshot();
            await lane.send(peer, bytes);
            if (lane.debugLastSendResult != 0) {
              lastError = StateError("quic send failed");
              continue;
            }
            if (before == null) {
              return;
            }
            final deadline = DateTime.now().add(fallbackProbeTimeout);
            while (DateTime.now().isBefore(deadline)) {
              final after = lane.metricsSnapshot();
              if (after == null) {
                return;
              }
              if (after.sendSuccess > before.sendSuccess) {
                return;
              }
              if (after.sendErrors > before.sendErrors) {
                lastError = StateError("quic send failed");
                break;
              }
              await Future<void>.delayed(fallbackProbeInterval);
            }
            if (lastError != null) {
              continue;
            }
            lastError = StateError("quic send timeout");
            continue;
          } else {
            await lane.send(peer, bytes);
            return;
          }
        } catch (err) {
          lastError = err;
        }
      }
      if (lastError != null) {
        // Propagate the most recent send failure.
        // ignore: only_throw_errors
        throw lastError;
      }
      throw StateError("all lanes failed to send");
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
