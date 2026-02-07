import 'dart:async';
import 'dart:io';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_blurhash/flutter_blurhash.dart';
import 'package:flutter_reactive_ble/flutter_reactive_ble.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:sqflite/sqflite.dart';
import 'package:veil_sdk/veil_sdk.dart';

void main() {
  runApp(const VeilAndroidApp());
}

class VeilAndroidApp extends StatelessWidget {
  const VeilAndroidApp({super.key});

  @override
  Widget build(BuildContext context) {
    final colorScheme = ColorScheme.fromSeed(
      seedColor: const Color(0xFFFB7C31),
      brightness: Brightness.dark,
      primary: const Color(0xFFFFA45C),
      secondary: const Color(0xFF3DD6A8),
      surface: const Color(0xFF0B111B),
      surfaceContainerHighest: const Color(0xFF101827),
    );
    return MaterialApp(
      title: 'VEIL Android',
      theme: ThemeData(
        colorScheme: colorScheme,
        useMaterial3: true,
        scaffoldBackgroundColor: const Color(0xFF0B0F17),
        appBarTheme: const AppBarTheme(
          backgroundColor: Color(0xFF0B0F17),
          elevation: 0,
        ),
        textTheme: ThemeData.dark().textTheme.copyWith(
          headlineMedium: const TextStyle(
            fontSize: 28,
            fontWeight: FontWeight.w700,
            letterSpacing: -0.4,
          ),
          titleLarge: const TextStyle(
            fontSize: 22,
            fontWeight: FontWeight.w600,
          ),
          titleMedium: const TextStyle(
            fontSize: 18,
            fontWeight: FontWeight.w600,
          ),
          bodyMedium: const TextStyle(
            fontSize: 14,
            height: 1.35,
          ),
        ),
        inputDecorationTheme: InputDecorationTheme(
          filled: true,
          fillColor: const Color(0xFF111827),
          border: OutlineInputBorder(
            borderRadius: BorderRadius.circular(14),
            borderSide: const BorderSide(color: Color(0xFF1F2937)),
          ),
          enabledBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(14),
            borderSide: const BorderSide(color: Color(0xFF1F2937)),
          ),
          focusedBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(14),
            borderSide: BorderSide(color: colorScheme.primary, width: 1.2),
          ),
          labelStyle: const TextStyle(color: Color(0xFF9CA3AF)),
          hintStyle: const TextStyle(color: Color(0xFF6B7280)),
        ),
        elevatedButtonTheme: ElevatedButtonThemeData(
          style: ElevatedButton.styleFrom(
            backgroundColor: colorScheme.primary,
            foregroundColor: const Color(0xFF0B0F17),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(14),
            ),
            padding: const EdgeInsets.symmetric(vertical: 14, horizontal: 18),
          ),
        ),
        outlinedButtonTheme: OutlinedButtonThemeData(
          style: OutlinedButton.styleFrom(
            side: const BorderSide(color: Color(0xFF1F2937)),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(14),
            ),
            padding: const EdgeInsets.symmetric(vertical: 14, horizontal: 18),
          ),
        ),
        chipTheme: ChipThemeData(
          backgroundColor: const Color(0xFF0F172A),
          side: const BorderSide(color: Color(0xFF1F2937)),
          labelStyle: const TextStyle(color: Colors.white),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(999),
          ),
        ),
        navigationBarTheme: NavigationBarThemeData(
          backgroundColor: const Color(0xFF0B0F17),
          indicatorColor: const Color(0xFF1F2937),
          labelTextStyle: WidgetStateProperty.all(
            const TextStyle(fontWeight: FontWeight.w600),
          ),
        ),
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
      isScrollControlled: true,
      builder: (context) => ComposeSheet(
        channelLabel: _controller.channelLabel.isNotEmpty
            ? _controller.channelLabel
            : _controller.tagHex.isNotEmpty
                ? 'Custom tag'
                : 'General',
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
        requireSignedPublic: _controller.requireSignedPublic,
        onToggleRequireSigned: (value) {
          setState(() => _controller.setRequireSignedPublic(value));
        },
        clockSkewSeconds: _controller.clockSkewSeconds,
        onClockSkewChanged: (value) {
          setState(() => _controller.setClockSkewSeconds(value));
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
  SharedPreferences? _prefs;
  final List<FeedEntry> _feed = [];
  final List<String> _events = [];
  final List<String> _suggestedFeeds = [
    'Public Square',
    'Local Builders',
    'Civic Updates',
    'Open Media',
  ];
  final Set<String> _trustedFeeds = {};

  String displayName = '';
  String recoveryPhrase = '';
  String namespaceChoice = 'Public Square';
  String peerId = 'android-client';
  String wsUrl = 'ws://127.0.0.1:9001';
  String tagHex = '';
  String channelLabel = '';
  static const String _channelPublisherKey =
      '0000000000000000000000000000000000000000000000000000000000000000';
  final List<String> _extraTags = [];
  final List<String> _forwardPeers = [];
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
  bool _requireSignedPublic = true;
  int _clockSkewSeconds = 0;
  bool _epochOverlapActive = false;
  Timer? _epochTimer;
  Timer? _healthTimer;
  int _epochRemainingSeconds = 0;

  bool get onboardingComplete => displayName.isNotEmpty;
  bool get relayReady => _relayReady;
  bool get connected => _connected;
  String get relayUrl => _relay?.url ?? '';
  bool get ghostMode => _ghostMode;
  bool get bleEnabled => _bleEnabled;
  bool get useLocalRelay => _useLocalRelay;
  int get epochRemainingSeconds => _epochRemainingSeconds;
  bool get epochOverlapActive => _epochOverlapActive;
  bool get requireSignedPublic => _requireSignedPublic;
  int get clockSkewSeconds => _clockSkewSeconds;
  LaneHealthSnapshot? get fastLaneHealth =>
      _client?.fastLane.healthSnapshot() ?? _lane?.healthSnapshot();
  LaneHealthSnapshot? get fallbackLaneHealth =>
      _client?.fallbackLane?.healthSnapshot() ?? _fallbackLane?.healthSnapshot();
  List<FeedEntry> get feed => List.unmodifiable(_feed);
  List<String> get events => List.unmodifiable(_events);
  List<String> get suggestedFeeds => List.unmodifiable(_suggestedFeeds);
  Set<String> get trustedFeeds => Set.unmodifiable(_trustedFeeds);
  List<String> get extraTags => List.unmodifiable(_extraTags);
  List<String> get forwardPeers => List.unmodifiable(_forwardPeers);

  void init() {
    () async {
      _prefs = await SharedPreferences.getInstance();
      await _loadPrefs();
      _startLocalRelay();
      _startEpochTimer();
      connect();
    }();
  }

  void dispose() {
    _client?.stop();
    _relay?.stop();
    _epochTimer?.cancel();
    _healthTimer?.cancel();
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
    _persistPrefs();
    notifyListeners();
  }

  void setDisplayName(String value) {
    displayName = value.trim();
    _persistPrefs();
    notifyListeners();
  }

  void setNamespaceChoice(String value) {
    namespaceChoice = value;
    _persistPrefs();
    notifyListeners();
  }

  void toggleTrustedFeed(String feed) {
    if (_trustedFeeds.contains(feed)) {
      _trustedFeeds.remove(feed);
    } else {
      _trustedFeeds.add(feed);
    }
    _events.insert(0, 'Trusted feeds: ${_trustedFeeds.length}');
    _persistPrefs();
    notifyListeners();
  }

  Future<void> _loadPrefs() async {
    final prefs = _prefs;
    if (prefs == null) return;
    displayName = prefs.getString('displayName') ?? displayName;
    namespaceChoice = prefs.getString('namespaceChoice') ?? namespaceChoice;
    peerId = prefs.getString('peerId') ?? peerId;
    wsUrl = prefs.getString('wsUrl') ?? wsUrl;
    tagHex = prefs.getString('tagHex') ?? tagHex;
    channelLabel = prefs.getString('channelLabel') ?? channelLabel;
    bleDeviceId = prefs.getString('bleDeviceId') ?? bleDeviceId;
    bleServiceUuid = prefs.getString('bleServiceUuid') ?? bleServiceUuid;
    bleCharacteristicUuid =
        prefs.getString('bleCharacteristicUuid') ?? bleCharacteristicUuid;
    _useLocalRelay = prefs.getBool('useLocalRelay') ?? _useLocalRelay;
    _ghostMode = prefs.getBool('ghostMode') ?? _ghostMode;
    _bleEnabled = prefs.getBool('bleEnabled') ?? _bleEnabled;
    _requireSignedPublic =
        prefs.getBool('requireSignedPublic') ?? _requireSignedPublic;
    _clockSkewSeconds =
        prefs.getInt('clockSkewSeconds') ?? _clockSkewSeconds;
    _extraTags
      ..clear()
      ..addAll(prefs.getStringList('extraTags') ?? const []);
    _forwardPeers
      ..clear()
      ..addAll(prefs.getStringList('forwardPeers') ?? const []);
    _trustedFeeds
      ..clear()
      ..addAll(prefs.getStringList('trustedFeeds') ?? const []);
    notifyListeners();
  }

  Future<void> _persistPrefs() async {
    final prefs = _prefs;
    if (prefs == null) return;
    await prefs.setString('displayName', displayName);
    await prefs.setString('namespaceChoice', namespaceChoice);
    await prefs.setString('peerId', peerId);
    await prefs.setString('wsUrl', wsUrl);
    await prefs.setString('tagHex', tagHex);
    await prefs.setString('channelLabel', channelLabel);
    await prefs.setString('bleDeviceId', bleDeviceId);
    await prefs.setString('bleServiceUuid', bleServiceUuid);
    await prefs.setString('bleCharacteristicUuid', bleCharacteristicUuid);
    await prefs.setBool('useLocalRelay', _useLocalRelay);
    await prefs.setBool('ghostMode', _ghostMode);
    await prefs.setBool('bleEnabled', _bleEnabled);
    await prefs.setBool('requireSignedPublic', _requireSignedPublic);
    await prefs.setInt('clockSkewSeconds', _clockSkewSeconds);
    await prefs.setStringList('extraTags', _extraTags);
    await prefs.setStringList('forwardPeers', _forwardPeers);
    await prefs.setStringList('trustedFeeds', _trustedFeeds.toList());
  }

  void setWsUrl(String value) {
    wsUrl = value.trim();
    _persistPrefs();
    notifyListeners();
  }

  void setPeerId(String value) {
    peerId = value.trim();
    _persistPrefs();
    notifyListeners();
  }

  void setTagHex(String value) {
    tagHex = value.trim();
    _persistPrefs();
    notifyListeners();
  }

  String _normalizeChannelLabel(String label) {
    final trimmed = label.trim();
    if (trimmed.isEmpty) return '';
    return trimmed.toLowerCase().replaceAll(RegExp(r'\\s+'), '-');
  }

  int _deriveChannelNamespace(int baseNamespace, String channelId) {
    final normalized = _normalizeChannelLabel(channelId);
    if (normalized.isEmpty) {
      return baseNamespace;
    }
    var hash = 2166136261;
    for (final unit in normalized.codeUnits) {
      hash ^= unit;
      hash = (hash * 16777619) & 0xffffffff;
    }
    final hash16 = hash & 0xffff;
    return (baseNamespace + hash16) & 0xffff;
  }

  Future<String> _deriveTagHexForLabel(String label) async {
    final normalized = _normalizeChannelLabel(label);
    if (normalized.isEmpty) return '';
    final namespace = _deriveChannelNamespace(1, normalized);
    return _bridge.deriveFeedTagHex(_channelPublisherKey, namespace);
  }

  void setChannelLabel(String value) {
    channelLabel = value;
    () async {
      final derived = await _deriveTagHexForLabel(value);
      if (derived.isNotEmpty) {
        tagHex = derived;
        _persistPrefs();
        notifyListeners();
      }
    }();
    _persistPrefs();
    notifyListeners();
  }

  Future<void> addSubscription(String value) async {
    final cleaned = value.trim();
    if (cleaned.isEmpty) return;
    final derived = cleaned.startsWith('tag:')
        ? cleaned.substring(4)
        : await _deriveTagHexForLabel(cleaned);
    if (derived.isEmpty) return;
    if (!_extraTags.contains(derived) && derived != tagHex) {
      _extraTags.add(derived);
      _client?.subscribe(derived);
      _events.insert(0, 'Joined channel $cleaned');
      _persistPrefs();
      notifyListeners();
    }
  }

  void removeSubscription(String value) {
    _extraTags.remove(value);
    _client?.unsubscribe(value);
    _events.insert(0, 'Unsubscribed from $value');
    _persistPrefs();
    notifyListeners();
  }

  void handleScanValue(String value) {
    final raw = value.trim();
    if (raw.isEmpty) return;
    final lower = raw.toLowerCase();
    if (lower.startsWith('peer:')) {
      addForwardPeer(raw.substring(5));
      return;
    }
    if (lower.startsWith('tag:')) {
      addSubscription(raw.substring(4));
      return;
    }
    if (lower.startsWith('ws://') ||
        lower.startsWith('wss://') ||
        lower.startsWith('quic://')) {
      addForwardPeer(raw);
      return;
    }
    final hex = lower.replaceAll(RegExp(r'[^0-9a-f]'), '');
    if (hex.length == 64) {
      addSubscription(hex);
      return;
    }
    _events.insert(0, 'Scan not recognized: $raw');
    notifyListeners();
  }

  void addForwardPeer(String value) {
    final cleaned = value.trim();
    if (cleaned.isEmpty) return;
    if (!_forwardPeers.contains(cleaned)) {
      _forwardPeers.add(cleaned);
      _client?.setForwardPeers(_forwardPeers);
      _events.insert(0, 'Added peer $cleaned');
      _persistPrefs();
      notifyListeners();
    }
  }

  void removeForwardPeer(String value) {
    _forwardPeers.remove(value);
    _client?.setForwardPeers(_forwardPeers);
    _events.insert(0, 'Removed peer $value');
    _persistPrefs();
    notifyListeners();
  }

  void setGhostMode(bool value) {
    _ghostMode = value;
    _events.insert(0, value ? 'Ghost mode enabled' : 'Ghost mode disabled');
    _persistPrefs();
    notifyListeners();
  }

  void setBleEnabled(bool value) {
    _bleEnabled = value;
    _events.insert(0, value ? 'BLE lane enabled' : 'BLE lane disabled');
    _persistPrefs();
    notifyListeners();
  }

  void setRequireSignedPublic(bool value) {
    _requireSignedPublic = value;
    _events.insert(
      0,
      value ? 'Signed public namespaces required' : 'Signed namespace optional',
    );
    _persistPrefs();
    notifyListeners();
  }

  void setClockSkewSeconds(String value) {
    final parsed = int.tryParse(value.trim()) ?? 0;
    _clockSkewSeconds = parsed.clamp(-3600, 3600);
    _events.insert(0, 'Clock skew set to $_clockSkewSeconds sec');
    _startEpochTimer();
    _persistPrefs();
    notifyListeners();
  }

  void setBleDeviceId(String value) {
    bleDeviceId = value.trim();
    _persistPrefs();
    notifyListeners();
  }

  void setBleServiceUuid(String value) {
    bleServiceUuid = value.trim();
    _persistPrefs();
    notifyListeners();
  }

  void setBleCharacteristicUuid(String value) {
    bleCharacteristicUuid = value.trim();
    _persistPrefs();
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
        onReconstructable: (root, have, need) {
          if (need > have && have >= need - 1) {
            _events.insert(0, 'Requesting missing shard for $root');
          }
          _markRequesting(root, need, have);
          _notify();
        },
        onPayload: (root, payload) {
          _events.insert(0, 'Payload $root (${payload.length} bytes)');
          _markReconstructed(root);
        },
      ),
      options: VeilClientOptions(
        requiredSignedNamespaces: _requireSignedPublic ? {1} : {},
        plugins: [
          AutoFetchPlugin(resolveTagForRoot: (_, __) => tagHex),
          ThreadContextPlugin(resolveTagForRoot: (_, __) => tagHex),
        ],
      ),
    );

    if (tagHex.isNotEmpty) {
      client.subscribe(tagHex);
    }
    for (final tag in _extraTags) {
      client.subscribe(tag);
    }
    if (_forwardPeers.isNotEmpty) {
      client.setForwardPeers(_forwardPeers);
    }
    client.start();

    _client = client;
    _lane = wsLane;
    _fallbackLane = fallbackLane;
    _connected = true;
    _events.insert(0, 'Connected via ${wsLane.url}');
    _startHealthTimer();
    _notify();
  }

  void disconnect() {
    _client?.stop();
    _connected = false;
    _events.insert(0, 'Disconnected');
    _healthTimer?.cancel();
    _notify();
  }

  Future<void> updateSubscription(String value) async {
    channelLabel = value.trim();
    tagHex = await _deriveTagHexForLabel(channelLabel);
    final client = _client;
    if (client == null) return;
    for (final sub in client.subscriptions()) {
      client.unsubscribe(sub);
    }
    if (tagHex.isNotEmpty) {
      client.subscribe(tagHex);
    }
    _events.insert(0, 'Joined channel $channelLabel');
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
          blurHash: 'LEHV6nWB2yk8pyo0adR*.7kCMdnj',
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
        entry.requestingMissing = false;
      }
    }
    _notify();
  }

  void _markRequesting(String root, int need, int have) {
    for (final entry in _feed) {
      if (entry.id == root) {
        entry.shardsTotal = max(entry.shardsTotal, need);
        entry.shardsHave = max(entry.shardsHave, have);
        entry.requestingMissing = have >= need - 1 && !entry.reconstructed;
      }
    }
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
      final nowSeconds =
          (DateTime.now().millisecondsSinceEpoch ~/ 1000) +
          _clockSkewSeconds;
      final offset = nowSeconds % epochSeconds;
      _epochRemainingSeconds = epochSeconds - offset;
      _epochOverlapActive = _epochRemainingSeconds <= 3600;
      notifyListeners();
    }

    update();
    _epochTimer?.cancel();
    _epochTimer = Timer.periodic(const Duration(seconds: 1), (_) => update());
  }

  void _startHealthTimer() {
    _healthTimer?.cancel();
    _healthTimer = Timer.periodic(const Duration(seconds: 2), (_) {
      if (_connected) {
        notifyListeners();
      }
    });
  }
}

