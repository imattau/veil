part of '../node_service.dart';

extension NodeServiceIdentityContacts on NodeService {
  Future<bool> rotateIdentity() async {
    if (!_beginBusyOperation('Rotate identity', exclusive: true)) return false;
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
      _endBusyOperation('Rotate identity');
      await refresh();
    }
    return false;
  }

  Future<Map<String, dynamic>?> exportIdentity() async {
    final result = await _getJson('/identity/export');
    if (result != null) {
      _setState(_state.copyWith(hasBackedUp: true));
    }
    return result;
  }

  Future<Map<String, dynamic>?> fetchObject(
    String root, {
    bool reportErrors = false,
  }) async {
    return await _getJson(
      '/object/$root',
      reportErrors: reportErrors,
      suppressStatusCodes: const <int>{404},
    );
  }

  Future<void> importIdentity(String secretKeyHex) async {
    final value = secretKeyHex.trim();
    if (value.isEmpty) {
      _setState(
        _state.copyWith(lastError: 'Import failed: secret key is empty'),
      );
      return;
    }
    if (!_beginBusyOperation('Import identity', exclusive: true)) return;
    try {
      final response = await _client
          .post(
            Uri.parse('$_baseUrl/identity/import'),
            headers: {'content-type': 'application/json', ..._authHeader},
            body: jsonEncode({'secret_key_hex': value}),
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
      _endBusyOperation('Import identity');
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
      _endBusyOperation('Save contact');
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
      _endBusyOperation('Delete contact');
    }
    return false;
  }
}
