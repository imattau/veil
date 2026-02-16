part of '../node_service.dart';

extension NodeServiceParsing on NodeService {
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
}
