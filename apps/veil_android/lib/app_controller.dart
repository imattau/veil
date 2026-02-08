import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:crypto/crypto.dart';
import 'package:flutter/material.dart';
import 'package:flutter_reactive_ble/flutter_reactive_ble.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:image_picker/image_picker.dart';
import 'package:mime/mime.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:sqflite/sqflite.dart';
import 'package:veil_sdk/veil_sdk.dart';

import 'models.dart';
import 'services/link_preview_service.dart';

class VeilAppController extends ChangeNotifier {
  final _bridge = const VeilBridge();
  final _ble = FlutterReactiveBle();
  final _secureStorage = const FlutterSecureStorage();
  final _picker = ImagePicker();
  final _linkPreviewService = LinkPreviewService();
  static final RegExp _mentionPattern = RegExp(r'@([0-9a-fA-F]{64})');
  final _publisher = const VeilPublisher();
  Attachment? _profileAvatar;
  String _profileBio = '';
  String _profileWebsite = '';
  String _profileLocation = '';
  DateTime? _profileLastPublished;
  bool _profilePublishing = false;
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
  final LocalWotPolicy _wotPolicy = LocalWotPolicy();

  String displayName = '';
  String recoveryPhrase = '';
  String namespaceChoice = 'Public Square';
  String peerId = 'android-client';
  String wsUrl = 'ws://127.0.0.1:9001';
  final List<String> _wsEndpoints = [];
  String tagHex = '';
  String channelLabel = '';
  static const String _channelPublisherKey =
      '0000000000000000000000000000000000000000000000000000000000000000';
  final List<String> _extraTags = [];
  final List<String> _forwardPeers = [];
  String bleDeviceId = '';
  String bleServiceUuid = '6e400001-b5a3-f393-e0a9-e50e24dcca9e';
  String bleCharacteristicUuid = '6e400003-b5a3-f393-e0a9-e50e24dcca9e';
  String quicEndpoint = '';
  bool _torEnabled = false;
  String torWsUrl = '';
  String torSocksHost = '127.0.0.1';
  int torSocksPort = 9050;
  String quicTrustedCertHex = '';

  VeilClient? _client;
  VeilLane? _lane;
  QuicLane? _quicLane;
  TorLane? _torLane;
  BleLane? _bleLane;
  VeilLane? _fallbackLane;
  LocalRelay? _relay;
  bool _relayReady = false;
  final bool _useLocalRelay = true;
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
  Attachment? get profileAvatar => _profileAvatar;
  String get profileBio => _profileBio;
  String get profileWebsite => _profileWebsite;
  String get profileLocation => _profileLocation;
  DateTime? get profileLastPublished => _profileLastPublished;
  bool get profilePublishing => _profilePublishing;
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
  String get quicEndpointValue => quicEndpoint;
  bool get torEnabled => _torEnabled;
  String get torWsUrlValue => torWsUrl;
  String get torSocksHostValue => torSocksHost;
  int get torSocksPortValue => torSocksPort;
  String get quicTrustedCertValue => quicTrustedCertHex;
  String? get wsUrlError {
    if (!wsUrl.startsWith('ws://') && !wsUrl.startsWith('wss://')) {
      return 'Use ws:// or wss://';
    }
    return null;
  }

