import "package:test/test.dart";
import "package:veil_sdk/src/identity/contact_bundle.dart";
import "package:veil_sdk/src/identity/contact_import.dart";
import "package:veil_sdk/src/identity/identity.dart";

void main() {
  test("contact bundle import classifies endpoints", () async {
    final identity = await generateIdentity();
    final bundle = ContactBundle(
      version: 1,
      pubkeyHex: identity.publicKeyHex,
      quicCertHex: "aa",
      endpoints: const [
        "quic://127.0.0.1:9000",
        "wss://node.example/ws",
        "https://node.example/ws",
        "custom://peer",
      ],
      createdAt: 1,
    );
    final signed = await bundle.sign(identity);
    final imported = await ContactBundleImport.fromQrString(signed.toQrString());
    expect(imported, isNotNull);
    expect(imported!.quicEndpoints.length, 1);
    expect(imported.wsEndpoints.length, 2);
    expect(imported.otherEndpoints.length, 1);
    expect(imported.pubkeyHex, identity.publicKeyHex);
    expect(imported.quicCertHex, "aa");
  });

  test("contact bundle import rejects invalid signature", () async {
    final identity = await generateIdentity();
    final bundle = ContactBundle(
      version: 1,
      pubkeyHex: identity.publicKeyHex,
      quicCertHex: "aa",
      endpoints: const ["quic://127.0.0.1:9000"],
      createdAt: 1,
      signatureHex: "deadbeef",
    );
    final imported = await ContactBundleImport.fromQrString(bundle.toQrString());
    expect(imported, isNull);
  });
}
