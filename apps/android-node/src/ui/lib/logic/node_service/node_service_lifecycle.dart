part of '../node_service.dart';

extension NodeServiceLifecycle on NodeService {
  Future<void> start() async {
    if (!_beginBusyOperation('Start', exclusive: true)) return;
    var started = false;
    try {
      final result = await _channel.invokeMethod('start');
      _applyServiceResult(result);
      started = true;
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Start failed: $err'));
    } finally {
      _endBusyOperation('Start');
    }
    if (!started) {
      return;
    }
    final ready = await _waitForNodeReady();
    if (!ready) {
      return;
    }
    await refresh();
    if (_state.identityHex == null || _state.identityHex!.isEmpty) {
      await rotateIdentity();
    }
    await connectEvents();
    await fetchFeed();
    _startPoller();
  }

  Future<bool> _waitForNodeReady() async {
    const timeout = Duration(seconds: 30);
    const step = Duration(seconds: 1);
    final deadline = DateTime.now().add(timeout);

    while (!_disposed && DateTime.now().isBefore(deadline)) {
      await _refreshServiceStatus();
      final hasToken = (_state.authToken ?? '').isNotEmpty;
      if (_state.running && hasToken) {
        return true;
      }
      await Future.delayed(step);
    }

    _setState(
      _state.copyWith(
        lastError: 'Node startup timed out after ${timeout.inSeconds}s',
      ),
    );
    return false;
  }

  void _startPoller() {
    _poller?.cancel();
    _poller = Timer.periodic(const Duration(seconds: 5), (_) => refresh());
  }

  Future<void> stop() async {
    if (!_beginBusyOperation('Stop', exclusive: true)) return;
    try {
      final result = await _channel.invokeMethod('stop');
      _applyServiceResult(result);
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Stop failed: $err'));
    } finally {
      _endBusyOperation('Stop');
      await disconnectEvents();
      _poller?.cancel();
    }
  }

  Future<void> connectEvents() async {
    if (_disposed) return;
    await disconnectEvents();

    await _refreshServiceStatus();
    final uri = Uri.parse('ws://127.0.0.1:${NodeService.rpcPort}/events');
    int attempts = 0;
    const maxAttempts = 5;

    while (attempts < maxAttempts && !_disposed) {
      final token = ++_eventsConnectionToken;
      IOWebSocketChannel? channel;
      try {
        channel = IOWebSocketChannel.connect(uri, headers: _authHeader);
        await channel.ready;
        if (_disposed || token != _eventsConnectionToken) {
          try {
            await channel.sink.close(ws_status.normalClosure);
          } catch (_) {
            // Best effort closure.
          }
          return;
        }

        _eventsChannel = channel;
        _eventsSub = channel.stream.listen(
          _handleEventMessage,
          onError: (err) {
            _handleEventsStreamClosed(token, error: err);
          },
          onDone: () {
            _handleEventsStreamClosed(token);
          },
          cancelOnError: true,
        );
        _eventsReconnectAttempts = 0;
        return;
      } catch (err) {
        if (token == _eventsConnectionToken) {
          _eventsSub = null;
          _eventsChannel = null;
        }
        attempts++;
        if (attempts >= maxAttempts) {
          _setState(
            _state.copyWith(
              lastError: 'WS connect failed after $maxAttempts attempts: $err',
            ),
          );
          return;
        }
        await Future.delayed(Duration(milliseconds: 500 * attempts));
      }
    }
  }

  Future<void> disconnectEvents() async {
    _eventsReconnectTimer?.cancel();
    _eventsReconnectTimer = null;
    _eventsConnectionToken++;

    final sub = _eventsSub;
    _eventsSub = null;

    final channel = _eventsChannel;
    _eventsChannel = null;

    await sub?.cancel();
    try {
      await channel?.sink.close(ws_status.normalClosure);
    } catch (_) {
      // Best effort closure.
    }
  }

  void _handleEventsStreamClosed(int token, {Object? error}) {
    if (_disposed || token != _eventsConnectionToken) {
      return;
    }

    _eventsSub = null;
    _eventsChannel = null;

    if (error != null) {
      _setState(_state.copyWith(lastError: 'WS error: $error'));
    }

    _scheduleEventsReconnect();
  }

  void _scheduleEventsReconnect() {
    if (_disposed) {
      return;
    }
    if (_eventsSub != null || _eventsChannel != null) {
      return;
    }
    if (_eventsReconnectTimer != null) {
      return;
    }

    _eventsReconnectAttempts++;
    final delaySecs = (1 * (1 << (_eventsReconnectAttempts - 1))).clamp(1, 60);
    debugPrint(
      '[NodeService] Scheduling WS reconnect in ${delaySecs}s (attempt $_eventsReconnectAttempts)',
    );

    _eventsReconnectTimer = Timer(Duration(seconds: delaySecs), () async {
      _eventsReconnectTimer = null;
      if (_disposed || _eventsSub != null || _eventsChannel != null) {
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
          final dropped = event.data['dropped'] == true;
          if (dropped) {
            _setState(
              _state.copyWith(lastError: 'Publish dropped after retries'),
            );
          }
        }

        _notifyListeners();
      }
    } catch (e) {
      debugPrint('[NodeService] Error parsing event: $e');
    }
  }
}
