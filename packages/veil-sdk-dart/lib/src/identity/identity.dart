import "dart:typed_data";

import "package:cryptography/cryptography.dart";

class IdentityKeypair {
  final Uint8List publicKey;
  final Uint8List secretKey;

  const IdentityKeypair({required this.publicKey, required this.secretKey});

  String get publicKeyHex => _hexEncode(publicKey);
  String get secretKeyHex => _hexEncode(secretKey);
}

abstract class IdentityStore {
  Future<IdentityKeypair?> load();
  Future<void> save(IdentityKeypair identity);
  Future<void> clear();
}

class MemoryIdentityStore implements IdentityStore {
  IdentityKeypair? _identity;

  @override
  Future<IdentityKeypair?> load() async => _identity;

  @override
  Future<void> save(IdentityKeypair identity) async {
    _identity = identity;
  }

  @override
  Future<void> clear() async {
    _identity = null;
  }
}

Future<IdentityKeypair> generateIdentity() async {
  final algorithm = Ed25519();
  final keyPair = await algorithm.newKeyPair();
  final keyPairData = await keyPair.extract();
  final publicKey = await keyPair.extractPublicKey();
  final secretBytes = await keyPairData.extractPrivateKeyBytes();
  return IdentityKeypair(
    publicKey: Uint8List.fromList(publicKey.bytes),
    secretKey: Uint8List.fromList(secretBytes),
  );
}

Future<IdentityKeypair> loadOrCreateIdentity(IdentityStore store) async {
  final existing = await store.load();
  if (existing != null) return existing;
  final created = await generateIdentity();
  await store.save(created);
  return created;
}

Future<Uint8List> signMessage(
  Uint8List message,
  IdentityKeypair identity,
) async {
  final algorithm = Ed25519();
  final keyPair = SimpleKeyPairData(
    identity.secretKey,
    publicKey: SimplePublicKey(identity.publicKey, type: KeyPairType.ed25519),
    type: KeyPairType.ed25519,
  );
  final signature = await algorithm.sign(message, keyPair: keyPair);
  return Uint8List.fromList(signature.bytes);
}

Future<bool> verifyMessage(
  Uint8List message,
  Uint8List signatureBytes,
  Uint8List publicKey,
) async {
  final algorithm = Ed25519();
  final signature = Signature(
    signatureBytes,
    publicKey: SimplePublicKey(publicKey, type: KeyPairType.ed25519),
  );
  return algorithm.verify(message, signature: signature);
}

String _hexEncode(Uint8List bytes) {
  final buffer = StringBuffer();
  for (final b in bytes) {
    buffer.write(b.toRadixString(16).padLeft(2, "0"));
  }
  return buffer.toString();
}

Uint8List hexDecode(String hex) {
  final normalized = hex.trim().toLowerCase();
  if (normalized.length % 2 != 0) {
    throw ArgumentError("invalid hex length");
  }
  final out = Uint8List(normalized.length ~/ 2);
  for (var i = 0; i < normalized.length; i += 2) {
    out[i ~/ 2] = int.parse(normalized.substring(i, i + 2), radix: 16);
  }
  return out;
}
