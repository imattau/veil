import 'package:flutter/foundation.dart';
import './node_service.dart';
import './models/node_event.dart';

class ConversationThread {
  final bool isGroup;
  final String id;
  final NodeEvent? lastMessage;
  final int unreadCount;

  const ConversationThread({
    required this.isGroup,
    required this.id,
    required this.lastMessage,
    this.unreadCount = 0,
  });
}

class MessagingController extends ChangeNotifier {
  final NodeService nodeService;
  final Map<String, int> _lastReadSeqByThread = {};

  MessagingController(this.nodeService) {
    nodeService.addListener(notifyListeners);
  }

  Future<void> publishDirectMessage({
    required String recipientPubkey,
    required String text,
    String? replyToRoot,
    String channelId = 'dm',
  }) async {
    await nodeService.publishDM(
      recipientPubkey: recipientPubkey,
      text: text,
      replyToRoot: replyToRoot,
      channelId: channelId,
    );
  }

  Future<void> publishGroupMessage({
    required String groupId,
    required String text,
    String? replyToRoot,
    String channelId = 'group',
  }) async {
    await nodeService.publishGroupMessage(
      groupId: groupId,
      text: text,
      replyToRoot: replyToRoot,
      channelId: channelId,
    );
  }

  /// Returns a list of all direct messages involving the current user.
  List<NodeEvent> get allDirectMessages =>
      nodeService.feedEvents.where((e) => e.isDirectMessage).toList();

  /// Returns a list of all group messages.
  List<NodeEvent> get allGroupMessages =>
      nodeService.feedEvents.where((e) => e.isGroupMessage).toList();

  /// Returns unique pubkeys the current user has DM conversations with.
  List<String> get directMessageContacts {
    final self = nodeService.state.identityHex;
    final contacts = <String>{};
    for (var m in allDirectMessages) {
      final author = m.authorPubkey;
      final recipient = m.data['recipient_pubkey_hex'] as String?;
      if (author != null && author != self) contacts.add(author);
      if (recipient != null && recipient != self) contacts.add(recipient);
    }
    return contacts.toList();
  }

  /// Returns unique group IDs.
  List<String> get groupIds {
    return allGroupMessages
        .map((m) => m.data['group_id'] as String?)
        .where((id) => id != null)
        .cast<String>()
        .toSet()
        .toList();
  }

  List<NodeEvent> getMessagesForContact(String pubkey) {
    final messages = allDirectMessages.where((m) {
      final author = m.authorPubkey;
      final recipient = m.data['recipient_pubkey_hex'] as String?;
      return author == pubkey || recipient == pubkey;
    }).toList();
    messages.sort((a, b) => b.seq.compareTo(a.seq));
    return messages;
  }

  List<NodeEvent> getMessagesForGroup(String groupId) {
    final messages = allGroupMessages
        .where((m) => m.data['group_id'] == groupId)
        .toList();
    messages.sort((a, b) => b.seq.compareTo(a.seq));
    return messages;
  }

  List<ConversationThread> get conversations {
    final dmThreads = directMessageContacts.map((pubkey) {
      final msgs = getMessagesForContact(pubkey);
      return ConversationThread(
        isGroup: false,
        id: pubkey,
        lastMessage: msgs.isNotEmpty ? msgs.first : null,
        unreadCount: getUnreadCountForThread(isGroup: false, id: pubkey),
      );
    });
    final groupThreads = groupIds.map((groupId) {
      final msgs = getMessagesForGroup(groupId);
      return ConversationThread(
        isGroup: true,
        id: groupId,
        lastMessage: msgs.isNotEmpty ? msgs.first : null,
        unreadCount: getUnreadCountForThread(isGroup: true, id: groupId),
      );
    });

    final all = [...dmThreads, ...groupThreads];
    all.sort(
      (a, b) => (b.lastMessage?.seq ?? 0).compareTo(a.lastMessage?.seq ?? 0),
    );
    return all;
  }

  List<String> suggestedRecipients({String query = ''}) {
    final self = nodeService.state.identityHex;
    final candidates = <String>{
      ...directMessageContacts,
      ...nodeService.profiles.keys,
      ...allDirectMessages
          .map((m) => m.data['recipient_pubkey_hex'] as String?)
          .whereType<String>(),
    };
    candidates.remove(self);
    final trimmed = query.trim().toLowerCase();
    final out = candidates
        .where((v) => trimmed.isEmpty || v.toLowerCase().contains(trimmed))
        .toList();
    out.sort((a, b) {
      final aSeq = _latestMessageSeqForPubkey(a);
      final bSeq = _latestMessageSeqForPubkey(b);
      return bSeq.compareTo(aSeq);
    });
    return out;
  }

  int getUnreadCountForThread({required bool isGroup, required String id}) {
    final key = _threadKey(isGroup: isGroup, id: id);
    final lastRead = _lastReadSeqByThread[key] ?? 0;
    final self = nodeService.state.identityHex;
    final messages = isGroup ? getMessagesForGroup(id) : getMessagesForContact(id);
    return messages.where((m) {
      if (m.seq <= lastRead) return false;
      return self == null || m.authorPubkey != self;
    }).length;
  }

  void markThreadRead({required bool isGroup, required String id}) {
    final messages = isGroup ? getMessagesForGroup(id) : getMessagesForContact(id);
    if (messages.isEmpty) {
      return;
    }
    final key = _threadKey(isGroup: isGroup, id: id);
    final maxSeq = messages.first.seq;
    final current = _lastReadSeqByThread[key] ?? 0;
    if (maxSeq > current) {
      _lastReadSeqByThread[key] = maxSeq;
      notifyListeners();
    }
  }

  int _latestMessageSeqForPubkey(String pubkey) {
    final messages = getMessagesForContact(pubkey);
    return messages.isNotEmpty ? messages.first.seq : 0;
  }

  String _threadKey({required bool isGroup, required String id}) {
    return '${isGroup ? 'group' : 'dm'}:$id';
  }

  String? getMessageContent(NodeEvent message) {
    final ciphertextRoot = message.data['ciphertext_root'] as String?;
    if (ciphertextRoot == null) return null;
    return nodeService.decryptedPayloads[ciphertextRoot];
  }

  @override
  void dispose() {
    nodeService.removeListener(notifyListeners);
    super.dispose();
  }
}
