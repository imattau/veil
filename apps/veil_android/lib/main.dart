import 'dart:io';
import 'dart:math';

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
      home: const RootShell(),
    );
  }
}

class RootShell extends StatefulWidget {
  const RootShell({super.key});

  @override
  State<RootShell> createState() => _RootShellState();
}

class _RootShellState extends State<RootShell> {
  final _controller = VeilAppController();
  int _tabIndex = 0;
  bool _showProtocolDetails = false;

  @override
  void initState() {
    super.initState();
    _controller.init();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _openCompose() {
    showModalBottomSheet(
      context: context,
      showDragHandle: true,
      builder: (context) => ComposeSheet(
        onPublish: (text) {
          _controller.publishLocalPost(text);
          Navigator.of(context).pop();
        },
      ),
    );
  }

  void _openSettings() {
    showModalBottomSheet(
      context: context,
      showDragHandle: true,
      builder: (context) => SettingsSheet(
        showProtocolDetails: _showProtocolDetails,
        onToggleDetails: (value) {
          setState(() => _showProtocolDetails = value);
        },
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _controller,
      builder: (context, _) {
        if (!_controller.onboardingComplete) {
          return OnboardingScreen(
            controller: _controller,
            onComplete: () => setState(() {}),
          );
        }
        return Scaffold(
          appBar: AppBar(
            title: const Text('VEIL'),
            actions: [
              IconButton(
                icon: const Icon(Icons.settings),
                onPressed: _openSettings,
              ),
            ],
          ),
          floatingActionButton: _tabIndex == 3
              ? null
              : FloatingActionButton(
                  onPressed: _openCompose,
                  child: const Icon(Icons.edit),
                ),
          bottomNavigationBar: NavigationBar(
            selectedIndex: _tabIndex,
            onDestinationSelected: (value) => setState(() => _tabIndex = value),
            destinations: const [
              NavigationDestination(
                icon: Icon(Icons.home_outlined),
                label: 'Home',
              ),
              NavigationDestination(
                icon: Icon(Icons.lock_outline),
                label: 'Vault',
              ),
              NavigationDestination(icon: Icon(Icons.public), label: 'Network'),
              NavigationDestination(
                icon: Icon(Icons.explore_outlined),
                label: 'Discover',
              ),
            ],
          ),
          body: IndexedStack(
            index: _tabIndex,
            children: [
              HomeFeed(
                controller: _controller,
                showProtocolDetails: _showProtocolDetails,
              ),
              VaultView(controller: _controller),
              NetworkView(controller: _controller),
              DiscoveryView(controller: _controller),
            ],
          ),
        );
      },
    );
  }
}

class VeilAppController extends ChangeNotifier {
  final _bridge = const VeilBridge();
  final List<FeedEntry> _feed = [];
  final List<String> _events = [];
  final List<String> _suggestedFeeds = [
    'Public Square',
    'Local Builders',
    'Civic Updates',
    'Open Media',
  ];

  String displayName = '';
  String recoveryPhrase = '';
  String namespaceChoice = 'Public Square';
  String peerId = 'android-client';
  String tagHex = '';

  VeilClient? _client;
  WebSocketLane? _lane;
  LocalRelay? _relay;
  bool _relayReady = false;
  bool _useLocalRelay = true;
  bool _connected = false;

  bool get onboardingComplete => displayName.isNotEmpty;
  bool get relayReady => _relayReady;
  bool get connected => _connected;
  String get relayUrl => _relay?.url ?? '';
  List<FeedEntry> get feed => List.unmodifiable(_feed);
  List<String> get events => List.unmodifiable(_events);
  List<String> get suggestedFeeds => List.unmodifiable(_suggestedFeeds);

  void init() {
    _startLocalRelay();
  }

  void dispose() {
    _client?.stop();
    _relay?.stop();
    super.dispose();
  }

  Future<void> _startLocalRelay() async {
    final relay = LocalRelay();
    await relay.start();
    _relay = relay;
    _relayReady = true;
    notifyListeners();
  }

  void setUseLocalRelay(bool value) {
    _useLocalRelay = value;
    notifyListeners();
  }

  void setDisplayName(String value) {
    displayName = value.trim();
    notifyListeners();
  }

  void setNamespaceChoice(String value) {
    namespaceChoice = value;
    notifyListeners();
  }

  void generateIdentity() {
    final words = [
      'ember',
      'veil',
      'lumen',
      'atlas',
      'cinder',
      'fjord',
      'mosaic',
      'echo',
      'prism',
      'ripple',
      'sable',
      'nova',
    ];
    final rand = Random();
    recoveryPhrase = List.generate(
      8,
      (_) => words[rand.nextInt(words.length)],
    ).join(' ');
  }

  Future<void> connect() async {
    _client?.stop();
    final db = await openDatabase('veil_android_cache.db');
    final store = SqfliteShardCacheStore(db: db);
    await store.init();

    final url = _useLocalRelay && _relay != null
        ? _relay!.url
        : _wsController.text.trim();
    final lane = WebSocketLane(
      url: Uri.parse(url.isEmpty ? 'ws://127.0.0.1:9001' : url),
      peerId: peerId,
    );

    final client = VeilClient(
      fastLane: lane,
      bridge: _bridge,
      cacheStore: store,
      hooks: VeilClientHooks(
        onShardMeta: (peer, meta) {
          _events.insert(0, 'Shard from $peer tag=${meta.tagHex}');
          _notify();
        },
        onPayload: (root, payload) {
          _events.insert(0, 'Payload $root (${payload.length} bytes)');
          _markReconstructed(root);
        },
      ),
      options: VeilClientOptions(
        plugins: [
          AutoFetchPlugin(resolveTagForRoot: (_, __) => tagHex),
          ThreadContextPlugin(resolveTagForRoot: (_, __) => tagHex),
        ],
      ),
    );

    tagHex = _tagController.text.trim();
    if (tagHex.isNotEmpty) {
      client.subscribe(tagHex);
    }
    client.start();

    _client = client;
    _lane = lane;
    _connected = true;
    _events.insert(0, 'Connected via ${lane.url}');
    _notify();
  }

  void disconnect() {
    _client?.stop();
    _connected = false;
    _events.insert(0, 'Disconnected');
    _notify();
  }

  void updateSubscription(String value) {
    tagHex = value.trim();
    final client = _client;
    if (client == null) return;
    for (final sub in client.subscriptions()) {
      client.unsubscribe(sub);
    }
    if (tagHex.isNotEmpty) {
      client.subscribe(tagHex);
    }
    _events.insert(0, 'Subscribed to $tagHex');
    _notify();
  }

  void publishLocalPost(String text) {
    if (text.trim().isEmpty) return;
    final entry = FeedEntry(
      id: DateTime.now().millisecondsSinceEpoch.toString(),
      author: displayName,
      body: text.trim(),
      reconstructed: true,
      timestamp: DateTime.now(),
    );
    _feed.insert(0, entry);
    _events.insert(0, 'Local post created');
    _notify();
  }

  void addSkeletons() {
    if (_feed.isNotEmpty) return;
    for (var i = 0; i < 3; i += 1) {
      _feed.add(
        FeedEntry(
          id: 'ghost-$i',
          author: '...',
          body: '...',
          reconstructed: false,
          isGhost: true,
          timestamp: DateTime.now(),
        ),
      );
    }
  }

  void _markReconstructed(String root) {
    for (final entry in _feed) {
      if (entry.id == root && !entry.reconstructed) {
        entry.reconstructed = true;
        entry.isGhost = false;
      }
    }
    _notify();
  }

  void _notify() {
    if (_feed.isEmpty) {
      addSkeletons();
    }
    notifyListeners();
  }
}

class FeedEntry {
  final String id;
  final String author;
  final String body;
  bool reconstructed;
  bool isGhost;
  final DateTime timestamp;

  FeedEntry({
    required this.id,
    required this.author,
    required this.body,
    required this.reconstructed,
    required this.timestamp,
    this.isGhost = false,
  });
}

class OnboardingScreen extends StatefulWidget {
  final VeilAppController controller;
  final VoidCallback onComplete;

  const OnboardingScreen({
    super.key,
    required this.controller,
    required this.onComplete,
  });

  @override
  State<OnboardingScreen> createState() => _OnboardingScreenState();
}

class _OnboardingScreenState extends State<OnboardingScreen> {
  final _nameController = TextEditingController();
  String _selected = 'Public Square';

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    return Scaffold(
      body: Container(
        padding: const EdgeInsets.all(24),
        decoration: const BoxDecoration(
          gradient: LinearGradient(
            colors: [Color(0xFF0B0E14), Color(0xFF111827)],
            begin: Alignment.topLeft,
            end: Alignment.bottomRight,
          ),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const SizedBox(height: 40),
            Image.asset(
              'assets/veil_header.png',
              height: 140,
              fit: BoxFit.cover,
            ),
            const SizedBox(height: 24),
            Text(
              'Welcome to VEIL',
              style: Theme.of(context).textTheme.headlineMedium,
            ),
            const SizedBox(height: 8),
            Text(
              'Your identity is created automatically. Choose a display name and a starting space.',
              style: Theme.of(
                context,
              ).textTheme.bodyMedium?.copyWith(color: Colors.white70),
            ),
            const SizedBox(height: 24),
            _InputField(label: 'Display name', controller: _nameController),
            const SizedBox(height: 12),
            DropdownButtonFormField<String>(
              value: _selected,
              decoration: const InputDecoration(
                labelText: 'Start in',
                filled: true,
                fillColor: Color(0xFF101827),
              ),
              items: const [
                DropdownMenuItem(
                  value: 'Public Square',
                  child: Text('Public Square'),
                ),
                DropdownMenuItem(
                  value: 'Private Circles',
                  child: Text('Private Circles'),
                ),
              ],
              onChanged: (value) {
                if (value != null) {
                  setState(() => _selected = value);
                }
              },
            ),
            const Spacer(),
            ElevatedButton(
              onPressed: () {
                controller.setDisplayName(_nameController.text);
                controller.setNamespaceChoice(_selected);
                controller.generateIdentity();
                widget.onComplete();
              },
              style: ElevatedButton.styleFrom(
                minimumSize: const Size.fromHeight(52),
              ),
              child: const Text('Continue'),
            ),
            const SizedBox(height: 12),
            Text(
              'Recovery phrase stored locally. You can export it later.',
              style: Theme.of(
                context,
              ).textTheme.bodySmall?.copyWith(color: Colors.white60),
            ),
          ],
        ),
      ),
    );
  }
}

