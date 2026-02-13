import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:http/http.dart' as http;
import 'package:web_socket_channel/io.dart';
import 'package:web_socket_channel/status.dart' as ws_status;

import './models/node_event.dart';
import './models/node_state.dart';

class NodeService extends ChangeNotifier {
  static const int _maxEvents = 300;
  NodeState _state = NodeState.initial();
  final MethodChannel _channel = const MethodChannel('veil/node_service');
  final http.Client _client = http.Client();
  IOWebSocketChannel? _eventsChannel;
  StreamSubscription? _eventsSub;
  Timer? _poller;
  Timer? _eventsReconnectTimer;
  DateTime? _lastStatusRefresh;
  bool _disposed = false;

  final List<NodeEvent> _events = [];
  final Set<int> _eventSeqs = <int>{};
  final List<NodeEvent> _feedEvents = [];
  final Map<String, NodeEvent> _profiles = {};
  final Map<String, NodeEvent> _latestLists = {};
  final Map<String, NodeEvent> _latestPrefs = {};
  final Map<String, String> _decryptedPayloads = {};
  List<Map<String, dynamic>> _contacts = const [];
  Map<String, List<String>> _policyLists = const {
    'trusted_pubkeys': [],
    'muted_pubkeys': [],
    'blocked_pubkeys': [],
  };

  List<NodeEvent> get events => List.unmodifiable(_events);
  List<NodeEvent> get feedEvents => List.unmodifiable(_feedEvents);
  Map<String, NodeEvent> get profiles => Map.unmodifiable(_profiles);
  Map<String, NodeEvent> get latestLists => Map.unmodifiable(_latestLists);
  Map<String, NodeEvent> get latestPrefs => Map.unmodifiable(_latestPrefs);
  Map<String, String> get decryptedPayloads =>
      Map.unmodifiable(_decryptedPayloads);
  List<Map<String, dynamic>> get contacts => List.unmodifiable(_contacts);
  Map<String, List<String>> get policyLists => Map.unmodifiable(_policyLists);

  NodeState get state => _state;

  String get _baseUrl => 'http://127.0.0.1:7788';

  Map<String, String> get _authHeader {
    final token = _state.authToken;
    if (token == null || token.isEmpty) {
      return const {};
    }
    return {'x-veil-token': token};
  }