class FeedEntry {
  final String id;
  final String author;
  final String body;
  final String? blurHash;
  bool reconstructed;
  bool isGhost;
  final DateTime timestamp;
  int shardsHave;
  int shardsTotal;
  bool requestingMissing;

  FeedEntry({
    required this.id,
    required this.author,
    required this.body,
    this.blurHash,
    required this.reconstructed,
    required this.timestamp,
    this.isGhost = false,
    this.shardsHave = 0,
    this.shardsTotal = 16,
    this.requestingMissing = false,
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
        decoration: const BoxDecoration(
          gradient: LinearGradient(
            colors: [Color(0xFF0B0E14), Color(0xFF111827)],
            begin: Alignment.topLeft,
            end: Alignment.bottomRight,
          ),
        ),
        child: SafeArea(
          child: LayoutBuilder(
            builder: (context, constraints) {
              return SingleChildScrollView(
                padding: const EdgeInsets.all(24),
                child: ConstrainedBox(
                  constraints: BoxConstraints(minHeight: constraints.maxHeight),
                  child: IntrinsicHeight(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const SizedBox(height: 12),
                        Image.asset(
                          'assets/veil_header.png',
                          height: 120,
                          fit: BoxFit.cover,
                        ),
                        const SizedBox(height: 20),
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
                        const SizedBox(height: 20),
                        _InputField(
                          label: 'Display name',
                          controller: _nameController,
                        ),
                        const SizedBox(height: 8),
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
                        const SizedBox(height: 12),
                        Text(
                          'Trust starter feeds',
                          style: Theme.of(
                            context,
                          ).textTheme.titleMedium?.copyWith(
                            color: Colors.white70,
                          ),
                        ),
                        const SizedBox(height: 8),
                        ...controller.suggestedFeeds.map(
                          (feed) => CheckboxListTile(
                            value: controller.trustedFeeds.contains(feed),
                            onChanged: (_) => controller.toggleTrustedFeed(feed),
                            dense: true,
                            contentPadding: EdgeInsets.zero,
                            title: Text(feed),
                            subtitle: const Text('Bootstrap recommendation'),
                          ),
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
                ),
              );
            },
          ),
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
        gradient: const LinearGradient(
          colors: [Color(0xFF0B1220), Color(0xFF121826)],
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
        ),
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: const Color(0xFF1F2937)),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withOpacity(0.25),
            blurRadius: 16,
            offset: const Offset(0, 10),
          ),
        ],
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
            gradient: const LinearGradient(
              colors: [Color(0xFF0F172A), Color(0xFF0B1220)],
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
            ),
            borderRadius: BorderRadius.circular(16),
            border: Border.all(color: const Color(0xFF1F2937)),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withOpacity(0.22),
                blurRadius: 12,
                offset: const Offset(0, 6),
              ),
            ],
          ),
          child: Stack(
            children: [
              _BlurPlaceholder(blurHash: entry.blurHash),
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
          gradient: const LinearGradient(
            colors: [Color(0xFF0B1220), Color(0xFF0F172A)],
            begin: Alignment.topLeft,
            end: Alignment.bottomRight,
          ),
          borderRadius: BorderRadius.circular(16),
          border: Border.all(color: const Color(0xFF1F2937)),
          boxShadow: [
            BoxShadow(
              color: Colors.black.withOpacity(0.25),
              blurRadius: 14,
              offset: const Offset(0, 8),
            ),
          ],
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
            if (entry.blurHash != null) ...[
              const SizedBox(height: 12),
              ClipRRect(
                borderRadius: BorderRadius.circular(12),
                child: SizedBox(
                  height: 180,
                  width: double.infinity,
                  child: BlurHash(
                    hash: entry.blurHash!,
                    duration: const Duration(milliseconds: 300),
                  ),
                ),
              ),
            ],
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
              if (entry.requestingMissing) ...[
                const SizedBox(height: 8),
                Row(
                  children: [
                    const Icon(Icons.radar, size: 16, color: Color(0xFF38BDF8)),
                    const SizedBox(width: 6),
                    Text(
                      'Requesting missing shard',
                      style: Theme.of(
                        context,
                      ).textTheme.bodySmall?.copyWith(color: Colors.white60),
                    ),
                  ],
                ),
              ],
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
              if (controller.epochOverlapActive) ...[
                const SizedBox(height: 8),
                Row(
                  children: [
                    const Icon(
                      Icons.swap_horiz,
                      size: 16,
                      color: Color(0xFF38BDF8),
                    ),
                    const SizedBox(width: 6),
                    Text(
                      'Overlap window active',
                      style: Theme.of(
                        context,
                      ).textTheme.bodySmall?.copyWith(color: Colors.white60),
                    ),
                  ],
                ),
              ],
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
    _tagController = TextEditingController(
      text: c.channelLabel.isNotEmpty ? c.channelLabel : c.tagHex,
    );
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
          title: 'Network Status',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(controller.connected ? 'Connected' : 'Offline'),
              const SizedBox(height: 8),
              Text(
                controller.useLocalRelay
                    ? 'Using internal relay'
                    : 'Using external relay',
              ),
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
              _LaneHealthTile(
                title: 'WebSocket Lane',
                icon: controller.connected ? Icons.wifi : Icons.wifi_off,
                label: 'WS',
                enabled: controller.connected,
                snapshot: controller.fastLaneHealth,
              ),
              _LaneHealthTile(
                title: 'Bluetooth Lane',
                icon: Icons.bluetooth,
                label: 'BLE',
                enabled: controller.bleEnabled,
                snapshot: controller.fallbackLaneHealth,
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
                      ? 'Internal relay is ready'
                      : 'Starting internal relay...',
                ),
              ),
              if (!controller.useLocalRelay) ...[
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
              ],
              _InputField(
                label: 'Channel',
                controller: _tagController,
                onChanged: controller.setChannelLabel,
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
    final peerController = TextEditingController();
    final tagController = TextEditingController();
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
                    subtitle: Text(
                      controller.trustedFeeds.contains(feed)
                          ? 'Trusted by you'
                          : 'Bootstrap recommendation',
                    ),
                    trailing: controller.trustedFeeds.contains(feed)
                        ? const Icon(Icons.verified, color: Color(0xFF34D399))
                        : const Icon(Icons.add_circle_outline),
                    onTap: () => controller.toggleTrustedFeed(feed),
                  ),
                )
                .toList(),
          ),
        ),
        const SizedBox(height: 16),
        _Panel(
          title: 'Add Peer',
          child: Column(
            children: [
              _InputField(
                label: 'Peer address (ws:// or quic://)',
                controller: peerController,
                onChanged: (_) {},
              ),
              Row(
                children: [
                  Expanded(
                    child: ElevatedButton(
                      onPressed: () =>
                          controller.addForwardPeer(peerController.text),
                      child: const Text('Add Peer'),
                    ),
                  ),
                  const SizedBox(width: 12),
                  OutlinedButton.icon(
                    onPressed: () => _openScanner(
                      context,
                      onResult: controller.handleScanValue,
                    ),
                    icon: const Icon(Icons.qr_code_scanner),
                    label: const Text('Scan'),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              ...controller.forwardPeers.map(
                (peer) => ListTile(
                  dense: true,
                  leading: const Icon(Icons.router, size: 18),
                  title: Text(peer),
                  trailing: IconButton(
                    icon: const Icon(Icons.close),
                    onPressed: () => controller.removeForwardPeer(peer),
                  ),
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        _Panel(
          title: 'Add Subscription',
          child: Column(
            children: [
              _InputField(
                label: 'Channel name (or tag:HEX)',
                controller: tagController,
                onChanged: (_) {},
              ),
              Row(
                children: [
                  Expanded(
                    child: ElevatedButton(
                      onPressed: () =>
                          controller.addSubscription(tagController.text),
                      child: const Text('Subscribe'),
                    ),
                  ),
                  const SizedBox(width: 12),
                  OutlinedButton.icon(
                    onPressed: () => _openScanner(
                      context,
                      onResult: controller.handleScanValue,
                    ),
                    icon: const Icon(Icons.qr_code_scanner),
                    label: const Text('Scan'),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              ...controller.extraTags.map(
                (tag) => ListTile(
                  dense: true,
                  leading: const Icon(Icons.tag, size: 18),
                  title: Text(tag),
                  trailing: IconButton(
                    icon: const Icon(Icons.close),
                    onPressed: () => controller.removeSubscription(tag),
                  ),
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

Future<void> _openScanner(
  BuildContext context, {
  required void Function(String value) onResult,
}) async {
  await showModalBottomSheet(
    context: context,
    isScrollControlled: true,
    backgroundColor: const Color(0xFF0B0F17),
    builder: (context) => _QrScannerSheet(onResult: onResult),
  );
}

class _QrScannerSheet extends StatefulWidget {
  final void Function(String value) onResult;

  const _QrScannerSheet({required this.onResult});

  @override
  State<_QrScannerSheet> createState() => _QrScannerSheetState();
}

class _QrScannerSheetState extends State<_QrScannerSheet> {
  bool _handled = false;

  void _handle(String value) {
    if (_handled) return;
    _handled = true;
    widget.onResult(value);
    Navigator.of(context).pop();
  }

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: SizedBox(
        height: MediaQuery.of(context).size.height * 0.6,
        child: Column(
          children: [
            const SizedBox(height: 12),
            Container(
              height: 4,
              width: 40,
              decoration: BoxDecoration(
                color: Colors.white24,
                borderRadius: BorderRadius.circular(999),
              ),
            ),
            const SizedBox(height: 12),
            Text(
              'Scan QR',
              style: Theme.of(context).textTheme.titleLarge,
            ),
            const SizedBox(height: 12),
            Expanded(
              child: ClipRRect(
                borderRadius: BorderRadius.circular(16),
                child: MobileScanner(
                  onDetect: (capture) {
                    for (final barcode in capture.barcodes) {
                      final raw = barcode.rawValue;
                      if (raw != null && raw.trim().isNotEmpty) {
                        _handle(raw.trim());
                        break;
                      }
                    }
                  },
                ),
              ),
            ),
            const SizedBox(height: 12),
            TextButton(
              onPressed: () => Navigator.of(context).pop(),
              child: const Text('Cancel'),
            ),
            const SizedBox(height: 12),
          ],
        ),
      ),
    );
  }
}

class _LaneHealthTile extends StatelessWidget {
  final String title;
  final IconData icon;
  final String label;
  final bool enabled;
  final LaneHealthSnapshot? snapshot;

  const _LaneHealthTile({
    required this.title,
    required this.icon,
    required this.label,
    required this.enabled,
    required this.snapshot,
  });

  @override
  Widget build(BuildContext context) {
    final health = snapshot;
    final sendOk = health?.outboundSendOk ?? 0;
    final sendErr = health?.outboundSendErr ?? 0;
    final inbound = health?.inboundReceived ?? 0;
    final dropped = health?.inboundDropped ?? 0;
    final queued = health?.outboundQueued ?? 0;
    final reconnects = health?.reconnectAttempts ?? 0;
    final sendTotal = sendOk + sendErr;
    final okRatio = sendTotal == 0 ? 1.0 : sendOk / sendTotal;

    return ListTile(
      dense: true,
      leading: Icon(icon, size: 18),
      title: Text(title),
      subtitle: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(enabled ? 'Healthy' : 'Idle'),
          const SizedBox(height: 4),
          Text(
            'ok ${(okRatio * 100).toStringAsFixed(0)}% · '
            'queued $queued · in $inbound · drop $dropped · retry $reconnects',
            style: Theme.of(context).textTheme.bodySmall?.copyWith(
              color: Colors.white60,
            ),
          ),
        ],
      ),
      trailing: Text(label),
    );
  }
}

class ComposeSheet extends StatefulWidget {
  final void Function(String text) onPublish;
  final String channelLabel;

  const ComposeSheet({
    super.key,
    required this.onPublish,
    required this.channelLabel,
  });

  @override
  State<ComposeSheet> createState() => _ComposeSheetState();
}

class _ComposeSheetState extends State<ComposeSheet> {
  final _controller = TextEditingController();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: SingleChildScrollView(
        padding: EdgeInsets.only(
          left: 16,
          right: 16,
          top: 16,
          bottom: 16 + MediaQuery.of(context).viewInsets.bottom,
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Compose', style: Theme.of(context).textTheme.titleLarge),
            const SizedBox(height: 6),
            Row(
              children: [
                const Icon(Icons.tag, size: 16, color: Color(0xFF60A5FA)),
                const SizedBox(width: 6),
                Text(
                  'Posting to ${widget.channelLabel}',
                  style: Theme.of(context).textTheme.bodySmall?.copyWith(
                    color: Colors.white70,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _controller,
              maxLines: 5,
              autofocus: true,
              decoration: const InputDecoration(
                hintText: 'Share an update...',
                filled: true,
                fillColor: Color(0xFF101827),
              ),
            ),
            const SizedBox(height: 12),
            ElevatedButton(
              onPressed: () => widget.onPublish(_controller.text),
              child: const Text('Publish'),
            ),
          ],
        ),
      ),
    );
  }
}

class SettingsSheet extends StatelessWidget {
  final bool showProtocolDetails;
  final ValueChanged<bool> onToggleDetails;
  final bool ghostMode;
  final ValueChanged<bool> onToggleGhostMode;
  final bool requireSignedPublic;
  final ValueChanged<bool> onToggleRequireSigned;
  final int clockSkewSeconds;
  final ValueChanged<String> onClockSkewChanged;

  const SettingsSheet({
    super.key,
    required this.showProtocolDetails,
    required this.onToggleDetails,
    required this.ghostMode,
    required this.onToggleGhostMode,
    required this.requireSignedPublic,
    required this.onToggleRequireSigned,
    required this.clockSkewSeconds,
    required this.onClockSkewChanged,
  });

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: ListView(
        padding: const EdgeInsets.all(16),
        shrinkWrap: true,
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
          SwitchListTile(
            value: requireSignedPublic,
            onChanged: onToggleRequireSigned,
            title: const Text('Require signed public posts'),
            subtitle: const Text('Drop unsigned objects in public namespaces.'),
          ),
          TextFormField(
            keyboardType: TextInputType.number,
            decoration: const InputDecoration(
              labelText: 'Clock skew seconds',
              helperText: 'Adjust for device clock drift (max +/-3600).',
            ),
            initialValue: clockSkewSeconds.toString(),
            onFieldSubmitted: onClockSkewChanged,
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
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        gradient: LinearGradient(
          colors: [
            scheme.surfaceContainerHighest.withOpacity(0.9),
            const Color(0xFF0B1220),
          ],
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
        ),
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: const Color(0xFF1F2937)),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withOpacity(0.25),
            blurRadius: 16,
            offset: const Offset(0, 8),
          ),
        ],
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
  final String? blurHash;

  const _BlurPlaceholder({this.blurHash});

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
    if (widget.blurHash != null) {
      return ClipRRect(
        borderRadius: BorderRadius.circular(16),
        child: BlurHash(
          hash: widget.blurHash!,
          duration: const Duration(milliseconds: 300),
        ),
      );
    }
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
