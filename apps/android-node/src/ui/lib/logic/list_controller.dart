import 'package:flutter/foundation.dart';
import './node_service.dart';

class ListController extends ChangeNotifier {
  final NodeService nodeService;

  ListController(this.nodeService) {
    nodeService.addListener(_onNodeServiceChange);
  }

  void _onNodeServiceChange() {
    notifyListeners();
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
  }

  @override
  void dispose() {
    nodeService.removeListener(_onNodeServiceChange);
    super.dispose();
  }
}
