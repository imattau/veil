import "dart:async";
import "dart:convert";
import "dart:io";
import "dart:typed_data";

import "package:web_socket_channel/web_socket_channel.dart";
import "package:web_socket_channel/io.dart";

import "lane.dart";

class WebSocketLane implements VeilLane {
  final Uri url;
  final String peerId;
  final String? trustedPeerCertHex;
  final Duration reconnectInitial;
  final Duration reconnectMax;
  final double backoffMultiplier;

  WebSocketChannel? _socket;
  final List<List<int>> _sendBuffer = [];
  final List<LaneMessage> _inbox = [];
  int _reconnectAttempts = 0;
  Duration _reconnectDelay;
  Timer? _reconnectTimer;
  bool _closed = false;

  int _outboundQueued = 0;
  int _outboundOk = 0;
  int _outboundErr = 0;
  int _inboundReceived = 0;
  int _inboundDropped = 0;
  String? _lastError;

  String? get lastError => _lastError;

  WebSocketLane({
    required this.url,
    required this.peerId,
    this.trustedPeerCertHex,
    this.reconnectInitial = const Duration(milliseconds: 250),
    this.reconnectMax = const Duration(seconds: 10),
    this.backoffMultiplier = 2.0,
  }) : _reconnectDelay = reconnectInitial {
    unawaited(_connect());
  }

  Future<void> _connect() async {
    try {
      final Map<String, String> headers = {};
      if (url.userInfo.isNotEmpty) {
        final auth = base64Encode(utf8.encode(url.userInfo));
        headers["Authorization"] = "Basic $auth";
      }

      final cleanUrl = Uri(
        scheme: url.scheme,
        host: url.host,
        port: url.port,
        path: url.path,
        query: url.query,
      );

      HttpClient? customClient;
      if (trustedPeerCertHex != null && trustedPeerCertHex!.isNotEmpty) {
        final ctx = SecurityContext(withTrustedRoots: false);
        ctx.setTrustedCertificatesBytes(_hexToBytes(trustedPeerCertHex!));
        customClient = HttpClient(context: ctx);
        customClient.badCertificateCallback = (cert, host, port) => true;
      }
      final socket = await WebSocket.connect(
        cleanUrl.toString(),
        customClient: customClient,
        headers: headers,
      );
      _socket = IOWebSocketChannel(socket);
      _socket!.stream.listen(
        (event) {
          try {
            if (event is List<int>) {
              _inbox.add(LaneMessage(peer: peerId, bytes: event));
            } else if (event is String) {
              _inbox.add(LaneMessage(peer: peerId, bytes: event.codeUnits));
            } else {
              _inboundDropped += 1;
              return;
            }
            _inboundReceived += 1;
          } catch (_) {
            _inboundDropped += 1;
          }
        },
        onError: (_) {
          _scheduleReconnect();
        },
        onDone: () {
          _scheduleReconnect();
        },
      );
    } catch (err) {
      _lastError = err.toString();
      _outboundErr += 1;
      _scheduleReconnect();
    }
  }

  List<int> _hexToBytes(String hex) {
    final buffer = <int>[];
    for (var i = 0; i < hex.length; i += 2) {
      buffer.add(int.parse(hex.substring(i, i + 2), radix: 16));
    }
    return Uint8List.fromList(buffer);
  }

  void _scheduleReconnect() {
    if (_closed) {
      return;
    }
    if (_reconnectTimer != null) {
      return;
    }
    final delay = _reconnectDelay;
    _reconnectDelay = Duration(
      milliseconds: (delay.inMilliseconds * backoffMultiplier)
          .clamp(0, reconnectMax.inMilliseconds)
          .toInt(),
    );
    _reconnectAttempts += 1;
    _reconnectTimer = Timer(delay, () {
      _reconnectTimer = null;
      unawaited(_connect());
      _flushBuffered();
    });
  }

  void _flushBuffered() {
    final socket = _socket;
    if (socket == null) {
      return;
    }
    while (_sendBuffer.isNotEmpty) {
      final bytes = _sendBuffer.removeAt(0);
      socket.sink.add(bytes);
      _outboundOk += 1;
    }
  }

  @override
  Future<void> send(String peer, List<int> bytes) async {
    if (_closed) {
      throw StateError("lane is closed");
    }
    final socket = _socket;
    if (socket == null) {
      _sendBuffer.add(bytes);
      _outboundQueued += 1;
      return;
    }
    try {
      socket.sink.add(bytes);
      _outboundOk += 1;
    } catch (err) {
      _outboundErr += 1;
      _sendBuffer.add(bytes);
      _scheduleReconnect();
      rethrow;
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
    _reconnectTimer?.cancel();
    await _socket?.sink.close();
  }
}
