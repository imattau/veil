import 'package:flutter_test/flutter_test.dart';
import 'package:veil_social/logic/models/node_event.dart';
import 'package:veil_social/logic/social_controller.dart';
import 'package:veil_social/logic/node_service.dart';
import 'package:veil_social/logic/messaging_controller.dart';

// Mock NodeService if needed, but since it's a ChangeNotifier we can use a real one
// or a simplified subclass for testing.

void main() {
  group('NodeEvent', () {
    // ... existing NodeEvent tests ...
  });

  group('SocialController', () {
    test('aggregates reactions and boosts', () {
      final service = NodeService();
      final controller = SocialController(service);

      service.testInjectEvent({
        'seq': 10,
        'event': 'feed_bundle',
        'data': {
          'kind': 'post',
          'object_root': 'root1',
          'text': 'main post'
        }
      });

      service.testInjectEvent({
        'seq': 11,
        'event': 'feed_bundle',
        'data': {
          'kind': 'reaction',
          'action_code': 'like',
          'target_root': 'root1'
        }
      });

      service.testInjectEvent({
        'seq': 12,
        'event': 'feed_bundle',
        'data': {
          'kind': 'repost',
          'target_root': 'root1'
        }
      });

      expect(controller.feed.length, 2); // post + repost
      expect(controller.getReactions('root1').length, 1);
      expect(controller.getReposts('root1').length, 1);
      expect(controller.getReactions('root1').first.reactionAction, 'like');
    });

    test('resolves display names from profiles', () {
      final service = NodeService();
      final controller = SocialController(service);

      service.testInjectEvent({
        'seq': 1,
        'event': 'feed_bundle',
        'data': {
          'kind': 'profile',
          'author_pubkey_hex': 'pub123',
          'display_name': 'Alice'
        }
      });

      expect(controller.getDisplayName('pub123'), 'Alice');
      expect(controller.getDisplayName('unknown'), 'unknown');
    });

    test('aggregates comments for a post', () {
      final service = NodeService();
      final controller = SocialController(service);

      service.testInjectEvent({
        'seq': 100,
        'event': 'feed_bundle',
        'data': {
          'kind': 'post',
          'object_root': 'parent_root',
          'text': 'parent'
        }
      });

      service.testInjectEvent({
        'seq': 101,
        'event': 'feed_bundle',
        'data': {
          'kind': 'post',
          'object_root': 'child1',
          'reply_to_root': 'parent_root',
          'text': 'reply 1'
        }
      });

      service.testInjectEvent({
        'seq': 102,
        'event': 'feed_bundle',
        'data': {
          'kind': 'post',
          'object_root': 'child2',
          'reply_to_root': 'parent_root',
          'text': 'reply 2'
        }
      });

      final comments = controller.getComments('parent_root');
      expect(comments.length, 2);
      expect(comments.any((e) => e.postText == 'reply 1'), true);
      expect(comments.any((e) => e.postText == 'reply 2'), true);
    });

    test('SocialController filters only posts and reposts for main feed', () {
      final service = NodeService();
      final controller = SocialController(service);

      // 1. A standard post (Should be in feed)
      service.testInjectEvent({
        'seq': 10,
        'event': 'feed_bundle',
        'data': {'kind': 'post', 'text': 'post 1'}
      });

      // 2. A profile update (Should NOT be in feed)
      service.testInjectEvent({
        'seq': 11,
        'event': 'feed_bundle',
        'data': {'kind': 'profile', 'display_name': 'Alice'}
      });

      // 3. A repost (Should be in feed)
      service.testInjectEvent({
        'seq': 12,
        'event': 'feed_bundle',
        'data': {'kind': 'repost', 'target_root': 'some_root'}
      });

      // 4. A reaction (Should NOT be in feed)
      service.testInjectEvent({
        'seq': 13,
        'event': 'feed_bundle',
        'data': {'kind': 'reaction', 'action_code': 'like'}
      });

      expect(controller.feed.length, 2);
      expect(controller.feed.any((e) => e.isPost), true);
      expect(controller.feed.any((e) => e.isRepost), true);
      expect(controller.feed.any((e) => e.isReaction), false);
    });

    test('SocialController excludes comments from main feed', () {
      final service = NodeService();
      final controller = SocialController(service);

      // Top level post
      service.testInjectEvent({
        'seq': 20,
        'event': 'feed_bundle',
        'data': {'kind': 'post', 'text': 'main'}
      });

      // Comment (has reply_to_root)
      service.testInjectEvent({
        'seq': 21,
        'event': 'feed_bundle',
        'data': {'kind': 'post', 'text': 'comment', 'reply_to_root': 'some_root'}
      });

      expect(controller.feed.length, 1);
      expect(controller.feed.first.postText, 'main');
    });

    test('SocialController includes self-posts from loopback', () {
      final service = NodeService();
      service.testSetIdentity('my_pubkey');
      final controller = SocialController(service);

      // Inject a post where I am the author
      service.testInjectEvent({
        'seq': 50,
        'event': 'feed_bundle',
        'data': {
          'kind': 'post',
          'author_pubkey_hex': 'my_pubkey',
          'text': 'My own post',
          'object_root': 'root_self'
        }
      });

      expect(controller.feed.length, 1);
      expect(controller.feed.first.postText, 'My own post');
      expect(controller.feed.first.authorPubkey, 'my_pubkey');
    });

    test('tracks identity backup state', () async {
      final service = NodeService();
      expect(service.state.hasBackedUp, false);

      // We can't easily mock the HTTP response here without a mock client,
      // but we can check if the flag is set after a call if we had a successful result.
      // For unit tests, we'll verify the model's copyWith works first.
      final state = service.state.copyWith(hasBackedUp: true);
      expect(state.hasBackedUp, true);
    });
  });

  group('MessagingController Secure Content', () {
    test('resolves decrypted content from service cache', () {
      final service = NodeService();
      final controller = MessagingController(service);

      final msg = NodeEvent.fromJson({
        'seq': 1,
        'event': 'feed_bundle',
        'data': {
          'kind': 'direct_message',
          'ciphertext_root': 'root_enc_1',
        }
      });

      // Inject the decrypted payload into service cache
      service.testInjectEvent({
        'seq': 2,
        'event': 'payload',
        'data': {
          'object_root': 'root_enc_1',
          'payload_b64': 'SGVsbG8gU2VjcmV0IQ==', // "Hello Secret!"
        }
      });

      final content = controller.getMessageContent(msg);
      expect(content, 'Hello Secret!');
    });
  });
}
