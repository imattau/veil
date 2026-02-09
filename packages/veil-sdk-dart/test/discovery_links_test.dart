import "package:test/test.dart";
import "package:veil_sdk/src/discovery_links.dart";

void main() {
  test("buildVpsDiscoveryLink includes endpoints and cert", () {
    final link = buildVpsDiscoveryLink(
      wsEndpoints: const ["wss://node.example/ws", "wss://node2.example/ws"],
      quicEndpoint: "quic://node.example:4444",
      quicCertHex: "aa",
      peers: const ["peer1", "peer2"],
      tags: const ["tag1", "tag2"],
    );
    expect(link.startsWith("veil://vps?"), isTrue);
    expect(link.contains("ws=wss%3A%2F%2Fnode.example%2Fws"), isTrue);
    expect(link.contains("quic=quic%3A%2F%2Fnode.example%3A4444"), isTrue);
    expect(link.contains("cert=aa"), isTrue);
    expect(link.contains("peer=peer1"), isTrue);
    expect(link.contains("tag=tag2"), isTrue);
  });

  test("buildVpsDiscoveryLink prefers certb64", () {
    final link = buildVpsDiscoveryLink(
      wsEndpoints: const ["wss://node.example/ws"],
      quicEndpoint: "quic://node.example:4444",
      quicCertHex: "aa",
      quicCertB64: "bb",
    );
    expect(link.contains("certb64=bb"), isTrue);
    expect(link.contains("cert=aa"), isFalse);
  });
}
