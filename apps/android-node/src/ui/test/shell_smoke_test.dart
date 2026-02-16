import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:veil_social/logic/list_controller.dart';
import 'package:veil_social/logic/messaging_controller.dart';
import 'package:veil_social/logic/node_service.dart';
import 'package:veil_social/logic/preferences_controller.dart';
import 'package:veil_social/logic/social_controller.dart';
import 'package:veil_social/ui/screens/composer_view.dart';
import 'package:veil_social/ui/screens/explore_view.dart';
import 'package:veil_social/ui/screens/inbox_view.dart';
import 'package:veil_social/ui/screens/profile_view.dart';
import 'package:veil_social/ui/screens/social_home.dart';

void main() {
  testWidgets('SocialHome shell renders core navigation', (
    WidgetTester tester,
  ) async {
    final service = NodeService();
    final prefs = PreferencesController(service);
    addTearDown(() {
      prefs.dispose();
      service.dispose();
    });

    await tester.pumpWidget(
      MaterialApp(
        home: SocialHome(service: service, preferencesController: prefs),
      ),
    );

    expect(find.text('Social'), findsOneWidget);
    expect(find.text('Home'), findsOneWidget);
    expect(find.text('Explore'), findsOneWidget);
    expect(find.text('Inbox'), findsOneWidget);
    expect(find.text('Profile'), findsOneWidget);
  });

  testWidgets('ExploreView shell renders empty state', (
    WidgetTester tester,
  ) async {
    final service = NodeService();
    final scrollController = ScrollController();
    addTearDown(() {
      scrollController.dispose();
      service.dispose();
    });

    await tester.pumpWidget(
      MaterialApp(
        home: ExploreView(
          service: service,
          scrollController: scrollController,
          topInset: 64,
          bottomInset: 96,
          onAddChannel: () {},
        ),
      ),
    );

    expect(find.text('No channels yet'), findsOneWidget);
    expect(find.text('Add Channel'), findsWidgets);
  });

  testWidgets('InboxView shell renders empty state', (
    WidgetTester tester,
  ) async {
    final service = NodeService();
    final socialController = SocialController(service);
    final messagingController = MessagingController(service);
    final scrollController = ScrollController();
    addTearDown(() {
      scrollController.dispose();
      messagingController.dispose();
      socialController.dispose();
      service.dispose();
    });

    await tester.pumpWidget(
      MaterialApp(
        home: InboxView(
          controller: messagingController,
          socialController: socialController,
          scrollController: scrollController,
          topInset: 64,
          bottomInset: 96,
        ),
      ),
    );

    expect(find.text('No messages yet'), findsOneWidget);
  });

  testWidgets('ProfileView shell renders section headers', (
    WidgetTester tester,
  ) async {
    final service = NodeService();
    service.testSetIdentity(
      'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
    );
    final socialController = SocialController(service);
    final listController = ListController(service);
    final prefs = PreferencesController(service);
    addTearDown(() {
      prefs.dispose();
      listController.dispose();
      socialController.dispose();
      service.dispose();
    });

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ProfileView(
            service: service,
            controller: socialController,
            listController: listController,
            preferencesController: prefs,
            topInset: 16,
            bottomInset: 24,
          ),
        ),
      ),
    );

    expect(find.text('ACCOUNT'), findsOneWidget);
    expect(find.text('CONNECTIONS'), findsOneWidget);
  });

  testWidgets('ComposerView shell renders title and editor', (
    WidgetTester tester,
  ) async {
    final service = NodeService();
    service.testSetReady(running: true);
    addTearDown(service.dispose);

    await tester.pumpWidget(MaterialApp(home: ComposerView(service: service)));

    expect(find.text('New Post'), findsOneWidget);
    expect(find.text("What's happening?"), findsOneWidget);
  });
}
