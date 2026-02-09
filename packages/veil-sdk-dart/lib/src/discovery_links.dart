String buildVpsDiscoveryLink({
  List<String> wsEndpoints = const [],
  String? quicEndpoint,
  String? quicCertHex,
  String? quicCertB64,
  List<String> peers = const [],
  List<String> tags = const [],
}) {
  final params = <String, List<String>>{};
  final ws = _dedupe(wsEndpoints.where((e) => e.isNotEmpty));
  if (ws.isNotEmpty) {
    params["ws"] = ws;
  }
  if (quicEndpoint != null && quicEndpoint.isNotEmpty) {
    params["quic"] = [quicEndpoint];
  }
  final certB64 = quicCertB64?.trim() ?? "";
  final certHex = quicCertHex?.trim() ?? "";
  if (certB64.isNotEmpty) {
    params["certb64"] = [certB64];
  } else if (certHex.isNotEmpty) {
    params["cert"] = [certHex];
  }
  final peerList = _dedupe(peers.where((e) => e.isNotEmpty));
  if (peerList.isNotEmpty) {
    params["peer"] = peerList;
  }
  final tagList = _dedupe(tags.where((e) => e.isNotEmpty));
  if (tagList.isNotEmpty) {
    params["tag"] = tagList;
  }
  final query = params.entries
      .expand(
        (entry) => entry.value.map(
          (value) =>
              "${Uri.encodeQueryComponent(entry.key)}=${Uri.encodeQueryComponent(value)}",
        ),
      )
      .join("&");
  return query.isEmpty ? "veil://vps" : "veil://vps?$query";
}

List<String> _dedupe(Iterable<String> values) {
  final out = <String>[];
  final seen = <String>{};
  for (final value in values) {
    if (seen.add(value)) {
      out.add(value);
    }
  }
  return out;
}
