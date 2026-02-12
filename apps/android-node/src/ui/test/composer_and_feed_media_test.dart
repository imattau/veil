import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:veil_social/logic/models/node_event.dart';
import 'package:veil_social/logic/node_service.dart';
import 'package:veil_social/logic/social_controller.dart';
import 'package:veil_social/ui/components/poll_widget.dart';
import 'package:veil_social/ui/components/veil_post_card.dart';
import 'package:veil_social/ui/screens/composer_view.dart';

class _FakeNodeService extends NodeService {
  final List<Map<String, dynamic>> publishedPolls = [];
  final List<Map<String, dynamic>> publishedVotes = [];

  @override
  Future<void> publishPoll({
    required String question,
    required List<String> options,
    int? endsAtUnixSeconds,
    String channelId = 'general',
    int namespace = 32,
  }) async {
    publishedPolls.add({
      'question': question,
      'options': options,
      'ends_at': endsAtUnixSeconds,
      'channel_id': channelId,
      'namespace': namespace,
    });
  }

  @override
  Future<void> publishPollVote({
    required String pollRoot,
    required int optionIndex,
    String channelId = 'general',
    int namespace = 32,
  }) async {
    publishedVotes.add({
      'poll_root': pollRoot,
      'option_index': optionIndex,
      'channel_id': channelId,
      'namespace': namespace,
    });
  }
}

Finder _labeledTextField(String label) {
  return find.byWidgetPredicate(
    (widget) =>
        widget is TextField && widget.decoration?.labelText?.trim() == label,
  );
}

void main() {
  testWidgets('VeilPostCard renders attached media in feed', (tester) async {
    final service = NodeService();
    final controller = SocialController(service);
    const mediaRoot =
        '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';

    // 1x1 transparent PNG.
    final imageBytes = base64Decode(
      'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO4B9l0AAAAASUVORK5CYII=',
    );
    controller.imageCache[mediaRoot] = Uint8List.fromList(imageBytes);

    final event = NodeEvent.fromJson({
      'seq': 1,
      'event': 'feed_bundle',
      'data': {
        'kind': 'post',
        'author_pubkey_hex': 'abc123',
        'text': 'post with media',
        'object_root': 'deadbeef',
        'media_roots': [mediaRoot],
      },
    });

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: VeilPostCard(event: event, controller: controller, isDetail: true),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.byType(Image), findsWidgets);
    expect(find.text('Media unavailable'), findsNothing);
    expect(find.byType(CircularProgressIndicator), findsNothing);
  });

  testWidgets('Composer poll button opens dialog and publishes poll', (tester) async {
    final service = _FakeNodeService();

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ComposerView(service: service),
        ),
      ),
    );

    await tester.tap(find.byIcon(Icons.poll_outlined));
    await tester.pumpAndSettle();

    expect(find.text('Create Poll'), findsOneWidget);

    await tester.enterText(_labeledTextField('Question'), 'Best snack?');
    await tester.enterText(_labeledTextField('Option 1'), 'Nuts');
    await tester.enterText(_labeledTextField('Option 2'), 'Fruit');

    await tester.tap(find.text('Create'));
    await tester.pumpAndSettle();

    expect(service.publishedPolls.length, 1);
    expect(service.publishedPolls.first['question'], 'Best snack?');
    expect(service.publishedPolls.first['options'], ['Nuts', 'Fruit']);
    expect(service.publishedPolls.first['channel_id'], 'general');
  });

  testWidgets('PollWidget publishes vote on option tap', (tester) async {
    final service = _FakeNodeService();
    final controller = SocialController(service);

    final pollEvent = NodeEvent.fromJson({
      'seq': 2,
      'event': 'feed_bundle',
      'data': {
        'kind': 'poll',
        'question': 'Coffee?',
        'options': ['Yes', 'No'],
        'object_root':
            'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
        'channel_id': 'general',
      },
    });

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: PollWidget(event: pollEvent, controller: controller),
        ),
      ),
    );

    await tester.tap(find.text('Yes'));
    await tester.pumpAndSettle();

    expect(service.publishedVotes.length, 1);
    expect(
      service.publishedVotes.first['poll_root'],
      'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
    );
    expect(service.publishedVotes.first['option_index'], 0);
  });
}
