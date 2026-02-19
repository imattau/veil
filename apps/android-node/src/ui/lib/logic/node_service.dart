import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:http/http.dart' as http;
import 'package:image/image.dart' as img;
import 'package:web_socket_channel/io.dart';
import 'package:web_socket_channel/status.dart' as ws_status;

import './models/node_event.dart';
import './models/node_state.dart';
import './models/profile_data.dart';
import './retry_backoff.dart';

part 'node_service/node_service_lifecycle.dart';
part 'node_service/node_service_refresh.dart';
part 'node_service/node_service_publish_objects.dart';
part 'node_service/node_service_publish_messaging.dart';
part 'node_service/node_service_publish_profiles.dart';
part 'node_service/node_service_policy.dart';
part 'node_service/node_service_identity_contacts.dart';
part 'node_service/node_service_feed_cache.dart';
part 'node_service/node_service_parsing.dart';

class NodeService extends ChangeNotifier {
  static const int rpcPort = 7788;
  static const int maxMediaPayloadBytes = 12 * 1024 * 1024;
  static const int _maxEvents = 300;

  NodeState _state = NodeState.initial();
  final MethodChannel _channel = const MethodChannel('veil/node_service');
  final http.Client _client = http.Client();
  IOWebSocketChannel? _eventsChannel;
  StreamSubscription? _eventsSub;
  Timer? _poller;
  Timer? _eventsReconnectTimer;
  int _eventsReconnectAttempts = 0;
  int _eventsConnectionToken = 0;
  DateTime? _lastStatusRefresh;
  bool _disposed = false;

  final List<NodeEvent> _events = [];
  final Set<int> _eventSeqs = <int>{};
  final List<NodeEvent> _feedEvents = [];
  final Map<String, ProfileData> _profiles = {};
  final Map<String, NodeEvent> _latestLists = {};
  final Map<String, NodeEvent> _latestPrefs = {};
  final Map<String, String> _decryptedPayloads = {};
  List<Map<String, dynamic>> _contacts = const [];
  Map<String, List<String>> _policyLists = const {
    'trusted_pubkeys': [],
    'muted_pubkeys': [],
    'blocked_pubkeys': [],
  };

  List<NodeEvent> get events => List.unmodifiable(_events);
  List<NodeEvent> get feedEvents => List.unmodifiable(_feedEvents);
  Map<String, ProfileData> get profiles => Map.unmodifiable(_profiles);
  Map<String, NodeEvent> get latestLists => Map.unmodifiable(_latestLists);
  Map<String, NodeEvent> get latestPrefs => Map.unmodifiable(_latestPrefs);
  Map<String, String> get decryptedPayloads =>
      Map.unmodifiable(_decryptedPayloads);
  List<Map<String, dynamic>> get contacts => List.unmodifiable(_contacts);
  Map<String, List<String>> get policyLists => Map.unmodifiable(_policyLists);

  NodeState get state => _state;

  String get _baseUrl => 'http://127.0.0.1:$rpcPort';

  Map<String, String> get _authHeader {
    final token = _state.authToken;
    if (token == null || token.isEmpty) {
      return const {};
    }
    return {'x-veil-token': token};
  }

  Future<void> start() => NodeServiceLifecycle(this).start();

  Future<void> stop() => NodeServiceLifecycle(this).stop();

  Future<void> connectEvents() => NodeServiceLifecycle(this).connectEvents();

  Future<void> disconnectEvents() =>
      NodeServiceLifecycle(this).disconnectEvents();

  Future<bool> rotateIdentity() =>
      NodeServiceIdentityContacts(this).rotateIdentity();

  Future<void> refresh() => NodeServiceRefresh(this).refresh();

  Future<void> fetchFeed() => NodeServiceRefresh(this).fetchFeed();

  Future<bool> publishRaw({required String payload, int namespace = 32}) {
    return NodeServicePublishObjects(
      this,
    ).publishRaw(payload: payload, namespace: namespace);
  }

  Future<bool> publishPost({
    required String text,
    String? replyToRoot,
    List<String> mediaRoots = const [],
    String channelId = 'general',
    int namespace = 32,
  }) {
    return NodeServicePublishObjects(this).publishPost(
      text: text,
      replyToRoot: replyToRoot,
      mediaRoots: mediaRoots,
      channelId: channelId,
      namespace: namespace,
    );
  }

  Future<bool> publishPoll({
    required String question,
    required List<String> options,
    int? endsAtUnixSeconds,
    String channelId = 'general',
    int namespace = 32,
  }) {
    return NodeServicePublishObjects(this).publishPoll(
      question: question,
      options: options,
      endsAtUnixSeconds: endsAtUnixSeconds,
      channelId: channelId,
      namespace: namespace,
    );
  }

  Future<bool> publishPollVote({
    required String pollRoot,
    required int optionIndex,
    String channelId = 'general',
    int namespace = 32,
  }) {
    return NodeServicePublishObjects(this).publishPollVote(
      pollRoot: pollRoot,
      optionIndex: optionIndex,
      channelId: channelId,
      namespace: namespace,
    );
  }

