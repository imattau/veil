import 'dart:async';
import 'dart:io';
import 'dart:math';
import 'dart:ui';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_blurhash/flutter_blurhash.dart';
import 'package:flutter_reactive_ble/flutter_reactive_ble.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:image_picker/image_picker.dart';
import 'package:crypto/crypto.dart';
import 'package:file_picker/file_picker.dart';
import 'package:mime/mime.dart';
import 'package:video_player/video_player.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:http/http.dart' as http;
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
    final colorSchemeDark = ColorScheme.fromSeed(
      seedColor: const Color(0xFFFB7C31),
      brightness: Brightness.dark,
      primary: const Color(0xFFFFA45C),
      secondary: const Color(0xFF3DD6A8),
      surface: const Color(0xFF0B111B),
      surfaceContainerHighest: const Color(0xFF101827),
    );
    final colorSchemeLight = ColorScheme.fromSeed(
      seedColor: const Color(0xFFFB7C31),
      brightness: Brightness.light,
      primary: const Color(0xFFEF6C00),
      secondary: const Color(0xFF0E9F6E),
      surface: const Color(0xFFF8FAFC),
      surfaceContainerHighest: const Color(0xFFE2E8F0),
    );
    return MaterialApp(
      title: 'VEIL Android',
      themeMode: ThemeMode.system,
      theme: _buildTheme(colorSchemeLight, Brightness.light),
      darkTheme: _buildTheme(colorSchemeDark, Brightness.dark),
      home: const RootShell(),
    );
  }
}

