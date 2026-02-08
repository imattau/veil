import "dart:convert";

class VpsProfile {
  final String host;
  final String wsUrl;
  final String? quicEndpoint;
  final String? quicCertB64;

  const VpsProfile({
    required this.host,
    required this.wsUrl,
    this.quicEndpoint,
    this.quicCertB64,
  });

  factory VpsProfile.fromDomain(
    String host, {
    int? quicPort,
    String? quicCertB64,
    bool secure = true,
  }) {
    final wsScheme = secure ? "wss" : "ws";
    final wsUrl = "$wsScheme://$host/ws";
    final quicEndpoint =
        quicPort == null ? null : "quic://$host:$quicPort";
    return VpsProfile(
      host: host,
      wsUrl: wsUrl,
      quicEndpoint: quicEndpoint,
      quicCertB64: quicCertB64,
    );
  }

  static VpsProfile? parseConfigJs(String host, String body) {
    final quicPort = _extractConfigValue(body, "VEIL_VPS_QUIC_PORT");
    final quicCertB64 = _extractConfigValue(body, "VEIL_VPS_QUIC_CERT_B64");
    final port = int.tryParse(quicPort ?? "");
    return VpsProfile.fromDomain(
      host,
      quicPort: port,
      quicCertB64: quicCertB64,
    );
  }

  String toProfileUri() {
    final params = <String, String>{"ws": wsUrl};
    if (quicEndpoint != null && quicEndpoint!.isNotEmpty) {
      params["quic"] = quicEndpoint!;
    }
    if (quicCertB64 != null && quicCertB64!.isNotEmpty) {
      params["certb64"] = quicCertB64!;
    }
    final query = params.entries
        .map((entry) =>
            "${Uri.encodeQueryComponent(entry.key)}=${Uri.encodeQueryComponent(entry.value)}")
        .join("&");
    return "veil://vps?$query";
  }

  String toDebugJson() {
    return jsonEncode({
      "host": host,
      "ws": wsUrl,
      if (quicEndpoint != null) "quic": quicEndpoint,
      if (quicCertB64 != null) "certb64": quicCertB64,
    });
  }
}

String? _extractConfigValue(String body, String key) {
  final lines = body.split("\n");
  for (final raw in lines) {
    final line = raw.trim();
    if (!line.startsWith("window.")) continue;
    if (!line.contains(key)) continue;
    final parts = line.split("=");
    if (parts.length < 2) continue;
    var value = parts[1].trim();
    if (value.endsWith(";")) {
      value = value.substring(0, value.length - 1);
    }
    value = value.trim();
    value = value.replaceAll("\"", "").replaceAll("'", "");
    if (value.isEmpty) return null;
    return value;
  }
  return null;
}
