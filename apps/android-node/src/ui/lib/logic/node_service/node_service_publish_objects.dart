part of '../node_service.dart';

extension NodeServicePublishObjects on NodeService {
  Future<bool> publishRaw({required String payload, int namespace = 32}) async {
    final text = payload.trim();
    if (text.isEmpty) {
      _setState(_state.copyWith(lastError: 'Payload is empty'));
      return false;
    }
    if (!_beginBusyOperation('Publish payload')) return false;
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/publish'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'payload': text}),
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
      _endBusyOperation('Publish payload');
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
      final encodedMediaRoots = <List<int>>[];
      for (final root in mediaRoots) {
        final encodedRoot = _hexRootToBytes(root);
        if (encodedRoot == null) {
          _setState(
            _state.copyWith(
              lastError: 'Post failed: invalid media root `$root`',
            ),
          );
          return false;
        }
        encodedMediaRoots.add(encodedRoot);
      }
      final encodedReplyToRoot = _hexRootToBytes(replyToRoot);
      if (replyToRoot != null && encodedReplyToRoot == null) {
        _setState(
          _state.copyWith(
            lastError: 'Post failed: invalid reply root `$replyToRoot`',
          ),
        );
        return false;
      }
      final bundle = {
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
        'channel_id': channelId,
        'author_pubkey_hex': _state.identityHex ?? '',
        'text': text,
        'media_roots': encodedMediaRoots,
        'reply_to_root': encodedReplyToRoot,
      };

      final response = await _postJsonWithAuthRetry(
        '/post',
        body: {'namespace': namespace, 'bundle': bundle},
      );

      if (response.statusCode >= 200 && response.statusCode < 300) {
        return true;
      } else {
        _setState(
          _state.copyWith(lastError: _formatHttpError('Post failed', response)),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Post failed: $err'));
    } finally {
      _endBusyOperation('Publish post');
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
      _endBusyOperation('Publish poll');
    }
    return false;
  }

  Future<bool> publishPollVote({
    required String pollRoot,
    required int optionIndex,
    String channelId = 'general',
    int namespace = 32,
  }) async {
    final pollRootBytes = _hexRootToBytes(pollRoot);
    if (pollRootBytes == null) {
      _setState(
        _state.copyWith(lastError: 'Poll vote failed: invalid poll root'),
      );
      return false;
    }
    if (!_beginBusyOperation('Publish poll vote')) return false;
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
      _endBusyOperation('Publish poll vote');
    }
    return false;
  }

  Future<bool> publishDeletion({
    required List<String> targetRoots,
    String? reason,
    int namespace = 32,
  }) async {
    if (!_beginBusyOperation('Publish deletion')) return false;
    try {
      final encodedTargetRoots = targetRoots
          .map(_hexRootToBytes)
          .whereType<List<int>>()
          .toList();
      final bundle = {
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
        'author_pubkey_hex': _state.identityHex ?? '',
        'target_roots': encodedTargetRoots,
        'reason': reason,
      };

      final response = await _client
          .post(
            Uri.parse('$_baseUrl/deletion'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'namespace': namespace, 'bundle': bundle}),
          )
          .timeout(const Duration(seconds: 4));

      if (response.statusCode >= 200 && response.statusCode < 300) {
        return true;
      } else {
        _setState(
          _state.copyWith(lastError: 'Deletion failed: ${response.statusCode}'),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Deletion failed: $err'));
    } finally {
      _endBusyOperation('Publish deletion');
    }
    return false;
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
      _endBusyOperation('Publish reaction');
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
      _endBusyOperation('Publish reaction');
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
      _endBusyOperation('Publish boost');
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
      _endBusyOperation('Publish boost');
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
      _endBusyOperation('Publish zap');
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
      _endBusyOperation('Publish zap');
    }
    return false;
  }
}
