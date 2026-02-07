import "dart:async";
import "dart:typed_data";

import "package:flutter/services.dart";

import "lane.dart";

class TorLane implements VeilLane {
  static const MethodChannel _channel = MethodChannel("veil_tor");

  final String url;
  final String socksHost;
  final int socksPort;
  int _id = 0;
  bool _closed = false;
  int _outboundQueued = 0;
  int _outboundOk = 0;
  int _outboundErr = 0;
  int _inboundReceived = 0;
  int _inboundDropped = 0;
  int _reconnectAttempts = 0;

  TorLane({
    required this.url,
    this.socksHost = "127.0.0.1",
    this.socksPort = 9050,
  }) {
    _connect();
  }

  static Future<bool> isSupported() async {
    try {
      final supported = await _channel.invokeMethod<bool>("isSupported");
      return supported ?? false;
    } catch (_) {
      return false;
    }
  }

  Future<void> _connect() async {
    try {
      final id = await _channel.invokeMethod<int>("connect", {
        "url": url,
        "socksHost": socksHost,
        "socksPort": socksPort,
      });
      _id = id ?? 0;
    } catch (_) {
      _outboundErr += 1;
    }
  }

  @override
  Future<void> send(String peer, List<int> bytes) async {
    if (_closed) {
      throw StateError("lane is closed");
    }
    if (_id == 0) {
      await _connect();
    }
    try {
      await _channel.invokeMethod<void>("send", {
        "id": _id,
        "bytes": Uint8List.fromList(bytes),
      });
      _outboundOk += 1;
    } catch (_) {
      _outboundErr += 1;
      _outboundQueued += 1;
    }
  }

  @override
  Future<LaneMessage?> recv() async {
    if (_id == 0) return null;
    try {
      final result = await _channel.invokeMethod<dynamic>("recv", {"id": _id});
      if (result is Map) {
        final bytes = result["bytes"];
        if (bytes is Uint8List) {
          _inboundReceived += 1;
          return LaneMessage(peer: "tor", bytes: bytes);
        }
      }
      return null;
    } catch (_) {
      _inboundDropped += 1;
      return null;
    }
  }

  @override
  LaneHealthSnapshot healthSnapshot() {
    return LaneHealthSnapshot(
      outboundQueued: _outboundQueued,
      outboundSendOk: _outboundOk,
      outboundSendErr: _outboundErr,
      inboundReceived: _inboundReceived,
      inboundDropped: _inboundDropped,
      reconnectAttempts: _reconnectAttempts,
    );
  }

  Future<void> close() async {
    _closed = true;
    if (_id != 0) {
      try {
        await _channel.invokeMethod<void>("close", {"id": _id});
      } catch (_) {}
    }
  }
}
