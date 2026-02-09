import "contact_bundle.dart";

class ContactBundleImportResult {
  final ContactBundle bundle;
  final List<String> wsEndpoints;
  final List<String> quicEndpoints;
  final List<String> otherEndpoints;
  final String pubkeyHex;
  final String quicCertHex;

  const ContactBundleImportResult({
    required this.bundle,
    required this.wsEndpoints,
    required this.quicEndpoints,
    required this.otherEndpoints,
    required this.pubkeyHex,
    required this.quicCertHex,
  });
}

class ContactBundleImport {
  static Future<ContactBundleImportResult?> fromQrString(String value) async {
    ContactBundle bundle;
    try {
      bundle = ContactBundle.fromQrString(value);
    } catch (_) {
      return null;
    }
    try {
      final ok = await bundle.verify();
      if (!ok) return null;
    } catch (_) {
      return null;
    }
    return fromBundle(bundle);
  }

  static ContactBundleImportResult fromBundle(ContactBundle bundle) {
    final wsEndpoints = <String>[];
    final quicEndpoints = <String>[];
    final otherEndpoints = <String>[];
    for (final endpoint in bundle.endpoints) {
      final lower = endpoint.toLowerCase();
      if (lower.startsWith("quic://")) {
        quicEndpoints.add(endpoint);
      } else if (lower.startsWith("ws://") ||
          lower.startsWith("wss://") ||
          lower.startsWith("http://") ||
          lower.startsWith("https://")) {
        wsEndpoints.add(endpoint);
      } else {
        otherEndpoints.add(endpoint);
      }
    }
    return ContactBundleImportResult(
      bundle: bundle,
      wsEndpoints: wsEndpoints,
      quicEndpoints: quicEndpoints,
      otherEndpoints: otherEndpoints,
      pubkeyHex: bundle.pubkeyHex,
      quicCertHex: bundle.quicCertHex,
    );
  }
}
