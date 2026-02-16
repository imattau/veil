part of '../node_service.dart';

extension NodeServicePublishMessaging on NodeService {
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
      _endBusyOperation('Publish direct message');
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
      _endBusyOperation('Publish group message');
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
      _endBusyOperation('Share group key');
    }
    return false;
  }
}
