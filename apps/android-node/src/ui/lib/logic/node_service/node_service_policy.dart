part of '../node_service.dart';

extension NodeServicePolicy on NodeService {
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
      _endBusyOperation('Subscribe');
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
      _endBusyOperation('Unsubscribe');
    }
  }

  Future<void> updatePolicyAction(String action, String pubkeyHex) async {
    final value = pubkeyHex.trim();
    if (value.isEmpty) {
      _setState(_state.copyWith(lastError: 'Pubkey is empty'));
      return;
    }
    if (!_beginBusyOperation('Policy update')) return;
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/policy/$action'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'pubkey_hex': value}),
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
      _endBusyOperation('Policy update');
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
      _endBusyOperation('Follow');
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
      _endBusyOperation('Mute');
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
      _endBusyOperation('Block');
    }
    return false;
  }

  Future<void> unblockPubkey(String pubkeyHex) async {
    await updatePolicyAction('unblock', pubkeyHex);
  }

  Future<Map<String, dynamic>?> explainPolicy(String pubkeyHex) async {
    final value = pubkeyHex.trim();
    if (value.isEmpty) {
      _setState(_state.copyWith(lastError: 'Pubkey is empty'));
      return null;
    }
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/policy/explain'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'pubkey_hex': value}),
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
}
