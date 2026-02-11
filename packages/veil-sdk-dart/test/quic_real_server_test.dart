import "dart:convert";
import "dart:io";

import "package:test/test.dart";
import "package:veil_sdk/src/lanes/quic_lane.dart";
import "package:veil_sdk/src/vps_profile.dart";

Future<String> _fetchConfigJs(String host) async {
  final client = HttpClient();
  client.connectionTimeout = const Duration(seconds: 6);
  try {
    final req = await client.getUrl(Uri.parse("https://$host/config.js"));
    final resp = await req.close();
    if (resp.statusCode >= 400) {
      throw StateError("config.js HTTP ${resp.statusCode}");
    }
    return await resp.transform(utf8.decoder).join();
  } finally {
    client.close(force: true);
  }
}

String _hexFromBase64(String value) {
  final bytes = base64Decode(value);
  final out = StringBuffer();
  for (final byte in bytes) {
    out.write(byte.toRadixString(16).padLeft(2, "0"));
  }
  return out.toString();
}

void main() {
  final runReal = Platform.environment["VEIL_REAL_QUIC"] == "1";
  final host =
      Platform.environment["VEIL_REAL_QUIC_HOST"] ?? "veilnode.3nostr.com";

  test(
    "real QUIC server handshake + send",
    () async {
      if (!await QuicLane.isSupported()) {
        fail("QuicLane not supported. Ensure native bridge is available.");
      }

      final body = await _fetchConfigJs(host);
      final profile = VpsProfile.parseConfigJs(host, body);
      expect(profile, isNotNull, reason: "config.js could not be parsed");
      final quicEndpoint = profile!.quicEndpoint;
      final certB64 = profile.quicCertB64;
      expect(quicEndpoint, isNotNull, reason: "config.js missing QUIC port");
      expect(certB64, isNotNull, reason: "config.js missing QUIC cert");

      final certHex = _hexFromBase64(certB64!);
      final fetched = await QuicLane.fetchPinnedCertHex(quicEndpoint!);
      expect(fetched, isNotNull, reason: "failed to fetch QUIC peer cert");

      final lane = QuicLane(
        endpoint: quicEndpoint,
        peerId: "real-server-test",
        trustedPeerCertHex: certHex,
      );
      expect(lane.debugHandle, greaterThan(0),
          reason: "failed to start QUIC lane");

      final target = quicEndpoint.replaceFirst("quic://", "");
      await lane.send(target, utf8.encode("veil-real-quic"));
      await Future<void>.delayed(const Duration(milliseconds: 250));

      final metrics = lane.metricsSnapshot();
      expect(metrics, isNotNull, reason: "missing QUIC metrics");
      expect(metrics!.sendAttempts, greaterThan(0),
          reason: "no QUIC send attempts recorded");
      expect(metrics.sendSuccess, greaterThan(0),
          reason:
              "QUIC send failed (errors=${metrics.sendErrors}, queued=${metrics.outboundQueued})");

      await lane.close();
    },
    skip: runReal ? false : "set VEIL_REAL_QUIC=1 to run",
  );
}
