import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:veil_social/logic/models/node_event.dart';
import 'package:veil_social/logic/messaging_controller.dart';
import 'package:veil_social/logic/node_service.dart';
import 'package:veil_social/logic/social_controller.dart';
import 'package:veil_social/ui/components/veil_post_card.dart';
import 'package:veil_social/ui/components/new_message_dialog.dart';
import 'package:veil_social/ui/screens/chat_detail_view.dart';
import 'package:veil_social/ui/screens/post_detail_view.dart';

void main() {
  testWidgets('VeilPostCard displays post content', (
    WidgetTester tester,
  ) async {
    final service = NodeService();
    final controller = SocialController(service);
    addTearDown(() {
      controller.dispose();
      service.dispose();
    });

    final event = NodeEvent.fromJson({
      'seq': 1,
      'event': 'feed_bundle',
      'data': {
        'kind': 'post',
        'text': 'Hello Veil!',
        'author_pubkey_hex': 'abcdef1234567890',
        'object_root': 'root1',
        'meta': {'created_at': 1600000000},
      },
    });

    service.testInjectEvent({
      'seq': 2,
      'event': 'feed_bundle',
      'data': {
        'kind': 'reaction',
        'action_code': 'like',
        'target_root': 'root1',
      },
    });

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: VeilPostCard(event: event, controller: controller),
        ),
      ),
    );

    expect(find.text('Hello Veil!'), findsOneWidget);
    expect(find.text('abcdef12'), findsOneWidget);
    expect(find.textContaining('@abcdef12'), findsOneWidget);
    // Likes are shown in footer; tray excludes like chips.
    expect(find.text('1'), findsOneWidget);
  });

  testWidgets('PostDetailView displays thread', (WidgetTester tester) async {
    final service = NodeService();
    final controller = SocialController(service);
    addTearDown(() {
      controller.dispose();
      service.dispose();
    });

    final parent = NodeEvent.fromJson({
      'seq': 1,
      'event': 'feed_bundle',
      'data': {'kind': 'post', 'text': 'Main Thread', 'object_root': 'root1'},
    });

    service.testInjectEvent({
      'seq': 2,
      'event': 'feed_bundle',
      'data': {
        'kind': 'post',
        'text': 'First Comment',
        'object_root': 'comment_root_1',
        'reply_to_root': 'root1',
      },
    });
    service.testInjectEvent({
      'seq': 3,
      'event': 'feed_bundle',
      'data': {
        'kind': 'post',
        'text': 'Nested Comment',
        'object_root': 'comment_root_2',
        'reply_to_root': 'comment_root_1',
      },
    });

    await tester.pumpWidget(
      MaterialApp(
        home: PostDetailView(post: parent, controller: controller),
      ),
    );

    expect(find.text('Main Thread'), findsOneWidget);
    expect(find.text('First Comment'), findsOneWidget);
    expect(find.text('Nested Comment'), findsOneWidget);
    expect(find.byType(TextField), findsOneWidget);
  });

  testWidgets('PostDetailView can reply to selected comment target', (
    WidgetTester tester,
  ) async {
    final service = NodeService();
    service.testSetIdentity('me');
    final controller = SocialController(service);
    addTearDown(() {
      controller.dispose();
      service.dispose();
    });

    final parent = NodeEvent.fromJson({
      'seq': 1,
      'event': 'feed_bundle',
      'data': {'kind': 'post', 'text': 'Main Thread', 'channel_id': 'general'},
    });

    service.testInjectEvent({
      'seq': 2,
      'event': 'feed_bundle',
      'data': {
        'kind': 'post',
        'text': 'First Comment',
        'object_root': 'comment_root_1',
        'reply_to_root': '',
        'author_pubkey_hex': 'alice',
      },
    });

    await tester.pumpWidget(
      MaterialApp(
        home: PostDetailView(post: parent, controller: controller),
      ),
    );

    await tester.tap(find.byKey(const Key('reply-action-comment_root_1')));
    await tester.pump();
    expect(find.textContaining('Replying to'), findsOneWidget);

    await tester.enterText(
      find.byKey(const Key('post-reply-input')),
      'Reply to first comment',
    );
    await tester.pump();
    final sendButton = tester.widget<IconButton>(
      find.byKey(const Key('post-reply-send')),
    );
    expect(sendButton.onPressed, isNotNull);
    sendButton.onPressed!.call();
    await tester.pumpAndSettle();

    expect(find.textContaining('Replying to'), findsNothing);
    await tester.pump(const Duration(seconds: 21));
  });

  testWidgets('ChatDetailView sends DM reply_to_root from selected message', (
    WidgetTester tester,
  ) async {
    final service = NodeService();
    service.testSetIdentity('me');
    final socialController = SocialController(service);
    final controller = _RecordingMessagingController(service);
    addTearDown(() {
      controller.dispose();
      socialController.dispose();
      service.dispose();
    });

    service.testInjectEvent({
      'seq': 1,
      'event': 'feed_bundle',
      'data': {
        'kind': 'direct_message',
        'author_pubkey_hex': 'alice',
        'recipient_pubkey_hex': 'me',
        'object_root': 'm1',
        'ciphertext_root': 'ct1',
      },
    });
    service.testInjectEvent({
      'seq': 2,
      'event': 'feed_bundle',
      'data': {
        'kind': 'direct_message',
        'author_pubkey_hex': 'me',
        'recipient_pubkey_hex': 'alice',
        'object_root': 'm2',
        'ciphertext_root': 'ct2',
      },
    });
    service.testInjectEvent({
      'seq': 3,
      'event': 'payload',
      'data': {
        'object_root': 'ct1',
        'payload_b64': base64Encode(utf8.encode('hello from alice')),
      },
    });
    service.testInjectEvent({
      'seq': 4,
      'event': 'payload',
      'data': {
        'object_root': 'ct2',
        'payload_b64': base64Encode(utf8.encode('hello from me')),
      },
    });

    await tester.pumpWidget(
      MaterialApp(
        home: ChatDetailView(
          title: 'Alice',
          pubkey: 'alice',
          controller: controller,
          socialController: socialController,
        ),
      ),
    );
    await tester.pump();

    await tester.longPress(find.text('hello from alice'));
    await tester.pump();
    expect(find.textContaining('Replying to'), findsWidgets);

    await tester.enterText(find.byType(TextField).last, 'reply text');
    await tester.pump();
    final sendButtonFinder = find.ancestor(
      of: find.byIcon(Icons.send_rounded).first,
      matching: find.byType(IconButton),
    );
    final sendButton = tester.widget<IconButton>(sendButtonFinder.first);
    expect(sendButton.onPressed, isNotNull);
    await tester.tap(sendButtonFinder.first);
    await tester.pumpAndSettle();

    expect(controller.lastDmRecipientPubkey, 'alice');
    expect(controller.lastDmText, 'reply text');
    expect(controller.lastDmReplyToRoot, 'm1');
  });

  testWidgets('NewMessageDialog handles input', (WidgetTester tester) async {
    final service = NodeService();
    service.testSetIdentity('me');
    final socialController = SocialController(service);
    final controller = MessagingController(service);
    addTearDown(() {
      controller.dispose();
      socialController.dispose();
      service.dispose();
    });

    service.testInjectEvent({
      'seq': 1,
      'event': 'feed_bundle',
      'data': {
        'kind': 'direct_message',
        'author_pubkey_hex': 'alice_pubkey',
        'recipient_pubkey_hex': 'me',
        'ciphertext_root': 'root1',
      },
    });

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: NewMessageDialog(
            controller: controller,
            socialController: socialController,
          ),
        ),
      ),
    );

    expect(find.text('New Message'), findsOneWidget);
    expect(find.text('Find contact'), findsOneWidget);
    expect(find.text('Suggested'), findsOneWidget);
    expect(find.text('alice_pu'), findsOneWidget);

    await tester.enterText(find.byType(TextField), 'alice');
    await tester.pump();

    expect(find.text('alice_pu'), findsOneWidget);
  });
}

class _RecordingMessagingController extends MessagingController {
  String? lastDmRecipientPubkey;
  String? lastDmText;
  String? lastDmReplyToRoot;
  String? lastGroupId;
  String? lastGroupText;
  String? lastGroupReplyToRoot;

  _RecordingMessagingController(super.nodeService);

  @override
  Future<void> publishDirectMessage({
    required String recipientPubkey,
    required String text,
    String? replyToRoot,
    String channelId = 'dm',
  }) async {
    lastDmRecipientPubkey = recipientPubkey;
    lastDmText = text;
    lastDmReplyToRoot = replyToRoot;
  }

  @override
  Future<void> publishGroupMessage({
    required String groupId,
    required String text,
    String? replyToRoot,
    String channelId = 'group',
  }) async {
    lastGroupId = groupId;
    lastGroupText = text;
    lastGroupReplyToRoot = replyToRoot;
  }
}
