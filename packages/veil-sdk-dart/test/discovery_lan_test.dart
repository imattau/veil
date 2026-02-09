import "dart:typed_data";

import "package:test/test.dart";
import "package:veil_sdk/src/discovery_lan.dart";

void main() {
  test("lan discovery encode/decode roundtrip", () {
    final msg = LanDiscoveryMessage(
      wsEndpoints: const ["wss://node.example/ws"],
      quicEndpoints: const ["quic://node.example:4444"],
      peerId: "peer1",
      timestampMs: 123,
    );
    final encoded = msg.encode();
    final decoded = LanDiscoveryMessage.decode(Uint8List.fromList(encoded));
    expect(decoded, isNotNull);
    expect(decoded!.wsEndpoints.first, "wss://node.example/ws");
    expect(decoded.quicEndpoints.first, "quic://node.example:4444");
    expect(decoded.peerId, "peer1");
    expect(decoded.timestampMs, 123);
  });

  test("lan discovery ignores invalid prefix", () {
    final bad = Uint8List.fromList([1, 2, 3, 4]);
    expect(LanDiscoveryMessage.decode(bad), isNull);
  });
}
