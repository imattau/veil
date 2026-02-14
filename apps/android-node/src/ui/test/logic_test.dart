import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:veil_social/logic/models/node_event.dart';
import 'package:veil_social/logic/social_controller.dart';
import 'package:veil_social/logic/node_contact_config.dart';
import 'package:veil_social/logic/node_service.dart';
import 'package:veil_social/logic/messaging_controller.dart';

class _OptimisticNodeService extends NodeService {
  final Completer<void> reactionCompleter = Completer<void>();
  final Completer<void> repostCompleter = Completer<void>();
  final Completer<void> postCompleter = Completer<void>();

  @override
  Future<bool> publishReaction({
    required String targetRoot,
    String actionCode = 'like',
    String channelId = 'general',
    int namespace = 32,
  }) async {
    await reactionCompleter.future;
    return true;
  }

  @override
  Future<bool> publishRepost({
    required String targetRoot,
    String? comment,
    String channelId = 'general',
    int namespace = 32,
  }) async {
    await repostCompleter.future;
    return true;
  }

  @override
  Future<bool> publishPost({
    required String text,
    String? replyToRoot,
    List<String> mediaRoots = const [],
    String channelId = 'general',
    int namespace = 32,
  }) async {
    await postCompleter.future;
    return true;
  }
}

// Mock NodeService if needed, but since it's a ChangeNotifier we can use a real one
// or a simplified subclass for testing.

