part of '../social_controller.dart';

extension SocialControllerQueries on SocialController {
  List<NodeEvent> get feed {
    final filtered = nodeService.feedEvents.where((e) {
      if (!e.isPost && !e.isRepost && !e.isPoll) return false;
      if (e.isPost && e.replyToRoot != null) {
        return false;
      }
      return true;
    }).toList();

    return filtered;
  }

  List<NodeEvent> getReactions(String objectRoot) => [
    ...nodeService
        .getReactionsFor(objectRoot)
        .where((e) => !_deletedRoots.contains(e.objectRoot)),
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
    ...nodeService
        .getRepostsFor(objectRoot)
        .where((e) => !_deletedRoots.contains(e.objectRoot)),
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
        (e) =>
            e.isPost &&
            e.replyToRoot == objectRoot &&
            !_deletedRoots.contains(e.objectRoot),
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

  bool hasReposted(String objectRoot) {
    if (nodeService.state.identityHex == null) return false;
    return getReposts(
      objectRoot,
    ).any((e) => e.authorPubkey == nodeService.state.identityHex);
  }

  String getDisplayName(String pubkey) {
    final profile = nodeService.profiles[pubkey];
    if (profile != null) {
      if (profile.displayName.isNotEmpty) return profile.displayName;
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

  bool _reconcileOptimisticEvents() {
    final now = DateTime.now().millisecondsSinceEpoch;
    final initialCount =
        _optimisticReactions.length +
        _optimisticReposts.length +
        _optimisticComments.values.fold(0, (sum, list) => sum + list.length);

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

    final finalCount =
        _optimisticReactions.length +
        _optimisticReposts.length +
        _optimisticComments.values.fold(0, (sum, list) => sum + list.length);

    return initialCount != finalCount;
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

  void _expireDeletedRoot(String root) {
    Future.delayed(const Duration(seconds: 20), () {
      final removed = _deletedRoots.remove(root);
      if (removed) {
        notifyListeners();
      }
    });
  }
}
