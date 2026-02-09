import "contact_import.dart";

class ContactBundleMergeResult {
  final List<String> wsEndpoints;
  final List<String> quicEndpoints;
  final Map<String, String> quicCertsByEndpoint;
  final String pubkeyHex;

  const ContactBundleMergeResult({
    required this.wsEndpoints,
    required this.quicEndpoints,
    required this.quicCertsByEndpoint,
    required this.pubkeyHex,
  });
}

ContactBundleMergeResult mergeContactBundleImport(
  ContactBundleImportResult import, {
  List<String> existingWs = const [],
  List<String> existingQuic = const [],
  Map<String, String> existingQuicCerts = const {},
}) {
  final ws = <String>[...existingWs];
  for (final endpoint in import.wsEndpoints) {
    if (!ws.contains(endpoint)) {
      ws.add(endpoint);
    }
  }
  final quic = <String>[...existingQuic];
  for (final endpoint in import.quicEndpoints) {
    if (!quic.contains(endpoint)) {
      quic.add(endpoint);
    }
  }
  final certs = Map<String, String>.from(existingQuicCerts);
  if (import.quicCertHex.isNotEmpty && import.quicEndpoints.isNotEmpty) {
    final first = import.quicEndpoints.first;
    final current = certs[first];
    if (current == null || current.isEmpty) {
      certs[first] = import.quicCertHex;
    }
  }
  return ContactBundleMergeResult(
    wsEndpoints: ws,
    quicEndpoints: quic,
    quicCertsByEndpoint: certs,
    pubkeyHex: import.pubkeyHex,
  );
}
