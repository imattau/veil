import "dart:convert";
import "dart:typed_data";

import "../utils/cbor_min.dart";
import "identity.dart";

class ContactBundle {
  final int version;
  final String pubkeyHex;
  final String quicCertHex;
  final List<String> endpoints;
  final int createdAt;
  final String? signatureHex;

  const ContactBundle({
    required this.version,
    required this.pubkeyHex,
    required this.quicCertHex,
    required this.endpoints,
    required this.createdAt,
    this.signatureHex,
  });

  ContactBundle copyWith({
    int? version,
    String? pubkeyHex,
    String? quicCertHex,
    List<String>? endpoints,
    int? createdAt,
    String? signatureHex,
  }) {
    return ContactBundle(
      version: version ?? this.version,
      pubkeyHex: pubkeyHex ?? this.pubkeyHex,
      quicCertHex: quicCertHex ?? this.quicCertHex,
      endpoints: endpoints ?? this.endpoints,
      createdAt: createdAt ?? this.createdAt,
      signatureHex: signatureHex ?? this.signatureHex,
    );
  }

  Uint8List signedBytes() {
    final payload = [
      version,
      pubkeyHex,
      quicCertHex,
      endpoints,
      createdAt,
    ];
    return encodeCbor(payload);
  }

  Uint8List toCborBytes() {
    final payload = [
      version,
      pubkeyHex,
      quicCertHex,
      endpoints,
      createdAt,
      signatureHex ?? "",
    ];
    return encodeCbor(payload);
  }

  String toQrString() {
    final data = base64UrlEncode(toCborBytes());
    return "veil://contact?b=$data";
  }

  Future<ContactBundle> sign(IdentityKeypair identity) async {
    final sig = await signMessage(signedBytes(), identity);
    return copyWith(signatureHex: _hexEncode(sig));
  }

  Future<bool> verify() async {
    if (signatureHex == null || signatureHex!.isEmpty) return false;
    final sig = hexDecode(signatureHex!);
    final pubkey = hexDecode(pubkeyHex);
    return verifyMessage(signedBytes(), sig, pubkey);
  }

  static ContactBundle fromCborBytes(Uint8List bytes) {
    final decoded = decodeCbor(bytes);
    if (decoded is! List || decoded.length < 5) {
      throw ArgumentError("invalid contact bundle CBOR");
    }
    return ContactBundle(
      version: decoded[0] as int,
      pubkeyHex: decoded[1] as String,
      quicCertHex: decoded[2] as String,
      endpoints: (decoded[3] as List).cast<String>(),
      createdAt: decoded[4] as int,
      signatureHex: decoded.length > 5 ? decoded[5] as String : null,
    );
  }

  static ContactBundle fromQrString(String value) {
    final uri = Uri.tryParse(value);
    if (uri == null) {
      throw ArgumentError("invalid QR string");
    }
    final data = uri.queryParameters["b"];
    if (data == null || data.isEmpty) {
      throw ArgumentError("missing bundle data");
    }
    return fromCborBytes(base64Url.decode(data));
  }
}

String _hexEncode(Uint8List bytes) {
  final buffer = StringBuffer();
  for (final b in bytes) {
    buffer.write(b.toRadixString(16).padLeft(2, "0"));
  }
  return buffer.toString();
}
