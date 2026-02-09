import "dart:convert";

import "identity/contact_bundle.dart";
import "vps_profile.dart";

class DiscoveryParseResult {
  final List<String> wsEndpoints;
  final List<String> quicEndpoints;
  final List<String> peers;
  final List<String> tags;
  final String? quicCertHex;
  final String? quicCertB64;
  final ContactBundle? contactBundle;
  final bool isVpsProfile;

  const DiscoveryParseResult({
    required this.wsEndpoints,
    required this.quicEndpoints,
    required this.peers,
    required this.tags,
    this.quicCertHex,
    this.quicCertB64,
    this.contactBundle,
    this.isVpsProfile = false,
  });
}

DiscoveryParseResult? parseDiscoveryInput(String value) {
  final raw = value.trim();
  if (raw.isEmpty) return null;
  if (raw.startsWith("veil://contact")) {
    try {
      final bundle = ContactBundle.fromQrString(raw);
      return DiscoveryParseResult(
        wsEndpoints: const [],
        quicEndpoints: const [],
        peers: const [],
        tags: const [],
        contactBundle: bundle,
        isVpsProfile: false,
      );
    } catch (_) {
      return null;
    }
  }
  if (raw.startsWith("veil://") || raw.startsWith("veil:vps:") || raw.startsWith("vps:")) {
    final uri = _normalizeVpsUri(raw);
    if (uri == null) return null;
    final ws = uri.queryParametersAll["ws"] ?? const [];
    final peers = uri.queryParametersAll["peer"] ?? const [];
    final tags = uri.queryParametersAll["tag"] ?? const [];
    final quic = uri.queryParametersAll["quic"] ?? const [];
    final cert = uri.queryParameters["cert"];
    final certB64 = uri.queryParameters["certb64"];
    return DiscoveryParseResult(
      wsEndpoints: ws,
      quicEndpoints: quic,
      peers: peers,
      tags: tags,
      quicCertHex: cert,
      quicCertB64: certB64,
      contactBundle: null,
      isVpsProfile: true,
    );
  }
  if (raw.startsWith("http://") || raw.startsWith("https://")) {
    final uri = Uri.tryParse(raw);
    if (uri != null && (uri.path.isEmpty || uri.path == "/" || uri.path.endsWith("/config.js"))) {
      return DiscoveryParseResult(
        wsEndpoints: const [],
        quicEndpoints: const [],
        peers: const [],
        tags: const [],
        contactBundle: null,
        isVpsProfile: true,
      );
    }
  }
  if (raw.startsWith("ws://") || raw.startsWith("wss://")) {
    return DiscoveryParseResult(
      wsEndpoints: [raw],
      quicEndpoints: const [],
      peers: const [],
      tags: const [],
      contactBundle: null,
      isVpsProfile: false,
    );
  }
  if (raw.startsWith("quic://")) {
    return DiscoveryParseResult(
      wsEndpoints: const [],
      quicEndpoints: [raw],
      peers: const [],
      tags: const [],
      contactBundle: null,
      isVpsProfile: false,
    );
  }
  if (raw.startsWith("peer:")) {
    return DiscoveryParseResult(
      wsEndpoints: const [],
      quicEndpoints: const [],
      peers: [raw.substring(5)],
      tags: const [],
      contactBundle: null,
      isVpsProfile: false,
    );
  }
  if (raw.startsWith("tag:")) {
    return DiscoveryParseResult(
      wsEndpoints: const [],
      quicEndpoints: const [],
      peers: const [],
      tags: [raw.substring(4)],
      contactBundle: null,
      isVpsProfile: false,
    );
  }
  final hex = raw.toLowerCase().replaceAll(RegExp(r"[^0-9a-f]"), "");
  if (hex.length == 64) {
    return DiscoveryParseResult(
      wsEndpoints: const [],
      quicEndpoints: const [],
      peers: const [],
      tags: [hex],
      contactBundle: null,
      isVpsProfile: false,
    );
  }
  return null;
}

VpsProfile? parseVpsProfileFromConfig(String host, String body) {
  return VpsProfile.parseConfigJs(host, body);
}

Uri? _normalizeVpsUri(String raw) {
  final lower = raw.toLowerCase();
  if (lower.startsWith("veil://")) {
    return Uri.tryParse(raw);
  }
  if (lower.startsWith("vps:")) {
    return Uri.tryParse("veil://${raw.substring(4)}");
  }
  if (lower.startsWith("veil:vps:")) {
    return Uri.tryParse("veil://${raw.substring(9)}");
  }
  return null;
}

String? tryDecodeVpsProfileFromConfigJs(String host, String body) {
  final profile = VpsProfile.parseConfigJs(host, body);
  if (profile == null) return null;
  return jsonEncode({
    "host": profile.host,
    "ws": profile.wsUrl,
    if (profile.quicEndpoint != null) "quic": profile.quicEndpoint,
    if (profile.quicCertB64 != null) "certb64": profile.quicCertB64,
  });
}
