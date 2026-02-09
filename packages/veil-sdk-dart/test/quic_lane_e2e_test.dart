import "dart:async";
import "dart:convert";
import "dart:io";

import "package:test/test.dart";
import "package:veil_sdk/src/lanes/quic_lane.dart";

Future<String> _repoRoot() async {
  var dir = Directory.current;
  final parts = dir.path.split(Platform.pathSeparator);
  if (parts.isNotEmpty && parts.last == "veil-sdk-dart") {
    return dir.parent.path;
  }
  return dir.path;
}

void main() {
  final libPath = Platform.environment["VEIL_SDK_BRIDGE_LIB"];
  final libExists = libPath != null && libPath.isNotEmpty && File(libPath).existsSync();
  if (!libExists) {
    test(
      "quic lane e2e",
      () {},
      skip:
          "Set VEIL_SDK_BRIDGE_LIB to the built veil_sdk_bridge library path "
          "(e.g. target/debug/libveil_sdk_bridge.so).",
    );
    return;
  }

  Future<Process> _startServer(String root) {
    return Process.start(
      "cargo",
      ["run", "-p", "veil_sdk_bridge", "--bin", "quic_test_server", "--", "--timeout-ms", "12000"],
      workingDirectory: root,
      environment: {"VEIL_QUIC_DEBUG": "1"},
    );
  }

  Future<({String addr, String certHex, StreamSubscription<String> sub, List<String> errors, Process process})>
  _waitReady(Process server) async {
    final lines = server.stdout
        .transform(utf8.decoder)
        .transform(const LineSplitter());
    final errors = server.stderr
        .transform(utf8.decoder)
        .transform(const LineSplitter());
    final errorBuffer = <String>[];
    final errorSub = errors.listen(errorBuffer.add);

    final readyCompleter = Completer<String>();
    final lineSub = lines.listen((line) {
      if (!readyCompleter.isCompleted && line.startsWith("READY ")) {
        readyCompleter.complete(line);
      }
    });

    try {
      final readyLine = await readyCompleter.future.timeout(
        const Duration(seconds: 15),
      );
      final parts = readyLine.split(" ");
      if (parts.length < 3) {
        throw StateError("invalid READY line: $readyLine");
      }
      return (
        addr: parts[1],
        certHex: parts[2],
        sub: lineSub,
        errors: errorBuffer,
        process: server,
      );
    } on TimeoutException {
      server.kill();
      await lineSub.cancel();
      await errorSub.cancel();
      fail("server did not emit READY line. stderr: ${errorBuffer.join("\n")}");
    }
    throw StateError("unreachable");
  }

  test("quic lane fetch cert e2e", () async {
    final root = await _repoRoot();
    final server = await _startServer(root);
    final ready = await _waitReady(server);
    final addr = ready.addr;
    final certHex = ready.certHex;

    final endpoint = "quic://$addr";
    final fetchedCert = await QuicLane.fetchPinnedCertHex(endpoint);
    expect(fetchedCert, isNotNull);
    expect(fetchedCert, certHex);

    ready.process.kill();
    await ready.sub.cancel();
  }, timeout: const Timeout(Duration(seconds: 25)));

  test("quic lane send e2e", () async {
    final root = await _repoRoot();
    final server = await _startServer(root);
    final ready = await _waitReady(server);
    final addr = ready.addr;
    final certHex = ready.certHex;

    final endpoint = "quic://$addr";
    final lane = QuicLane(
      endpoint: endpoint,
      peerId: "server",
      trustedPeerCertHex: certHex,
    );
    expect(lane.debugHandle, greaterThan(0), reason: "QUIC lane failed to start");
    final payload = utf8.encode("hello-quic");
    for (var i = 0; i < 3; i += 1) {
      await lane.send(addr, payload);
      final snapshot = lane.healthSnapshot();
      if (snapshot.outboundSendErr == 0 && snapshot.outboundSendOk > 0) {
        break;
      }
      await Future<void>.delayed(const Duration(milliseconds: 50));
    }
    final afterSend = lane.healthSnapshot();
    expect(
      afterSend.outboundSendErr,
      0,
      reason: "QUIC send failed via Dart FFI (lastResult=${lane.debugLastSendResult})",
    );

    await lane.close();
    ready.process.kill();
    await ready.sub.cancel();
  }, timeout: const Timeout(Duration(seconds: 40)));
}
