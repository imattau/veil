import 'dart:convert';
import 'dart:typed_data';
import 'package:flutter/foundation.dart';
import './node_service.dart';
import './models/node_event.dart';

class _OptimisticReaction {
  final String objectRoot;
  final String action;
  final String? authorPubkey;
  final int createdAtMs;

  const _OptimisticReaction({
    required this.objectRoot,
    required this.action,
    required this.authorPubkey,
    required this.createdAtMs,
  });
}

class _OptimisticRepost {
  final String objectRoot;
  final String? authorPubkey;
  final int createdAtMs;

  const _OptimisticRepost({
    required this.objectRoot,
    required this.authorPubkey,
    required this.createdAtMs,
  });
}

class SocialController extends ChangeNotifier {
  final NodeService nodeService;
  final Map<String, Uint8List> imageCache = {};
  final List<_OptimisticReaction> _optimisticReactions = [];
  final List<_OptimisticRepost> _optimisticReposts = [];
  final Map<String, List<NodeEvent>> _optimisticComments = {};
  int _optimisticSeq = -1;

  SocialController(this.nodeService) {
    nodeService.addListener(_onServiceUpdate);
  }

  void _onServiceUpdate() {
    _reconcileOptimisticEvents();
    // Check for profiles with images
    for (var profile in nodeService.profiles.values) {
      final root = profile.data['avatar_media_root'] as String?;
      if (root != null && !imageCache.containsKey(root)) {
        _fetchImage(root);
      }
    }
    // Preload post media referenced by feed bundles.
    for (var event in nodeService.feedEvents) {
      for (var mediaRoot in event.mediaRoots) {
        if (!imageCache.containsKey(mediaRoot)) {
          _fetchImage(mediaRoot);
        }
      }
    }
    notifyListeners();
  }

  Future<void> _fetchImage(String root) async {
    final res = await nodeService.fetchObject(root);
    if (res != null && res['object_b64'] != null) {
      imageCache[root] = base64.decode(res['object_b64']);
      notifyListeners();
    }
  }

  List<NodeEvent> get feed {
    final filtered = nodeService.feedEvents.where((e) {
      // Must be a post, repost, or poll
      if (!e.isPost && !e.isRepost && !e.isPoll) return false;
      // If it's a post, it must NOT be a reply to another post
      if (e.isPost && e.replyToRoot != null) {
        debugPrint('[SocialController] Filtering out comment: ${e.seq}');
        return false;
      }
      return true;
    }).toList();

    debugPrint(
      '[SocialController] Feed filtered: ${filtered.length} posts from ${nodeService.feedEvents.length} events',
    );
    return filtered;
  }

  List<NodeEvent> getReactions(String objectRoot) => [
    ...nodeService.getReactionsFor(objectRoot),
    ...(() {
      final pending = _optimisticReactions
          .where((e) => e.objectRoot == objectRoot)
          .toList();
      final out = <NodeEvent>[];
      for (var i = 0; i < pending.length; i++) {
        final entry = pending[i];
        out.add(
          NodeEvent.fromJson({
            'seq': -1000000 - i,
            'event': 'feed_bundle',
            'data': {
              'kind': 'reaction',
              'target_root': objectRoot,
              'action_code': entry.action,
              'author_pubkey_hex': entry.authorPubkey ?? '',
            },
          }),
        );
      }
      return out;
    })(),
  ];

  List<NodeEvent> getReposts(String objectRoot) => [
    ...nodeService.getRepostsFor(objectRoot),
    ...(() {
      final pending = _optimisticReposts
          .where((e) => e.objectRoot == objectRoot)
          .toList();
      final out = <NodeEvent>[];
      for (var i = 0; i < pending.length; i++) {
        final entry = pending[i];
        out.add(
          NodeEvent.fromJson({
            'seq': -2000000 - i,
            'event': 'feed_bundle',
            'data': {
              'kind': 'repost',
              'target_root': objectRoot,
              'author_pubkey_hex': entry.authorPubkey ?? '',
            },
          }),
        );
      }
      return out;
    })(),
  ];

  List<NodeEvent> getComments(String objectRoot) {
    return [
      ...nodeService.feedEvents.where(
        (e) => e.isPost && e.replyToRoot == objectRoot,
      ),
      ...(_optimisticComments[objectRoot] ?? const []),
    ];
  }