ThemeData _buildTheme(ColorScheme scheme, Brightness brightness) {
  final isDark = brightness == Brightness.dark;
  final background = isDark ? const Color(0xFF0B0F17) : const Color(0xFFF8FAFC);
  final onSurface = isDark ? Colors.white : const Color(0xFF0F172A);
  final border = isDark ? const Color(0xFF1F2937) : const Color(0xFFE2E8F0);
  return ThemeData(
    colorScheme: scheme,
    useMaterial3: true,
    scaffoldBackgroundColor: background,
    appBarTheme: AppBarTheme(
      backgroundColor: background,
      elevation: 0,
      foregroundColor: onSurface,
    ),
    textTheme: (isDark ? ThemeData.dark() : ThemeData.light()).textTheme.copyWith(
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
      fillColor: isDark ? const Color(0xFF111827) : const Color(0xFFF1F5F9),
      border: OutlineInputBorder(
        borderRadius: BorderRadius.circular(14),
        borderSide: BorderSide(color: border),
      ),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(14),
        borderSide: BorderSide(color: border),
      ),
      focusedBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(14),
        borderSide: BorderSide(color: scheme.primary, width: 1.2),
      ),
      labelStyle: TextStyle(color: isDark ? const Color(0xFF9CA3AF) : const Color(0xFF475569)),
      hintStyle: TextStyle(color: isDark ? const Color(0xFF6B7280) : const Color(0xFF64748B)),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: scheme.primary,
        foregroundColor: isDark ? const Color(0xFF0B0F17) : Colors.white,
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
      backgroundColor: isDark ? const Color(0xFF0F172A) : const Color(0xFFE2E8F0),
      side: const BorderSide(color: Color(0xFF1F2937)),
      labelStyle: TextStyle(color: onSurface),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(999),
      ),
    ),
    navigationBarTheme: NavigationBarThemeData(
      backgroundColor: background,
      indicatorColor: isDark ? const Color(0xFF1F2937) : const Color(0xFFE2E8F0),
      labelTextStyle: WidgetStateProperty.all(
        const TextStyle(fontWeight: FontWeight.w600),
      ),
    ),
  );
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
  late final Future<void> _initFuture;

  @override
  void initState() {
    super.initState();
    _initFuture = _controller.init();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _openCompose() {
    Navigator.of(context).push(
      MaterialPageRoute(
        fullscreenDialog: true,
        builder: (context) => ComposeScreen(
          channelLabel: _controller.channelLabel.isNotEmpty
              ? _controller.channelLabel
              : _controller.tagHex.isNotEmpty
                  ? 'Custom tag'
                  : 'General',
          onPublish: (text, attachments) {
            _controller.publishLocalPost(text, attachments: attachments);
            Navigator.of(context).pop();
          },
        ),
      ),
    );
  }

  void _openSettings() {
    showModalBottomSheet(
      context: context,
      showDragHandle: true,
      isScrollControlled: true,
      backgroundColor: Colors.transparent,
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
        maxCacheEntries: _controller.maxCacheEntries,
        maxPublishQueue: _controller.maxPublishQueue,
        onMaxCacheEntriesChanged: (value) {
          setState(() => _controller.setMaxCacheEntries(value));
        },
        onMaxPublishQueueChanged: (value) {
          setState(() => _controller.setMaxPublishQueue(value));
        },
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<void>(
      future: _initFuture,
      builder: (context, snapshot) {
        if (snapshot.connectionState != ConnectionState.done) {
          return const Scaffold(
            body: Center(child: CircularProgressIndicator()),
          );
        }
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
                onDestinationSelected: (value) =>
                    setState(() => _tabIndex = value),
                destinations: const [
                  NavigationDestination(
                    icon: Icon(Icons.home_outlined),
                    label: 'Home',
                  ),
                  NavigationDestination(
                    icon: Icon(Icons.lock_outline),
                    label: 'Vault',
                  ),
                  NavigationDestination(
                    icon: Icon(Icons.public),
                    label: 'Network',
                  ),
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
      },
    );
  }
}

class VeilAppController extends ChangeNotifier {
  final _bridge = const VeilBridge();
  final _ble = FlutterReactiveBle();
  final _secureStorage = const FlutterSecureStorage();
  final _linkPreviewService = LinkPreviewService();
  final _publisher = const VeilPublisher();
  final PublishQueue _publishQueue = PublishQueue();
  bool _publishInFlight = false;
  int _publishAttempts = 0;
  Database? _db;
  int _maxCacheEntries = 50000;
  int _maxPublishQueue = 500;
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
  int get maxCacheEntries => _maxCacheEntries;
  int get maxPublishQueue => _maxPublishQueue;
  String? get wsUrlError {
    if (!wsUrl.startsWith('ws://') && !wsUrl.startsWith('wss://')) {
      return 'Use ws:// or wss://';
    }
    return null;
  }

  String? get channelError {
    if (channelLabel.isEmpty) return null;
    final isHex = RegExp(r'^[0-9a-fA-F]{64}$').hasMatch(channelLabel);
    final isTag = channelLabel.toLowerCase().startsWith('tag:');
    if (isHex || isTag) {
      return null;
    }
    return 'Use a label or tag:HEX';
  }
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

  String get connectionStatus {
    if (!_connected) return 'OFFLINE';
    final fast = fastLaneHealth;
    final fallback = fallbackLaneHealth;
    final fastOk = _laneHealthy(fast);
    final fallbackOk = _laneHealthy(fallback);
    if (fastOk && (fallbackOk || !_bleEnabled)) {
      return 'LIVE';
    }
    return 'DEGRADED';
  }

  bool _laneHealthy(LaneHealthSnapshot? snapshot) {
    if (snapshot == null) return false;
    return snapshot.outboundSendErr < 3 || snapshot.outboundSendOk > 0;
  }

  Future<void> init() async {
    _prefs = await SharedPreferences.getInstance();
    await _loadPrefs();
    await _openDb();
    await _loadPublishQueue();
    _startLocalRelay();
    _startEpochTimer();
    connect();
  }

  void dispose() {
    _client?.stop();
    _relay?.stop();
    _epochTimer?.cancel();
    _healthTimer?.cancel();
    _db?.close();
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
    _maxCacheEntries = prefs.getInt('maxCacheEntries') ?? _maxCacheEntries;
    _maxPublishQueue = prefs.getInt('maxPublishQueue') ?? _maxPublishQueue;
    _publishQueue.updateMaxSize(_maxPublishQueue);
    _extraTags
      ..clear()
      ..addAll(prefs.getStringList('extraTags') ?? const []);
    _forwardPeers
      ..clear()
      ..addAll(prefs.getStringList('forwardPeers') ?? const []);
    _trustedFeeds
      ..clear()
      ..addAll(prefs.getStringList('trustedFeeds') ?? const []);
    recoveryPhrase =
        await _secureStorage.read(key: 'recoveryPhrase') ?? recoveryPhrase;
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
    await prefs.setInt('maxCacheEntries', _maxCacheEntries);
    await prefs.setInt('maxPublishQueue', _maxPublishQueue);
    await prefs.setStringList('extraTags', _extraTags);
    await prefs.setStringList('forwardPeers', _forwardPeers);
    await prefs.setStringList('trustedFeeds', _trustedFeeds.toList());
  }

  Future<void> _openDb() async {
    if (_db != null) return;
    _db = await openDatabase('veil_android_cache.db');
    await _db!.execute(
      'CREATE TABLE IF NOT EXISTS publish_queue (object_root TEXT PRIMARY KEY, bytes BLOB, enqueued_at INTEGER)',
    );
  }

  Future<void> _loadPublishQueue() async {
    final db = _db;
    if (db == null) return;
    final rows = await db.query(
      'publish_queue',
      orderBy: 'enqueued_at ASC',
    );
    for (final row in rows) {
      final root = row['object_root'] as String?;
      final bytes = row['bytes'] as List<int>?;
      if (root == null || bytes == null) continue;
      _publishQueue.enqueue(
        PublishObject(objectRootHex: root, objectBytes: Uint8List.fromList(bytes)),
      );
    }
  }

  Future<void> _persistPublishObject(PublishObject object) async {
    final db = _db;
    if (db == null) return;
    await db.insert(
      'publish_queue',
      {
        'object_root': object.objectRootHex,
        'bytes': object.objectBytes,
        'enqueued_at': DateTime.now().millisecondsSinceEpoch,
      },
      conflictAlgorithm: ConflictAlgorithm.ignore,
    );
  }

  Future<void> _removePublishObject(String objectRootHex) async {
    final db = _db;
    if (db == null) return;
    await db.delete(
      'publish_queue',
      where: 'object_root = ?',
      whereArgs: [objectRootHex],
    );
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

  void setMaxCacheEntries(String value) {
    final parsed = int.tryParse(value.trim()) ?? _maxCacheEntries;
    _maxCacheEntries = parsed.clamp(1000, 200000);
    _events.insert(0, 'Cache limit $_maxCacheEntries entries');
    _persistPrefs();
    notifyListeners();
  }

  void setMaxPublishQueue(String value) {
    final parsed = int.tryParse(value.trim()) ?? _maxPublishQueue;
    _maxPublishQueue = parsed.clamp(50, 5000);
    _publishQueue.updateMaxSize(_maxPublishQueue);
    _events.insert(0, 'Publish queue limit $_maxPublishQueue entries');
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
    final rand = Random.secure();
    recoveryPhrase = List.generate(
      8,
      (_) => words[rand.nextInt(words.length)],
    ).join(' ');
    _secureStorage.write(key: 'recoveryPhrase', value: recoveryPhrase);
  }

  Future<void> connect() async {
    _client?.stop();
    await _openDb();
    final store = SqfliteShardCacheStore(db: _db!);
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
        maxCacheEntries: _maxCacheEntries,
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
    _drainPublishQueue();
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

  void publishLocalPost(String text, {List<Attachment> attachments = const []}) {
    if (text.trim().isEmpty) return;
    final previews = _linkPreviewService.extractCached(text);
    final shardTotal = attachments.isEmpty
        ? 1
        : attachments.fold<int>(0, (sum, a) => sum + a.chunkCount);
    final entry = FeedEntry(
      id: DateTime.now().millisecondsSinceEpoch.toString(),
      author: displayName,
      body: text.trim(),
      attachments: attachments,
      linkPreviews: previews,
      reconstructed: true,
      shardsHave: shardTotal,
      shardsTotal: shardTotal,
      timestamp: DateTime.now(),
    );
    _feed.insert(0, entry);
    _events.insert(0, 'Local post created');
    _notify();
    _enqueuePublishObjects(text, attachments);
    _linkPreviewService.prefetch(text).then((_) {
      final updated = _linkPreviewService.extractCached(text);
      for (final item in _feed) {
        if (item.id == entry.id) {
          item.linkPreviews
            ..clear()
            ..addAll(updated);
        }
      }
      notifyListeners();
    });
  }

  void _enqueuePublishObjects(String text, List<Attachment> attachments) {
    final bytes = attachments.map((a) => a.bytes).toList();
    final mimes = attachments.map((a) => a.mime).toList();
    _publisher.buildPostWithAttachments(text, bytes, mimes).then((batch) {
      _publishQueue.enqueue(batch.rootObject);
      _publishQueue.enqueueAll(batch.relatedObjects);
      _persistPublishObject(batch.rootObject);
      for (final obj in batch.relatedObjects) {
        _persistPublishObject(obj);
      }
      _events.insert(
        0,
        'Queued ${1 + batch.relatedObjects.length} objects for publish',
      );
      notifyListeners();
      _drainPublishQueue();
    });
  }

  Future<void> _drainPublishQueue() async {
    if (_publishInFlight) return;
    final client = _client;
    if (client == null) return;
    _publishInFlight = true;
    try {
      var next = _publishQueue.pop();
      while (next != null) {
        try {
          await client.publishBytes(next.objectBytes);
          _events.insert(0, 'Published object ${next.objectRootHex}');
          await _removePublishObject(next.objectRootHex);
          _publishAttempts = 0;
        } catch (_) {
          _publishQueue.enqueue(next);
          _publishAttempts += 1;
          final delayMs = _backoffForAttempt(_publishAttempts);
          _events.insert(0, 'Publish retry in ${delayMs}ms');
          notifyListeners();
          await Future.delayed(Duration(milliseconds: delayMs));
        }
        next = _publishQueue.pop();
      }
    } finally {
      _publishInFlight = false;
      notifyListeners();
    }
  }

  int _backoffForAttempt(int attempt) {
    final capped = attempt.clamp(1, 6);
    return 300 * (1 << (capped - 1));
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
        entry.fadedIn = false;
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
  final List<Attachment> attachments;
  final List<LinkPreview> linkPreviews;
  bool reconstructed;
  bool isGhost;
  final DateTime timestamp;
  int shardsHave;
  int shardsTotal;
  bool requestingMissing;
  bool fadedIn;

  FeedEntry({
    required this.id,
    required this.author,
    required this.body,
    this.blurHash,
    this.attachments = const [],
    List<LinkPreview> linkPreviews = const [],
    required this.reconstructed,
    required this.timestamp,
    this.isGhost = false,
    this.shardsHave = 0,
    this.shardsTotal = 16,
    this.requestingMissing = false,
    this.fadedIn = false,
  }) : linkPreviews = linkPreviews;

  factory FeedEntry.empty() => FeedEntry(
    id: 'empty',
    author: '',
    body: '',
    reconstructed: false,
    timestamp: DateTime.now(),
  );
}

class Attachment {
  final String name;
  final String mime;
  final Uint8List bytes;
  final String hashHex;
  final int size;
  final bool isImage;
  final bool isVideo;
  final int chunkCount;
  final MediaDescriptorV1? descriptor;

  const Attachment({
    required this.name,
    required this.mime,
    required this.bytes,
    required this.hashHex,
    required this.size,
    required this.isImage,
    required this.isVideo,
    required this.chunkCount,
    this.descriptor,
  });
}

class LinkPreview {
  final Uri url;
  final String title;
  final String? description;
  final String? imageUrl;

  const LinkPreview({
    required this.url,
    required this.title,
    this.description,
    this.imageUrl,
  });
}

class LinkPreviewService {
  final Map<String, LinkPreview> _cache = {};
  final RegExp _urlRegex = RegExp(r'(https?://[^\\s]+)', caseSensitive: false);

  List<LinkPreview> extractCached(String text) {
    final matches = _urlRegex.allMatches(text);
    final previews = <LinkPreview>[];
    for (final match in matches) {
      final url = match.group(0);
      if (url == null) continue;
      final preview = _cache[url];
      if (preview != null) {
        previews.add(preview);
      }
    }
    return previews;
  }

  Future<void> prefetch(String text) async {
    final matches = _urlRegex.allMatches(text);
    for (final match in matches) {
      final url = match.group(0);
      if (url == null || _cache.containsKey(url)) continue;
      final uri = Uri.tryParse(url);
      if (uri == null) continue;
      try {
        final response = await http.get(uri);
        if (response.statusCode < 200 || response.statusCode >= 300) {
          continue;
        }
        final preview = _parseOpenGraph(uri, response.body);
        if (preview != null) {
          _cache[url] = preview;
        }
      } catch (_) {}
    }
  }

  LinkPreview? _parseOpenGraph(Uri url, String html) {
    String? title;
    String? description;
    String? image;

    final metaTag = RegExp(r'<meta[^>]+>', caseSensitive: false);
    final attr = RegExp("(property|name)=[\"']([^\"']+)[\"']");
    final content = RegExp("content=[\"']([^\"']+)[\"']");
    for (final match in metaTag.allMatches(html)) {
      final tag = match.group(0) ?? '';
      final attrMatch = attr.firstMatch(tag);
      final contentMatch = content.firstMatch(tag);
      if (attrMatch == null || contentMatch == null) continue;
      final key = attrMatch.group(2)?.toLowerCase();
      final value = contentMatch.group(1);
      if (key == null || value == null) continue;
      if (key == 'og:title' && title == null) title = value;
      if (key == 'og:description' && description == null) {
        description = value;
      }
      if (key == 'og:image' && image == null) image = value;
    }

    if (title == null || title.isEmpty) {
      final titleMatch = RegExp(r'<title>([^<]+)</title>', caseSensitive: false)
          .firstMatch(html);
      title = titleMatch?.group(1)?.trim();
    }

    if (title == null || title.isEmpty) {
      return null;
    }
    return LinkPreview(
      url: url,
      title: title,
      description: description,
      imageUrl: image,
    );
  }
}

class LinkPreviewCard extends StatelessWidget {
  final LinkPreview preview;

  const LinkPreviewCard({required this.preview});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: InkWell(
        onTap: () => launchUrl(preview.url, mode: LaunchMode.externalApplication),
        child: Container(
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.surfaceContainerHighest,
            borderRadius: BorderRadius.circular(14),
            border: Border.all(color: const Color(0xFF1F2937)),
          ),
          child: Row(
            children: [
              if (preview.imageUrl != null)
                ClipRRect(
                  borderRadius: const BorderRadius.only(
                    topLeft: Radius.circular(14),
                    bottomLeft: Radius.circular(14),
                  ),
                  child: Image.network(
                    preview.imageUrl!,
                    width: 96,
                    height: 96,
                    fit: BoxFit.cover,
                    errorBuilder: (_, __, ___) => const SizedBox(
                      width: 96,
                      height: 96,
                      child: Icon(Icons.link),
                    ),
                  ),
                ),
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.all(12),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        preview.title,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: Theme.of(context).textTheme.titleSmall,
                      ),
                      if (preview.description != null) ...[
                        const SizedBox(height: 6),
                        Text(
                          preview.description!,
                          maxLines: 3,
                          overflow: TextOverflow.ellipsis,
                          style: Theme.of(context)
                              .textTheme
                              .bodySmall
                              ?.copyWith(color: Colors.white70),
                        ),
                      ],
                      const SizedBox(height: 6),
                      Text(
                        preview.url.host,
                        style: Theme.of(context)
                            .textTheme
                            .bodySmall
                            ?.copyWith(color: Colors.white54),
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class VideoAttachmentPreview extends StatefulWidget {
  final Uint8List bytes;
  final String title;

  const VideoAttachmentPreview({required this.bytes, required this.title});

  @override
  State<VideoAttachmentPreview> createState() => _VideoAttachmentPreviewState();
}

class _VideoAttachmentPreviewState extends State<VideoAttachmentPreview> {
  VideoPlayerController? _controller;
  bool _initialized = false;

  @override
  void initState() {
    super.initState();
    _init();
  }

  Future<void> _init() async {
    final temp = await File('${Directory.systemTemp.path}/${DateTime.now().millisecondsSinceEpoch}.mp4').create();
    await temp.writeAsBytes(widget.bytes, flush: true);
    final controller = VideoPlayerController.file(temp);
    await controller.initialize();
    setState(() {
      _controller = controller;
      _initialized = true;
    });
  }

  @override
  void dispose() {
    _controller?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (!_initialized || _controller == null) {
      return Container(
        color: const Color(0xFF0F172A),
        child: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              const Icon(Icons.play_circle_outline, size: 32),
              const SizedBox(height: 6),
              Text(
                widget.title,
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                textAlign: TextAlign.center,
                style: Theme.of(context).textTheme.bodySmall,
              ),
            ],
          ),
        ),
      );
    }
    return Stack(
      children: [
        AspectRatio(
          aspectRatio: _controller!.value.aspectRatio,
          child: VideoPlayer(_controller!),
        ),
        Align(
          alignment: Alignment.center,
          child: IconButton(
            icon: Icon(
              _controller!.value.isPlaying ? Icons.pause : Icons.play_arrow,
              color: Colors.white,
            ),
            onPressed: () {
              setState(() {
                if (_controller!.value.isPlaying) {
                  _controller!.pause();
                } else {
                  _controller!.play();
                }
              });
            },
          ),
        ),
      ],
    );
  }
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
    return ListView.builder(
      padding: const EdgeInsets.all(16),
      itemCount: items.isEmpty ? 2 : items.length + 2,
      itemBuilder: (context, index) {
        if (index == 0) {
          return HeaderCard(controller: controller);
        }
        if (index == 1) {
          return const SizedBox(height: 16);
        }
        if (items.isEmpty) {
          return const Center(child: CircularProgressIndicator());
        }
        final entry = items[index - 2];
        return PostCard(
          entry: entry,
          showProtocolDetails: showProtocolDetails,
        );
      },
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
            label: Text(controller.connectionStatus),
            backgroundColor: controller.connectionStatus == 'LIVE'
                ? const Color(0xFF134E4A)
                : controller.connectionStatus == 'DEGRADED'
                    ? const Color(0xFF3F2F0B)
                    : const Color(0xFF3B1D1D),
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
    return _PostCardAnimated(
      entry: entry,
      showProtocolDetails: showProtocolDetails,
    );
  }
}

class _PostCardAnimated extends StatefulWidget {
  final FeedEntry entry;
  final bool showProtocolDetails;

  const _PostCardAnimated({
    required this.entry,
    required this.showProtocolDetails,
  });

  @override
  State<_PostCardAnimated> createState() => _PostCardAnimatedState();
}

class _PostCardAnimatedState extends State<_PostCardAnimated>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 400),
  );

  @override
  void initState() {
    super.initState();
    _controller.forward();
    widget.entry.fadedIn = true;
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final entry = widget.entry;
    return FadeTransition(
      opacity: CurvedAnimation(parent: _controller, curve: Curves.easeOut),
      child: Padding(
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
              if (entry.attachments.isNotEmpty) ...[
                const SizedBox(height: 12),
                SizedBox(
                  height: 120,
                  child: ListView.separated(
                    scrollDirection: Axis.horizontal,
                    itemCount: entry.attachments.length,
                    separatorBuilder: (_, __) => const SizedBox(width: 12),
                    itemBuilder: (context, index) {
                      final attachment = entry.attachments[index];
                      return ClipRRect(
                        borderRadius: BorderRadius.circular(12),
                        child: AspectRatio(
                          aspectRatio: 1,
                          child: attachment.isVideo
                              ? VideoAttachmentPreview(
                                  bytes: attachment.bytes,
                                  title: attachment.name,
                                )
                              : attachment.isImage
                              ? Image.memory(
                                  attachment.bytes,
                                  fit: BoxFit.cover,
                                )
                              : Container(
                                  color: const Color(0xFF0F172A),
                                  padding: const EdgeInsets.all(12),
                                  child: Column(
                                    mainAxisAlignment: MainAxisAlignment.center,
                                    children: [
                                      const Icon(Icons.insert_drive_file),
                                      const SizedBox(height: 6),
                                      Text(
                                        attachment.name,
                                        maxLines: 2,
                                        overflow: TextOverflow.ellipsis,
                                        textAlign: TextAlign.center,
                                        style: Theme.of(context)
                                            .textTheme
                                            .bodySmall,
                                      ),
                                      const SizedBox(height: 6),
                                      Text(
                                        '${attachment.chunkCount} chunks',
                                        style: Theme.of(context)
                                            .textTheme
                                            .bodySmall
                                            ?.copyWith(color: Colors.white70),
                                      ),
                                    ],
                                  ),
                                ),
                        ),
                      );
                    },
                  ),
                ),
              ] else if (entry.blurHash != null) ...[
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
              if (entry.linkPreviews.isNotEmpty) ...[
                const SizedBox(height: 12),
                ...entry.linkPreviews.map(
                  (preview) => LinkPreviewCard(preview: preview),
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
                      ).textTheme.bodySmall?.copyWith(
                        color: Colors.white70,
                      ),
                    ),
                  ],
                ),
                if (entry.requestingMissing) ...[
                  const SizedBox(height: 8),
                  Row(
                    children: [
                      const Icon(
                        Icons.radar,
                        size: 16,
                        color: Color(0xFF38BDF8),
                      ),
                      const SizedBox(width: 6),
                      Text(
                        'Requesting missing shard',
                        style: Theme.of(
                          context,
                        ).textTheme.bodySmall?.copyWith(
                          color: Colors.white70,
                        ),
                      ),
                    ],
                  ),
                ],
              ],
              if (widget.showProtocolDetails) ...[
                const SizedBox(height: 8),
                Text(
                  'protocol details available in Inspect',
                  style: Theme.of(context).textTheme.bodySmall?.copyWith(
                    color: Colors.white70,
                  ),
                ),
              ],
              Align(
                alignment: Alignment.centerRight,
                child: TextButton.icon(
                  onPressed: () => _openInspect(context, entry),
                  icon: const Icon(Icons.info_outline, size: 18),
                  label: const Text('Inspect'),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

void _openInspect(BuildContext context, FeedEntry entry) {
  showModalBottomSheet(
    context: context,
    showDragHandle: true,
    backgroundColor: Colors.transparent,
    builder: (context) => _InspectSheet(entry: entry),
  );
}

class _InspectSheet extends StatelessWidget {
  final FeedEntry entry;

  const _InspectSheet({required this.entry});

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: ClipRRect(
        borderRadius: const BorderRadius.vertical(top: Radius.circular(24)),
        child: BackdropFilter(
          filter: ImageFilter.blur(sigmaX: 14, sigmaY: 14),
          child: Container(
            decoration: BoxDecoration(
              color: Theme.of(context).colorScheme.surface.withOpacity(0.85),
            ),
            child: ListView(
              padding: const EdgeInsets.all(16),
              shrinkWrap: true,
              children: [
                Text('Inspect Post', style: Theme.of(context).textTheme.titleLarge),
                const SizedBox(height: 12),
                _InspectRow(label: 'object_root', value: entry.id),
                _InspectRow(
                  label: 'status',
                  value: entry.reconstructed ? 'reconstructed' : 'pending',
                ),
                _InspectRow(
                  label: 'shards',
                  value: '${entry.shardsHave}/${entry.shardsTotal}',
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _InspectRow extends StatelessWidget {
  final String label;
  final String value;

  const _InspectRow({required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 110,
            child: Text(
              label,
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: Colors.white70,
              ),
            ),
          ),
          Expanded(
            child: Text(
              value,
              style: Theme.of(context).textTheme.bodyMedium,
            ),
          ),
        ],
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
    final theme = Theme.of(context);
    final wsError = _wsController.text.isNotEmpty
        ? controller.wsUrlError
        : null;
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
              const SizedBox(height: 8),
              ExpansionTile(
                title: const Text('Advanced connection'),
                subtitle: Text(
                  controller.useLocalRelay
                      ? 'Relay and channel settings'
                      : 'External relay settings',
                  style: theme.textTheme.bodySmall,
                ),
                childrenPadding: const EdgeInsets.only(top: 8),
                children: [
                  if (!controller.useLocalRelay) ...[
                    _InputField(
                      label: 'WebSocket URL',
                      controller: _wsController,
                      onChanged: (value) {
                        controller.setWsUrl(value);
                        setState(() {});
                      },
                      errorText: wsError,
                    ),
                    _InputField(
                      label: 'Peer ID',
                      controller: _peerController,
                      onChanged: (value) {
                        controller.setPeerId(value);
                        setState(() {});
                      },
                    ),
                  ],
                  _InputField(
                    label: 'Channel',
                    controller: _tagController,
                    onChanged: controller.setChannelLabel,
                    errorText: controller.channelError,
                    onScan: () => _openScanner(
                      context,
                      onResult: controller.handleScanValue,
                    ),
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
                        ? () =>
                              controller.updateSubscription(_tagController.text)
                        : null,
                    child: const Text('Update Channel'),
                  ),
                ],
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
                onScan: () => _openScanner(
                  context,
                  onResult: controller.handleScanValue,
                ),
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
                ],
              ),
              const SizedBox(height: 8),
              ...controller.forwardPeers.map(
                (peer) => ListTile(
                  dense: true,
                  leading: const Icon(Icons.router, size: 18),
                  title: Text(peer),
                  trailing: SizedBox(
                    width: 48,
                    height: 48,
                    child: IconButton(
                      icon: const Icon(Icons.close),
                      onPressed: () => controller.removeForwardPeer(peer),
                    ),
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
                onScan: () => _openScanner(
                  context,
                  onResult: controller.handleScanValue,
                ),
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
                ],
              ),
              const SizedBox(height: 8),
              ...controller.extraTags.map(
                (tag) => Dismissible(
                  key: ValueKey(tag),
                  direction: DismissDirection.endToStart,
                  background: Container(
                    alignment: Alignment.centerRight,
                    padding: const EdgeInsets.only(right: 16),
                    color: const Color(0xFF7F1D1D),
                    child: const Icon(Icons.delete, color: Colors.white),
                  ),
                  onDismissed: (_) => controller.removeSubscription(tag),
                  child: ListTile(
                    dense: true,
                    leading: const Icon(Icons.tag, size: 18),
                    title: Text(tag),
                    trailing: const Icon(Icons.chevron_right),
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
  bool _torchOn = false;
  final MobileScannerController _scannerController = MobileScannerController();

  void _handle(String value) {
    if (_handled) return;
    _handled = true;
    widget.onResult(value);
    Navigator.of(context).pop();
  }

  @override
  void dispose() {
    _scannerController.dispose();
    super.dispose();
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
                child: Stack(
                  children: [
                    MobileScanner(
                      controller: _scannerController,
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
                    Align(
                      alignment: Alignment.center,
                      child: Container(
                        width: 220,
                        height: 220,
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(16),
                          border: Border.all(
                            color: const Color(0xFF60A5FA),
                            width: 2,
                          ),
                        ),
                      ),
                    ),
                    Positioned(
                      right: 12,
                      top: 12,
                      child: IconButton(
                        icon: Icon(
                          _torchOn ? Icons.flash_on : Icons.flash_off,
                          color: Colors.white,
                        ),
                        onPressed: () async {
                          await _scannerController.toggleTorch();
                          if (mounted) {
                            setState(() => _torchOn = !_torchOn);
                          }
                        },
                      ),
                    ),
                  ],
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

class ComposeScreen extends StatefulWidget {
  final void Function(String text, List<Attachment> attachments) onPublish;
  final String channelLabel;

  const ComposeScreen({
    super.key,
    required this.onPublish,
    required this.channelLabel,
  });

  @override
  State<ComposeScreen> createState() => _ComposeScreenState();
}

class _ComposeScreenState extends State<ComposeScreen> {
  final _controller = TextEditingController();
  final _picker = ImagePicker();
  final List<Attachment> _attachments = [];

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Compose'),
        actions: [
          TextButton(
            onPressed: () => widget.onPublish(_controller.text, _attachments),
            child: const Text('Publish'),
          ),
        ],
      ),
      body: SafeArea(
        child: SingleChildScrollView(
          padding: EdgeInsets.only(
            left: 16,
            right: 16,
            top: 16,
            bottom: 16 + MediaQuery.of(context).viewInsets.bottom,
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
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
                maxLines: 8,
                autofocus: true,
                decoration: const InputDecoration(
                  hintText: 'Share an update...',
                ),
              ),
              const SizedBox(height: 16),
              Row(
                children: [
                  ElevatedButton.icon(
                    onPressed: _pickImage,
                    icon: const Icon(Icons.image_outlined),
                    label: const Text('Attach image'),
                  ),
                  const SizedBox(width: 12),
                  OutlinedButton.icon(
                    onPressed: _pickFile,
                    icon: const Icon(Icons.attach_file),
                    label: const Text('Attach file'),
                  ),
                  const SizedBox(width: 12),
                  Text(
                    '${_attachments.length} attached',
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Colors.white70,
                    ),
                  ),
                ],
              ),
              if (_attachments.isNotEmpty) ...[
                const SizedBox(height: 12),
                SizedBox(
                  height: 120,
                  child: ListView.separated(
                    scrollDirection: Axis.horizontal,
                    itemCount: _attachments.length,
                    separatorBuilder: (_, __) => const SizedBox(width: 12),
                    itemBuilder: (context, index) {
                      final attachment = _attachments[index];
                      return Stack(
                        children: [
                          ClipRRect(
                            borderRadius: BorderRadius.circular(12),
                            child: attachment.isImage
                                ? Image.memory(
                                    attachment.bytes,
                                    width: 120,
                                    height: 120,
                                    fit: BoxFit.cover,
                                  )
                                : Container(
                                    width: 120,
                                    height: 120,
                                    color: const Color(0xFF0F172A),
                                    child: Column(
                                      mainAxisAlignment: MainAxisAlignment.center,
                                      children: [
                                        const Icon(Icons.insert_drive_file),
                                        const SizedBox(height: 6),
                                        Text(
                                          attachment.name,
                                          maxLines: 2,
                                          overflow: TextOverflow.ellipsis,
                                          textAlign: TextAlign.center,
                                          style: Theme.of(context)
                                              .textTheme
                                              .bodySmall,
                                        ),
                                      ],
                                    ),
                                  ),
                          ),
                          Positioned(
                            right: 6,
                            top: 6,
                            child: InkWell(
                              onTap: () {
                                setState(() => _attachments.removeAt(index));
                              },
                              child: Container(
                                width: 28,
                                height: 28,
                                decoration: BoxDecoration(
                                  color: Colors.black54,
                                  borderRadius: BorderRadius.circular(14),
                                ),
                                child: const Icon(Icons.close, size: 16),
                              ),
                            ),
                          ),
                        ],
                      );
                    },
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _pickImage() async {
    final image = await _picker.pickImage(source: ImageSource.gallery);
    if (image == null) return;
    final bytes = await image.readAsBytes();
    final digest = sha256.convert(bytes).toString();
    final chunks = splitIntoFileChunks(bytes);
    final chunkRoots = chunks
        .map((chunk) => sha256.convert(chunk.data).toString())
        .toList();
    final descriptor = MediaDescriptorV1(
      mime: image.mimeType ?? 'image/*',
      size: bytes.length,
      hashHex: digest,
      chunkRoots: chunkRoots,
    );
    setState(() {
      _attachments.add(
        Attachment(
          name: image.name,
          mime: image.mimeType ?? 'image/*',
          bytes: bytes,
          hashHex: digest,
          size: bytes.length,
          isImage: true,
          isVideo: false,
          chunkCount: chunks.length,
          descriptor: descriptor,
        ),
      );
    });
  }

  Future<void> _pickFile() async {
    final result = await FilePicker.platform.pickFiles(withData: true);
    if (result == null || result.files.isEmpty) return;
    final file = result.files.first;
    final bytes = file.bytes;
    if (bytes == null) return;
    final mime = lookupMimeType(file.name) ?? 'application/octet-stream';
    final digest = sha256.convert(bytes).toString();
    final chunks = splitIntoFileChunks(bytes);
    final chunkRoots = chunks
        .map((chunk) => sha256.convert(chunk.data).toString())
        .toList();
    final descriptor = MediaDescriptorV1(
      mime: mime,
      size: bytes.length,
      hashHex: digest,
      chunkRoots: chunkRoots,
    );
    setState(() {
      _attachments.add(
        Attachment(
          name: file.name,
          mime: mime,
          bytes: bytes,
          hashHex: digest,
          size: bytes.length,
          isImage: mime.startsWith('image/'),
          isVideo: mime.startsWith('video/'),
          chunkCount: chunks.length,
          descriptor: descriptor,
        ),
      );
    });
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
  final int maxCacheEntries;
  final int maxPublishQueue;
  final ValueChanged<String> onMaxCacheEntriesChanged;
  final ValueChanged<String> onMaxPublishQueueChanged;

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
    required this.maxCacheEntries,
    required this.maxPublishQueue,
    required this.onMaxCacheEntriesChanged,
    required this.onMaxPublishQueueChanged,
  });

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: ClipRRect(
        borderRadius: const BorderRadius.vertical(top: Radius.circular(24)),
        child: BackdropFilter(
          filter: ImageFilter.blur(sigmaX: 16, sigmaY: 16),
          child: Container(
            color: Theme.of(context).colorScheme.surface.withOpacity(0.85),
            child: DraggableScrollableSheet(
              initialChildSize: 0.65,
              minChildSize: 0.4,
              maxChildSize: 0.95,
              expand: false,
              builder: (context, scrollController) {
                return ListView(
                  controller: scrollController,
                  padding: const EdgeInsets.all(16),
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
                    const SizedBox(height: 12),
                    TextFormField(
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(
                        labelText: 'Cache entry limit',
                        helperText: 'Approx shard cache size cap.',
                      ),
                      initialValue: maxCacheEntries.toString(),
                      onFieldSubmitted: onMaxCacheEntriesChanged,
                    ),
                    const SizedBox(height: 12),
                    TextFormField(
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(
                        labelText: 'Publish queue limit',
                        helperText: 'Max queued objects pending send.',
                      ),
                      initialValue: maxPublishQueue.toString(),
                      onFieldSubmitted: onMaxPublishQueueChanged,
                    ),
                  ],
                );
              },
            ),
          ),
        ),
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
  final String? errorText;
  final VoidCallback? onScan;

  const _InputField({
    required this.label,
    required this.controller,
    this.onChanged,
    this.errorText,
    this.onScan,
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
          errorText: errorText,
          suffixIcon: onScan == null
              ? null
              : IconButton(
                  icon: const Icon(Icons.qr_code_scanner),
                  onPressed: onScan,
                ),
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
