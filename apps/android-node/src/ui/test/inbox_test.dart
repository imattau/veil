import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:veil_social/logic/messaging_controller.dart';
import 'package:veil_social/logic/social_controller.dart';
import 'package:veil_social/logic/node_service.dart';
import 'package:veil_social/ui/screens/inbox_view.dart';

void main() {
  testWidgets('InboxView shows empty state when no messages', (WidgetTester tester) async {
    final service = NodeService();
    final socialController = SocialController(service);
    final messagingController = MessagingController(service);

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: InboxView(
            controller: messagingController,
            socialController: socialController,
          ),
        ),
      ),
    );

    expect(find.text('No messages yet'), findsOneWidget);
  });

  testWidgets('InboxView displays conversation list', (WidgetTester tester) async {
    final service = NodeService();
    service.testSetIdentity('me');
    final socialController = SocialController(service);
    final messagingController = MessagingController(service);

    // Inject a DM
    service.testInjectEvent({
      'seq': 1,
      'event': 'feed_bundle',
      'data': {
        'kind': 'direct_message',
        'author_pubkey_hex': 'alice_pubkey',
        'recipient_pubkey_hex': 'me',
        'ciphertext_root': 'root1',
      }
    });

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: InboxView(
            controller: messagingController,
            socialController: socialController,
          ),
        ),
      ),
    );

    // Since no profile injected, should show first 8 chars of pubkey
    expect(find.text('alice_pu'), findsOneWidget);
  });
}