  String? _normalizeWsEndpoint(String raw) {
    final trimmed = raw.trim();
    if (trimmed.isEmpty) return null;
    final lower = trimmed.toLowerCase();
    if (lower.startsWith('ws://') || lower.startsWith('wss://')) {
      final uri = Uri.tryParse(trimmed);
      if (uri == null || uri.host.isEmpty) return null;
      return Uri(
        scheme: uri.scheme,
        host: uri.host,
        port: uri.hasPort && uri.port != 0 ? uri.port : null,
        path: uri.path.isEmpty ? '/ws' : uri.path,
        query: uri.query.isEmpty ? null : uri.query,
      ).toString();
    }
    if (lower.startsWith('http://') || lower.startsWith('https://')) {
      final uri = Uri.tryParse(trimmed);
      if (uri == null || uri.host.isEmpty) return null;
      final scheme = uri.scheme == 'https' ? 'wss' : 'ws';
      final port = uri.hasPort && uri.port != 0 ? uri.port : null;
      final path = (uri.path.isEmpty || uri.path == '/') ? '/ws' : uri.path;
      final normalized = Uri(
        scheme: scheme,
        host: uri.host,
        port: port,
        path: path,
        query: uri.query.isEmpty ? null : uri.query,
      ).toString();
      return normalized;
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
  LaneHealthSnapshot? get quicLaneHealth => _quicLane?.healthSnapshot();
  LaneHealthSnapshot? get torLaneHealth => _torLane?.healthSnapshot();
  LaneHealthSnapshot? get bleLaneHealth => _bleLane?.healthSnapshot();
  LaneHealthSnapshot? get fallbackLaneHealth =>
      _client?.fallbackLane?.healthSnapshot() ??
      _fallbackLane?.healthSnapshot();
  List<FeedEntry> get feed => List.unmodifiable(_feed);
  List<FeedEntry> get visibleFeed =>
      List.unmodifiable(_feed.where((entry) => !isBlocked(entry.authorKey)));
  List<String> get events => List.unmodifiable(_events);
  List<String> get suggestedFeeds => List.unmodifiable(_suggestedFeeds);
  Set<String> get trustedFeeds => Set.unmodifiable(_trustedFeeds);
  List<String> get followedUsers => _wotPolicy.trusted;
  List<String> get mutedUsers => _wotPolicy.muted;
  List<String> get blockedUsers => _wotPolicy.blocked;
  List<String> get extraTags => List.unmodifiable(_extraTags);
  List<String> get forwardPeers => List.unmodifiable(_forwardPeers);
  List<String> get wsEndpoints => List.unmodifiable(_wsEndpoints);

  String get connectionStatus {
    if (!_connected) return 'OFFLINE';
    final fast = fastLaneHealth;
    final fallback = fallbackLaneHealth;
    final fastOk = _laneHealthy(fast);
    final fallbackOk = _laneHealthy(fallback);
    final needsFallback = _bleEnabled || _torEnabled;
    if (fastOk && (fallbackOk || !needsFallback)) {
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
    await _startLocalRelay();
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
    try {
      await relay.start();
      _relay = relay;
      _relayReady = true;
      notifyListeners();
    } catch (err) {
      _relay = null;
      _relayReady = false;
      _events.insert(0, 'Local relay failed to start: $err');
      _notify();
    }
  }

  Future<void> _ensureLocalRelay() async {
    if (_relay == null) {
      await _startLocalRelay();
      return;
    }
    final uri = Uri.tryParse(_relay!.url);
    if (uri == null || uri.port == 0) {
      _relay?.stop();
      _relay = null;
      _relayReady = false;
      await _startLocalRelay();
      return;
    }
    try {
      final socket = await Socket.connect(
        uri.host,
        uri.port,
        timeout: const Duration(milliseconds: 400),
      );
      socket.destroy();
    } catch (_) {
      _relay?.stop();
      _relay = null;
      _relayReady = false;
      _events.insert(0, 'Local relay restarted');
      await _startLocalRelay();
    }
  }

  void setUseLocalRelay(bool value) {}

  void setDisplayName(String value) {
    displayName = value.trim();
    _persistPrefs();
    notifyListeners();
  }

  void updateProfileDetails({String? bio, String? website, String? location}) {
    if (bio != null) _profileBio = bio;
    if (website != null) _profileWebsite = website;
    if (location != null) _profileLocation = location;
    _persistPrefs();
    notifyListeners();
  }

  Future<void> pickProfileAvatar() async {
    final image = await _picker.pickImage(source: ImageSource.gallery);
    if (image == null) return;
    final bytes = await image.readAsBytes();
    final mime = image.mimeType ?? lookupMimeType(image.name) ?? 'image/*';
    _profileAvatar = Attachment(
      name: image.name,
      mime: mime,
      bytes: bytes,
      hashHex: sha256.convert(bytes).toString(),
      size: bytes.length,
      isImage: true,
      isVideo: false,
      chunkCount: splitIntoFileChunks(bytes).length,
    );
    _persistPrefs();
    notifyListeners();
  }

  void clearProfileAvatar() {
    _profileAvatar = null;
    _persistPrefs();
    notifyListeners();
  }

  void publishProfile() {
    if (displayName.trim().isEmpty) return;
    final avatar = _profileAvatar;
    _profilePublishing = true;
    notifyListeners();
    () async {
      MediaDescriptorV1? avatarDescriptor;
      if (avatar != null) {
        final chunkObjects = await _publisher.buildFileChunks(avatar.bytes);
        for (final chunk in chunkObjects) {
          _publishQueue.enqueue(chunk);
          _persistPublishObject(chunk);
        }
        final roots = chunkObjects.map((obj) => obj.objectRootHex).toList();
        if (roots.isNotEmpty) {
          avatarDescriptor = MediaDescriptorV1(
            mime: avatar.mime,
            size: avatar.size,
            hashHex: avatar.hashHex,
            chunkRoots: roots,
          );
        }
      }

      final profileBytes = encodeProfile(
        ProfileV1(
          displayName: displayName,
          bio: _profileBio.isEmpty ? null : _profileBio,
          avatar: avatarDescriptor,
          website: _profileWebsite.isEmpty ? null : _profileWebsite,
          location: _profileLocation.isEmpty ? null : _profileLocation,
        ),
      );
      final profileObject = await _publisher.buildObject(profileBytes);
      _publishQueue.enqueue(profileObject);
      _persistPublishObject(profileObject);
      _events.insert(0, 'Profile update queued');
      _profileLastPublished = DateTime.now();
      _profilePublishing = false;
      _persistPrefs();
      notifyListeners();
      _drainPublishQueue();
    }();
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
    var wsChanged = false;
    final normalizedWs = _normalizeWsEndpoint(wsUrl);
    if (normalizedWs == null) {
      if (wsUrl.isNotEmpty) {
        wsChanged = true;
      }
      wsUrl = '';
    } else if (normalizedWs != wsUrl) {
      wsUrl = normalizedWs;
      wsChanged = true;
    }
    _wsEndpoints
      ..clear()
      ..addAll(
        (prefs.getStringList('wsEndpoints') ?? [])
            .map(_normalizeWsEndpoint)
            .whereType<String>(),
      );
    if ((prefs.getStringList('wsEndpoints') ?? []).length != _wsEndpoints.length) {
      wsChanged = true;
    }
    tagHex = prefs.getString('tagHex') ?? tagHex;
    channelLabel = prefs.getString('channelLabel') ?? channelLabel;
    bleDeviceId = prefs.getString('bleDeviceId') ?? bleDeviceId;
    bleServiceUuid = prefs.getString('bleServiceUuid') ?? bleServiceUuid;
    bleCharacteristicUuid =
        prefs.getString('bleCharacteristicUuid') ?? bleCharacteristicUuid;
    quicEndpoint = prefs.getString('quicEndpoint') ?? quicEndpoint;
    quicTrustedCertHex =
        prefs.getString('quicTrustedCertHex') ?? quicTrustedCertHex;
    _torEnabled = prefs.getBool('torEnabled') ?? _torEnabled;
    torWsUrl = prefs.getString('torWsUrl') ?? torWsUrl;
    torSocksHost = prefs.getString('torSocksHost') ?? torSocksHost;
    torSocksPort = prefs.getInt('torSocksPort') ?? torSocksPort;
    _ghostMode = prefs.getBool('ghostMode') ?? _ghostMode;
    _bleEnabled = prefs.getBool('bleEnabled') ?? _bleEnabled;
    _requireSignedPublic =
        prefs.getBool('requireSignedPublic') ?? _requireSignedPublic;
    _clockSkewSeconds = prefs.getInt('clockSkewSeconds') ?? _clockSkewSeconds;
    _maxCacheEntries = prefs.getInt('maxCacheEntries') ?? _maxCacheEntries;
    _maxPublishQueue = prefs.getInt('maxPublishQueue') ?? _maxPublishQueue;
    _publishQueue.updateMaxSize(_maxPublishQueue);
    _profileBio = prefs.getString('profileBio') ?? _profileBio;
    _profileWebsite = prefs.getString('profileWebsite') ?? _profileWebsite;
    _profileLocation = prefs.getString('profileLocation') ?? _profileLocation;
    final profilePublishedMs = prefs.getInt('profileLastPublished');
    if (profilePublishedMs != null && profilePublishedMs > 0) {
      _profileLastPublished = DateTime.fromMillisecondsSinceEpoch(
        profilePublishedMs,
      );
    }
    _extraTags
      ..clear()
      ..addAll(prefs.getStringList('extraTags') ?? const []);
    _forwardPeers
      ..clear()
      ..addAll(prefs.getStringList('forwardPeers') ?? const []);
    _trustedFeeds
      ..clear()
      ..addAll(prefs.getStringList('trustedFeeds') ?? const []);
    final followed = prefs.getStringList('followedUsers') ?? const [];
    final muted = prefs.getStringList('mutedUsers') ?? const [];
    final blocked = prefs.getStringList('blockedUsers') ?? const [];
    for (final id in followed) {
      _wotPolicy.trust(id);
    }
    for (final id in muted) {
      _wotPolicy.mute(id);
    }
    for (final id in blocked) {
      _wotPolicy.block(id);
    }
    recoveryPhrase =
        await _secureStorage.read(key: 'recoveryPhrase') ?? recoveryPhrase;
    if (wsChanged) {
      await _persistPrefs();
    }
    notifyListeners();
  }

  Future<void> _persistPrefs() async {
    final prefs = _prefs;
    if (prefs == null) return;
    await prefs.setString('displayName', displayName);
    await prefs.setString('namespaceChoice', namespaceChoice);
    await prefs.setString('peerId', peerId);
    await prefs.setString('wsUrl', wsUrl);
    await prefs.setStringList('wsEndpoints', _wsEndpoints);
    await prefs.setString('tagHex', tagHex);
    await prefs.setString('channelLabel', channelLabel);
    await prefs.setString('bleDeviceId', bleDeviceId);
    await prefs.setString('bleServiceUuid', bleServiceUuid);
    await prefs.setString('bleCharacteristicUuid', bleCharacteristicUuid);
    await prefs.setString('quicEndpoint', quicEndpoint);
    await prefs.setString('quicTrustedCertHex', quicTrustedCertHex);
    await prefs.setBool('torEnabled', _torEnabled);
    await prefs.setString('torWsUrl', torWsUrl);
    await prefs.setString('torSocksHost', torSocksHost);
    await prefs.setInt('torSocksPort', torSocksPort);
    await prefs.setBool('ghostMode', _ghostMode);
    await prefs.setBool('bleEnabled', _bleEnabled);
    await prefs.setBool('requireSignedPublic', _requireSignedPublic);
    await prefs.setInt('clockSkewSeconds', _clockSkewSeconds);
    await prefs.setInt('maxCacheEntries', _maxCacheEntries);
    await prefs.setInt('maxPublishQueue', _maxPublishQueue);
    await prefs.setString('profileBio', _profileBio);
    await prefs.setString('profileWebsite', _profileWebsite);
    await prefs.setString('profileLocation', _profileLocation);
    if (_profileLastPublished != null) {
      await prefs.setInt(
        'profileLastPublished',
        _profileLastPublished!.millisecondsSinceEpoch,
      );
    }
    await prefs.setStringList('extraTags', _extraTags);
    await prefs.setStringList('forwardPeers', _forwardPeers);
    await prefs.setStringList('trustedFeeds', _trustedFeeds.toList());
    await prefs.setStringList('followedUsers', _wotPolicy.trusted);
    await prefs.setStringList('mutedUsers', _wotPolicy.muted);
    await prefs.setStringList('blockedUsers', _wotPolicy.blocked);
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
    final rows = await db.query('publish_queue', orderBy: 'enqueued_at ASC');
    for (final row in rows) {
      final root = row['object_root'] as String?;
      final bytes = row['bytes'] as List<int>?;
      if (root == null || bytes == null) continue;
      _publishQueue.enqueue(
        PublishObject(
          objectRootHex: root,
          objectBytes: Uint8List.fromList(bytes),
        ),
      );
    }
  }

  Future<void> _persistPublishObject(PublishObject object) async {
    final db = _db;
    if (db == null) return;
    await db.insert('publish_queue', {
      'object_root': object.objectRootHex,
      'bytes': object.objectBytes,
      'enqueued_at': DateTime.now().millisecondsSinceEpoch,
    }, conflictAlgorithm: ConflictAlgorithm.ignore);
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
    final normalized = _normalizeWsEndpoint(value);
    if (normalized == null) {
      wsUrl = '';
      _events.insert(0, 'Invalid WebSocket URL');
    } else {
      wsUrl = normalized;
    }
    _persistPrefs();
    notifyListeners();
  }

  void addWsEndpoint(String value) {
    final normalized = _normalizeWsEndpoint(value);
    if (normalized == null) {
      _events.insert(0, 'Endpoint must start with ws:// or wss://');
      _notify();
      return;
    }
    if (!_wsEndpoints.contains(normalized)) {
      _wsEndpoints.add(normalized);
      _persistPrefs();
      _events.insert(0, 'Added WebSocket endpoint');
      _notify();
    }
  }

  void removeWsEndpoint(String value) {
    if (_wsEndpoints.remove(value)) {
      _persistPrefs();
      _events.insert(0, 'Removed WebSocket endpoint');
      _notify();
    }
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
    if (_importVpsProfile(raw)) {
      _events.insert(0, 'Imported VPS profile');
      _notify();
      return;
    }
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
        lower.startsWith('http://') ||
        lower.startsWith('https://')) {
      addWsEndpoint(raw);
      return;
    }
    if (lower.startsWith('quic://')) {
      setQuicEndpoint(raw);
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

  bool _importVpsProfile(String raw) {
    final lower = raw.toLowerCase();
    if (!lower.startsWith('veil://') &&
        !lower.startsWith('veil:vps:') &&
        !lower.startsWith('vps:')) {
      return false;
    }

    Uri? uri;
    if (lower.startsWith('veil://')) {
      uri = Uri.tryParse(raw);
    } else if (lower.startsWith('vps:')) {
      uri = Uri.tryParse('veil://${raw.substring(4)}');
    } else if (lower.startsWith('veil:vps:')) {
      uri = Uri.tryParse('veil://${raw.substring(9)}');
    }
    if (uri == null) return false;
    final host = uri.host.toLowerCase();
    if (host.isNotEmpty && host != 'vps') {
      return false;
    }
    final wsEndpoints = uri.queryParametersAll['ws'] ?? const [];
    final peers = uri.queryParametersAll['peer'] ?? const [];
    final tags = uri.queryParametersAll['tag'] ?? const [];
    final quic = uri.queryParameters['quic'];
    final cert = uri.queryParameters['cert'];
    final certB64 = uri.queryParameters['certb64'];

    for (final ws in wsEndpoints) {
      addWsEndpoint(ws);
      if (wsUrl.isEmpty) {
        wsUrl = ws;
      }
    }
    if (quic != null && quic.isNotEmpty) {
      setQuicEndpoint(quic);
    }
    if (cert != null && cert.isNotEmpty) {
      setQuicTrustedCert(cert);
    } else if (certB64 != null && certB64.isNotEmpty) {
      try {
        final bytes = base64Decode(certB64);
        setQuicTrustedCert(_bytesToHex(bytes));
      } catch (_) {
        _events.insert(0, 'Invalid QUIC cert (base64)');
      }
    } else if (quic != null && quic.isNotEmpty) {
      _events.insert(0, 'Pinning QUIC cert from server…');
      Future.microtask(() async {
        if (!await QuicLane.isSupported()) {
          _events.insert(0, 'QUIC not supported on this device');
          _notify();
          return;
        }
        final cert = await QuicLane.fetchPinnedCertHex(quic);
        if (cert == null || cert.isEmpty) {
          _events.insert(0, 'Failed to pin QUIC cert');
        } else {
          quicTrustedCertHex = cert;
          await _persistPrefs();
          _events.insert(0, 'Pinned QUIC certificate');
        }
        _notify();
      });
    }
    for (final peer in peers) {
      addForwardPeer(peer);
    }
    for (final tag in tags) {
      addSubscription(tag);
    }
    _persistPrefs();
    notifyListeners();
    return wsEndpoints.isNotEmpty ||
        peers.isNotEmpty ||
        tags.isNotEmpty ||
        (quic != null && quic.isNotEmpty);
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

  void setQuicEndpoint(String value) {
    quicEndpoint = value.trim();
    _persistPrefs();
    notifyListeners();
  }

  void setTorEnabled(bool value) {
    _torEnabled = value;
    _persistPrefs();
    notifyListeners();
  }

  void setTorWsUrl(String value) {
    torWsUrl = value.trim();
    _persistPrefs();
    notifyListeners();
  }

  void setTorSocksHost(String value) {
    torSocksHost = value.trim();
    _persistPrefs();
    notifyListeners();
  }

  void setTorSocksPort(String value) {
    final parsed = int.tryParse(value.trim());
    if (parsed == null) return;
    torSocksPort = parsed;
    _persistPrefs();
    notifyListeners();
  }

  void setQuicTrustedCert(String value) {
    quicTrustedCertHex = value.trim();
    _persistPrefs();
    notifyListeners();
  }

  String _bytesToHex(List<int> bytes) {
    final buffer = StringBuffer();
    for (final b in bytes) {
      buffer.write(b.toRadixString(16).padLeft(2, '0'));
    }
    return buffer.toString();
  }

  Future<void> pinQuicCertFromServer() async {
    if (quicEndpoint.isEmpty) {
      _events.insert(0, 'Set QUIC endpoint first');
      _notify();
      return;
    }
    if (!await QuicLane.isSupported()) {
      _events.insert(0, 'QUIC not supported on this device');
      _notify();
      return;
    }
    _events.insert(0, 'Fetching QUIC certificate...');
    _notify();
    final cert = await QuicLane.fetchPinnedCertHex(quicEndpoint);
    if (cert == null || cert.isEmpty) {
      _events.insert(0, 'Failed to fetch QUIC cert');
      _notify();
      return;
    }
    quicTrustedCertHex = cert;
    await _persistPrefs();
    _events.insert(0, 'Pinned QUIC certificate');
    _notify();
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
    if (_useLocalRelay) {
      await _ensureLocalRelay();
    }

    final String primaryUrl;
    List<String> endpoints;
    if (_relay != null) {
      primaryUrl = _relay!.url;
      endpoints = [primaryUrl, ..._wsEndpoints];
      if (endpoints.length == 1 && wsUrl.isNotEmpty) {
        endpoints = [primaryUrl, wsUrl];
      }
    } else if (_wsEndpoints.isNotEmpty) {
      primaryUrl = _wsEndpoints.first;
      endpoints = List<String>.from(_wsEndpoints);
    } else {
      primaryUrl = wsUrl.isEmpty ? 'ws://127.0.0.1:9001' : wsUrl;
      endpoints = [primaryUrl];
    }

    endpoints = endpoints
        .map(_normalizeWsEndpoint)
        .whereType<String>()
        .where((value) => value.startsWith('ws://') || value.startsWith('wss://'))
        .toList();
    if (endpoints.isEmpty) {
      final fallback = _relay?.url ?? wsUrl;
      final normalizedFallback = _normalizeWsEndpoint(fallback);
      if (normalizedFallback != null) {
        endpoints = [normalizedFallback];
      }
    }
    final wsLanes = endpoints
        .map(
          (endpoint) => WebSocketLane(url: Uri.parse(endpoint), peerId: peerId),
        )
        .toList();
    final VeilLane wsLane = wsLanes.length > 1
        ? MultiLane(lanes: wsLanes)
        : wsLanes.first;
    _lane = wsLane;

    if (quicEndpoint.isNotEmpty && await QuicLane.isSupported()) {
      _quicLane = QuicLane(
        endpoint: quicEndpoint,
        peerId: peerId,
        trustedPeerCertHex: quicTrustedCertHex.isEmpty
            ? null
            : quicTrustedCertHex,
      );
    }

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
    _bleLane = bleLane;

    TorLane? torLane;
    if (_torEnabled && torWsUrl.isNotEmpty && await TorLane.isSupported()) {
      torLane = TorLane(
        url: torWsUrl,
        socksHost: torSocksHost,
        socksPort: torSocksPort,
      );
    }
    _torLane = torLane;

    final primaryLane = _quicLane ?? wsLane;
    final VeilLane fastLane;
    final VeilLane? fallbackLane;
    if (_ghostMode && torLane != null) {
      fastLane = torLane;
      fallbackLane = primaryLane;
    } else if (_ghostMode && bleLane != null) {
      fastLane = bleLane;
      fallbackLane = primaryLane;
    } else {
      fastLane = primaryLane;
      fallbackLane = torLane ?? bleLane;
    }

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
    _events.insert(0, 'Connected via $primaryUrl');
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

  void publishLocalPost(
    String text, {
    List<Attachment> attachments = const [],
  }) {
    if (text.trim().isEmpty) return;
    final mentions = _extractMentions(text);
    final previews = _linkPreviewService.extractCached(text);
    final shardTotal = attachments.isEmpty
        ? 1
        : attachments.fold<int>(0, (sum, a) => sum + a.chunkCount);
    final entry = FeedEntry(
      id: DateTime.now().millisecondsSinceEpoch.toString(),
      author: displayName,
      authorKey: displayName.isEmpty ? 'self' : displayName.toLowerCase(),
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
    _enqueuePublishObjects(text, attachments, mentions);
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

  void _enqueuePublishObjects(
    String text,
    List<Attachment> attachments,
    List<String> mentions,
  ) {
    final bytes = attachments.map((a) => a.bytes).toList();
    final mimes = attachments.map((a) => a.mime).toList();
    _publisher.buildPostWithAttachments(text, bytes, mimes, mentions).then((
      batch,
    ) {
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

  List<String> _extractMentions(String text) {
    final matches = _mentionPattern.allMatches(text);
    final mentions = <String>{};
    for (final match in matches) {
      final value = match.group(1);
      if (value != null && value.length == 64) {
        mentions.add(value.toLowerCase());
      }
    }
    return mentions.toList();
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
          authorKey: '',
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

  TrustTier trustTierFor(String authorKey) {
    if (authorKey.isEmpty) return TrustTier.unknown;
    return _wotPolicy.classify(authorKey);
  }

  bool isBlocked(String authorKey) =>
      authorKey.isNotEmpty &&
      _wotPolicy.classify(authorKey) == TrustTier.blocked;

  void followUser(String authorKey) {
    if (authorKey.isEmpty) return;
    _wotPolicy.trust(authorKey);
    _persistPrefs();
    _events.insert(0, 'Followed $authorKey');
    _notify();
  }

  void muteUser(String authorKey) {
    if (authorKey.isEmpty) return;
    _wotPolicy.mute(authorKey);
    _persistPrefs();
    _events.insert(0, 'Muted $authorKey');
    _notify();
  }

  void blockUser(String authorKey) {
    if (authorKey.isEmpty) return;
    _wotPolicy.block(authorKey);
    _persistPrefs();
    _events.insert(0, 'Blocked $authorKey');
    _notify();
  }

  void unmuteUser(String authorKey) {
    if (authorKey.isEmpty) return;
    _wotPolicy.unmute(authorKey);
    _persistPrefs();
    _notify();
  }

  void unblockUser(String authorKey) {
    if (authorKey.isEmpty) return;
    _wotPolicy.unblock(authorKey);
    _persistPrefs();
    _notify();
  }

  void unfollowUser(String authorKey) {
    if (authorKey.isEmpty) return;
    _wotPolicy.untrust(authorKey);
    _persistPrefs();
    _notify();
  }

  void _startEpochTimer() {
    void update() {
      const epochSeconds = 86400;
      final nowSeconds =
          (DateTime.now().millisecondsSinceEpoch ~/ 1000) + _clockSkewSeconds;
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
