import 'package:flutter_test/flutter_test.dart';
import 'package:veil_social/main.dart';

void main() {
  testWidgets('Network status button opens slide-out drawer', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(const VeilApp());
    await tester.pump(const Duration(milliseconds: 300));

    expect(find.text('Network Details'), findsNothing);

    await tester.tap(find.byTooltip('Network status'));
    await tester.pump(const Duration(milliseconds: 300));
    await tester.pump(const Duration(milliseconds: 300));

    expect(find.text('Network Details'), findsOneWidget);
    expect(find.text('Connection'), findsOneWidget);
    expect(find.text('Queue + Cache'), findsOneWidget);
    expect(find.text('Traffic'), findsOneWidget);
  });
}
