import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:http/http.dart' as http;
import 'package:web_socket_channel/io.dart';
import 'package:web_socket_channel/status.dart' as ws_status;

import '../models/node_event.dart';
import '../models/node_state.dart';

class NodeService extends ChangeNotifier {
  NodeState _state = NodeState.initial();
  final MethodChannel _channel = const MethodChannel('veil/node_service');
  final http.Client _client = http.Client();
  IOWebSocketChannel? _eventsChannel;
  StreamSubscription? _eventsSub;
  final List<NodeEvent> _events = [];
  final List<NodeEvent> _feedEvents = [];

  List<NodeEvent> get events => List.unmodifiable(_events);
  List<NodeEvent> get feedEvents => List.unmodifiable(_feedEvents);

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
      await refresh();
      await connectEvents();
    }
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
    }
  }

  Future<void> connectEvents() async {
    if (_eventsChannel != null) {
      return;
    }
    final uri = Uri.parse('ws://127.0.0.1:7788/events');
    try {
      _eventsChannel = IOWebSocketChannel.connect(
        uri,
        headers: _authHeader,
      );
      _eventsSub = _eventsChannel!.stream.listen(
        _handleEventMessage,
        onError: (err) {
          _setState(_state.copyWith(lastError: 'WS error: $err'));
        },
        onDone: () {
          _eventsChannel = null;
        },
      );
    } catch (err) {
      _setState(_state.copyWith(lastError: 'WS connect failed: $err'));
    }
  }

  Future<void> disconnectEvents() async {
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
          .post(
            Uri.parse('$_baseUrl/identity/rotate'),
            headers: _authHeader,
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode >= 200 && response.statusCode < 300) {
        final payload = jsonDecode(response.body);
        if (payload is Map<String, dynamic>) {
          final identity = payload['public_key_hex'] as String?;
          _setState(_state.copyWith(identityHex: identity));
        }
      } else {
        _setState(_state.copyWith(
          lastError: 'Rotate failed: ${response.statusCode}',
        ));
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Rotate failed: $err'));
    } finally {
      _setState(_state.copyWith(busy: false));
      await refresh();
    }
  }

  Future<void> refresh() async {
    final health = await _getJson('/health');
    final identity = await _getJson('/identity');
    final status = await _getJson('/status');
    final policy = await _getJson('/policy');
    final lastError = _state.lastError;

    _setState(
      _state.copyWith(
        healthPayload: health ?? _state.healthPayload,
        identityHex: identity?['public_key_hex'] as String? ?? _state.identityHex,
        statusPayload: status ?? _state.statusPayload,
        policySummary: policy ?? _state.policySummary,
        lastError: lastError,
        lastUpdated: DateTime.now(),
      ),
    );
  }

  Future<void> publishRaw({
    required String payload,
    int namespace = 32,
  }) async {
    if (_state.busy) return;
    if (payload.trim().isEmpty) {
      _setState(_state.copyWith(lastError: 'Payload is empty'));
      return;
    }
    _setState(_state.copyWith(busy: true, lastError: _state.lastError));
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/publish'),
            headers: {
              'content-type': 'application/json',
              ..._authHeader,
            },
            body: jsonEncode({'namespace': namespace, 'payload': payload}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode < 200 || response.statusCode >= 300) {
        _setState(_state.copyWith(
          lastError: 'Publish failed: ${response.statusCode}',
        ));
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Publish failed: $err'));
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
    _setState(_state.copyWith(busy: true, lastError: _state.lastError));
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/policy/$action'),
            headers: {
              'content-type': 'application/json',
              ..._authHeader,
            },
            body: jsonEncode({'pubkey_hex': pubkeyHex}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode < 200 || response.statusCode >= 300) {
        _setState(_state.copyWith(
          lastError: 'Policy update failed: ${response.statusCode}',
        ));
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
            headers: {
              'content-type': 'application/json',
              ..._authHeader,
            },
            body: jsonEncode({'pubkey_hex': pubkeyHex}),
          )
          .timeout(const Duration(seconds: 4));
      if (response.statusCode < 200 || response.statusCode >= 300) {
        _setState(_state.copyWith(
          lastError: 'Policy explain failed: ${response.statusCode}',
        ));
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

  Future<Map<String, dynamic>?> _getJson(String path) async {
    final uri = Uri.parse('$_baseUrl$path');
    try {
      final response = await _client
          .get(uri, headers: _authHeader)
          .timeout(const Duration(seconds: 4));
      if (response.statusCode < 200 || response.statusCode >= 300) {
        _setState(_state.copyWith(
          lastError: 'HTTP ${response.statusCode} on $path',
        ));
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
      _setState(_state.copyWith(
        running: running,
        lastError: error,
        authToken: token ?? _state.authToken,
      ));
    }
  }

  void _handleEventMessage(dynamic message) {
    if (message is! String) return;
    try {
      final payload = jsonDecode(message);
      if (payload is Map<String, dynamic>) {
        final event = NodeEvent.fromJson(payload);
        _events.insert(0, event);
        if (_events.length > 50) {
          _events.removeRange(50, _events.length);
        }
        if (event.event == 'feed_bundle') {
          _feedEvents.insert(0, event);
          if (_feedEvents.length > 50) {
            _feedEvents.removeRange(50, _feedEvents.length);
          }
        }
        notifyListeners();
      }
    } catch (_) {
      // Ignore malformed messages
    }
  }

  void _setState(NodeState next) {
    _state = next;
    notifyListeners();
  }

  @override
  void dispose() {
    _client.close();
    _eventsSub?.cancel();
    _eventsChannel?.sink.close(ws_status.goingAway);
    super.dispose();
  }
}