class HomeFeed extends StatelessWidget {
  final VeilAppController controller;
  final bool showProtocolDetails;

  const HomeFeed({
    super.key,
    required this.controller,
    required this.showProtocolDetails,
  });

  @override
  Widget build(BuildContext context) {
    final items = controller.feed;
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        HeaderCard(controller: controller),
        const SizedBox(height: 16),
        if (items.isEmpty)
          const Center(child: CircularProgressIndicator())
        else
          ...items.map(
            (entry) => PostCard(
              entry: entry,
              showProtocolDetails: showProtocolDetails,
            ),
          ),
      ],
    );
  }
}

class HeaderCard extends StatelessWidget {
  final VeilAppController controller;

  const HeaderCard({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: const Color(0xFF0B1220),
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: const Color(0xFF1F2937)),
      ),
      child: Row(
        children: [
          Image.asset('assets/veil_logo.png', width: 48, height: 48),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  controller.displayName.isEmpty
                      ? 'Operator'
                      : controller.displayName,
                  style: Theme.of(context).textTheme.titleLarge,
                ),
                const SizedBox(height: 4),
                Text(
                  controller.namespaceChoice,
                  style: Theme.of(
                    context,
                  ).textTheme.bodyMedium?.copyWith(color: Colors.white70),
                ),
              ],
            ),
          ),
          Chip(
            label: Text(controller.connected ? 'LIVE' : 'OFFLINE'),
            backgroundColor: controller.connected
                ? const Color(0xFF134E4A)
                : const Color(0xFF3F2F0B),
          ),
        ],
      ),
    );
  }
}

