import "package:flutter_secure_storage/flutter_secure_storage.dart";

import "identity.dart";

class SecureIdentityStore implements IdentityStore {
  static const _keyPublic = "veil_identity_public_key";
  static const _keySecret = "veil_identity_secret_key";

  final FlutterSecureStorage _storage;

  const SecureIdentityStore({FlutterSecureStorage? storage})
    : _storage = storage ?? const FlutterSecureStorage();

  @override
  Future<IdentityKeypair?> load() async {
    final pubHex = await _storage.read(key: _keyPublic);
    final secHex = await _storage.read(key: _keySecret);
    if (pubHex == null || secHex == null) return null;
    return IdentityKeypair(
      publicKey: hexDecode(pubHex),
      secretKey: hexDecode(secHex),
    );
  }

  @override
  Future<void> save(IdentityKeypair identity) async {
    await _storage.write(key: _keyPublic, value: identity.publicKeyHex);
    await _storage.write(key: _keySecret, value: identity.secretKeyHex);
  }

  @override
  Future<void> clear() async {
    await _storage.delete(key: _keyPublic);
    await _storage.delete(key: _keySecret);
  }
}
