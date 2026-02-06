import "dart:async";

import "package:flutter_reactive_ble/flutter_reactive_ble.dart";

import "lane.dart";

class BleLane implements VeilLane {
  final FlutterReactiveBle ble;
  final Uuid serviceUuid;
  final Uuid characteristicUuid;
  final String deviceId;
  final int mtu;

  final List<LaneMessage> _inbox = [];
  final List<List<int>> _sendBuffer = [];
  StreamSubscription<List<int>>? _subscription;

  BleLane({
    required this.ble,
    required this.serviceUuid,
    required this.characteristicUuid,
    required this.deviceId,
    this.mtu = 180,
  }) {
    _subscription = ble
        .subscribeToCharacteristic(QualifiedCharacteristic(
          serviceId: serviceUuid,
          characteristicId: characteristicUuid,
          deviceId: deviceId,
        ))
        .listen((data) {
          _inbox.add(LaneMessage(peer: deviceId, bytes: data));
        });
  }

  @override
  Future<void> send(String peer, List<int> bytes) async {
    final frames = splitIntoFrames(bytes);
    for (final frame in frames) {
      _sendBuffer.add(frame);
      await ble.writeCharacteristicWithoutResponse(
        QualifiedCharacteristic(
          serviceId: serviceUuid,
          characteristicId: characteristicUuid,
          deviceId: peer,
        ),
        value: frame,
      );
    }
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

  Future<void> close() async {
    await _subscription?.cancel();
    _subscription = null;
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