class PostCard extends StatelessWidget {
  final FeedEntry entry;
  final bool showProtocolDetails;

  const PostCard({
    super.key,
    required this.entry,
    required this.showProtocolDetails,
  });

  @override
  Widget build(BuildContext context) {
    if (entry.isGhost) {
      return Padding(
        padding: const EdgeInsets.only(bottom: 12),
        child: Container(
          height: 120,
          decoration: BoxDecoration(
            color: const Color(0xFF0F172A),
            borderRadius: BorderRadius.circular(16),
            border: Border.all(color: const Color(0xFF1F2937)),
          ),
        ),
      );
    }
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: Container(
        padding: const EdgeInsets.all(16),
        decoration: BoxDecoration(
          color: const Color(0xFF0B1220),
          borderRadius: BorderRadius.circular(16),
          border: Border.all(color: const Color(0xFF1F2937)),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                const CircleAvatar(
                  radius: 18,
                  backgroundColor: Color(0xFF1E293B),
                  child: Icon(Icons.person, size: 18),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    entry.author,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ),
                if (entry.reconstructed)
                  Row(
                    children: const [
                      Icon(Icons.verified, size: 16, color: Color(0xFF34D399)),
                      SizedBox(width: 4),
                      Text('Reconstructed'),
                    ],
                  ),
              ],
            ),
            const SizedBox(height: 12),
            Text(entry.body),
            if (showProtocolDetails) ...[
              const SizedBox(height: 12),
              Text(
                'object_root: ${entry.id}',
                style: Theme.of(
                  context,
                ).textTheme.bodySmall?.copyWith(color: Colors.white60),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class VaultView extends StatelessWidget {
  final VeilAppController controller;

  const VaultView({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(16),
      children: const [
        _Panel(
          title: 'Private Vault',
          child: Text(
            'Encrypted conversations will appear here. Rotating rendezvous tags keep private circles private.',
          ),
        ),
      ],
    );
  }
}

class NetworkView extends StatelessWidget {
  final VeilAppController controller;

  const NetworkView({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        _Panel(
          title: 'Network Health',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(controller.connected ? 'Connected' : 'Offline'),
              const SizedBox(height: 8),
              Text('Local relay: ${controller.relayUrl}'),
            ],
          ),
        ),
        const SizedBox(height: 16),
        _Panel(
          title: 'Recent Activity',
          child: Column(
            children: controller.events
                .take(8)
                .map(
                  (event) => ListTile(
                    dense: true,
                    leading: const Icon(Icons.waves, size: 18),
                    title: Text(event),
                  ),
                )
                .toList(),
          ),
        ),
      ],
    );
  }
}

class DiscoveryView extends StatelessWidget {
  final VeilAppController controller;

