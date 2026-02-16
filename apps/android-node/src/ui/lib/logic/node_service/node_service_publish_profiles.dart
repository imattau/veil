part of '../node_service.dart';

extension NodeServicePublishProfiles on NodeService {
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
      _endBusyOperation('Publish list');
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
      _endBusyOperation('Publish preferences');
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
      final hasAvatar = avatarMediaRoot != null && avatarMediaRoot.isNotEmpty;
      final encodedAvatarRoot = hasAvatar ? _hexRootToBytes(avatarMediaRoot) : null;

      if (hasAvatar && encodedAvatarRoot == null) {
        _setState(
          _state.copyWith(
            lastError: 'Profile update failed: invalid avatar media root',
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
        'display_name': displayName,
        'bio': bio,
        'avatar_media_root': encodedAvatarRoot,
      };

      final response = await _postJsonWithAuthRetry(
        '/profile',
        body: {'namespace': namespace, 'bundle': bundle},
      );

      if (response.statusCode >= 200 && response.statusCode < 300) {
        final selfPubkey = _state.identityHex;
        if (selfPubkey != null) {
          _profiles[selfPubkey] = ProfileData(
            pubkey: selfPubkey,
            displayName: displayName,
            bio: bio,
            avatarMediaRoot: avatarMediaRoot,
            lightningAddress: lightningAddress,
            updatedAt: DateTime.now().millisecondsSinceEpoch ~/ 1000,
          );
        }
        await refresh();
        await fetchFeed();
        return true;
      } else {
        _setState(
          _state.copyWith(
            lastError: _formatHttpError('Profile update failed', response),
          ),
        );
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Profile update failed: $err'));
    } finally {
      _endBusyOperation('Publish profile');
    }
    return false;
  }

  Future<String?> uploadMedia(Uint8List bytes) async {
    if (!_beginBusyOperation('Upload media')) return null;
    try {
      final uploadBytes = _prepareUploadBytes(bytes);
      if (uploadBytes == null) {
        return null;
      }

      final response = await _postJsonWithAuthRetry(
        '/publish_object',
        body: {'namespace': 32, 'payload_b64': base64.encode(uploadBytes)},
        timeout: const Duration(seconds: 20),
      );

      if (response.statusCode >= 200 && response.statusCode < 300) {
        final data = jsonDecode(response.body);
        if (data is Map) {
          final objectRoot = data['object_root'] as String?;
          if (objectRoot != null && _isValidPubkeyHex(objectRoot)) {
            return objectRoot.toLowerCase();
          }
        }
        _setState(
          _state.copyWith(
            lastError: 'Upload failed: invalid object_root returned by node',
          ),
        );
        return null;
      } else {
        _setState(
          _state.copyWith(
            lastError: _formatHttpError('Upload failed', response),
          ),
        );
        return null;
      }
    } catch (err) {
      _setState(_state.copyWith(lastError: 'Upload failed: $err'));
      return null;
    } finally {
      _endBusyOperation('Upload media');
    }
  }

  Uint8List? _prepareUploadBytes(Uint8List source) {
    if (source.length <= NodeService.maxMediaPayloadBytes) {
      return source;
    }

    final decoded = img.decodeImage(source);
    if (decoded == null) {
      _setState(
        _state.copyWith(
          lastError:
              'Upload failed: media too large (${source.length} bytes > ${NodeService.maxMediaPayloadBytes} bytes)',
        ),
      );
      return null;
    }

    const scales = <double>[1.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.33, 0.25];
    const qualities = <int>[85, 75, 65, 55, 45, 35, 25];

    for (final scale in scales) {
      final resized = scale == 1.0
          ? decoded
          : img.copyResize(
              decoded,
              width: (decoded.width * scale).round().clamp(320, decoded.width),
              interpolation: img.Interpolation.average,
            );

      for (final quality in qualities) {
        final encoded = img.encodeJpg(resized, quality: quality);
        if (encoded.length <= NodeService.maxMediaPayloadBytes) {
          return Uint8List.fromList(encoded);
        }
      }
    }

    _setState(
      _state.copyWith(
        lastError:
            'Upload failed: media exceeds ${NodeService.maxMediaPayloadBytes} bytes after compression',
      ),
    );
    return null;
  }
}
