part of '../social_controller.dart';

extension SocialControllerActions on SocialController {
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

  Future<void> unrepost(String objectRoot) async {
    final selfPubkey = nodeService.state.identityHex;
    if (selfPubkey == null) return;

    final myRepost = getReposts(objectRoot).firstWhere(
      (e) => e.authorPubkey == selfPubkey,
      orElse: () => NodeEvent(seq: 0, event: 'none', data: const {}),
    );

    if (myRepost.event == 'none') return;
    final repostRoot = myRepost.objectRoot;
    if (repostRoot == null) return;

    _deletedRoots.add(repostRoot);
    _expireDeletedRoot(repostRoot);

    _optimisticReposts.removeWhere((r) => r.objectRoot == objectRoot);
    notifyListeners();

    final previousError = nodeService.state.lastError;
    await nodeService.publishDeletion(targetRoots: [repostRoot]);

    final failed =
        nodeService.state.lastError != previousError &&
        (nodeService.state.lastError?.startsWith('Deletion failed') ?? false);
    if (failed) {
      _deletedRoots.remove(repostRoot);
      notifyListeners();
    }
  }

  Future<void> unlikePost(String objectRoot) async {
    final selfPubkey = nodeService.state.identityHex;
    if (selfPubkey == null) return;

    final myLike = getReactions(objectRoot).firstWhere(
      (e) => e.authorPubkey == selfPubkey && e.reactionAction == 'like',
      orElse: () => NodeEvent(seq: 0, event: 'none', data: const {}),
    );

    if (myLike.event == 'none') return;
    final reactionRoot = myLike.objectRoot;
    if (reactionRoot == null) return;

    _deletedRoots.add(reactionRoot);
    _expireDeletedRoot(reactionRoot);

    _optimisticReactions.removeWhere(
      (r) => r.objectRoot == objectRoot && r.action == 'like',
    );
    notifyListeners();

    final previousError = nodeService.state.lastError;
    await nodeService.publishDeletion(targetRoots: [reactionRoot]);

    final failed =
        nodeService.state.lastError != previousError &&
        (nodeService.state.lastError?.startsWith('Deletion failed') ?? false);
    if (failed) {
      _deletedRoots.remove(reactionRoot);
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
}
