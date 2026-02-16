part of '../node_service.dart';

extension NodeServiceFeedCache on NodeService {
  void _pruneFeedEvents() {
    if (_feedEvents.length > 500) {
      _feedEvents.removeRange(500, _feedEvents.length);
    }
  }

  void _addFeedEvent(NodeEvent event, {bool skipPrune = false}) {
    if (_feedEvents.any((e) => e.seq == event.seq)) {
      return;
    }

    _feedEvents.add(event);
    _feedEvents.sort((a, b) => b.seq.compareTo(a.seq));

    if (!skipPrune) {
      _pruneFeedEvents();
    }

    if (event.isProfile) {
      final pubkey = event.authorPubkey;
      if (pubkey != null) {
        final existing = _profiles[pubkey];
        if (existing == null || event.seq > existing.updatedAt) {
          _profiles[pubkey] = ProfileData.fromEvent(event);
          debugPrint('[NodeService] Cached profile for $pubkey');
        }
      }
    }

    if (event.isList) {
      final pubkey = event.authorPubkey;
      final kind = event.listKind;
      if (pubkey != null && kind != null) {
        final key = '$pubkey:$kind';
        final existing = _latestLists[key];
        if (existing == null || event.seq > existing.seq) {
          _latestLists[key] = event;
          debugPrint('[NodeService] Cached list $kind for $pubkey');
        }
      }
    }

    if (event.isAppPreferences) {
      final pubkey = event.authorPubkey;
      final appId = event.appId;
      if (pubkey != null && appId != null) {
        final key = '$pubkey:$appId';
        final existing = _latestPrefs[key];
        if (existing == null || event.seq > existing.seq) {
          _latestPrefs[key] = event;
          debugPrint('[NodeService] Cached preferences for $appId by $pubkey');
        }
      }
    }
  }

  List<NodeEvent> getReactionsFor(String objectRoot) {
    return _feedEvents
        .where((e) => e.isReaction && e.targetRoot == objectRoot)
        .toList();
  }

  List<NodeEvent> getRepostsFor(String objectRoot) {
    return _feedEvents
        .where((e) => e.isRepost && e.targetRoot == objectRoot)
        .toList();
  }

  @visibleForTesting
  void testInjectEvent(Map<String, dynamic> json) {
    final event = NodeEvent.fromJson(json);
    if (!_insertEvent(event)) {
      return;
    }
    if (event.isFeedBundle) {
      _addFeedEvent(event);
    }
    if (event.isPayload) {
      final root = event.data['object_root'] as String?;
      final text = event.decryptedText;
      if (root != null && text != null) {
        _decryptedPayloads[root] = text;
      }
    }
    _notifyListeners();
  }
}
