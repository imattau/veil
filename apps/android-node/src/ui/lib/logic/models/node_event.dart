import 'dart:convert';
import 'package:flutter/foundation.dart';

@immutable
class NodeEvent {
  final int seq;
  final String event;
  final Map<String, dynamic> data;

  const NodeEvent({required this.seq, required this.event, required this.data});

  factory NodeEvent.fromJson(Map<String, dynamic> json) {
    final event = json['event'] as String? ?? 'unknown';
    final data = _normalizeEventData(event, json['data']);
    return NodeEvent(
      seq: (json['seq'] as num?)?.toInt() ?? 0,
      event: event,
      data: data,
    );
  }

  bool get isFeedBundle => event == 'feed_bundle';
  bool get isPayload => event == 'payload';
  String? get bundleKind => isFeedBundle ? data['kind'] as String? : null;

  bool get isPost => bundleKind == 'post';
  bool get isReaction => bundleKind == 'reaction';
  bool get isRepost => bundleKind == 'repost';
  bool get isPoll => bundleKind == 'poll';
  bool get isZap => bundleKind == 'zap';
  bool get isProfile => bundleKind == 'profile';
  bool get isLiveStatus => bundleKind == 'live_status';
  bool get isDirectMessage => bundleKind == 'direct_message';
  bool get isGroupMessage => bundleKind == 'group_message';

  String? get authorPubkey => data['author_pubkey_hex'] as String?;
  String? get channelId => data['channel_id'] as String?;
  String? get lightningAddress =>
      isProfile ? data['lightning_address'] as String? : null;
  int? get createdAt => (data['meta']?['created_at'] as num?)?.toInt();

  // Post specific
  String? get postText => isPost ? data['text'] as String? : null;
  List<String> get mediaRoots =>
      (data['media_roots'] as List?)?.cast<String>() ?? [];
  String? get replyToRoot => data['reply_to_root'] as String?;

  // Reaction specific
  String? get reactionAction =>
      isReaction ? data['action_code'] as String? : null;
  String? get targetRoot => data['target_root'] as String?;

  // Repost specific
  String? get repostComment => isRepost ? data['comment'] as String? : null;

  String? get objectRoot => data['object_root'] as String?;

  // Payload specific (decrypted content)
  String? get decryptedText {
    if (!isPayload) return null;
    final b64 = data['payload_b64'] as String?;
    if (b64 == null) return null;
    try {
      return utf8.decode(base64.decode(b64));
    } catch (_) {
      return null;
    }
  }

  // Live status specific
  String? get statusText =>
      isLiveStatus ? data['status_text'] as String? : null;
  String? get statusEmoji => isLiveStatus ? data['emoji'] as String? : null;
}

Map<String, dynamic> _normalizeEventData(String event, dynamic rawData) {
  final data = rawData is Map
      ? Map<String, dynamic>.from(rawData)
      : <String, dynamic>{};
  if (event != 'feed_bundle') {
    return data;
  }
  return _normalizeFeedBundleData(data);
}

Map<String, dynamic> _normalizeFeedBundleData(Map<String, dynamic> data) {
  final kind = data['kind'];
  if (kind is String && kind.isNotEmpty) {
    return data;
  }

  final bundleField = data['bundle'];
  if (bundleField is Map) {
    final normalized = _normalizeFeedBundleData(
      Map<String, dynamic>.from(bundleField),
    );
    final root = data['object_root'];
    if (root is String &&
        root.isNotEmpty &&
        normalized['object_root'] == null) {
      normalized['object_root'] = root;
    }
    return normalized;
  }

  if (data.length == 1) {
    final key = data.keys.first;
    final value = data[key];
    if (value is Map) {
      final normalizedKind = _normalizeBundleKindKey(key);
      if (normalizedKind != null) {
        final flattened = Map<String, dynamic>.from(value);
        flattened['kind'] = normalizedKind;
        return flattened;
      }
    }
  }
  return data;
}

String? _normalizeBundleKindKey(String raw) {
  final known = <String>{
    'profile',
    'media',
    'post',
    'reaction',
    'direct_message',
    'group_message',
    'channel_directory',
    'endorsement',
    'follow',
    'mute',
    'block',
    'namespace_signature_policy',
    'list',
    'group_metadata',
    'zap',
    'app_preferences',
    'deletion',
    'repost',
    'poll',
    'poll_vote',
    'live_status',
  };

  final lower = raw.toLowerCase();
  if (known.contains(lower)) {
    return lower;
  }

  final snake = raw
      .replaceAllMapped(RegExp(r'([a-z0-9])([A-Z])'), (m) => '${m[1]}_${m[2]}')
      .toLowerCase();
  if (known.contains(snake)) {
    return snake;
  }
  return null;
}