  const DiscoveryView({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        _Panel(
          title: 'Suggested Feeds',
          child: Column(
            children: controller.suggestedFeeds
                .map(
                  (feed) => ListTile(
                    dense: true,
                    title: Text(feed),
                    subtitle: const Text('Bootstrap recommendation'),
                    trailing: const Icon(Icons.add_circle_outline),
                  ),
                )
                .toList(),
          ),
        ),
      ],
    );
  }
}

class ComposeSheet extends StatelessWidget {
  final void Function(String text) onPublish;

  const ComposeSheet({super.key, required this.onPublish});

  @override
  Widget build(BuildContext context) {
    final controller = TextEditingController();
    return Padding(
      padding: const EdgeInsets.all(16),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('Compose', style: Theme.of(context).textTheme.titleLarge),
          const SizedBox(height: 12),
          TextField(
            controller: controller,
            maxLines: 5,
            decoration: const InputDecoration(
              hintText: 'Share an update...',
              filled: true,
              fillColor: Color(0xFF101827),
            ),
          ),
          const SizedBox(height: 12),
          ElevatedButton(
            onPressed: () => onPublish(controller.text),
            child: const Text('Publish'),
          ),
        ],
      ),
    );
  }
}

class SettingsSheet extends StatelessWidget {
  final bool showProtocolDetails;
  final ValueChanged<bool> onToggleDetails;

  const SettingsSheet({
    super.key,
    required this.showProtocolDetails,
    required this.onToggleDetails,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(16),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          SwitchListTile(
            value: showProtocolDetails,
            onChanged: onToggleDetails,
            title: const Text('Show protocol details'),
            subtitle: const Text('Reveal object_root and lane metadata.'),
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
