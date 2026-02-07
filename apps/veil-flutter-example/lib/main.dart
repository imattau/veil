import 'package:flutter/material.dart';
import 'package:sqflite/sqflite.dart';
import 'package:veil_sdk/veil_sdk.dart';

void main() {
  runApp(const VeilExampleApp());
}

class VeilExampleApp extends StatelessWidget {
  const VeilExampleApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'VEIL Flutter Example',
      theme: ThemeData.dark(useMaterial3: true),
      home: const VeilHomePage(),
    );
  }
}

class VeilHomePage extends StatefulWidget {
  const VeilHomePage({super.key});

  @override
  State<VeilHomePage> createState() => _VeilHomePageState();
}

class _VeilHomePageState extends State<VeilHomePage> {
  final _bridge = const VeilBridge();
  final _lane = WebSocketLane(
    url: Uri.parse('ws://127.0.0.1:9001'),
    peerId: 'flutter-client',
  );

  VeilClient? _client;
  final List<String> _events = [];

  @override
  void initState() {
    super.initState();
    _initClient();
  }

  Future<void> _initClient() async {
    final db = await openDatabase('veil_cache.db');
    final store = SqfliteShardCacheStore(db: db);
    await store.init();

    final client = VeilClient(
      fastLane: _lane,
      bridge: _bridge,
      cacheStore: store,
      hooks: VeilClientHooks(
        onShardMeta: (peer, meta) {
          setState(
            () => _events.insert(0, 'Shard from $peer tag=${meta.tagHex}'),
          );
        },
        onPayload: (root, payload) {
          setState(
            () => _events.insert(
              0,
              'Payload for $root (${payload.length} bytes)',
            ),
          );
        },
      ),
      options: VeilClientOptions(
        plugins: [
          AutoFetchPlugin(resolveTagForRoot: (root, _) => ''),
          ThreadContextPlugin(resolveTagForRoot: (root, _) => ''),
        ],
      ),
    );
    client.subscribe('');
    client.start();
    setState(() => _client = client);
  }

  @override
  void dispose() {
    _client?.stop();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('VEIL Flutter Example')),
      body: ListView.builder(
        padding: const EdgeInsets.all(16),
        itemCount: _events.length,
        itemBuilder: (context, index) {
          return Card(
            child: Padding(
              padding: const EdgeInsets.all(12),
              child: Text(_events[index]),
            ),
          );
        },
      ),
    );
  }
}
