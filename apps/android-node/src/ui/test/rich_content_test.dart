import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:veil_social/logic/models/node_event.dart';
import 'package:veil_social/logic/node_service.dart';
import 'package:veil_social/logic/social_controller.dart';
import 'package:veil_social/ui/components/rich_text_view.dart';
import 'package:veil_social/ui/components/nested_post_card.dart';

void main() {
  group('RichTextView', () {
    testWidgets('renders hashtags, mentions, and links', (WidgetTester tester) async {
      const text = 'Hello #veil and @alice check https://veil.io';
      
      await tester.pumpWidget(
        const MaterialApp(
          home: Scaffold(
            body: RichTextView(text: text),
          ),
        ),
      );

      expect(find.byType(RichText), findsWidgets);
      expect(find.textContaining('#veil', findRichText: true), findsOneWidget);
      expect(find.textContaining('@alice', findRichText: true), findsOneWidget);
    });
  });

  group('NestedPostCard', () {
    testWidgets('renders original post data', (WidgetTester tester) async {
      final service = NodeService();
      final controller = SocialController(service);

      // Inject the original post
      service.testInjectEvent({
        'seq': 1,
        'event': 'feed_bundle',
        'data': {
          'kind': 'post',
          'object_root': 'orig123',
          'author_pubkey_hex': 'pub_orig',
          'text': 'I am the original post',
        }
      });

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: NestedPostCard(
              targetRoot: 'orig123',
              controller: controller,
            ),
          ),
        ),
      );

      expect(find.text('I am the original post'), findsOneWidget);
      expect(find.text('pub_orig'), findsOneWidget);
    });

    testWidgets('shows loading state when post missing', (WidgetTester tester) async {
      final service = NodeService();
      final controller = SocialController(service);

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: NestedPostCard(
              targetRoot: 'missing',
              controller: controller,
            ),
          ),
        ),
      );

      expect(find.text('Post not found or still syncing...'), findsOneWidget);
    });
  });
}
