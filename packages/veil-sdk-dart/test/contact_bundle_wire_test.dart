import "package:test/test.dart";
import "package:veil_sdk/src/identity/contact_bundle.dart";
import "package:veil_sdk/src/identity/contact_import.dart";
import "package:veil_sdk/src/identity/contact_wire.dart";
import "package:veil_sdk/src/identity/identity.dart";

void main() {
  test("merge contact bundle import dedupes and pins cert", () async {
    final identity = await generateIdentity();
    final bundle = ContactBundle(
      version: 1,
      pubkeyHex: identity.publicKeyHex,
      quicCertHex: "aa",
      endpoints: const [
        "quic://127.0.0.1:9000",
        "wss://node.example/ws",
        "custom://peer",
      ],
      createdAt: 1,
    );
    final signed = await bundle.sign(identity);
    final imported =
        await ContactBundleImport.fromQrString(signed.toQrString());
    expect(imported, isNotNull);
    final merged = mergeContactBundleImport(
      imported!,
      existingWs: const ["wss://node.example/ws"],
      existingQuic: const ["quic://127.0.0.1:9000"],
      existingQuicCerts: const {},
    );
    expect(merged.wsEndpoints, ["wss://node.example/ws"]);
    expect(merged.quicEndpoints, ["quic://127.0.0.1:9000"]);
    expect(merged.quicCertsByEndpoint["quic://127.0.0.1:9000"], "aa");
    expect(merged.pubkeyHex, identity.publicKeyHex);
  });

  test("merge preserves ordering and appends new endpoints", () async {
    final identity = await generateIdentity();
    final bundle = ContactBundle(
      version: 1,
      pubkeyHex: identity.publicKeyHex,
      quicCertHex: "aa",
      endpoints: const [
        "quic://127.0.0.1:9000",
        "wss://node.example/ws",
        "wss://node2.example/ws",
      ],
      createdAt: 1,
    );
    final signed = await bundle.sign(identity);
    final imported =
        await ContactBundleImport.fromQrString(signed.toQrString());
    expect(imported, isNotNull);

    final merged = mergeContactBundleImport(
      imported!,
      existingWs: const ["wss://node.example/ws"],
      existingQuic: const ["quic://127.0.0.1:9000"],
      existingQuicCerts: const {},
    );

    expect(
      merged.wsEndpoints,
      ["wss://node.example/ws", "wss://node2.example/ws"],
    );
    expect(merged.quicEndpoints, ["quic://127.0.0.1:9000"]);
  });
}
