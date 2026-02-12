import 'dart:convert';
import 'dart:typed_data';
import 'package:flutter/foundation.dart';
import './node_service.dart';
import './models/node_event.dart';

class SocialController extends ChangeNotifier {
  final NodeService nodeService;
  final Map<String, Uint8List> imageCache = {};

  SocialController(this.nodeService) {
    nodeService.addListener(_onServiceUpdate);
  }

  void _onServiceUpdate() {
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

  List<NodeEvent> getReactions(String objectRoot) =>
      nodeService.getReactionsFor(objectRoot);

  List<NodeEvent> getReposts(String objectRoot) =>
      nodeService.getRepostsFor(objectRoot);

  List<NodeEvent> getComments(String objectRoot) {
    return nodeService.feedEvents
        .where((e) => e.isPost && e.replyToRoot == objectRoot)
        .toList();
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
    await nodeService.publishPost(
      text: text,
      replyToRoot: parentRoot,
      channelId: channelId ?? 'general',
    );
  }

  Future<void> reactToPost(
    String objectRoot, {
    String action = 'like',
    String? channelId,
  }) async {
    await nodeService.publishReaction(
      targetRoot: objectRoot,
      actionCode: action,
      channelId: channelId ?? 'general',
    );
  }

  Future<void> repost(
    String objectRoot, {
    String? comment,
    String? channelId,
  }) async {
    await nodeService.publishRepost(
      targetRoot: objectRoot,
      comment: comment,
      channelId: channelId ?? 'general',
    );
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
