import "dart:typed_data";

import "package:test/test.dart";
import "package:veil_sdk/src/identity/contact_bundle.dart";
import "package:veil_sdk/src/identity/identity.dart";

void main() {
  test("contact bundle signs and verifies", () async {
    final identity = await generateIdentity();
    final bundle = ContactBundle(
      version: 1,
      pubkeyHex: identity.publicKeyHex,
      quicCertHex: "aa",
      endpoints: const ["quic://127.0.0.1:9000"],
      createdAt: 1,
    );
    final signed = await bundle.sign(identity);
    expect(signed.signatureHex, isNotNull);
    expect(await signed.verify(), isTrue);

    final encoded = signed.toCborBytes();
    final decoded = ContactBundle.fromCborBytes(encoded);
    expect(decoded.pubkeyHex, identity.publicKeyHex);
    expect(await decoded.verify(), isTrue);
  });
}
