part of '../social_controller.dart';

extension SocialControllerMedia on SocialController {
  void _onServiceUpdate() {
    if (_isDisposed) return;
    _reconcileOptimisticEvents();

    for (var entry in nodeService.profiles.entries) {
      final pubkey = entry.key;
      final profile = entry.value;
      if (_processedProfilePubkeys.contains(pubkey)) continue;

      final root = profile.avatarMediaRoot;
      if (root != null) {
        _processedProfilePubkeys.add(pubkey);
        if (!imageCache.containsKey(root)) {
          _fetchImage(root);
        }
      }
    }

    if (nodeService.feedEvents.length > _lastProcessedEventCount) {
      final newEvents = nodeService.feedEvents.take(
        nodeService.feedEvents.length - _lastProcessedEventCount,
      );
      for (var event in newEvents) {
        for (var mediaRoot in event.mediaRoots) {
          if (!imageCache.containsKey(mediaRoot)) {
            _fetchImage(mediaRoot);
          }
        }
      }
      _lastProcessedEventCount = nodeService.feedEvents.length;
    }

    notifyListeners();
  }

  Future<void> _fetchImage(String root) async {
    if (_isDisposed || _fetchingImages.contains(root)) return;
    final now = DateTime.now();
    final nextAllowed = _nextFetchAllowed[root];
    if (nextAllowed != null && now.isBefore(nextAllowed)) return;

    _fetchingImages.add(root);
    try {
      final res = await nodeService.fetchObject(root);
      if (_isDisposed) return;
      if (res != null && res['object_b64'] != null) {
        if (imageCache.length >= 100) {
          imageCache.remove(imageCache.keys.first);
        }
        imageCache[root] = base64.decode(res['object_b64']);
        _fetchFailures.remove(root);
        _nextFetchAllowed.remove(root);
        notifyListeners();
      } else {
        final fails = (_fetchFailures[root] ?? 0) + 1;
        _fetchFailures[root] = fails;
        final backoffSecs = (5 * (1 << (fails - 1))).clamp(5, 600);
        _nextFetchAllowed[root] = now.add(Duration(seconds: backoffSecs));
      }
    } finally {
      _fetchingImages.remove(root);
    }
  }
}
