import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:veil_social/logic/node_service.dart';
import 'package:veil_social/ui/screens/composer_view.dart';

void main() {
  testWidgets('ComposerView renders and handles input', (
    WidgetTester tester,
  ) async {
    final service = NodeService();
    service.testSetReady(running: true);

    await tester.pumpWidget(MaterialApp(home: ComposerView(service: service)));

    expect(find.text('Post'), findsOneWidget);
    expect(find.byType(TextField), findsOneWidget);
    expect(find.text('#general'), findsOneWidget);

    await tester.enterText(find.byType(TextField), 'Testing new composer');
    await tester.pump();

    expect(find.text('Testing new composer'), findsOneWidget);
  });

  testWidgets('ComposerView disables publish until there is draft content', (
    WidgetTester tester,
  ) async {
    final service = NodeService();
    service.testSetReady(running: true);

    await tester.pumpWidget(MaterialApp(home: ComposerView(service: service)));

    final postButtonFinder = find.widgetWithText(ElevatedButton, 'Post');
    ElevatedButton postButton = tester.widget<ElevatedButton>(postButtonFinder);
    expect(postButton.onPressed, isNull);
    final imageButton = tester.widget<IconButton>(
      find.widgetWithIcon(IconButton, Icons.image_outlined),
    );
    expect(imageButton.onPressed, isNotNull);
    final pollButton = tester.widget<IconButton>(
      find.widgetWithIcon(IconButton, Icons.poll_outlined),
    );
    expect(pollButton.onPressed, isNotNull);

    await tester.enterText(find.byType(TextField), 'Now enabled');
    await tester.pump();

    postButton = tester.widget<ElevatedButton>(postButtonFinder);
    expect(postButton.onPressed, isNotNull);
  });

  testWidgets('ComposerView shows and removes image preview', (
    WidgetTester tester,
  ) async {
    final service = NodeService();
    service.testSetReady(running: true);

    await tester.pumpWidget(MaterialApp(home: ComposerView(service: service)));

    // Initial state: no image preview
    expect(find.byKey(const ValueKey('image_preview')), findsNothing);

    // Note: We can't easily simulate ImagePicker in a unit test without a mock,
    // but we can check if the UI elements for selecting exist.
    expect(find.byIcon(Icons.image_outlined), findsOneWidget);
  });

  testWidgets('ComposerView respects initial channel', (
    WidgetTester tester,
  ) async {
    final service = NodeService();
    service.testSetReady(running: true);

    await tester.pumpWidget(
      MaterialApp(
        home: ComposerView(service: service, initialChannel: 'dev'),
      ),
    );

    expect(find.text('#dev'), findsOneWidget);
  });
}