void main() {
  group('NodeEvent', () {
    test('parses modern tagged feed bundle event', () {
      final event = NodeEvent.fromJson({
        'seq': 1,
        'event': 'feed_bundle',
        'data': {'kind': 'post', 'text': 'hello'},
      });
      expect(event.isPost, true);
      expect(event.postText, 'hello');
    });

    test('parses legacy externally-tagged feed bundle event', () {
      final event = NodeEvent.fromJson({
        'seq': 2,
        'event': 'feed_bundle',
        'data': {
          'Post': {'text': 'legacy post', 'author_pubkey_hex': 'aa'},
        },
      });
      expect(event.isPost, true);
      expect(event.bundleKind, 'post');
      expect(event.postText, 'legacy post');
    });

    test('parses nested bundle envelope shape', () {
      final event = NodeEvent.fromJson({
        'seq': 3,
        'event': 'feed_bundle',
        'data': {
          'bundle': {
            'Repost': {'target_root': 'abc', 'comment': 'boost'},
          },
          'object_root': 'root123',
        },
      });
      expect(event.isRepost, true);
      expect(event.targetRoot, 'abc');
      expect(event.objectRoot, 'root123');
    });

    test('normalizes byte-array roots to hex strings', () {
      final bytes = List<int>.generate(32, (i) => i);
      final event = NodeEvent.fromJson({
        'seq': 4,
        'event': 'feed_bundle',
        'data': {
          'kind': 'reaction',
          'target_root': bytes,
          'object_root': bytes,
        },
      });
      expect(
        event.targetRoot,
        '000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f',
      );
      expect(
        event.objectRoot,
        '000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f',
      );
    });
  });

  group('NodeContactConfig', () {
    test('derives defaults from bare host', () {
      final config = deriveNodeContactConfig('veilnode.3nostr.com');
      expect(config, isNotNull);
      expect(config!.peerId, 'veilnode.3nostr.com');
      expect(config.wsUrl, 'wss://veilnode.3nostr.com/ws');
      expect(config.quicAddr, 'veilnode.3nostr.com:5000');
    });

    test('derives from explicit websocket URL', () {
      final config = deriveNodeContactConfig('wss://example.com:8443/socket');
      expect(config, isNotNull);
      expect(config!.peerId, 'example.com');
      expect(config.wsUrl, 'wss://example.com:8443/socket');
      expect(config.quicAddr, 'example.com:5000');
    });

    test('supports veil vps profile URI', () {
      final config = deriveNodeContactConfig(
        'veil://vps?ws=wss%3A%2F%2Frelay.example%2Fws&quic=quic%3A%2F%2Frelay.example%3A5001',
      );
      expect(config, isNotNull);
      expect(config!.peerId, 'relay.example');
      expect(config.wsUrl, 'wss://relay.example/ws');
      expect(config.quicAddr, 'relay.example:5001');
    });
  });

  group('NodeService', () {
    test('deduplicates repeated event sequence numbers', () {
      final service = NodeService();
      final eventJson = {
        'seq': 42,
        'event': 'feed_bundle',
        'data': {'kind': 'post', 'object_root': 'root42', 'text': 'hello'},
      };

      service.testInjectEvent(eventJson);
      service.testInjectEvent(eventJson);

      expect(service.events.length, 1);
      expect(service.feedEvents.length, 1);
    });
  });

  group('SocialController', () {
    test(
      'optimistically updates reaction/repost/comment before publish finishes',
      () async {
        final service = _OptimisticNodeService();
        final controller = SocialController(service);

        final reactionFuture = controller.reactToPost('rootA');
        final repostFuture = controller.repost('rootA');
        final commentFuture = controller.submitReply('hello now', 'rootA');

        expect(controller.getReactions('rootA').length, 1);
        expect(controller.getReposts('rootA').length, 1);
        expect(controller.getComments('rootA').length, 1);
        expect(controller.getComments('rootA').first.postText, 'hello now');

        service.reactionCompleter.complete();
        service.repostCompleter.complete();
        service.postCompleter.complete();

        await Future.wait([reactionFuture, repostFuture, commentFuture]);
      },
    );

    test('aggregates reactions and boosts', () {
      final service = NodeService();
      final controller = SocialController(service);

      service.testInjectEvent({
        'seq': 10,
        'event': 'feed_bundle',
        'data': {'kind': 'post', 'object_root': 'root1', 'text': 'main post'},
      });

      service.testInjectEvent({
        'seq': 11,
        'event': 'feed_bundle',
        'data': {
          'kind': 'reaction',
          'action_code': 'like',
          'target_root': 'root1',
        },
      });

      service.testInjectEvent({
        'seq': 12,
        'event': 'feed_bundle',
        'data': {'kind': 'repost', 'target_root': 'root1'},
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
          'display_name': 'Alice',
        },
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
          'text': 'parent',
        },
      });

      service.testInjectEvent({
        'seq': 101,
        'event': 'feed_bundle',
        'data': {
          'kind': 'post',
          'object_root': 'child1',
          'reply_to_root': 'parent_root',
          'text': 'reply 1',
        },
      });

      service.testInjectEvent({
        'seq': 102,
        'event': 'feed_bundle',
        'data': {
          'kind': 'post',
          'object_root': 'child2',
          'reply_to_root': 'parent_root',
          'text': 'reply 2',
        },
      });

      final comments = controller.getComments('parent_root');
      expect(comments.length, 2);
      expect(comments.any((e) => e.postText == 'reply 1'), true);
      expect(comments.any((e) => e.postText == 'reply 2'), true);
    });

    test('matches comments and reactions when roots come as byte arrays', () {
      final service = NodeService();
      final controller = SocialController(service);
      final root = List<int>.generate(32, (i) => i);
      const rootHex =
          '000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f';

      service.testInjectEvent({
        'seq': 200,
        'event': 'feed_bundle',
        'data': {'kind': 'post', 'object_root': root, 'text': 'parent'},
      });
      service.testInjectEvent({
        'seq': 201,
        'event': 'feed_bundle',
        'data': {'kind': 'post', 'reply_to_root': root, 'text': 'reply'},
      });
      service.testInjectEvent({
        'seq': 202,
        'event': 'feed_bundle',
        'data': {
          'kind': 'reaction',
          'target_root': root,
          'action_code': 'like',
        },
      });

      expect(controller.getComments(rootHex).length, 1);
      expect(controller.getReactions(rootHex).length, 1);
    });

    test(
      'SocialController filters posts, reposts, and polls for main feed',
      () {
        final service = NodeService();
        final controller = SocialController(service);

        // 1. A standard post (Should be in feed)
        service.testInjectEvent({
          'seq': 10,
          'event': 'feed_bundle',
          'data': {'kind': 'post', 'text': 'post 1'},
        });

        // 2. A profile update (Should NOT be in feed)
        service.testInjectEvent({
          'seq': 11,
          'event': 'feed_bundle',
          'data': {'kind': 'profile', 'display_name': 'Alice'},
        });

        // 3. A repost (Should be in feed)
        service.testInjectEvent({
          'seq': 12,
          'event': 'feed_bundle',
          'data': {'kind': 'repost', 'target_root': 'some_root'},
        });

        // 3b. A poll (Should be in feed)
        service.testInjectEvent({
          'seq': 125,
          'event': 'feed_bundle',
          'data': {
            'kind': 'poll',
            'question': 'Tea or coffee?',
            'options': ['Tea', 'Coffee'],
          },
        });

        // 4. A reaction (Should NOT be in feed)
        service.testInjectEvent({
          'seq': 13,
          'event': 'feed_bundle',
          'data': {'kind': 'reaction', 'action_code': 'like'},
        });

        expect(controller.feed.length, 3);
        expect(controller.feed.any((e) => e.isPost), true);
        expect(controller.feed.any((e) => e.isRepost), true);
        expect(controller.feed.any((e) => e.isPoll), true);
        expect(controller.feed.any((e) => e.isReaction), false);
      },
    );

    test('SocialController excludes comments from main feed', () {
      final service = NodeService();
      final controller = SocialController(service);

      // Top level post
      service.testInjectEvent({
        'seq': 20,
        'event': 'feed_bundle',
        'data': {'kind': 'post', 'text': 'main'},
      });

      // Comment (has reply_to_root)
      service.testInjectEvent({
        'seq': 21,
        'event': 'feed_bundle',
        'data': {
          'kind': 'post',
          'text': 'comment',
          'reply_to_root': 'some_root',
        },
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
          'object_root': 'root_self',
        },
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
        'data': {'kind': 'direct_message', 'ciphertext_root': 'root_enc_1'},
      });

      // Inject the decrypted payload into service cache
      service.testInjectEvent({
        'seq': 2,
        'event': 'payload',
        'data': {
          'object_root': 'root_enc_1',
          'payload_b64': 'SGVsbG8gU2VjcmV0IQ==', // "Hello Secret!"
        },
      });

      final content = controller.getMessageContent(msg);
      expect(content, 'Hello Secret!');
    });
  });
}