  Future<bool> subscribeTag(String tag) =>
      NodeServicePolicy(this).subscribeTag(tag);

  Future<bool> unsubscribeTag(String tag) =>
      NodeServicePolicy(this).unsubscribeTag(tag);

  Future<bool> publishDeletion({
    required List<String> targetRoots,
    String? reason,
    int namespace = 32,
  }) {
    return NodeServicePublishObjects(this).publishDeletion(
      targetRoots: targetRoots,
      reason: reason,
      namespace: namespace,
    );
  }

  Future<bool> publishReaction({
    required String targetRoot,
    String actionCode = 'like',
    String channelId = 'general',
    int namespace = 32,
  }) {
    return NodeServicePublishObjects(this).publishReaction(
      targetRoot: targetRoot,
      actionCode: actionCode,
      channelId: channelId,
      namespace: namespace,
    );
  }

  Future<bool> publishRepost({
    required String targetRoot,
    String? comment,
    String channelId = 'general',
    int namespace = 32,
  }) {
    return NodeServicePublishObjects(this).publishRepost(
      targetRoot: targetRoot,
      comment: comment,
      channelId: channelId,
      namespace: namespace,
    );
  }

  Future<bool> publishZap({
    required String targetRoot,
    required int amount,
    String channelId = 'general',
    int namespace = 32,
    String? message,
  }) {
    return NodeServicePublishObjects(this).publishZap(
      targetRoot: targetRoot,
      amount: amount,
      channelId: channelId,
      namespace: namespace,
      message: message,
    );
  }

  Future<bool> publishDM({
    required String recipientPubkey,
    required String text,
    String? replyToRoot,
    String channelId = 'dm',
    int namespace = 32,
  }) {
    return NodeServicePublishMessaging(this).publishDM(
      recipientPubkey: recipientPubkey,
      text: text,
      replyToRoot: replyToRoot,
      channelId: channelId,
      namespace: namespace,
    );
  }

  Future<bool> publishGroupMessage({
    required String groupId,
    required String text,
    String? replyToRoot,
    List<String> memberPubkeys = const [],
    String channelId = 'group',
    int namespace = 32,
  }) {
    return NodeServicePublishMessaging(this).publishGroupMessage(
      groupId: groupId,
      text: text,
      replyToRoot: replyToRoot,
      memberPubkeys: memberPubkeys,
      channelId: channelId,
      namespace: namespace,
    );
  }

  Future<bool> shareGroupKey({
    required String groupId,
    required List<String> memberPubkeys,
    String channelId = 'group',
    bool rotateKey = false,
    int namespace = 32,
  }) {
    return NodeServicePublishMessaging(this).shareGroupKey(
      groupId: groupId,
      memberPubkeys: memberPubkeys,
      channelId: channelId,
      rotateKey: rotateKey,
      namespace: namespace,
    );
  }

  Future<bool> publishList({
    required String title,
    required String listKind,
    required List<Map<String, dynamic>> items,
    String channelId = 'general',
    int namespace = 32,
  }) {
    return NodeServicePublishProfiles(this).publishList(
      title: title,
      listKind: listKind,
      items: items,
      channelId: channelId,
      namespace: namespace,
    );
  }

  Future<bool> publishAppPreferences({
    required String appId,
    required Map<String, dynamic> preferencesJson,
    String channelId = 'general',
    int namespace = 32,
  }) {
    return NodeServicePublishProfiles(this).publishAppPreferences(
      appId: appId,
      preferencesJson: preferencesJson,
      channelId: channelId,
      namespace: namespace,
    );
  }

  Future<bool> publishProfile({
    required String displayName,
    required String bio,
    String? lightningAddress,
    String? avatarMediaRoot,
    String channelId = 'general',
    int namespace = 32,
  }) {
    return NodeServicePublishProfiles(this).publishProfile(
      displayName: displayName,
      bio: bio,
      lightningAddress: lightningAddress,
      avatarMediaRoot: avatarMediaRoot,
      channelId: channelId,
      namespace: namespace,
    );
  }

  Future<String?> uploadMedia(Uint8List bytes) =>
      NodeServicePublishProfiles(this).uploadMedia(bytes);

  Future<void> updatePolicyAction(String action, String pubkeyHex) =>
      NodeServicePolicy(this).updatePolicyAction(action, pubkeyHex);

  Future<bool> followPubkey(
    String followeePubkeyHex, {
    String channelId = 'general',
    int namespace = 32,
  }) {
    return NodeServicePolicy(this).followPubkey(
      followeePubkeyHex,
      channelId: channelId,
      namespace: namespace,
    );
  }

  Future<void> unfollowPubkey(String pubkeyHex) =>
      NodeServicePolicy(this).unfollowPubkey(pubkeyHex);

