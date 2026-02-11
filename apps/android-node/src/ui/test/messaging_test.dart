import 'package:flutter_test/flutter_test.dart';
import 'package:veil_social/logic/models/node_event.dart';
import 'package:veil_social/logic/node_service.dart';
import 'package:veil_social/logic/messaging_controller.dart';

void main() {
  group('MessagingController', () {
    test('identifies unique DM contacts', () {
      final service = NodeService();
      service.testSetIdentity('my_pubkey');
      
      final controller = MessagingController(service);

      // DM from Alice to Me
      service.testInjectEvent({
        'seq': 10,
        'event': 'feed_bundle',
        'data': {
          'kind': 'direct_message',
          'author_pubkey_hex': 'alice_pubkey',
          'recipient_pubkey_hex': 'my_pubkey',
          'ciphertext_root': 'root1'
        }
      });

      // DM from Me to Bob
      service.testInjectEvent({
        'seq': 11,
        'event': 'feed_bundle',
        'data': {
          'kind': 'direct_message',
          'author_pubkey_hex': 'my_pubkey',
          'recipient_pubkey_hex': 'bob_pubkey',
          'ciphertext_root': 'root2'
        }
      });

      final contacts = controller.directMessageContacts;
      expect(contacts.contains('alice_pubkey'), true);
      expect(contacts.contains('bob_pubkey'), true);
      expect(contacts.length, 2);
    });

    test('groups messages by groupId', () {
      final service = NodeService();
      final controller = MessagingController(service);

      service.testInjectEvent({
        'seq': 20,
        'event': 'feed_bundle',
        'data': {
          'kind': 'group_message',
          'group_id': 'group_alpha',
          'author_pubkey_hex': 'alice',
          'ciphertext_root': 'root3'
        }
      });

      service.testInjectEvent({
        'seq': 21,
        'event': 'feed_bundle',
        'data': {
          'kind': 'group_message',
          'group_id': 'group_beta',
          'author_pubkey_hex': 'bob',
          'ciphertext_root': 'root4'
        }
      });

      expect(controller.groupIds.length, 2);
      expect(controller.getMessagesForGroup('group_alpha').length, 1);
      expect(controller.getMessagesForGroup('group_alpha').first.authorPubkey, 'alice');
    });

    test('getMessageContent resolves decrypted text', () {
      final service = NodeService();
      final controller = MessagingController(service);

      final msg = NodeEvent.fromJson({
        'seq': 1,
        'event': 'feed_bundle',
        'data': {
          'kind': 'direct_message',
          'ciphertext_root': 'cipher1',
        }
      });

      service.testInjectEvent({
        'seq': 2,
        'event': 'payload',
        'data': {
          'object_root': 'cipher1',
          'payload_b64': 'SGVsbG8=', // "Hello"
        }
      });

      expect(controller.getMessageContent(msg), 'Hello');
    });
  });
}
