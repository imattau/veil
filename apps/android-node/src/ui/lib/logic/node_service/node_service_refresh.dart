part of '../node_service.dart';

extension NodeServiceRefresh on NodeService {
  Future<void> refresh() async {
    if (!_beginBusyOperation('Refresh')) return;
    try {
      await _refreshServiceStatus();

      final results = await Future.wait([
        _getJson('/health'),
        _getJson('/identity'),
        _getJson('/status'),
        _getJson('/policy'),
        _getJson('/policy/lists'),
        _getJson('/contact'),
        _getJson('/subscriptions'),
        _getJson('/feed'),
      ]);

      final health = results[0];
      final identity = results[1];
      final status = results[2];
      final policy = results[3];
      final policyLists = results[4];
      final contactList = results[5];
      final subs = results[6];
      final feed = results[7];

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

      if (feed != null) {
        _processFeedResult(feed);
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Refresh failed: $err'));
    } finally {
      _endBusyOperation('Refresh');
    }
  }

  Future<void> fetchFeed() async {
    final result = await _getJson('/feed');
    _processFeedResult(result);
  }

  void _processFeedResult(Map<String, dynamic>? result) {
    var changed = false;
    if (result != null && result['events'] is List) {
      final list = result['events'] as List;
      debugPrint('[NodeService] Processing ${list.length} feed events');
      for (var item in list) {
        if (item is Map<String, dynamic>) {
          final event = NodeEvent.fromJson(item);
          if (!_insertEvent(event)) {
            continue;
          }
          changed = true;
          if (event.isFeedBundle) {
            _addFeedEvent(event, skipPrune: true);
          }
        }
      }
    }
    if (changed) {
      _pruneFeedEvents();
      _notifyListeners();
    }
  }

  Future<http.Response> _postJsonWithAuthRetry(
    String path, {
    required Map<String, dynamic> body,
    Duration timeout = const Duration(seconds: 4),
  }) async {
    final uri = Uri.parse('$_baseUrl$path');

    Future<http.Response> send() {
      return _client
          .post(
            uri,
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode(body),
          )
          .timeout(timeout);
    }

    await _refreshServiceStatus();
    var response = await send();
    if (response.statusCode == 401) {
      await _refreshServiceStatus();
      response = await send();
    }
    return response;
  }

  String _formatHttpError(
    String prefix,
    http.Response response, {
    bool includeCode = true,
  }) {
    final status = response.statusCode;
    try {
      final decoded = jsonDecode(response.body);
      if (decoded is Map) {
        final message = decoded['message']?.toString().trim();
        final code = decoded['code']?.toString().trim();
        if (message != null && message.isNotEmpty) {
          final details = <String>[
            if (includeCode && code != null && code.isNotEmpty) code,
            '$status',
          ];
          return '$prefix: $message (${details.join(', ')})';
        }
      }
    } catch (_) {
      // Best effort parse.
    }
    return '$prefix: $status';
  }

  Future<Map<String, dynamic>?> _getJson(
    String path, {
    bool reportErrors = true,
    Set<int> suppressStatusCodes = const <int>{},
  }) async {
    await _refreshServiceStatus();
    final uri = Uri.parse('$_baseUrl$path');
    try {
      var response = await _client
          .get(uri, headers: _authHeader)
          .timeout(const Duration(seconds: 4));

      if (response.statusCode == 401) {
        await _refreshServiceStatus();
        response = await _client
            .get(uri, headers: _authHeader)
            .timeout(const Duration(seconds: 4));
        if (response.statusCode == 401) {
          _setState(
            _state.copyWith(
              lastError: 'Authentication failed for $path',
              isOffline: false,
            ),
          );
          return null;
        }
      }

      if (response.statusCode < 200 || response.statusCode >= 300) {
        if (reportErrors &&
            !suppressStatusCodes.contains(response.statusCode)) {
          _setState(
            _state.copyWith(
              lastError: _formatHttpError('Request failed on $path', response),
              isOffline: false,
            ),
          );
        }
        return null;
      }

      _setState(_state.copyWith(isOffline: false));
      final payload = jsonDecode(response.body);
      if (payload is Map<String, dynamic>) {
        return payload;
      }
      return null;
    } catch (err) {
      final isNetworkError = err is TimeoutException || err is SocketException;
      if (reportErrors) {
        _setState(
          _state.copyWith(
            lastError: isNetworkError
                ? 'Cannot reach node ($err)'
                : 'Request failed: $err',
            isOffline: isNetworkError,
          ),
        );
      } else if (isNetworkError) {
        _setState(_state.copyWith(isOffline: true));
      }
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
}