  int getZapTotal(String objectRoot) {
    int total = 0;
    for (var e in nodeService.feedEvents.where(
      (e) => e.isZap && e.targetRoot == objectRoot,
    )) {
      total += (e.data['amount'] as num?)?.toInt() ?? 0;
    }
    return total;
  }

  List<NodeEvent> get liveStatuses {
    final Map<String, NodeEvent> latest = {};
    for (var e in nodeService.feedEvents.where((e) => e.isLiveStatus)) {
      final pubkey = e.authorPubkey;
      if (pubkey != null) {
        final existing = latest[pubkey];
        if (existing == null || e.seq > existing.seq) {
          latest[pubkey] = e;
        }
      }
    }
    return latest.values.toList();
  }

  bool hasLiked(String objectRoot) {
    if (nodeService.state.identityHex == null) return false;
    return getReactions(objectRoot).any(
      (e) =>
          e.authorPubkey == nodeService.state.identityHex &&
          e.reactionAction == 'like',
    );
  }

  String getDisplayName(String pubkey) {
    final profile = nodeService.profiles[pubkey];
    if (profile != null) {
      final name = profile.data['display_name'] as String?;
      if (name != null && name.isNotEmpty) return name;
    }
    return pubkey.length >= 8 ? pubkey.substring(0, 8) : pubkey;
  }

  Set<String> get followedPubkeys =>
      nodeService.policyLists['trusted_pubkeys']?.toSet() ?? const {};

  Set<String> get mutedPubkeys =>
      nodeService.policyLists['muted_pubkeys']?.toSet() ?? const {};

  Set<String> get blockedPubkeys =>
      nodeService.policyLists['blocked_pubkeys']?.toSet() ?? const {};

  bool isFollowed(String pubkey) => followedPubkeys.contains(pubkey);

  bool isMuted(String pubkey) => mutedPubkeys.contains(pubkey);

  bool isBlocked(String pubkey) => blockedPubkeys.contains(pubkey);

  Future<void> submitReply(
    String text,
    String parentRoot, {
    String? channelId,
  }) async {
    final normalized = text.trim();
    if (normalized.isEmpty) return;
    final optimistic = NodeEvent.fromJson({
      'seq': _optimisticSeq--,
      'event': 'feed_bundle',
      'data': {
        'kind': 'post',
        'text': normalized,
        'reply_to_root': parentRoot,
        'author_pubkey_hex': nodeService.state.identityHex ?? '',
        'channel_id': channelId ?? 'general',
        'meta': {
          'version': 1,
          'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
        },
      },
    });
    _optimisticComments
        .putIfAbsent(parentRoot, () => <NodeEvent>[])
        .add(optimistic);
    notifyListeners();
    _expireOptimisticComment(parentRoot, optimistic);
    final previousError = nodeService.state.lastError;
    await nodeService.publishPost(
      text: normalized,
      replyToRoot: parentRoot,
      channelId: channelId ?? 'general',
    );
    final failed =
        nodeService.state.lastError != previousError &&
        (nodeService.state.lastError?.startsWith('Post failed') ?? false);
    if (failed) {
      _optimisticComments[parentRoot]?.remove(optimistic);
      if (_optimisticComments[parentRoot]?.isEmpty ?? false) {
        _optimisticComments.remove(parentRoot);
      }
      notifyListeners();
    }
  }

  Future<void> reactToPost(
    String objectRoot, {
    String action = 'like',
    String? channelId,
  }) async {
    final optimistic = _OptimisticReaction(
      objectRoot: objectRoot,
      action: action,
      authorPubkey: nodeService.state.identityHex,
      createdAtMs: DateTime.now().millisecondsSinceEpoch,
    );
    _optimisticReactions.add(optimistic);
    notifyListeners();
    _expireOptimisticReaction(optimistic);
    final previousError = nodeService.state.lastError;
    await nodeService.publishReaction(
      targetRoot: objectRoot,
      actionCode: action,
      channelId: channelId ?? 'general',
    );
    final failed =
        nodeService.state.lastError != previousError &&
        (nodeService.state.lastError?.startsWith('Reaction failed') ?? false);
    if (failed) {
      _optimisticReactions.remove(optimistic);
      notifyListeners();
    }
  }

