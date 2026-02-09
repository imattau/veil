import "dart:async";
import "dart:convert";
import "dart:io";
import "dart:typed_data";

const _lanPrefix = "VEIL_LAN1";

class LanDiscoveryMessage {
  final List<String> wsEndpoints;
  final List<String> quicEndpoints;
  final String? peerId;
  final int timestampMs;

  const LanDiscoveryMessage({
    required this.wsEndpoints,
    required this.quicEndpoints,
    this.peerId,
    required this.timestampMs,
  });

  Uint8List encode() {
    final payload = jsonEncode({
      "ws": wsEndpoints,
      "quic": quicEndpoints,
      if (peerId != null) "peer": peerId,
      "ts": timestampMs,
    });
    final data = utf8.encode(payload);
    final prefix = utf8.encode(_lanPrefix);
    final out = Uint8List(prefix.length + data.length);
    out.setAll(0, prefix);
    out.setAll(prefix.length, data);
    return out;
  }

  static LanDiscoveryMessage? decode(Uint8List bytes) {
    final prefix = utf8.encode(_lanPrefix);
    if (bytes.length < prefix.length) return null;
    for (var i = 0; i < prefix.length; i += 1) {
      if (bytes[i] != prefix[i]) return null;
    }
    final body = bytes.sublist(prefix.length);
    try {
      final decoded = jsonDecode(utf8.decode(body));
      if (decoded is! Map<String, dynamic>) return null;
      final wsRaw = decoded["ws"];
      final quicRaw = decoded["quic"];
      final ws = wsRaw is List ? wsRaw.whereType<String>().toList() : <String>[];
      final quic =
          quicRaw is List ? quicRaw.whereType<String>().toList() : <String>[];
      final peer = decoded["peer"] as String?;
      final ts = decoded["ts"] is int
          ? decoded["ts"] as int
          : DateTime.now().millisecondsSinceEpoch;
      return LanDiscoveryMessage(
        wsEndpoints: ws,
        quicEndpoints: quic,
        peerId: peer,
        timestampMs: ts,
      );
    } catch (_) {
      return null;
    }
  }
}

class LanDiscovery {
  final int port;
  final Duration broadcastInterval;
  final String? peerId;
  RawDatagramSocket? _socket;
  Timer? _timer;
  StreamSubscription? _subscription;

  LanDiscovery({
    this.port = 45555,
    this.broadcastInterval = const Duration(seconds: 3),
    this.peerId,
  });

  Future<void> start({
    required List<String> wsEndpoints,
    required List<String> quicEndpoints,
    required void Function(LanDiscoveryMessage msg) onMessage,
  }) async {
    if (_socket != null) return;
    final socket = await RawDatagramSocket.bind(
      InternetAddress.anyIPv4,
      port,
      reuseAddress: true,
      reusePort: true,
    );
    socket.broadcastEnabled = true;
    _socket = socket;

    _subscription = socket.listen((event) {
      if (event != RawSocketEvent.read) return;
      final datagram = socket.receive();
      if (datagram == null) return;
      final msg = LanDiscoveryMessage.decode(datagram.data);
      if (msg == null) return;
      onMessage(msg);
    });

    _timer = Timer.periodic(broadcastInterval, (_) {
      final msg = LanDiscoveryMessage(
        wsEndpoints: wsEndpoints,
        quicEndpoints: quicEndpoints,
        peerId: peerId,
        timestampMs: DateTime.now().millisecondsSinceEpoch,
      );
      final data = msg.encode();
      socket.send(data, InternetAddress("255.255.255.255"), port);
    });
  }

  Future<void> stop() async {
    _timer?.cancel();
    _timer = null;
    await _subscription?.cancel();
    _subscription = null;
    _socket?.close();
    _socket = null;
  }
}