  Future<bool> mutePubkey(
    String mutedPubkeyHex, {
    String channelId = 'general',
    String? reason,
    int namespace = 32,
  }) {
    return NodeServicePolicy(this).mutePubkey(
      mutedPubkeyHex,
      channelId: channelId,
      reason: reason,
      namespace: namespace,
    );
  }

  Future<void> unmutePubkey(String pubkeyHex) =>
      NodeServicePolicy(this).unmutePubkey(pubkeyHex);

  Future<bool> blockPubkey(
    String blockedPubkeyHex, {
    String channelId = 'general',
    String? reason,
    int namespace = 32,
  }) {
    return NodeServicePolicy(this).blockPubkey(
      blockedPubkeyHex,
      channelId: channelId,
      reason: reason,
      namespace: namespace,
    );
  }

  Future<void> unblockPubkey(String pubkeyHex) =>
      NodeServicePolicy(this).unblockPubkey(pubkeyHex);

  Future<Map<String, dynamic>?> explainPolicy(String pubkeyHex) =>
      NodeServicePolicy(this).explainPolicy(pubkeyHex);

  Future<Map<String, dynamic>?> exportIdentity() =>
      NodeServiceIdentityContacts(this).exportIdentity();

  Future<Map<String, dynamic>?> fetchObject(
    String root, {
    bool reportErrors = false,
  }) {
    return NodeServiceIdentityContacts(
      this,
    ).fetchObject(root, reportErrors: reportErrors);
  }

  Future<void> importIdentity(String secretKeyHex) =>
      NodeServiceIdentityContacts(this).importIdentity(secretKeyHex);

  Future<bool> saveContact({
    required String peerId,
    String? wsUrl,
    String? quicAddr,
    String? pubkeyHex,
    String? rpcUrl,
  }) {
    return NodeServiceIdentityContacts(this).saveContact(
      peerId: peerId,
      wsUrl: wsUrl,
      quicAddr: quicAddr,
      pubkeyHex: pubkeyHex,
      rpcUrl: rpcUrl,
    );
  }

  Future<bool> deleteContact(String peerId) =>
      NodeServiceIdentityContacts(this).deleteContact(peerId);

  List<NodeEvent> getReactionsFor(String objectRoot) =>
      NodeServiceFeedCache(this).getReactionsFor(objectRoot);

  List<NodeEvent> getRepostsFor(String objectRoot) =>
      NodeServiceFeedCache(this).getRepostsFor(objectRoot);

  @visibleForTesting
  void testInjectEvent(Map<String, dynamic> json) =>
      NodeServiceFeedCache(this).testInjectEvent(json);

  void clearError() {
    _setState(_state.copyWith(lastError: null));
  }

  bool _beginBusyOperation(String operation, {bool exclusive = false}) {
    if (exclusive && _state.busy) {
      _setState(
        _state.copyWith(
          lastError: '$operation skipped: node is performing other tasks',
        ),
      );
      return false;
    }
    if (_state.activeOperations.contains(operation)) {
      _setState(_state.copyWith(lastError: '$operation already in progress'));
      return false;
    }
    final next = Set<String>.from(_state.activeOperations)..add(operation);
    _setState(
      _state.copyWith(activeOperations: next, lastError: _state.lastError),
    );
    return true;
  }

  void _endBusyOperation(String operation) {
    if (_disposed) return;
    final next = Set<String>.from(_state.activeOperations)..remove(operation);
    _setState(_state.copyWith(activeOperations: next));
  }

  bool _insertEvent(NodeEvent event) {
    if (_eventSeqs.contains(event.seq)) {
      return false;
    }
    _events.insert(0, event);
    _eventSeqs.add(event.seq);
    if (_events.length > _maxEvents) {
      final removed = _events.removeLast();
      _eventSeqs.remove(removed.seq);
    }
    return true;
  }

  void _setState(NodeState next) {
    if (_state == next) return;
    _state = next;
    notifyListeners();
  }

  void _notifyListeners() {
    notifyListeners();
  }

  @visibleForTesting
  void testSetIdentity(String pubkey) {
    _setState(_state.copyWith(identityHex: pubkey));
  }

  @visibleForTesting
  void testSetReady({required bool running, String? authToken}) {
    _setState(
      _state.copyWith(
        running: running,
        authToken: authToken ?? _state.authToken ?? 'test-token',
      ),
    );
  }

  @override
  void dispose() {
    _disposed = true;
    _poller?.cancel();
    _eventsReconnectTimer?.cancel();
    _eventsReconnectTimer = null;
    _eventsConnectionToken++;

    final sub = _eventsSub;
    _eventsSub = null;
    if (sub != null) {
      unawaited(sub.cancel());
    }

    final channel = _eventsChannel;
    _eventsChannel = null;
    if (channel != null) {
      try {
        unawaited(channel.sink.close(ws_status.normalClosure));
      } catch (_) {
        // Best effort closure.
      }
    }

    _client.close();
    super.dispose();
  }
}
