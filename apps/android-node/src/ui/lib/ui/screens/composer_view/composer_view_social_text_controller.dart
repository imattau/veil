part of '../composer_view.dart';

class SocialTextEditingController extends TextEditingController {
  @override
  TextSpan buildTextSpan({
    required BuildContext context,
    TextStyle? style,
    required bool withComposing,
  }) {
    final text = value.text;
    final List<InlineSpan> spans = [];
    final pattern = RegExp(
      r'(https?:\/\/[^\s]+|#[A-Za-z0-9_]+|@[A-Za-z0-9_]+)',
    );

    var cursor = 0;
    for (final match in pattern.allMatches(text)) {
      if (match.start > cursor) {
        spans.add(
          TextSpan(text: text.substring(cursor, match.start), style: style),
        );
      }

      final token = match.group(0)!;
      spans.add(
        TextSpan(
          text: token,
          style: style?.copyWith(
            color: VeilTheme.accent,
            fontWeight: FontWeight.bold,
          ),
        ),
      );

      cursor = match.end;
    }

    if (cursor < text.length) {
      spans.add(TextSpan(text: text.substring(cursor), style: style));
    }

    return TextSpan(style: style, children: spans);
  }
}
