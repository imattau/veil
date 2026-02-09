import "dart:async";
import "dart:convert";
import "dart:io";
import "dart:typed_data";

import "package:test/test.dart";
import "package:veil_sdk/src/lanes/lane.dart";
import "package:veil_sdk/src/lanes/multi_lane.dart";
import "package:veil_sdk/src/lanes/quic_lane.dart";
import "package:veil_sdk/src/lanes/websocket_lane.dart";
import "package:veil_sdk/src/identity/contact_bundle.dart";
import "package:veil_sdk/src/identity/identity.dart";
import "package:veil_sdk/src/utils/cbor_min.dart";

Future<String> _repoRoot() async {
  var dir = Directory.current;
  final parts = dir.path.split(Platform.pathSeparator);
  if (parts.isNotEmpty && parts.last == "veil-sdk-dart") {
    return dir.parent.path;
  }
  return dir.path;
}

Future<Process> _startQuicServer(String root) {
  return Process.start(
    "cargo",
    [
      "run",
      "-p",
      "veil_sdk_bridge",
      "--bin",
      "quic_test_server",
      "--",
      "--timeout-ms",
      "20000",
    ],
    workingDirectory: root,
    environment: {
      "VEIL_QUIC_DEBUG": "1",
      "VEIL_QUIC_ECHO": "1",
      "VEIL_QUIC_INSECURE": "1",
    },
  );
}

Future<Process> _startQuicRelay(String root) {
  return Process.start(
    "cargo",
    [
      "run",
      "-p",
      "veil_sdk_bridge",
      "--bin",
      "quic_relay_server",
    ],
    workingDirectory: root,
    environment: {
      "VEIL_QUIC_DEBUG": "1",
      "VEIL_QUIC_INSECURE": "1",
    },
  );
}

Future<({
  String addr,
  String certHex,
  StreamSubscription<String> sub,
  StreamSubscription<String> errSub,
  Process process,
})> _waitQuicReady(Process server) async {
  final lines = server.stdout
      .transform(utf8.decoder)
      .transform(const LineSplitter());
  final errLines = server.stderr
      .transform(utf8.decoder)
      .transform(const LineSplitter());
  final readyCompleter = Completer<String>();
  final sub = lines.listen((line) {
    if (!readyCompleter.isCompleted && line.startsWith("READY ")) {
      readyCompleter.complete(line);
    }
  });
  final errSub = errLines.listen((line) {
    stdout.writeln("relay: $line");
  });
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
    sub: sub,
    errSub: errSub,
    process: server,
  );
}

Future<HttpServer> _startWebSocketEchoServer() async {
  final server = await HttpServer.bind("127.0.0.1", 0);
  server.transform(WebSocketTransformer()).listen((socket) {
    socket.listen((event) {
      socket.add(event);
    });
  });
  return server;
}

Future<HttpServer> _startWebSocketBroadcastServer() async {
  final server = await HttpServer.bind("127.0.0.1", 0);
  final sockets = <WebSocket>{};
  server.transform(WebSocketTransformer()).listen((socket) {
    sockets.add(socket);
    socket.listen(
      (event) {
        for (final target in sockets) {
          if (target != socket) {
            target.add(event);
          }
        }
      },
      onDone: () => sockets.remove(socket),
      onError: (_) => sockets.remove(socket),
    );
  });
  return server;
}

Uint8List _messageToBytes(
  String channel,
  String fromPubkeyHex,
  Uint8List payload,
  Uint8List signature,
) {
  return encodeCbor([
    channel,
    fromPubkeyHex,
    payload,
    signature,
  ]);
}

Uint8List _messageSignedBytes(
  String channel,
  String fromPubkeyHex,
  Uint8List payload,
) {
  return encodeCbor([channel, fromPubkeyHex, payload]);
}

({String channel, String from, Uint8List payload, Uint8List signature}) _parseMessage(
  Uint8List bytes,
) {
  final decoded = decodeCbor(bytes);
  if (decoded is! List || decoded.length < 4) {
    throw StateError("invalid swarm message");
  }
  return (
    channel: decoded[0] as String,
    from: decoded[1] as String,
    payload: decoded[2] as Uint8List,
    signature: decoded[3] as Uint8List,
  );
}

