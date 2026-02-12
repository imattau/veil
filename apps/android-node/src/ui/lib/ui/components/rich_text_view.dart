import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import '../theme/veil_theme.dart';

class RichTextView extends StatelessWidget {
  final String text;
  final TextStyle? style;

  const RichTextView({super.key, required this.text, this.style});

  @override
  Widget build(BuildContext context) {
    final effectiveStyle = style ?? Theme.of(context).textTheme.bodyMedium;
    if (!text.contains('#') && !text.contains('@') && !text.contains('http')) {
      return Text(text, style: effectiveStyle, softWrap: true);
    }

    final tokenPattern = RegExp(
      r'(https?:\/\/[^\s]+|#[A-Za-z0-9_]+|@[A-Za-z0-9_]+)',
    );
    final List<InlineSpan> spans = [];
    var cursor = 0;
    for (final match in tokenPattern.allMatches(text)) {
      if (match.start > cursor) {
        spans.add(
          TextSpan(
            text: text.substring(cursor, match.start),
            style: effectiveStyle,
          ),
        );
      }
      final token = match.group(0)!;
      if (token.startsWith('#')) {
        spans.add(
          TextSpan(
            text: token,
            style: const TextStyle(
              color: VeilTheme.accent,
              fontWeight: FontWeight.bold,
            ),
            recognizer: TapGestureRecognizer()
              ..onTap = () {
                debugPrint('Tapped hashtag: $token');
              },
          ),
        );
      } else if (token.startsWith('@')) {
        spans.add(
          TextSpan(
            text: token,
            style: const TextStyle(color: VeilTheme.accent),
            recognizer: TapGestureRecognizer()
              ..onTap = () {
                debugPrint('Tapped mention: $token');
              },
          ),
        );
      } else {
        spans.add(
          TextSpan(
            text: token,
            style: const TextStyle(
              color: VeilTheme.accent,
              decoration: TextDecoration.underline,
            ),
            recognizer: TapGestureRecognizer()
              ..onTap = () {
                debugPrint('Tapped link: $token');
              },
          ),
        );
      }
      cursor = match.end;
    }
    if (cursor < text.length) {
      spans.add(TextSpan(text: text.substring(cursor), style: effectiveStyle));
    }

    return Text.rich(
      TextSpan(children: spans, style: effectiveStyle),
      softWrap: true,
    );
  }
}
