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
  NodeState _state = NodeState.initial();
  final MethodChannel _channel = const MethodChannel('veil/node_service');
  final http.Client _client = http.Client();
  IOWebSocketChannel? _eventsChannel;
  StreamSubscription? _eventsSub;
  Timer? _poller;
  Timer? _eventsReconnectTimer;
  bool _disposed = false;

  final List<NodeEvent> _events = [];
  final List<NodeEvent> _feedEvents = [];
  final Map<String, NodeEvent> _profiles = {};
  final Map<String, String> _decryptedPayloads = {};

  List<NodeEvent> get events => List.unmodifiable(_events);
  List<NodeEvent> get feedEvents => List.unmodifiable(_feedEvents);
  Map<String, NodeEvent> get profiles => Map.unmodifiable(_profiles);
  Map<String, String> get decryptedPayloads =>
      Map.unmodifiable(_decryptedPayloads);

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
    if (_state.busy) return;
    _setState(_state.copyWith(busy: true, lastError: _state.lastError));
    try {
      final result = await _channel.invokeMethod('start');
      _applyServiceResult(result);
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Start failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
      await Future.delayed(const Duration(seconds: 2));
      await _refreshServiceStatus();
      await refresh();

      if (_state.identityHex == null || _state.identityHex!.isEmpty) {
        await rotateIdentity();
      }

      await fetchFeed();
      await connectEvents();
      _startPoller();
    }
  }

  void _startPoller() {
    _poller?.cancel();
    _poller = Timer.periodic(const Duration(seconds: 5), (_) => refresh());
  }

  Future<void> stop() async {
    if (_state.busy) return;
    _setState(_state.copyWith(busy: true, lastError: _state.lastError));
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

  Future<void> rotateIdentity() async {
    if (_state.busy) return;
    _setState(_state.copyWith(busy: true, lastError: _state.lastError));
    try {
      final response = await _client
          .post(Uri.parse('$_baseUrl/identity/rotate'), headers: _authHeader)
          .timeout(const Duration(seconds: 4));
      if (response.statusCode >= 200 && response.statusCode < 300) {
        final payload = jsonDecode(response.body);
        if (payload is Map<String, dynamic>) {
          final identity = payload['public_key_hex'] as String?;
          _setState(_state.copyWith(identityHex: identity));
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
  }

  Future<void> refresh() async {
    await _refreshServiceStatus();
    try {
      final health = await _getJson('/health');
      final identity = await _getJson('/identity');
      final status = await _getJson('/status');
      final policy = await _getJson('/policy');
      final subs = await _getJson('/subscriptions');

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
    if (result != null && result['events'] is List) {
      final list = result['events'] as List;
      debugPrint('[NodeService] Fetched ${list.length} events from history');
      for (var item in list) {
        if (item is Map<String, dynamic>) {
          final event = NodeEvent.fromJson(item);
          _events.add(event);
          if (event.isFeedBundle) {
            _addFeedEvent(event);
          }
        }
      }
      notifyListeners();
    }
  }

  Future<void> publishRaw({required String payload, int namespace = 32}) async {
    if (_state.busy) return;
    if (payload.trim().isEmpty) {
      _setState(_state.copyWith(lastError: 'Payload is empty'));
      return;
    }
    _setState(_state.copyWith(busy: true));
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/publish'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'payload': payload}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode < 200 || response.statusCode >= 300) {
        _setState(
          _state.copyWith(lastError: 'Publish failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Publish failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
  }

  Future<void> publishPost({
    required String text,
    String? replyToRoot,
    List<String> mediaRoots = const [],
    String channelId = 'general',
    int namespace = 32,
  }) async {
    if (_state.busy) return;
    _setState(_state.copyWith(busy: true));
    try {
      final bundle = {
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
        'channel_id': channelId,
        'author_pubkey_hex': _state.identityHex ?? '',
        'text': text,
        'media_roots': mediaRoots,
        'reply_to_root': replyToRoot,
      };

      final response = await _client
          .post(
            Uri.parse('$_baseUrl/post'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));

      if (response.statusCode < 200 || response.statusCode >= 300) {
        _setState(
          _state.copyWith(lastError: 'Post failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Post failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
  }

  Future<void> publishPoll({
    required String question,
    required List<String> options,
    int? endsAtUnixSeconds,
    String channelId = 'general',
    int namespace = 32,
  }) async {
    if (_state.busy) return;
    _setState(_state.copyWith(busy: true));
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
      if (response.statusCode < 200 || response.statusCode >= 300) {
        _setState(
          _state.copyWith(lastError: 'Poll failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Poll failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
  }

  Future<void> publishPollVote({
    required String pollRoot,
    required int optionIndex,
    String channelId = 'general',
    int namespace = 32,
  }) async {
    if (_state.busy) return;
    _setState(_state.copyWith(busy: true));
    try {
      await _refreshServiceStatus();
      final bundle = {
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
        'channel_id': channelId,
        'author_pubkey_hex': _state.identityHex ?? '',
        'poll_root': pollRoot,
        'option_index': optionIndex,
      };
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/poll_vote'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode < 200 || response.statusCode >= 300) {
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
  }

  Future<bool> subscribeTag(String tag) async {
    final normalized = tag.trim().replaceFirst(RegExp(r'^#'), '');
    if (normalized.isEmpty) {
      _setState(_state.copyWith(lastError: 'Channel is empty'));
      return false;
    }
    if (_state.busy) return false;
    _setState(_state.copyWith(busy: true));
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
    if (_state.busy) return false;
    _setState(_state.copyWith(busy: true));
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

  Future<void> publishReaction({
    required String targetRoot,
    String actionCode = 'like',
    String channelId = 'general',
    int namespace = 32,
  }) async {
    if (_state.busy) return;
    _setState(_state.copyWith(busy: true));
    try {
      await _refreshServiceStatus();
      final bundle = {
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
        'channel_id': channelId,
        'author_pubkey_hex': _state.identityHex ?? '',
        'target_root': targetRoot,
        'action_code': actionCode,
      };
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/reaction'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode < 200 || response.statusCode >= 300) {
        _setState(
          _state.copyWith(lastError: 'Reaction failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Reaction failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
  }

  Future<void> publishRepost({
    required String targetRoot,
    String? comment,
    String channelId = 'general',
    int namespace = 32,
  }) async {
    if (_state.busy) return;
    _setState(_state.copyWith(busy: true));
    try {
      await _refreshServiceStatus();
      final bundle = {
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
        'channel_id': channelId,
        'author_pubkey_hex': _state.identityHex ?? '',
        'target_root': targetRoot,
        'comment': comment,
      };
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/repost'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode < 200 || response.statusCode >= 300) {
        _setState(
          _state.copyWith(lastError: 'Boost failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Boost failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
  }

  Future<void> publishZap({
    required String targetRoot,
    required int amount,
    String channelId = 'general',
    int namespace = 32,
    String? message,
  }) async {
    if (_state.busy) return;
    _setState(_state.copyWith(busy: true));
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
        'target_root': targetRoot,
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
      if (response.statusCode < 200 || response.statusCode >= 300) {
        _setState(
          _state.copyWith(lastError: 'Zap failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Zap failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
  }

  Future<void> publishDM({
    required String recipientPubkey,
    required String text,
    String? replyToRoot,
    String channelId = 'dm',
    int namespace = 32,
  }) async {
    if (_state.busy) return;
    _setState(_state.copyWith(busy: true));
    try {
      // For this simplified UI, we are sending the text to /direct_message.
      // In production, the node would perform E2E encryption before broadcasting.
      final bundle = {
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
        'channel_id': channelId,
        'author_pubkey_hex': _state.identityHex ?? '',
        'recipient_pubkey_hex': recipientPubkey,
        'ciphertext_root':
            'enc_${DateTime.now().millisecondsSinceEpoch}', // Placeholder for actual encryption root
        'reply_to_root': replyToRoot,
      };

      final response = await _client
          .post(
            Uri.parse('$_baseUrl/direct_message'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));

      if (response.statusCode < 200 || response.statusCode >= 300) {
        _setState(
          _state.copyWith(lastError: 'DM failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'DM failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
    }
  }

  Future<void> publishGroupMessage({
    required String groupId,
    required String text,
    String? replyToRoot,
    String channelId = 'group',
    int namespace = 32,
  }) async {
    if (_state.busy) return;
    _setState(_state.copyWith(busy: true));
    try {
      await _refreshServiceStatus();
      final bundle = {
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
        'channel_id': channelId,
        'author_pubkey_hex': _state.identityHex ?? '',
        'group_id': groupId,
        // Placeholder until encrypted message objects are wired.
        'ciphertext_root': 'grp_${DateTime.now().millisecondsSinceEpoch}',
        'reply_to_root': replyToRoot,
      };

      final response = await _client
          .post(
            Uri.parse('$_baseUrl/group_message'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));

      if (response.statusCode < 200 || response.statusCode >= 300) {
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
  }

  Future<void> publishProfile({
    required String displayName,
    required String bio,
    String? lightningAddress,
    String? avatarMediaRoot,
    String channelId = 'general',
    int namespace = 32,
  }) async {
    if (_state.busy) return;
    _setState(_state.copyWith(busy: true));
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
        'avatar_media_root': avatarMediaRoot,
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
          _profiles[selfPubkey] = NodeEvent(
            seq: 999999, // High seq to override historical ones
            event: 'feed_bundle',
            data: bundle,
          );
        }
        await refresh();
        await fetchFeed();
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
  }

  Future<String?> uploadMedia(Uint8List bytes) async {
    if (_state.busy) return null;
    _setState(_state.copyWith(busy: true));
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
        _setState(
          _state.copyWith(lastError: 'Upload failed: ${response.statusCode}'),
        );
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
    if (_state.busy) return;
    if (pubkeyHex.trim().isEmpty) {
      _setState(_state.copyWith(lastError: 'Pubkey is empty'));
      return;
    }
    _setState(_state.copyWith(busy: true));
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
    if (_state.busy) return;
    _setState(_state.copyWith(busy: true));
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

        // Add to main event log (newest first)
        _events.insert(0, event);
        if (_events.length > 100) {
          _events.removeLast();
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

    // Explicitly notify so SocialController sees the new item
    notifyListeners();
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
    _events.insert(0, event);
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

  void clearError() {
    _setState(_state.copyWith(lastError: null));
  }

  void _setState(NodeState next) {
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
