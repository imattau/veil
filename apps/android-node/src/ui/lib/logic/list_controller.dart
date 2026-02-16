import 'dart:async';
import 'package:flutter/foundation.dart';
import './node_service.dart';

class ListController extends ChangeNotifier {
  final NodeService nodeService;
  Future<void>? _pendingOp;
  bool _isDisposed = false;

  ListController(this.nodeService) {
    nodeService.addListener(_onNodeServiceChange);
  }

  void _onNodeServiceChange() {
    if (_isDisposed) return;
    notifyListeners();
  }

  @override
  void notifyListeners() {
    if (_isDisposed) return;
    super.notifyListeners();
  }

  List<Map<String, dynamic>> getMyListItems(String kind) {
    final self = nodeService.state.identityHex;
    if (self == null) return [];
    final event = nodeService.latestLists['$self:$kind'];
    return event?.listItems ?? [];
  }

  // Bookmarks specific helpers
  List<String> get bookmarkRoots {
    return getMyListItems('bookmark')
        .where((item) => item['type'] == 'Object')
        .map((item) => item['value'] as String)
        .toList();
  }

  bool isBookmarked(String root) => bookmarkRoots.contains(root);

  Future<void> toggleBookmark(String root) async {
    final completer = Completer<void>();
    final previous = _pendingOp;
    _pendingOp = completer.future;

    if (previous != null) {
      await previous;
    }

    try {
      if (_isDisposed) return;
      final roots = bookmarkRoots;
      if (roots.contains(root)) {
        roots.remove(root);
      } else {
        roots.add(root);
      }

      final items = roots.map((r) => {'type': 'Object', 'value': r}).toList();

      await nodeService.publishList(
        title: 'Bookmarks',
        listKind: 'bookmark',
        items: items,
      );
    } finally {
      completer.complete();
    }
  }

  @override
  void dispose() {
    _isDisposed = true;
    nodeService.removeListener(_onNodeServiceChange);
    super.dispose();
  }
}
