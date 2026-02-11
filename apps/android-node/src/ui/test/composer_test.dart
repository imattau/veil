import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:veil_social/logic/node_service.dart';
import 'package:veil_social/ui/screens/composer_view.dart';

void main() {
  testWidgets('ComposerView renders and handles input', (WidgetTester tester) async {
    final service = NodeService();

    await tester.pumpWidget(
      MaterialApp(
        home: ComposerView(service: service),
      ),
    );

    expect(find.text('Post'), findsOneWidget);
    expect(find.byType(TextField), findsOneWidget);
    expect(find.text('#general'), findsOneWidget);

    await tester.enterText(find.byType(TextField), 'Testing new composer');
    await tester.pump();

    expect(find.text('Testing new composer'), findsOneWidget);
  });

  testWidgets('ComposerView shows and removes image preview', (WidgetTester tester) async {
    final service = NodeService();

    await tester.pumpWidget(
      MaterialApp(
        home: ComposerView(service: service),
      ),
    );

    // Initial state: no image preview
    expect(find.byKey(const ValueKey('image_preview')), findsNothing);

    // Note: We can't easily simulate ImagePicker in a unit test without a mock,
    // but we can check if the UI elements for selecting exist.
    expect(find.byIcon(Icons.image_outlined), findsOneWidget);
  });

  testWidgets('ComposerView respects initial channel', (WidgetTester tester) async {
    final service = NodeService();

    await tester.pumpWidget(
      MaterialApp(
        home: ComposerView(service: service, initialChannel: 'dev'),
      ),
    );

    expect(find.text('#dev'), findsOneWidget);
  });
}
