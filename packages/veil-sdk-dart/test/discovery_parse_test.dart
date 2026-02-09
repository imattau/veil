import "package:test/test.dart";
import "package:veil_sdk/src/discovery_parse.dart";
import "package:veil_sdk/src/identity/contact_bundle.dart";
import "package:veil_sdk/src/identity/identity.dart";

void main() {
  test("parseDiscoveryInput ignores invalid contact bundle", () {
    final bundle = "veil://contact?b=YQ"; // invalid bundle data
    final result = parseDiscoveryInput(bundle);
    expect(result, isNull);
  });

  test("parseDiscoveryInput handles valid contact bundle", () async {
    final identity = await generateIdentity();
    final bundle = ContactBundle(
      version: 1,
      pubkeyHex: identity.publicKeyHex,
      quicCertHex: "aa",
      endpoints: const ["quic://127.0.0.1:9000"],
      createdAt: 1,
    );
    final signed = await bundle.sign(identity);
    final result = parseDiscoveryInput(signed.toQrString());
    expect(result, isNotNull);
    expect(result!.contactBundle, isNotNull);
    expect(result.contactBundle!.pubkeyHex, identity.publicKeyHex);
  });

  test("parseDiscoveryInput handles vps link", () {
    final value =
        "veil://vps?ws=wss%3A%2F%2Fnode.example%2Fws&quic=quic%3A%2F%2Fnode.example%3A4444&cert=aa&peer=p1&tag=t1";
    final result = parseDiscoveryInput(value);
    expect(result, isNotNull);
    expect(result!.wsEndpoints.first, "wss://node.example/ws");
    expect(result.quicEndpoints.first, "quic://node.example:4444");
    expect(result.quicCertHex, "aa");
    expect(result.peers.first, "p1");
    expect(result.tags.first, "t1");
    expect(result.isVpsProfile, isTrue);
  });

  test("parseDiscoveryInput handles ws/quic/peer/tag", () {
    expect(parseDiscoveryInput("wss://node.example/ws")!.wsEndpoints.length, 1);
    expect(parseDiscoveryInput("quic://node.example:4444")!.quicEndpoints.length, 1);
    expect(parseDiscoveryInput("peer:abc")!.peers.first, "abc");
    expect(parseDiscoveryInput("tag:dead")!.tags.first, "dead");
  });
}
