import 'dart:io';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_reactive_ble/flutter_reactive_ble.dart';
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
        ghostMode: _controller.ghostMode,
        onToggleGhostMode: (value) {
          setState(() => _controller.setGhostMode(value));
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
  final _ble = FlutterReactiveBle();
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
  String wsUrl = 'ws://127.0.0.1:9001';
  String tagHex = '';
  String bleDeviceId = '';
  String bleServiceUuid = '6e400001-b5a3-f393-e0a9-e50e24dcca9e';
  String bleCharacteristicUuid = '6e400003-b5a3-f393-e0a9-e50e24dcca9e';

  VeilClient? _client;
  WebSocketLane? _lane;
  VeilLane? _fallbackLane;
  LocalRelay? _relay;
  bool _relayReady = false;
  bool _useLocalRelay = true;
  bool _connected = false;
  bool _ghostMode = false;
  bool _bleEnabled = false;
  Timer? _epochTimer;
  int _epochRemainingSeconds = 0;

  bool get onboardingComplete => displayName.isNotEmpty;
  bool get relayReady => _relayReady;
  bool get connected => _connected;
  String get relayUrl => _relay?.url ?? '';
  bool get ghostMode => _ghostMode;
  bool get bleEnabled => _bleEnabled;
  bool get useLocalRelay => _useLocalRelay;
  int get epochRemainingSeconds => _epochRemainingSeconds;
  List<FeedEntry> get feed => List.unmodifiable(_feed);
  List<String> get events => List.unmodifiable(_events);
  List<String> get suggestedFeeds => List.unmodifiable(_suggestedFeeds);

  void init() {
    _startLocalRelay();
    _startEpochTimer();
  }

  void dispose() {
    _client?.stop();
    _relay?.stop();
    _epochTimer?.cancel();
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

  void setWsUrl(String value) {
    wsUrl = value.trim();
    notifyListeners();
  }

  void setPeerId(String value) {
    peerId = value.trim();
    notifyListeners();
  }

  void setTagHex(String value) {
    tagHex = value.trim();
    notifyListeners();
  }

  void setGhostMode(bool value) {
    _ghostMode = value;
    _events.insert(0, value ? 'Ghost mode enabled' : 'Ghost mode disabled');
    notifyListeners();
  }

  void setBleEnabled(bool value) {
    _bleEnabled = value;
    _events.insert(0, value ? 'BLE lane enabled' : 'BLE lane disabled');
    notifyListeners();
  }

  void setBleDeviceId(String value) {
    bleDeviceId = value.trim();
    notifyListeners();
  }

  void setBleServiceUuid(String value) {
    bleServiceUuid = value.trim();
    notifyListeners();
  }

  void setBleCharacteristicUuid(String value) {
    bleCharacteristicUuid = value.trim();
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

    final url = _useLocalRelay && _relay != null ? _relay!.url : wsUrl;
    final wsLane = WebSocketLane(
      url: Uri.parse(url.isEmpty ? 'ws://127.0.0.1:9001' : url),
      peerId: peerId,
    );

    BleLane? bleLane;
    if (_bleEnabled && bleDeviceId.isNotEmpty) {
      try {
        bleLane = BleLane(
          ble: _ble,
          serviceUuid: Uuid.parse(bleServiceUuid),
          characteristicUuid: Uuid.parse(bleCharacteristicUuid),
          deviceId: bleDeviceId,
        );
      } catch (err) {
        _events.insert(0, 'BLE init failed: $err');
      }
    }

    final fastLane = _ghostMode && bleLane != null ? bleLane : wsLane;
    final fallbackLane = _ghostMode ? wsLane : bleLane;

    final client = VeilClient(
      fastLane: fastLane,
      fallbackLane: fallbackLane,
      bridge: _bridge,
      cacheStore: store,
      hooks: VeilClientHooks(
        onShardMeta: (peer, meta) {
          _events.insert(0, 'Shard from $peer tag=${meta.tagHex}');
          _updateProgressFromShard(meta);
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

    if (tagHex.isNotEmpty) {
      client.subscribe(tagHex);
    }
    client.start();

    _client = client;
    _lane = wsLane;
    _fallbackLane = fallbackLane;
    _connected = true;
    _events.insert(0, 'Connected via ${wsLane.url}');
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
          shardsHave: 0,
          shardsTotal: 16,
        ),
      );
    }
  }

  void _markReconstructed(String root) {
    for (final entry in _feed) {
      if (entry.id == root && !entry.reconstructed) {
        entry.reconstructed = true;
        entry.isGhost = false;
        entry.shardsHave = entry.shardsTotal;
      }
    }
    _notify();
  }

  void _updateProgressFromShard(ShardMeta meta) {
    if (_feed.isEmpty) {
      addSkeletons();
    }
    final ghost = _feed.firstWhere(
      (entry) => entry.isGhost,
      orElse: () => _feed.isNotEmpty ? _feed.first : FeedEntry.empty(),
    );
    if (ghost.id == 'empty') {
      return;
    }
    ghost.shardsTotal = max(ghost.shardsTotal, meta.n);
    ghost.shardsHave = min(ghost.shardsTotal, ghost.shardsHave + 1);
  }

  void _notify() {
    if (_feed.isEmpty) {
      addSkeletons();
    }
    notifyListeners();
  }

  void _startEpochTimer() {
    void update() {
      const epochSeconds = 86400;
      final nowSeconds = DateTime.now().millisecondsSinceEpoch ~/ 1000;
      final offset = nowSeconds % epochSeconds;
      _epochRemainingSeconds = epochSeconds - offset;
      notifyListeners();
    }

    update();
    _epochTimer?.cancel();
    _epochTimer = Timer.periodic(const Duration(seconds: 1), (_) => update());
  }
}

class FeedEntry {
  final String id;
  final String author;
  final String body;
  bool reconstructed;
  bool isGhost;
  final DateTime timestamp;
  int shardsHave;
  int shardsTotal;

  FeedEntry({
    required this.id,
    required this.author,
    required this.body,
    required this.reconstructed,
    required this.timestamp,
    this.isGhost = false,
    this.shardsHave = 0,
    this.shardsTotal = 16,
  });

  factory FeedEntry.empty() => FeedEntry(
    id: 'empty',
    author: '',
    body: '',
    reconstructed: false,
    timestamp: DateTime.now(),
  );
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
          height: 160,
          decoration: BoxDecoration(
            color: const Color(0xFF0F172A),
            borderRadius: BorderRadius.circular(16),
            border: Border.all(color: const Color(0xFF1F2937)),
          ),
          child: Stack(
            children: [
              const _BlurPlaceholder(),
              Align(
                alignment: Alignment.bottomRight,
                child: _ShardProgressRing(
                  have: entry.shardsHave,
                  total: entry.shardsTotal,
                ),
              ),
            ],
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
            if (!entry.reconstructed) ...[
              const SizedBox(height: 12),
              Row(
                children: [
                  _ShardProgressRing(
                    have: entry.shardsHave,
                    total: entry.shardsTotal,
                  ),
                  const SizedBox(width: 8),
                  Text(
                    'Collecting shards',
                    style: Theme.of(
                      context,
                    ).textTheme.bodySmall?.copyWith(color: Colors.white60),
                  ),
                ],
              ),
            ],
            if (showProtocolDetails) ...[
              const SizedBox(height: 12),
              Text(
                'object_root: ${entry.id}',
                style: Theme.of(
                  context,
                ).textTheme.bodySmall?.copyWith(color: Colors.white60),
              ),
              const SizedBox(height: 6),
              Text(
                'signature: ${entry.reconstructed ? 'unknown' : 'pending'}',
                style: Theme.of(
                  context,
                ).textTheme.bodySmall?.copyWith(color: Colors.white60),
              ),
              const SizedBox(height: 6),
              Text(
                'lane: ws',
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
    final remaining = controller.epochRemainingSeconds;
    final hours = (remaining ~/ 3600).toString().padLeft(2, '0');
    final minutes = ((remaining % 3600) ~/ 60).toString().padLeft(2, '0');
    final seconds = (remaining % 60).toString().padLeft(2, '0');
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        _Panel(
          title: 'Private Vault',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Encrypted conversations will appear here. Rotating rendezvous tags keep private circles private.',
              ),
              const SizedBox(height: 12),
              Text(
                'Next rotation in $hours:$minutes:$seconds',
                style: Theme.of(
                  context,
                ).textTheme.bodyMedium?.copyWith(color: Colors.white70),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class NetworkView extends StatefulWidget {
  final VeilAppController controller;

  const NetworkView({super.key, required this.controller});

  @override
  State<NetworkView> createState() => _NetworkViewState();
}

class _NetworkViewState extends State<NetworkView> {
  late final TextEditingController _wsController;
  late final TextEditingController _peerController;
  late final TextEditingController _tagController;
  late final TextEditingController _bleDeviceController;
  late final TextEditingController _bleServiceController;
  late final TextEditingController _bleCharController;

  @override
  void initState() {
    super.initState();
    final c = widget.controller;
    _wsController = TextEditingController(text: c.wsUrl);
    _peerController = TextEditingController(text: c.peerId);
    _tagController = TextEditingController(text: c.tagHex);
    _bleDeviceController = TextEditingController(text: c.bleDeviceId);
    _bleServiceController = TextEditingController(text: c.bleServiceUuid);
    _bleCharController = TextEditingController(text: c.bleCharacteristicUuid);
  }

  @override
  void dispose() {
    _wsController.dispose();
    _peerController.dispose();
    _tagController.dispose();
    _bleDeviceController.dispose();
    _bleServiceController.dispose();
    _bleCharController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
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
              const SizedBox(height: 8),
              Text('Ghost mode: ${controller.ghostMode ? 'On' : 'Off'}'),
            ],
          ),
        ),
        const SizedBox(height: 16),
        _Panel(
          title: 'Lane Status',
          child: Column(
            children: [
              ListTile(
                dense: true,
                leading: Icon(
                  controller.connected ? Icons.wifi : Icons.wifi_off,
                  size: 18,
                ),
                title: const Text('WebSocket Lane'),
                subtitle: Text(controller.connected ? 'Healthy' : 'Idle'),
                trailing: const Text('WS'),
              ),
              ListTile(
                dense: true,
                leading: const Icon(Icons.bluetooth, size: 18),
                title: const Text('Bluetooth Lane'),
                subtitle: Text(controller.bleEnabled ? 'Enabled' : 'Disabled'),
                trailing: const Text('BLE'),
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        _Panel(
          title: 'Connection',
          child: Column(
            children: [
              SwitchListTile(
                value: controller.useLocalRelay,
                onChanged: controller.relayReady
                    ? (value) =>
                          setState(() => controller.setUseLocalRelay(value))
                    : null,
                title: const Text('Use local relay'),
                subtitle: Text(
                  controller.relayReady
                      ? 'Local relay at ${controller.relayUrl}'
                      : 'Starting local relay...',
                ),
              ),
              _InputField(
                label: 'WebSocket URL',
                controller: _wsController,
                onChanged: controller.setWsUrl,
              ),
              _InputField(
                label: 'Peer ID',
                controller: _peerController,
                onChanged: controller.setPeerId,
              ),
              _InputField(
                label: 'Subscribe Tag (hex)',
                controller: _tagController,
                onChanged: controller.setTagHex,
              ),
              const SizedBox(height: 12),
              Row(
                children: [
                  Expanded(
                    child: ElevatedButton(
                      onPressed: controller.connected
                          ? null
                          : controller.connect,
                      child: const Text('Connect'),
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: OutlinedButton(
                      onPressed: controller.connected
                          ? controller.disconnect
                          : null,
                      child: const Text('Disconnect'),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              OutlinedButton(
                onPressed: controller.connected
                    ? () => controller.updateSubscription(_tagController.text)
                    : null,
                child: const Text('Update Subscription'),
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        _Panel(
          title: 'Bluetooth Lane',
          child: Column(
            children: [
              SwitchListTile(
                value: controller.bleEnabled,
                onChanged: (value) =>
                    setState(() => controller.setBleEnabled(value)),
                title: const Text('Enable BLE lane'),
                subtitle: const Text('Requires a paired BLE device id.'),
              ),
              _InputField(
                label: 'BLE Device ID',
                controller: _bleDeviceController,
                onChanged: controller.setBleDeviceId,
              ),
              _InputField(
                label: 'Service UUID',
                controller: _bleServiceController,
                onChanged: controller.setBleServiceUuid,
              ),
              _InputField(
                label: 'Characteristic UUID',
                controller: _bleCharController,
                onChanged: controller.setBleCharacteristicUuid,
              ),
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
  final bool ghostMode;
  final ValueChanged<bool> onToggleGhostMode;

  const SettingsSheet({
    super.key,
    required this.showProtocolDetails,
    required this.onToggleDetails,
    required this.ghostMode,
    required this.onToggleGhostMode,
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
          SwitchListTile(
            value: ghostMode,
            onChanged: onToggleGhostMode,
            title: const Text('Ghost mode'),
            subtitle: const Text('Prefer privacy lanes (preview).'),
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
  final ValueChanged<String>? onChanged;

  const _InputField({
    required this.label,
    required this.controller,
    this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: TextField(
        controller: controller,
        onChanged: onChanged,
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

class _BlurPlaceholder extends StatefulWidget {
  const _BlurPlaceholder();

  @override
  State<_BlurPlaceholder> createState() => _BlurPlaceholderState();
}

class _BlurPlaceholderState extends State<_BlurPlaceholder>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 2),
  )..repeat(reverse: true);

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _controller,
      builder: (context, _) {
        final t = _controller.value;
        return Container(
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(16),
            gradient: LinearGradient(
              colors: [
                Color.lerp(
                  const Color(0xFF111827),
                  const Color(0xFF1E293B),
                  t,
                )!,
                Color.lerp(
                  const Color(0xFF0B1220),
                  const Color(0xFF1F2937),
                  t,
                )!,
              ],
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
            ),
          ),
        );
      },
    );
  }
}

class _ShardProgressRing extends StatelessWidget {
  final int have;
  final int total;

  const _ShardProgressRing({required this.have, required this.total});

  @override
  Widget build(BuildContext context) {
    final progress = total == 0 ? 0.0 : have / total;
    return Container(
      margin: const EdgeInsets.all(12),
      padding: const EdgeInsets.all(6),
      decoration: BoxDecoration(
        color: const Color(0xFF0B1220).withOpacity(0.85),
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: const Color(0xFF1F2937)),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          SizedBox(
            width: 28,
            height: 28,
            child: CircularProgressIndicator(
              value: progress,
              strokeWidth: 3,
              color: const Color(0xFF34D399),
              backgroundColor: const Color(0xFF1F2937),
            ),
          ),
          const SizedBox(width: 8),
          Text(
            '$have/$total',
            style: Theme.of(
              context,
            ).textTheme.labelMedium?.copyWith(color: Colors.white70),
          ),
        ],
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