void main() {
  final libPath = Platform.environment["VEIL_SDK_BRIDGE_LIB"];
  final libExists = libPath != null && libPath.isNotEmpty && File(libPath).existsSync();

  if (!libExists) {
    test(
      "p2p multilane harness",
      () {},
      skip:
          "Set VEIL_SDK_BRIDGE_LIB to the built veil_sdk_bridge library path "
          "(e.g. target/debug/libveil_sdk_bridge.so).",
    );
    return;
  }

  test("p2p multilane harness (quic + websocket)", () async {
    final root = await _repoRoot();
    final quicServer = await _startQuicServer(root);
    final quicReady = await _waitQuicReady(quicServer);
    final quicEndpoint = "quic://${quicReady.addr}";

    final wsServer = await _startWebSocketEchoServer();
    final wsUrl = Uri.parse("ws://127.0.0.1:${wsServer.port}/ws");

    final quicLane = QuicLane(
      endpoint: quicEndpoint,
      peerId: "quic-peer",
      trustedPeerCertHex: quicReady.certHex,
    );
    final wsLane = WebSocketLane(url: wsUrl, peerId: "ws-peer");
    await Future<void>.delayed(const Duration(milliseconds: 300));

    final multi = MultiLane(
      lanes: [quicLane, wsLane],
      sendMode: MultiLaneSendMode.broadcast,
    );

    final payload = utf8.encode("p2p-multilane");
    await multi.send(quicReady.addr, payload);

    LaneMessage? received;
    final deadline = DateTime.now().add(const Duration(seconds: 10));
    while (DateTime.now().isBefore(deadline)) {
      received = await multi.recv();
      if (received != null) {
        break;
      }
      await Future<void>.delayed(const Duration(milliseconds: 50));
    }

    await quicLane.close();
    await wsLane.close();
    await wsServer.close(force: true);
    quicReady.process.kill();
    await quicReady.sub.cancel();
    await quicReady.errSub.cancel();

    expect(received, isNotNull, reason: "no message received via multilane");
    expect(received!.bytes, payload);
  }, timeout: const Timeout(Duration(seconds: 30)));

  test("p2p multilane harness (quic primary -> ws fallback)", () async {
    final wsServer = await _startWebSocketEchoServer();
    final wsUrl = Uri.parse("ws://127.0.0.1:${wsServer.port}/ws");

    final quicLane = QuicLane(
      endpoint: "quic://127.0.0.1:9",
      peerId: "quic-primary",
      trustedPeerCertHex: "",
    );
    final wsLane = WebSocketLane(url: wsUrl, peerId: "ws-fallback");
    await Future<void>.delayed(const Duration(milliseconds: 300));

    final multi = MultiLane(
      lanes: [quicLane, wsLane],
      sendMode: MultiLaneSendMode.primaryThenFallback,
    );

    final payload = utf8.encode("fallback-test");
    // Use an invalid QUIC peer to force a synchronous QUIC send error so
    // the fallback path can be exercised deterministically.
    await multi.send("invalid", payload);

    LaneMessage? received;
    final deadline = DateTime.now().add(const Duration(seconds: 10));
    while (DateTime.now().isBefore(deadline)) {
      received = await multi.recv();
      if (received != null) {
        break;
      }
      await Future<void>.delayed(const Duration(milliseconds: 50));
    }

    await quicLane.close();
    await wsLane.close();
    await wsServer.close(force: true);

    expect(received, isNotNull, reason: "fallback message not received");
    expect(received!.bytes, payload);
  }, timeout: const Timeout(Duration(seconds: 30)));

  test("p2p multilane swarm (ws channels + identities)", () async {
    final wsServer = await _startWebSocketBroadcastServer();
    final wsUrl = Uri.parse("ws://127.0.0.1:${wsServer.port}/swarm");

    final identityA = await generateIdentity();
    final identityB = await generateIdentity();
    final identityC = await generateIdentity();

    final bundleA = await ContactBundle(
      version: 1,
      pubkeyHex: identityA.publicKeyHex,
      quicCertHex: "aa",
      endpoints: const [],
      createdAt: 1,
    ).sign(identityA);
    final bundleB = await ContactBundle(
      version: 1,
      pubkeyHex: identityB.publicKeyHex,
      quicCertHex: "bb",
      endpoints: const [],
      createdAt: 1,
    ).sign(identityB);
    final bundleC = await ContactBundle(
      version: 1,
      pubkeyHex: identityC.publicKeyHex,
      quicCertHex: "cc",
      endpoints: const [],
      createdAt: 1,
    ).sign(identityC);
    expect(await bundleA.verify(), isTrue);
    expect(await bundleB.verify(), isTrue);
    expect(await bundleC.verify(), isTrue);

    final laneA = MultiLane(
      lanes: [WebSocketLane(url: wsUrl, peerId: "ws-a")],
      sendMode: MultiLaneSendMode.broadcast,
    );
    final laneB = MultiLane(
      lanes: [WebSocketLane(url: wsUrl, peerId: "ws-b")],
      sendMode: MultiLaneSendMode.broadcast,
    );
    final laneC = MultiLane(
      lanes: [WebSocketLane(url: wsUrl, peerId: "ws-c")],
      sendMode: MultiLaneSendMode.broadcast,
    );
    await Future<void>.delayed(const Duration(milliseconds: 300));

    final subA = {"alpha", "beta"};
    final subB = {"alpha"};
    final subC = {"beta"};

    final payloadA = Uint8List.fromList(utf8.encode("hello-alpha"));
    final payloadB = Uint8List.fromList(utf8.encode("hello-beta"));

    final sigA = await signMessage(
      _messageSignedBytes("alpha", identityA.publicKeyHex, payloadA),
      identityA,
    );
    final sigB = await signMessage(
      _messageSignedBytes("beta", identityB.publicKeyHex, payloadB),
      identityB,
    );

    await laneA.send(
      "ws",
      _messageToBytes("alpha", identityA.publicKeyHex, payloadA, sigA),
    );
    await laneB.send(
      "ws",
      _messageToBytes("beta", identityB.publicKeyHex, payloadB, sigB),
    );

    final gotA = <String>{};
    final gotB = <String>{};
    final gotC = <String>{};

    final deadline = DateTime.now().add(const Duration(seconds: 10));
    while (DateTime.now().isBefore(deadline)) {
      final msgA = await laneA.recv();
      if (msgA != null) {
        final parsed = _parseMessage(Uint8List.fromList(msgA.bytes));
        final ok = await verifyMessage(
          _messageSignedBytes(parsed.channel, parsed.from, parsed.payload),
          parsed.signature,
          hexDecode(parsed.from),
        );
        if (ok && subA.contains(parsed.channel)) {
          gotA.add(parsed.channel);
        }
      }
      final msgB = await laneB.recv();
      if (msgB != null) {
        final parsed = _parseMessage(Uint8List.fromList(msgB.bytes));
        final ok = await verifyMessage(
          _messageSignedBytes(parsed.channel, parsed.from, parsed.payload),
          parsed.signature,
          hexDecode(parsed.from),
        );
        if (ok && subB.contains(parsed.channel)) {
          gotB.add(parsed.channel);
        }
      }
      final msgC = await laneC.recv();
      if (msgC != null) {
        final parsed = _parseMessage(Uint8List.fromList(msgC.bytes));
        final ok = await verifyMessage(
          _messageSignedBytes(parsed.channel, parsed.from, parsed.payload),
          parsed.signature,
          hexDecode(parsed.from),
        );
        if (ok && subC.contains(parsed.channel)) {
          gotC.add(parsed.channel);
        }
      }
    if (gotA.contains("beta") &&
          gotB.contains("alpha") &&
          gotC.contains("beta")) {
        break;
      }
      await Future<void>.delayed(const Duration(milliseconds: 50));
    }

    await wsServer.close(force: true);

    expect(gotA.contains("beta"), isTrue, reason: "A should receive beta");
    expect(gotB.contains("alpha"), isTrue, reason: "B should receive alpha");
    expect(gotC.contains("beta"), isTrue, reason: "C should receive beta");
  }, timeout: const Timeout(Duration(seconds: 30)));

  test("p2p multilane swarm (quic relay + identities)", () async {
    final root = await _repoRoot();
    final relay = await _startQuicRelay(root);
    final ready = await _waitQuicReady(relay);
    final endpoint = "quic://${ready.addr}";

    final identityA = await generateIdentity();
    final identityB = await generateIdentity();
    final identityC = await generateIdentity();

    final laneA = MultiLane(
      lanes: [
        QuicLane(
          endpoint: endpoint,
          peerId: "quic-a",
          trustedPeerCertHex: ready.certHex,
        ),
      ],
      sendMode: MultiLaneSendMode.broadcast,
    );
    final laneB = MultiLane(
      lanes: [
        QuicLane(
          endpoint: endpoint,
          peerId: "quic-b",
          trustedPeerCertHex: ready.certHex,
        ),
      ],
      sendMode: MultiLaneSendMode.broadcast,
    );
    final laneC = MultiLane(
      lanes: [
        QuicLane(
          endpoint: endpoint,
          peerId: "quic-c",
          trustedPeerCertHex: ready.certHex,
        ),
      ],
      sendMode: MultiLaneSendMode.broadcast,
    );

    final joinPayload = Uint8List.fromList(utf8.encode("join"));
    final joinSigA = await signMessage(
      _messageSignedBytes("join", identityA.publicKeyHex, joinPayload),
      identityA,
    );
    final joinSigB = await signMessage(
      _messageSignedBytes("join", identityB.publicKeyHex, joinPayload),
      identityB,
    );
    final joinSigC = await signMessage(
      _messageSignedBytes("join", identityC.publicKeyHex, joinPayload),
      identityC,
    );
    await laneA.send(
      ready.addr,
      _messageToBytes("join", identityA.publicKeyHex, joinPayload, joinSigA),
    );
    await laneB.send(
      ready.addr,
      _messageToBytes("join", identityB.publicKeyHex, joinPayload, joinSigB),
    );
    await laneC.send(
      ready.addr,
      _messageToBytes("join", identityC.publicKeyHex, joinPayload, joinSigC),
    );
    await Future<void>.delayed(const Duration(milliseconds: 250));

    final payloadA = Uint8List.fromList(utf8.encode("q-alpha"));
    final payloadB = Uint8List.fromList(utf8.encode("q-beta"));
    final sigA = await signMessage(
      _messageSignedBytes("alpha", identityA.publicKeyHex, payloadA),
      identityA,
    );
    final sigB = await signMessage(
      _messageSignedBytes("beta", identityB.publicKeyHex, payloadB),
      identityB,
    );

    await laneA.send(
      ready.addr,
      _messageToBytes("alpha", identityA.publicKeyHex, payloadA, sigA),
    );
    await laneB.send(
      ready.addr,
      _messageToBytes("beta", identityB.publicKeyHex, payloadB, sigB),
    );

    final gotA = <String>{};
    final gotB = <String>{};
    final gotC = <String>{};
    final deadline = DateTime.now().add(const Duration(seconds: 12));
    while (DateTime.now().isBefore(deadline)) {
      final msgA = await laneA.recv();
      if (msgA != null) {
        final parsed = _parseMessage(Uint8List.fromList(msgA.bytes));
        final ok = await verifyMessage(
          _messageSignedBytes(parsed.channel, parsed.from, parsed.payload),
          parsed.signature,
          hexDecode(parsed.from),
        );
        stdout.writeln("quic A recv ${parsed.channel} ok=$ok");
        if (ok) gotA.add(parsed.channel);
      }
      final msgB = await laneB.recv();
      if (msgB != null) {
        final parsed = _parseMessage(Uint8List.fromList(msgB.bytes));
        final ok = await verifyMessage(
          _messageSignedBytes(parsed.channel, parsed.from, parsed.payload),
          parsed.signature,
          hexDecode(parsed.from),
        );
        stdout.writeln("quic B recv ${parsed.channel} ok=$ok");
        if (ok) gotB.add(parsed.channel);
      }
      final msgC = await laneC.recv();
      if (msgC != null) {
        final parsed = _parseMessage(Uint8List.fromList(msgC.bytes));
        final ok = await verifyMessage(
          _messageSignedBytes(parsed.channel, parsed.from, parsed.payload),
          parsed.signature,
          hexDecode(parsed.from),
        );
        stdout.writeln("quic C recv ${parsed.channel} ok=$ok");
        if (ok) gotC.add(parsed.channel);
      }
      if (gotA.contains("beta") && gotB.contains("alpha") && gotC.contains("beta")) {
        break;
      }
      await Future<void>.delayed(const Duration(milliseconds: 50));
    }

    ready.process.kill();
    await ready.sub.cancel();
    await ready.errSub.cancel();

    expect(gotA.contains("beta"), isTrue, reason: "A should receive beta");
    expect(gotB.contains("alpha"), isTrue, reason: "B should receive alpha");
    expect(gotC.contains("beta"), isTrue, reason: "C should receive beta");
  }, timeout: const Timeout(Duration(seconds: 40)));

  // BLE and Tor harnesses require Flutter runtime and platform channels.
}
