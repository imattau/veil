import 'dart:convert';

import 'package:flutter/foundation.dart';

import './node_service.dart';
import './zap_controller.dart';
import './models/node_event.dart';

part 'social_controller/social_controller_models.dart';
part 'social_controller/social_controller_media.dart';
part 'social_controller/social_controller_queries.dart';
part 'social_controller/social_controller_actions.dart';

class SocialController extends ChangeNotifier {
  final NodeService nodeService;
  late final ZapController zapController;
  final Map<String, Uint8List> imageCache = {};
  final Set<String> _fetchingImages = {};
  final Map<String, int> _fetchFailures = {};
  final Map<String, DateTime> _nextFetchAllowed = {};
  final List<_OptimisticReaction> _optimisticReactions = [];
  final List<_OptimisticRepost> _optimisticReposts = [];
  final Map<String, List<NodeEvent>> _optimisticComments = {};
  final Set<String> _deletedRoots = {};
  int _optimisticSeq = -1;
  bool _isDisposed = false;

  int _lastProcessedEventCount = 0;
  final Set<String> _processedProfilePubkeys = {};

  SocialController(this.nodeService) {
    zapController = ZapController(nodeService);
    nodeService.addListener(_onServiceUpdate);
  }

  @override
  void notifyListeners() {
    if (_isDisposed) return;
    super.notifyListeners();
  }

  @override
  void dispose() {
    _isDisposed = true;
    nodeService.removeListener(_onServiceUpdate);
    super.dispose();
  }
}
