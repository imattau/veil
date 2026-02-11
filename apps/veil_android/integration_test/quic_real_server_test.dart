import "dart:convert";
import "dart:io";

import "package:flutter_test/flutter_test.dart";
import "package:integration_test/integration_test.dart";
import "package:veil_android/app_controller.dart";
import "package:veil_sdk/veil_sdk.dart";

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  const runReal = bool.fromEnvironment("VEIL_REAL_QUIC");
  const host = String.fromEnvironment(
    "VEIL_REAL_QUIC_HOST",
    defaultValue: "veilnode.3nostr.com",
  );

  testWidgets(
    "android real QUIC via app controller",
    (tester) async {
      final controller = VeilAppController();
      await controller.init();

      final ok = await controller.importVpsFromDomain(host);
      expect(ok, isTrue, reason: "failed to import VPS profile");
      expect(controller.quicEndpointValue, isNotEmpty,
          reason: "VPS profile missing QUIC endpoint");

      final certHex = controller.quicTrustedCertValue;
      final endpoint = controller.quicEndpointValue;
      String? pinned;
      if (certHex.isEmpty) {
        pinned = await QuicLane.fetchPinnedCertHex(endpoint);
        expect(pinned, isNotNull, reason: "failed to fetch QUIC cert");
        controller.setQuicCertFor(endpoint, pinned!);
      } else {
        pinned = await QuicLane.fetchPinnedCertHex(endpoint);
        if (pinned == null || pinned.isEmpty) {
          print("QUIC: fetched cert missing; using config cert");
        } else if (pinned != certHex) {
          print("QUIC: cert mismatch (config != fetched); using fetched cert");
          controller.setQuicCertFor(endpoint, pinned);
        } else {
          print("QUIC: cert match (config == fetched)");
        }
      }

      final lane = QuicLane(
        endpoint: endpoint,
        peerId: "android-real-test",
        trustedPeerCertHex: controller.quicTrustedCertValue,
      );
      expect(lane.debugHandle, greaterThan(0),
          reason: "failed to start QUIC lane");

      final target = endpoint.replaceFirst("quic://", "");
      await lane.send(target, utf8.encode("veil-android-real-quic"));
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
    skip: runReal ? false : true,
  );
}
