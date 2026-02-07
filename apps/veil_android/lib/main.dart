import 'dart:io';

import 'package:flutter/material.dart';
import 'package:sqflite/sqflite.dart';
import 'package:veil_sdk/veil_sdk.dart';

void main() {
  runApp(const VeilAndroidApp());
}

class VeilAndroidApp extends StatelessWidget {
  const VeilAndroidApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'VEIL Android',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFFFB7C31),
          brightness: Brightness.dark,
        ),
        useMaterial3: true,
      ),
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
  final _events = <String>[];
  final _wsController = TextEditingController(text: 'ws://127.0.0.1:9001');
  final _peerController = TextEditingController(text: 'android-client');
  final _tagController = TextEditingController(text: '');

  VeilClient? _client;
  WebSocketLane? _lane;
  LocalRelay? _relay;
  bool _useLocalRelay = true;
  bool _relayReady = false;
  bool _connected = false;

  @override
  void initState() {
    super.initState();
    _startLocalRelay();
  }

  @override
  void dispose() {
    _client?.stop();
    _relay?.stop();
    _wsController.dispose();
    _peerController.dispose();
    _tagController.dispose();
    super.dispose();
  }

  Future<void> _startLocalRelay() async {
    final relay = LocalRelay();
    await relay.start();
    setState(() {
      _relay = relay;
      _relayReady = true;
      if (_useLocalRelay) {
        _wsController.text = relay.url;
      }
    });
  }

  Future<void> _connect() async {
    _client?.stop();
    final db = await openDatabase('veil_android_cache.db');
    final store = SqfliteShardCacheStore(db: db);
    await store.init();

    final lane = WebSocketLane(
      url: Uri.parse(_wsController.text.trim()),
      peerId: _peerController.text.trim(),
    );
    final client = VeilClient(
      fastLane: lane,
      bridge: _bridge,
      cacheStore: store,
      hooks: VeilClientHooks(
        onShardMeta: (peer, meta) {
          _pushEvent('Shard from $peer tag=${meta.tagHex}');
        },
        onPayload: (root, payload) {
          _pushEvent('Payload $root (${payload.length} bytes)');
        },
      ),
      options: VeilClientOptions(
        plugins: [
          AutoFetchPlugin(
            resolveTagForRoot: (_, __) => _tagController.text.trim(),
          ),
          ThreadContextPlugin(
            resolveTagForRoot: (_, __) => _tagController.text.trim(),
          ),
        ],
      ),
    );

    final tag = _tagController.text.trim();
    if (tag.isNotEmpty) {
      client.subscribe(tag);
    }
    client.start();

    setState(() {
      _lane = lane;
      _client = client;
      _connected = true;
      _events.insert(0, 'Connected to ${_wsController.text.trim()}');
    });
  }

  void _disconnect() {
    _client?.stop();
    setState(() {
      _connected = false;
      _events.insert(0, 'Disconnected');
    });
  }

  void _toggleLocalRelay(bool value) {
    setState(() {
      _useLocalRelay = value;
      if (_useLocalRelay && _relay != null) {
        _wsController.text = _relay!.url;
      }
    });
  }

  void _updateSubscription() {
    final client = _client;
    if (client == null) return;
    for (final sub in client.subscriptions()) {
      client.unsubscribe(sub);
    }
    final tag = _tagController.text.trim();
    if (tag.isNotEmpty) {
      client.subscribe(tag);
      _pushEvent('Subscribed to $tag');
    }
  }

  void _pushEvent(String message) {
    setState(() {
      _events.insert(0, message);
      if (_events.length > 120) {
        _events.removeRange(120, _events.length);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      appBar: AppBar(
        title: const Text('VEIL Android'),
        actions: [
          Padding(
            padding: const EdgeInsets.only(right: 16),
            child: Chip(
              label: Text(_connected ? 'ONLINE' : 'OFFLINE'),
              backgroundColor: _connected
                  ? const Color(0xFF134E4A)
                  : const Color(0xFF3F2F0B),
            ),
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          ClipRRect(
            borderRadius: BorderRadius.circular(16),
            child: Image.asset(
              'assets/veil_header.png',
              height: 140,
              fit: BoxFit.cover,
            ),
          ),
          const SizedBox(height: 16),
          Row(
            children: [
              Image.asset('assets/veil_logo.png', width: 48, height: 48),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('VEIL Android', style: theme.textTheme.headlineSmall),
                    const SizedBox(height: 4),
                    Text(
                      'Self-contained relay + client runtime',
                      style: theme.textTheme.bodyMedium?.copyWith(
                        color: Colors.white70,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
          const SizedBox(height: 16),
          _Panel(
            title: 'Connection',
            child: Column(
              children: [
                SwitchListTile(
                  value: _useLocalRelay,
                  onChanged: _relayReady ? _toggleLocalRelay : null,
                  title: const Text('Use local relay'),
                  subtitle: Text(
                    _relayReady
                        ? 'Local relay at ${_relay?.url}'
                        : 'Starting local relay...',
                  ),
                ),
                _InputField(label: 'WebSocket URL', controller: _wsController),
                _InputField(label: 'Peer ID', controller: _peerController),
                _InputField(
                  label: 'Subscribe Tag (hex)',
                  controller: _tagController,
                ),
                const SizedBox(height: 12),
                Row(
                  children: [
                    Expanded(
                      child: ElevatedButton(
                        onPressed: _connected ? null : _connect,
                        child: const Text('Start'),
                      ),
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: OutlinedButton(
                        onPressed: _connected ? _disconnect : null,
                        child: const Text('Stop'),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                OutlinedButton(
                  onPressed: _connected ? _updateSubscription : null,
                  child: const Text('Update Subscription'),
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          _Panel(
            title: 'Status',
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  _connected ? 'Lane ready: ${_lane?.peerId}' : 'Not connected',
                  style: theme.textTheme.bodyLarge,
                ),
                const SizedBox(height: 8),
                Text(
                  _relayReady
                      ? 'Local relay: ${_relay?.url}'
                      : 'Local relay: starting...',
                  style: theme.textTheme.bodyMedium?.copyWith(
                    color: Colors.white70,
                  ),
                ),
                const SizedBox(height: 8),
                Text(
                  'Events recorded: ${_events.length}',
                  style: theme.textTheme.bodyMedium?.copyWith(
                    color: Colors.white70,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          _Panel(
            title: 'Activity',
            child: Column(
              children: _events.isEmpty
                  ? [
                      const Padding(
                        padding: EdgeInsets.all(12),
                        child: Text('No events yet.'),
                      ),
                    ]
                  : _events
                        .take(40)
                        .map(
                          (event) => ListTile(
                            title: Text(event),
                            dense: true,
                            leading: const Icon(Icons.bolt, size: 18),
                          ),
                        )
                        .toList(),
            ),
          ),
        ],
      ),
    );
  }
}

class _Panel extends StatelessWidget {
  final String title;
  final Widget child;

  const _Panel({required this.title, required this.child});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: const Color(0xFF0B1220),
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: const Color(0xFF1F2937)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            title.toUpperCase(),
            style: Theme.of(context).textTheme.labelLarge?.copyWith(
              letterSpacing: 2,
              color: Colors.white70,
            ),
          ),
          const SizedBox(height: 12),
          child,
        ],
      ),
    );
  }
}

class _InputField extends StatelessWidget {
  final String label;
  final TextEditingController controller;

  const _InputField({required this.label, required this.controller});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: TextField(
        controller: controller,
        decoration: InputDecoration(
          labelText: label,
          filled: true,
          fillColor: const Color(0xFF101827),
          border: OutlineInputBorder(borderRadius: BorderRadius.circular(12)),
        ),
      ),
    );
  }
}

class LocalRelay {
  HttpServer? _server;
  final _sockets = <WebSocket>[];
  String url = '';

  Future<void> start() async {
    _server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    url = 'ws://127.0.0.1:${_server!.port}';
    _server!.listen((request) async {
      if (!WebSocketTransformer.isUpgradeRequest(request)) {
        request.response
          ..statusCode = HttpStatus.badRequest
          ..write('Expected WebSocket upgrade');
        await request.response.close();
        return;
      }
      final socket = await WebSocketTransformer.upgrade(request);
      _sockets.add(socket);
      socket.listen(
        (data) {
          for (final peer in _sockets) {
            if (peer != socket && peer.readyState == WebSocket.open) {
              peer.add(data);
            }
          }
        },
        onDone: () => _sockets.remove(socket),
        onError: (_) => _sockets.remove(socket),
      );
    });
  }

  void stop() {
    for (final socket in _sockets) {
      socket.close();
    }
    _sockets.clear();
    _server?.close();
  }
}
