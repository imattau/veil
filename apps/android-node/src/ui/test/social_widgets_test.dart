import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:veil_social/logic/models/node_event.dart';
import 'package:veil_social/logic/node_service.dart';
import 'package:veil_social/logic/social_controller.dart';
import 'package:veil_social/logic/messaging_controller.dart';
import 'package:veil_social/ui/components/veil_post_card.dart';
import 'package:veil_social/ui/components/new_message_dialog.dart';
import 'package:veil_social/ui/screens/post_detail_view.dart';

void main() {
  testWidgets('VeilPostCard displays post content', (WidgetTester tester) async {
    final service = NodeService();
    final controller = SocialController(service);

    final event = NodeEvent.fromJson({
      'seq': 1,
      'event': 'feed_bundle',
      'data': {
        'kind': 'post',
        'text': 'Hello Veil!',
        'author_pubkey_hex': 'abcdef1234567890',
        'object_root': 'root1',
        'meta': {'created_at': 1600000000}
      }
    });

    service.testInjectEvent({
      'seq': 2,
      'event': 'feed_bundle',
      'data': {
        'kind': 'reaction',
        'action_code': 'like',
        'target_root': 'root1'
      }
    });

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: VeilPostCard(
            event: event,
            controller: controller,
          ),
        ),
      ),
    );

    expect(find.text('Hello Veil!'), findsOneWidget);
    expect(find.text('abcdef12'), findsOneWidget);
    expect(find.textContaining('@abcdef12'), findsOneWidget);
    // Should show 1 like (from both ReactionTray and _PostFooter)
    expect(find.text('1'), findsNWidgets(2));
  });

  testWidgets('PostDetailView displays thread', (WidgetTester tester) async {
    final service = NodeService();
    final controller = SocialController(service);

    final parent = NodeEvent.fromJson({
      'seq': 1,
      'event': 'feed_bundle',
      'data': {
        'kind': 'post',
        'text': 'Main Thread',
        'object_root': 'root1',
      }
    });

    service.testInjectEvent({
      'seq': 2,
      'event': 'feed_bundle',
      'data': {
        'kind': 'post',
        'text': 'First Comment',
        'reply_to_root': 'root1',
      }
    });

    await tester.pumpWidget(
      MaterialApp(
        home: PostDetailView(
          post: parent,
          controller: controller,
        ),
      ),
    );

    expect(find.text('Main Thread'), findsOneWidget);
    expect(find.text('First Comment'), findsOneWidget);
    expect(find.byType(TextField), findsOneWidget);
  });

  testWidgets('NewMessageDialog handles input', (WidgetTester tester) async {
    final service = NodeService();
    final controller = MessagingController(service);

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: NewMessageDialog(controller: controller),
        ),
      ),
    );

    expect(find.text('New Message'), findsOneWidget);
    
    // Find text fields by their label text
    final pubkeyField = find.ancestor(
      of: find.text('Recipient Public Key'),
      matching: find.byType(TextField),
    );
    final messageField = find.ancestor(
      of: find.text('Message'),
      matching: find.byType(TextField),
    );

    await tester.enterText(pubkeyField, 'pub123');
    await tester.enterText(messageField, 'Hello Alice');
    await tester.pump();

    expect(find.text('pub123'), findsOneWidget);
    expect(find.text('Hello Alice'), findsOneWidget);
  });
}