  Future<void> repost(
    String objectRoot, {
    String? comment,
    String? channelId,
  }) async {
    final optimistic = _OptimisticRepost(
      objectRoot: objectRoot,
      authorPubkey: nodeService.state.identityHex,
      createdAtMs: DateTime.now().millisecondsSinceEpoch,
    );
    _optimisticReposts.add(optimistic);
    notifyListeners();
    _expireOptimisticRepost(optimistic);
    final previousError = nodeService.state.lastError;
    await nodeService.publishRepost(
      targetRoot: objectRoot,
      comment: comment,
      channelId: channelId ?? 'general',
    );
    final failed =
        nodeService.state.lastError != previousError &&
        (nodeService.state.lastError?.startsWith('Boost failed') ?? false);
    if (failed) {
      _optimisticReposts.remove(optimistic);
      notifyListeners();
    }
  }

  void _reconcileOptimisticEvents() {
    final now = DateTime.now().millisecondsSinceEpoch;
    _optimisticReactions.removeWhere(
      (pending) => now - pending.createdAtMs > 20000,
    );
    _optimisticReposts.removeWhere(
      (pending) => now - pending.createdAtMs > 20000,
    );
    if (_optimisticReactions.isNotEmpty) {
      _optimisticReactions.removeWhere((pending) {
        return nodeService.feedEvents.any((e) {
          return e.isReaction &&
              e.targetRoot == pending.objectRoot &&
              (e.reactionAction ?? 'like') == pending.action &&
              (pending.authorPubkey == null ||
                  pending.authorPubkey!.isEmpty ||
                  e.authorPubkey == pending.authorPubkey);
        });
      });
    }
    if (_optimisticReposts.isNotEmpty) {
      _optimisticReposts.removeWhere((pending) {
        return nodeService.feedEvents.any((e) {
          return e.isRepost &&
              e.targetRoot == pending.objectRoot &&
              (pending.authorPubkey == null ||
                  pending.authorPubkey!.isEmpty ||
                  e.authorPubkey == pending.authorPubkey);
        });
      });
    }
    if (_optimisticComments.isNotEmpty) {
      final toRemove = <String>[];
      _optimisticComments.forEach((parentRoot, pendingList) {
        pendingList.removeWhere((pending) {
          return nodeService.feedEvents.any((e) {
            return e.isPost &&
                e.replyToRoot == parentRoot &&
                (e.postText ?? '').trim() == (pending.postText ?? '').trim() &&
                ((pending.authorPubkey ?? '').isEmpty ||
                    e.authorPubkey == pending.authorPubkey);
          });
        });
        if (pendingList.isEmpty) {
          toRemove.add(parentRoot);
        }
      });
      for (final parentRoot in toRemove) {
        _optimisticComments.remove(parentRoot);
      }
    }
  }

  void _expireOptimisticReaction(_OptimisticReaction pending) {
    Future.delayed(const Duration(seconds: 20), () {
      final removed = _optimisticReactions.remove(pending);
      if (removed) {
        notifyListeners();
      }
    });
  }

  void _expireOptimisticRepost(_OptimisticRepost pending) {
    Future.delayed(const Duration(seconds: 20), () {
      final removed = _optimisticReposts.remove(pending);
      if (removed) {
        notifyListeners();
      }
    });
  }

  void _expireOptimisticComment(String parentRoot, NodeEvent pending) {
    Future.delayed(const Duration(seconds: 20), () {
      final list = _optimisticComments[parentRoot];
      if (list == null) return;
      final removed = list.remove(pending);
      if (list.isEmpty) {
        _optimisticComments.remove(parentRoot);
      }
      if (removed) {
        notifyListeners();
      }
    });
  }

  Future<void> followUser(String pubkey, {String? channelId}) {
    return nodeService.followPubkey(pubkey, channelId: channelId ?? 'general');
  }

  Future<void> unfollowUser(String pubkey) {
    return nodeService.unfollowPubkey(pubkey);
  }

  Future<void> muteUser(String pubkey, {String? channelId}) {
    return nodeService.mutePubkey(pubkey, channelId: channelId ?? 'general');
  }

  Future<void> unmuteUser(String pubkey) {
    return nodeService.unmutePubkey(pubkey);
  }

  Future<void> blockUser(String pubkey, {String? channelId}) {
    return nodeService.blockPubkey(pubkey, channelId: channelId ?? 'general');
  }

  Future<void> unblockUser(String pubkey) {
    return nodeService.unblockPubkey(pubkey);
  }

  @override
  void dispose() {
    nodeService.removeListener(_onServiceUpdate);
    super.dispose();
  }
}