  Future<void> start() async {
    if (!_beginBusyOperation('Start')) return;
    var started = false;
    try {
      final result = await _channel.invokeMethod('start');
      _applyServiceResult(result);
      started = true;
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Start failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    if (!started) {
      return;
    }
    await Future.delayed(const Duration(seconds: 2));
    await _refreshServiceStatus();
    if (!_state.running) {
      return;
    }
    await refresh();
    if (_state.identityHex == null || _state.identityHex!.isEmpty) {
      await rotateIdentity();
    }
    await fetchFeed();
    await connectEvents();
    _startPoller();
  }

  void _startPoller() {
    _poller?.cancel();
    _poller = Timer.periodic(const Duration(seconds: 5), (_) => refresh());
  }

  Future<void> stop() async {
    if (!_beginBusyOperation('Stop')) return;
    try {
      final result = await _channel.invokeMethod('stop');
      _applyServiceResult(result);
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Stop failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
      await disconnectEvents();
      _poller?.cancel();
    }
  }

  Future<void> connectEvents() async {
    if (_eventsChannel != null) {
      return;
    }
    await _refreshServiceStatus();
    final uri = Uri.parse('ws://127.0.0.1:7788/events');
    int attempts = 0;
    const maxAttempts = 5;

    while (attempts < maxAttempts) {
      try {
        _eventsChannel = IOWebSocketChannel.connect(uri, headers: _authHeader);
        await _eventsChannel!.ready;
        _eventsSub = _eventsChannel!.stream.listen(
          _handleEventMessage,
          onError: (err) {
            _setState(_state.copyWith(lastError: 'WS error: $err'));
            _eventsChannel = null;
            _scheduleEventsReconnect();
          },
          onDone: () {
            _eventsChannel = null;
            _scheduleEventsReconnect();
          },
        );
        return;
      } catch (err) {
        attempts++;
        if (attempts >= maxAttempts) {
          _setState(
            _state.copyWith(
              lastError: 'WS connect failed after $maxAttempts attempts: $err',
            ),
          );
          _eventsChannel = null;
          return;
        }
        await Future.delayed(Duration(milliseconds: 500 * attempts));
      }
    }
  }

  Future<void> disconnectEvents() async {
    _eventsReconnectTimer?.cancel();
    _eventsReconnectTimer = null;
    await _eventsSub?.cancel();
    _eventsSub = null;
    await _eventsChannel?.sink.close(ws_status.goingAway);
    _eventsChannel = null;
  }

  Future<bool> rotateIdentity() async {
    if (!_beginBusyOperation('Rotate identity')) return false;
    try {
      final response = await _client
          .post(Uri.parse('$_baseUrl/identity/rotate'), headers: _authHeader)
          .timeout(const Duration(seconds: 4));
      if (response.statusCode >= 200 && response.statusCode < 300) {
        final payload = jsonDecode(response.body);
        if (payload is Map<String, dynamic>) {
          final identity = payload['public_key_hex'] as String?;
          _setState(_state.copyWith(identityHex: identity));
          return true;
        }
      } else {
        _setState(
          _state.copyWith(lastError: 'Rotate failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Rotate failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
      await refresh();
    }
    return false;
  }

  Future<void> refresh() async {
    await _refreshServiceStatus();
    try {
      final health = await _getJson('/health');
      final identity = await _getJson('/identity');
      final status = await _getJson('/status');
      final policy = await _getJson('/policy');
      final policyLists = await _getJson('/policy/lists');
      final contactList = await _getJson('/contact');
      final subs = await _getJson('/subscriptions');

      if (policyLists != null) {
        _policyLists = _parsePolicyLists(policyLists);
      }
      if (contactList != null) {
        _contacts = _parseContacts(contactList);
      }

      _setState(
        _state.copyWith(
          healthPayload: health,
          identityHex:
              identity?['public_key_hex'] as String? ?? _state.identityHex,
          statusPayload: status,
          policySummary: policy,
          subscriptions:
              (subs?['subscriptions'] as List?)?.cast<String>() ??
              _state.subscriptions,
          lastUpdated: DateTime.now(),
        ),
      );
      await fetchFeed();
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Refresh failed: $err'));
    }
  }

  Future<void> fetchFeed() async {
    final result = await _getJson('/feed');
    var changed = false;
    if (result != null && result['events'] is List) {
      final list = result['events'] as List;
      debugPrint('[NodeService] Fetched ${list.length} events from history');
      for (var item in list) {
        if (item is Map<String, dynamic>) {
          final event = NodeEvent.fromJson(item);
          if (!_insertEvent(event)) {
            continue;
          }
          changed = true;
          if (event.isFeedBundle) {
            _addFeedEvent(event);
          }
        }
      }
    }
    if (changed) {
      notifyListeners();
    }
  }

  Future<bool> publishRaw({required String payload, int namespace = 32}) async {
    if (!_beginBusyOperation('Publish payload')) return false;
    if (payload.trim().isEmpty) {
      _setState(_state.copyWith(lastError: 'Payload is empty'));
      _setState(_state.copyWith(busy: false));
      return false;
    }
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/publish'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'payload': payload}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode >= 200 && response.statusCode < 300) {
        return true;
      } else {
        _setState(
          _state.copyWith(lastError: 'Publish failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Publish failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<bool> publishPost({
    required String text,
    String? replyToRoot,
    List<String> mediaRoots = const [],
    String channelId = 'general',
    int namespace = 32,
  }) async {
    if (!_beginBusyOperation('Publish post')) return false;
    try {
      final encodedMediaRoots = mediaRoots
          .map(_hexRootToBytes)
          .whereType<List<int>>()
          .toList();
      final bundle = {
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
        'channel_id': channelId,
        'author_pubkey_hex': _state.identityHex ?? '',
        'text': text,
        'media_roots': encodedMediaRoots,
        'reply_to_root': _hexRootToBytes(replyToRoot),
      };

      final response = await _client
          .post(
            Uri.parse('$_baseUrl/post'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));

      if (response.statusCode >= 200 && response.statusCode < 300) {
        return true;
      } else {
        _setState(
          _state.copyWith(lastError: 'Post failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Post failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<bool> publishPoll({
    required String question,
    required List<String> options,
    int? endsAtUnixSeconds,
    String channelId = 'general',
    int namespace = 32,
  }) async {
    if (!_beginBusyOperation('Publish poll')) return false;
    try {
      await _refreshServiceStatus();
      final bundle = {
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
        'channel_id': channelId,
        'author_pubkey_hex': _state.identityHex ?? '',
        'question': question,
        'options': options,
        'ends_at': endsAtUnixSeconds,
      };
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/poll'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode >= 200 && response.statusCode < 300) {
        return true;
      } else {
        _setState(
          _state.copyWith(lastError: 'Poll failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Poll failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<bool> publishPollVote({
    required String pollRoot,
    required int optionIndex,
    String channelId = 'general',
    int namespace = 32,
  }) async {
    if (!_beginBusyOperation('Publish poll vote')) return false;
    final pollRootBytes = _hexRootToBytes(pollRoot);
    if (pollRootBytes == null) {
      _setState(
        _state.copyWith(lastError: 'Poll vote failed: invalid poll root'),
      );
      _setState(_state.copyWith(busy: false));
      return false;
    }
    try {
      await _refreshServiceStatus();
      final bundle = {
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
        'channel_id': channelId,
        'author_pubkey_hex': _state.identityHex ?? '',
        'poll_root': pollRootBytes,
        'option_index': optionIndex,
      };
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/poll_vote'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode >= 200 && response.statusCode < 300) {
        return true;
      } else {
        _setState(
          _state.copyWith(
            lastError: 'Poll vote failed: ${response.statusCode}',
          ),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Poll vote failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<bool> subscribeTag(String tag) async {
    final normalized = tag.trim().replaceFirst(RegExp(r'^#'), '');
    if (normalized.isEmpty) {
      _setState(_state.copyWith(lastError: 'Channel is empty'));
      return false;
    }
    if (!_beginBusyOperation('Subscribe')) return false;
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/subscribe'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'tag': normalized}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode < 200 || response.statusCode >= 300) {
        _setState(
          _state.copyWith(
            lastError: 'Subscribe failed: ${response.statusCode}',
          ),
        );
        return false;
      }
      await refresh();
      return true;
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Subscribe failed: $err'));
      return false;
    } finally {
      _setState(_state.copyWith(busy: false));
    }
  }

  Future<bool> unsubscribeTag(String tag) async {
    final normalized = tag.trim().replaceFirst(RegExp(r'^#'), '');
    if (normalized.isEmpty) {
      _setState(_state.copyWith(lastError: 'Channel is empty'));
      return false;
    }
    if (!_beginBusyOperation('Unsubscribe')) return false;
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/unsubscribe'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'tag': normalized}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode < 200 || response.statusCode >= 300) {
        _setState(
          _state.copyWith(
            lastError: 'Unsubscribe failed: ${response.statusCode}',
          ),
        );
        return false;
      }
      await refresh();
      return true;
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Unsubscribe failed: $err'));
      return false;
    } finally {
      _setState(_state.copyWith(busy: false));
    }
  }

  Future<bool> publishReaction({
    required String targetRoot,
    String actionCode = 'like',
    String channelId = 'general',
    int namespace = 32,
  }) async {
    if (!_beginBusyOperation('Publish reaction')) return false;
    final targetRootBytes = _hexRootToBytes(targetRoot);
    if (targetRootBytes == null) {
      _setState(
        _state.copyWith(lastError: 'Reaction failed: invalid target root'),
      );
      _setState(_state.copyWith(busy: false));
      return false;
    }
    try {
      await _refreshServiceStatus();
      final bundle = {
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
        'channel_id': channelId,
        'author_pubkey_hex': _state.identityHex ?? '',
        'target_root': targetRootBytes,
        'action_code': actionCode,
      };
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/reaction'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode >= 200 && response.statusCode < 300) {
        return true;
      } else {
        _setState(
          _state.copyWith(lastError: 'Reaction failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Reaction failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<bool> publishRepost({
    required String targetRoot,
    String? comment,
    String channelId = 'general',
    int namespace = 32,
  }) async {
    if (!_beginBusyOperation('Publish boost')) return false;
    final targetRootBytes = _hexRootToBytes(targetRoot);
    if (targetRootBytes == null) {
      _setState(
        _state.copyWith(lastError: 'Boost failed: invalid target root'),
      );
      _setState(_state.copyWith(busy: false));
      return false;
    }
    try {
      await _refreshServiceStatus();
      final bundle = {
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
        'channel_id': channelId,
        'author_pubkey_hex': _state.identityHex ?? '',
        'target_root': targetRootBytes,
        'comment': comment,
      };
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/repost'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode >= 200 && response.statusCode < 300) {
        return true;
      } else {
        _setState(
          _state.copyWith(lastError: 'Boost failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Boost failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<bool> publishZap({
    required String targetRoot,
    required int amount,
    String channelId = 'general',
    int namespace = 32,
    String? message,
  }) async {
    if (!_beginBusyOperation('Publish zap')) return false;
    final targetRootBytes = _hexRootToBytes(targetRoot);
    if (targetRootBytes == null) {
      _setState(_state.copyWith(lastError: 'Zap failed: invalid target root'));
      _setState(_state.copyWith(busy: false));
      return false;
    }
    try {
      await _refreshServiceStatus();
      final bundle = {
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
        'channel_id': channelId,
        'author_pubkey_hex': _state.identityHex ?? '',
        'amount': amount,
        'unit': 'sats',
        'target_root': targetRootBytes,
        'receipt_proof': null,
        'message': message,
      };
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/zap'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode >= 200 && response.statusCode < 300) {
        return true;
      } else {
        _setState(
          _state.copyWith(lastError: 'Zap failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Zap failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<bool> publishDM({
    required String recipientPubkey,
    required String text,
    String? replyToRoot,
    String channelId = 'dm',
    int namespace = 32,
  }) async {
    if (!_beginBusyOperation('Publish direct message')) return false;
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/direct_message_text'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({
              'namespace': namespace,
              'channel_id': channelId,
              'recipient_pubkey_hex': recipientPubkey,
              'text': text,
              'reply_to_root': _hexRootToBytes(replyToRoot),
            }),
          )
          .timeout(const Duration(seconds: 4));

      if (response.statusCode >= 200 && response.statusCode < 300) {
        return true;
      } else {
        _setState(
          _state.copyWith(lastError: 'DM failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'DM failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<bool> publishGroupMessage({
    required String groupId,
    required String text,
    String? replyToRoot,
    List<String> memberPubkeys = const [],
    String channelId = 'group',
    int namespace = 32,
  }) async {
    if (!_beginBusyOperation('Publish group message')) return false;
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/group_message_text'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({
              'namespace': namespace,
              'channel_id': channelId,
              'group_id': groupId,
              'text': text,
              'reply_to_root': _hexRootToBytes(replyToRoot),
              'member_pubkeys': memberPubkeys,
            }),
          )
          .timeout(const Duration(seconds: 4));

      if (response.statusCode >= 200 && response.statusCode < 300) {
        return true;
      } else {
        _setState(
          _state.copyWith(
            lastError: 'Group message failed: ${response.statusCode}',
          ),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Group message failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<bool> shareGroupKey({
    required String groupId,
    required List<String> memberPubkeys,
    String channelId = 'group',
    bool rotateKey = false,
    int namespace = 32,
  }) async {
    if (!_beginBusyOperation('Share group key')) return false;
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/group_key/share'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({
              'namespace': namespace,
              'channel_id': channelId,
              'group_id': groupId,
              'member_pubkeys': memberPubkeys,
              'rotate_key': rotateKey,
            }),
          )
          .timeout(const Duration(seconds: 4));

      if (response.statusCode >= 200 && response.statusCode < 300) {
        return true;
      } else {
        _setState(
          _state.copyWith(
            lastError: 'Group key share failed: ${response.statusCode}',
          ),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Group key share failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<bool> publishList({
    required String title,
    required String listKind,
    required List<Map<String, dynamic>> items,
    String channelId = 'general',
    int namespace = 32,
  }) async {
    if (!_beginBusyOperation('Publish list')) return false;
    try {
      final bundle = {
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
        'channel_id': channelId,
        'author_pubkey_hex': _state.identityHex ?? '',
        'title': title,
        'list_kind': listKind,
        'items': items,
      };

      final response = await _client
          .post(
            Uri.parse('$_baseUrl/list'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));

      if (response.statusCode >= 200 && response.statusCode < 300) {
        await refresh();
        await fetchFeed();
        return true;
      } else {
        _setState(
          _state.copyWith(
            lastError: 'List publish failed: ${response.statusCode}',
          ),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'List publish failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<bool> publishAppPreferences({
    required String appId,
    required Map<String, dynamic> preferencesJson,
    String channelId = 'general',
    int namespace = 32,
  }) async {
    if (!_beginBusyOperation('Publish preferences')) return false;
    try {
      final bundle = {
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
        'channel_id': channelId,
        'author_pubkey_hex': _state.identityHex ?? '',
        'app_id': appId,
        'settings_json': preferencesJson,
      };

      final response = await _client
          .post(
            Uri.parse('$_baseUrl/app_preferences'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));

      if (response.statusCode >= 200 && response.statusCode < 300) {
        await refresh();
        await fetchFeed();
        return true;
      } else {
        _setState(
          _state.copyWith(
            lastError: 'Preferences publish failed: ${response.statusCode}',
          ),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Preferences publish failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<bool> publishProfile({
    required String displayName,
    required String bio,
    String? lightningAddress,
    String? avatarMediaRoot,
    String channelId = 'general',
    int namespace = 32,
  }) async {
    if (!_beginBusyOperation('Publish profile')) return false;
    try {
      final bundle = {
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
        'channel_id': channelId,
        'author_pubkey_hex': _state.identityHex ?? '',
        'display_name': displayName,
        'bio': bio,
        'avatar_media_root': _hexRootToBytes(avatarMediaRoot),
        'lightning_address': lightningAddress,
      };

      final response = await _client
          .post(
            Uri.parse('$_baseUrl/profile'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));

      if (response.statusCode >= 200 && response.statusCode < 300) {
        // Optimistically update the local cache for immediate UI feedback
        final selfPubkey = _state.identityHex;
        if (selfPubkey != null) {
          final optimisticEvent = NodeEvent.fromJson({
            'seq': -1,
            'event': 'feed_bundle',
            'data': {'kind': 'profile', ...bundle},
          });
          _profiles[selfPubkey] = optimisticEvent;
        }
        await refresh();
        await fetchFeed();
        return true;
      } else {
        _setState(
          _state.copyWith(
            lastError: 'Profile update failed: ${response.statusCode}',
          ),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Profile update failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<String?> uploadMedia(Uint8List bytes) async {
    if (!_beginBusyOperation('Upload media')) return null;
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/publish_object'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({
              'namespace': 32,
              'payload_b64': base64.encode(bytes),
            }),
          )
          .timeout(const Duration(seconds: 10));

      if (response.statusCode >= 200 && response.statusCode < 300) {
        final data = jsonDecode(response.body);
        return data['object_root'] as String?;
      } else {
        String message = 'Upload failed: ${response.statusCode}';
        try {
          final errBody = jsonDecode(response.body);
          if (errBody is Map && errBody['message'] != null) {
            message = '${errBody['message']} (${response.statusCode})';
          }
        } catch (_) {}
        _setState(_state.copyWith(lastError: message));
        return null;
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Upload failed: $err'));
      return null;
    } finally {
      _setState(_state.copyWith(busy: false));
    }
  }

  Future<void> updatePolicyAction(String action, String pubkeyHex) async {
    if (!_beginBusyOperation('Policy update')) return;
    if (pubkeyHex.trim().isEmpty) {
      _setState(_state.copyWith(lastError: 'Pubkey is empty'));
      _setState(_state.copyWith(busy: false));
      return;
    }
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/policy/$action'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'pubkey_hex': pubkeyHex}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode < 200 || response.statusCode >= 300) {
        _setState(
          _state.copyWith(
            lastError: 'Policy update failed: ${response.statusCode}',
          ),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Policy update failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
      await refresh();
    }
  }

  Future<bool> followPubkey(
    String followeePubkeyHex, {
    String channelId = 'general',
    int namespace = 32,
  }) async {
    final value = followeePubkeyHex.trim().toLowerCase();
    if (!_isValidPubkeyHex(value)) {
      _setState(_state.copyWith(lastError: 'Follow failed: invalid pubkey'));
      return false;
    }
    if (!_beginBusyOperation('Follow')) return false;
    try {
      final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
      final bundle = {
        'meta': {'version': 1, 'created_at': now},
        'channel_id': channelId,
        'follower_pubkey_hex': _state.identityHex ?? '',
        'followee_pubkey_hex': value,
        'at_step': now,
      };
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/follow'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode >= 200 && response.statusCode < 300) {
        await refresh();
        return true;
      } else {
        _setState(
          _state.copyWith(lastError: 'Follow failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Follow failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<void> unfollowPubkey(String pubkeyHex) async {
    await updatePolicyAction('untrust', pubkeyHex);
  }

  Future<bool> mutePubkey(
    String mutedPubkeyHex, {
    String channelId = 'general',
    String? reason,
    int namespace = 32,
  }) async {
    final value = mutedPubkeyHex.trim().toLowerCase();
    if (!_isValidPubkeyHex(value)) {
      _setState(_state.copyWith(lastError: 'Mute failed: invalid pubkey'));
      return false;
    }
    if (!_beginBusyOperation('Mute')) return false;
    try {
      final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
      final bundle = {
        'meta': {'version': 1, 'created_at': now},
        'channel_id': channelId,
        'muter_pubkey_hex': _state.identityHex ?? '',
        'muted_pubkey_hex': value,
        'reason': reason,
        'at_step': now,
      };
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/mute'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode >= 200 && response.statusCode < 300) {
        await refresh();
        return true;
      } else {
        _setState(
          _state.copyWith(lastError: 'Mute failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Mute failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<void> unmutePubkey(String pubkeyHex) async {
    await updatePolicyAction('unmute', pubkeyHex);
  }

  Future<bool> blockPubkey(
    String blockedPubkeyHex, {
    String channelId = 'general',
    String? reason,
    int namespace = 32,
  }) async {
    final value = blockedPubkeyHex.trim().toLowerCase();
    if (!_isValidPubkeyHex(value)) {
      _setState(_state.copyWith(lastError: 'Block failed: invalid pubkey'));
      return false;
    }
    if (!_beginBusyOperation('Block')) return false;
    try {
      final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
      final bundle = {
        'meta': {'version': 1, 'created_at': now},
        'channel_id': channelId,
        'blocker_pubkey_hex': _state.identityHex ?? '',
        'blocked_pubkey_hex': value,
        'reason': reason,
        'at_step': now,
      };
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/block'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode >= 200 && response.statusCode < 300) {
        await refresh();
        return true;
      } else {
        _setState(
          _state.copyWith(lastError: 'Block failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Block failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<void> unblockPubkey(String pubkeyHex) async {
    await updatePolicyAction('unblock', pubkeyHex);
  }

  Future<Map<String, dynamic>?> explainPolicy(String pubkeyHex) async {
    if (pubkeyHex.trim().isEmpty) {
      _setState(_state.copyWith(lastError: 'Pubkey is empty'));
      return null;
    }
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/policy/explain'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'pubkey_hex': pubkeyHex}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode < 200 || response.statusCode >= 300) {
        _setState(
          _state.copyWith(
            lastError: 'Policy explain failed: ${response.statusCode}',
          ),
        );
        return null;
      }
      final payload = jsonDecode(response.body);
      if (payload is Map<String, dynamic>) {
        return payload;
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Policy explain failed: $err'));
    }
    return null;
  }

  Future<Map<String, dynamic>?> exportIdentity() async {
    final result = await _getJson('/identity/export');
    if (result != null) {
      _setState(_state.copyWith(hasBackedUp: true));
    }
    return result;
  }

  Future<Map<String, dynamic>?> fetchObject(String root) async {
    return await _getJson('/object/$root');
  }

  Future<void> importIdentity(String secretKeyHex) async {
    if (!_beginBusyOperation('Import identity')) return;
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/identity/import'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'secret_key_hex': secretKeyHex}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode >= 200 && response.statusCode < 300) {
        await refresh();
      } else {
        _setState(
          _state.copyWith(lastError: 'Import failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Import failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
  }

  Future<bool> saveContact({
    required String peerId,
    String? wsUrl,
    String? quicAddr,
    String? pubkeyHex,
    String? rpcUrl,
  }) async {
    final normalizedPeerId = peerId.trim();
    if (normalizedPeerId.isEmpty) {
      _setState(_state.copyWith(lastError: 'Contact failed: peer id is empty'));
      return false;
    }
    final normalizedPubkey = (pubkeyHex ?? '').trim().toLowerCase();
    if (normalizedPubkey.isNotEmpty && !_isValidPubkeyHex(normalizedPubkey)) {
      _setState(_state.copyWith(lastError: 'Contact failed: invalid pubkey'));
      return false;
    }
    if (!_beginBusyOperation('Save contact')) return false;
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/contact'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({
              'contact': {
                'peer_id': normalizedPeerId,
                'ws_url': (wsUrl ?? '').trim().isEmpty ? null : wsUrl!.trim(),
                'quic_addr': (quicAddr ?? '').trim().isEmpty
                    ? null
                    : quicAddr!.trim(),
                'pubkey_hex': normalizedPubkey,
                'rpc_url': (rpcUrl ?? '').trim().isEmpty
                    ? null
                    : rpcUrl!.trim(),
                'lan_addrs': const <String>[],
              },
            }),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode >= 200 && response.statusCode < 300) {
        await refresh();
        return true;
      } else {
        _setState(
          _state.copyWith(
            lastError: 'Contact save failed: ${response.statusCode}',
          ),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Contact save failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<bool> deleteContact(String peerId) async {
    final normalizedPeerId = peerId.trim();
    if (normalizedPeerId.isEmpty) {
      _setState(
        _state.copyWith(lastError: 'Contact delete failed: peer id empty'),
      );
      return false;
    }
    if (!_beginBusyOperation('Delete contact')) return false;
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/contact/delete'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'peer_id': normalizedPeerId}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode >= 200 && response.statusCode < 300) {
        await refresh();
        return true;
      } else {
        _setState(
          _state.copyWith(
            lastError: 'Contact delete failed: ${response.statusCode}',
          ),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Contact delete failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
    return false;
  }

  Future<Map<String, dynamic>?> _getJson(String path) async {
    await _refreshServiceStatus();
    final uri = Uri.parse('$_baseUrl$path');
    try {
      var response = await _client
          .get(uri, headers: _authHeader)
          .timeout(const Duration(seconds: 4));
      if (response.statusCode == 401 || response.statusCode == 403) {
        await _refreshServiceStatus();
        response = await _client
            .get(uri, headers: _authHeader)
            .timeout(const Duration(seconds: 4));
      }
      if (response.statusCode < 200 || response.statusCode >= 300) {
        _setState(
          _state.copyWith(lastError: 'HTTP ${response.statusCode} on $path'),
        );
        return null;
      }
      final payload = jsonDecode(response.body);
      if (payload is Map<String, dynamic>) {
        return payload;
      }
      return null;
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Request failed: $err'));
      return null;
    }
  }

  void _applyServiceResult(dynamic result) {
    if (result is Map) {
      final running = result['running'] == true;
      final error = result['error'] as String?;
      final token = result['token'] as String?;
      _setState(
        _state.copyWith(
          running: running,
          lastError: error,
          authToken: token ?? _state.authToken,
        ),
      );
    }
  }

  Future<void> _refreshServiceStatus() async {
    final now = DateTime.now();
    if (_lastStatusRefresh != null &&
        now.difference(_lastStatusRefresh!) < const Duration(seconds: 1)) {
      return;
    }
    _lastStatusRefresh = now;
    try {
      final result = await _channel.invokeMethod('status');
      if (result is Map) {
        _applyServiceResult(result);
      }
    } catch (_) {
      // Best effort; HTTP/WS calls will report actionable errors.
    }
  }

  void _scheduleEventsReconnect() {
    if (_disposed || _eventsChannel != null) {
      return;
    }
    if (_eventsReconnectTimer != null) {
      return;
    }
    _eventsReconnectTimer = Timer(const Duration(seconds: 1), () async {
      _eventsReconnectTimer = null;
      if (_disposed || _eventsChannel != null) {
        return;
      }
      await connectEvents();
    });
  }

  void _handleEventMessage(dynamic message) {
    if (message is! String) return;
    try {
      final payload = jsonDecode(message);
      if (payload is Map<String, dynamic>) {
        final event = NodeEvent.fromJson(payload);
        debugPrint(
          '[NodeService] Received event: ${event.event} (seq: ${event.seq})',
        );

        if (!_insertEvent(event)) {
          return;
        }

        if (event.isFeedBundle) {
          debugPrint(
            '[NodeService] Processing feed bundle: ${event.bundleKind}',
          );
          debugPrint('[NodeService] Raw bundle data: ${event.data}');
          _addFeedEvent(event);
        }

        if (event.isPayload) {
          final root = event.data['object_root'] as String?;
          final text = event.decryptedText;
          if (root != null && text != null) {
            _decryptedPayloads[root] = text;
          }
        }

        if (event.event == 'publish_failed') {
          final attempts = (event.data['attempts'] as num?)?.toInt();
          final dropped = event.data['dropped'] == true;
          final retryAfterMs = (event.data['retry_after_ms'] as num?)?.toInt();
          final message = dropped
              ? 'Publish dropped after retries'
              : 'Publish failed${attempts != null ? ' (attempt $attempts)' : ''}${retryAfterMs != null ? ', retry in ${retryAfterMs}ms' : ''}';
          _setState(_state.copyWith(lastError: message));
        }

        notifyListeners();
      }
    } catch (e) {
      debugPrint('[NodeService] Error parsing event: $e');
    }
  }

  void _addFeedEvent(NodeEvent event) {
    if (_feedEvents.any((e) => e.seq == event.seq)) {
      debugPrint('[NodeService] Skipping duplicate event seq: ${event.seq}');
      return;
    }

    _feedEvents.add(event);
    _feedEvents.sort((a, b) => b.seq.compareTo(a.seq));
    debugPrint('[NodeService] Feed updated. New size: ${_feedEvents.length}');

    if (_feedEvents.length > 500) {
      _feedEvents.removeLast();
    }

    if (event.isProfile) {
      final pubkey = event.authorPubkey;
      if (pubkey != null) {
        final existing = _profiles[pubkey];
        if (existing == null || event.seq > existing.seq) {
          _profiles[pubkey] = event;
          debugPrint('[NodeService] Cached profile for $pubkey');
        }
      }
    }

    if (event.isList) {
      final pubkey = event.authorPubkey;
      final kind = event.listKind;
      if (pubkey != null && kind != null) {
        final key = '$pubkey:$kind';
        final existing = _latestLists[key];
        if (existing == null || event.seq > existing.seq) {
          _latestLists[key] = event;
          debugPrint('[NodeService] Cached list $kind for $pubkey');
        }
      }
    }

    if (event.isAppPreferences) {
      final pubkey = event.authorPubkey;
      final appId = event.appId;
      if (pubkey != null && appId != null) {
        final key = '$pubkey:$appId';
        final existing = _latestPrefs[key];
        if (existing == null || event.seq > existing.seq) {
          _latestPrefs[key] = event;
          debugPrint('[NodeService] Cached preferences for $appId by $pubkey');
        }
      }
    }
  }

  List<NodeEvent> getReactionsFor(String objectRoot) {
    return _feedEvents
        .where((e) => e.isReaction && e.targetRoot == objectRoot)
        .toList();
  }

  List<NodeEvent> getRepostsFor(String objectRoot) {
    return _feedEvents
        .where((e) => e.isRepost && e.targetRoot == objectRoot)
        .toList();
  }

  @visibleForTesting
  void testInjectEvent(Map<String, dynamic> json) {
    final event = NodeEvent.fromJson(json);
    if (!_insertEvent(event)) {
      return;
    }
    if (event.isFeedBundle) {
      _addFeedEvent(event);
    }
    if (event.isPayload) {
      final root = event.data['object_root'] as String?;
      final text = event.decryptedText;
      if (root != null && text != null) {
        _decryptedPayloads[root] = text;
      }
    }
    notifyListeners();
  }

  List<int>? _hexRootToBytes(String? root) {
    if (root == null) return null;
    final value = root.trim();
    if (value.length != 64) return null;
    final bytes = <int>[];
    for (var i = 0; i < value.length; i += 2) {
      final part = value.substring(i, i + 2);
      final parsed = int.tryParse(part, radix: 16);
      if (parsed == null) {
        return null;
      }
      bytes.add(parsed);
    }
    return bytes;
  }

  bool _isValidPubkeyHex(String value) {
    return RegExp(r'^[0-9a-fA-F]{64}$').hasMatch(value);
  }

  Map<String, List<String>> _parsePolicyLists(Map<String, dynamic> json) {
    List<String> readList(String key) {
      final raw = json[key];
      if (raw is! List) return const [];
      return raw
          .whereType<String>()
          .map((entry) => entry.trim().toLowerCase())
          .where(_isValidPubkeyHex)
          .toSet()
          .toList()
        ..sort();
    }

    return {
      'trusted_pubkeys': readList('trusted_pubkeys'),
      'muted_pubkeys': readList('muted_pubkeys'),
      'blocked_pubkeys': readList('blocked_pubkeys'),
    };
  }

  List<Map<String, dynamic>> _parseContacts(Map<String, dynamic> json) {
    final raw = json['contacts'];
    if (raw is! List) return const [];
    return raw
        .whereType<Map>()
        .map((entry) => Map<String, dynamic>.from(entry))
        .toList();
  }

  void clearError() {
    _setState(_state.copyWith(lastError: null));
  }

  bool _beginBusyOperation(String operation) {
    if (_state.busy) {
      _setState(
        _state.copyWith(
          lastError: '$operation skipped: another operation is in progress',
        ),
      );
      return false;
    }
    _setState(_state.copyWith(busy: true, lastError: _state.lastError));
    return true;
  }

  bool _insertEvent(NodeEvent event) {
    if (_eventSeqs.contains(event.seq)) {
      return false;
    }
    _events.insert(0, event);
    _eventSeqs.add(event.seq);
    if (_events.length > _maxEvents) {
      final removed = _events.removeLast();
      _eventSeqs.remove(removed.seq);
    }
    return true;
  }

  void _setState(NodeState next) {
    if (_state == next) return;
    _state = next;
    notifyListeners();
  }

  @visibleForTesting
  void testSetIdentity(String pubkey) {
    _setState(_state.copyWith(identityHex: pubkey));
  }

  @override
  void dispose() {
    _disposed = true;
    _client.close();
    _eventsReconnectTimer?.cancel();
    _eventsSub?.cancel();
    _eventsChannel?.sink.close(ws_status.goingAway);
    _poller?.cancel();
    super.dispose();
  }
}
