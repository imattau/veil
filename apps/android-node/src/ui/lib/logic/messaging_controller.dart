import 'dart:convert';
import 'package:flutter/foundation.dart';
import './node_service.dart';
import './models/node_event.dart';

class MessagingController extends ChangeNotifier {
  final NodeService nodeService;

  MessagingController(this.nodeService) {
    nodeService.addListener(notifyListeners);
  }

  Future<void> publishDirectMessage({
    required String recipientPubkey,
    required String text,
    String? replyToRoot,
    String channelId = 'dm',
  }) async {
    // Note: in a real implementation, we would encrypt 'text' here and get a ciphertext_root.
    // For now, we publish to the /direct_message endpoint which the node handles.
    // However, our node API currently expects the bundle to be fully formed.
    // We'll use publishRaw to the DM endpoint.
    
    final bundle = {
      'meta': {
        'version': 1,
        'created_at': DateTime.now().millisecondsSinceEpoch ~/ 1000,
      },
      'channel_id': channelId,
      'author_pubkey_hex': nodeService.state.identityHex ?? '',
      'recipient_pubkey_hex': recipientPubkey,
      'ciphertext_root': 'temp_root_until_encrypted', // Placeholder
      'reply_to_root': replyToRoot,
    };

    // We'll actually implement a new method in node_service for this to be cleaner
    await nodeService.publishDM(
      recipientPubkey: recipientPubkey,
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
    return allDirectMessages.where((m) {
      final author = m.authorPubkey;
      final recipient = m.data['recipient_pubkey_hex'] as String?;
      return author == pubkey || recipient == pubkey;
    }).toList();
  }

  List<NodeEvent> getMessagesForGroup(String groupId) {
    return allGroupMessages.where((m) => m.data['group_id'] == groupId).toList();
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
